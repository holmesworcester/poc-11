//! The daemon runtime (protocol-agnostic). [`serve`] binds a listener and runs the
//! turn loop over two recurring workers — transport/ingress and egress — generic
//! over any [`Projector`]. Ingress classifies a frame by trying `P::decode`
//! (bytes `P` rejects are dropped, volatile, §5); egress ships raw fact bytes to
//! peers and reconciles a volatile sent-set each turn (recurrence = liveness).
//!
//! Invariant checklist (Verus):
//! - [ ] Runtime ingress never projects directly; accepted frames go through
//!       decode plus core admission.
//! - [ ] Frames rejected by the route decoder create no durable or validated
//!       state.
//! - [ ] Runtime egress sends stored bytes only; sending a fact is not evidence
//!       that it is valid.
//! - [ ] Socket, clock, and SQLite failures cannot create validated state.
//! - [ ] Recurring egress is a liveness helper only and carries no authority.
//! - [ ] Any deterministic runtime-turn logic that affects validity moves to
//!       `core::turn` before being treated as proven.
use std::collections::HashSet;
use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use super::admit::admit;
use super::item::FactId;
use super::projector::Projector;
use crate::helpers::clock_unproven::now_ms;
use crate::helpers::sqlite_unproven::SqliteIndex;
use crate::helpers::tcp_unproven as tcp;

pub fn serve<P: Projector>(
    idx: &SqliteIndex,
    listen_addr: &str,
    peers: &[String],
) -> Result<(), String> {
    let listener = TcpListener::bind(listen_addr).map_err(stringify)?;
    let addr = listener.local_addr().map_err(stringify)?;
    listener.set_nonblocking(true).map_err(stringify)?;
    // Announce readiness so the test harness can wait for it; flush (stdout is
    // block-buffered when piped).
    {
        let mut so = std::io::stdout();
        let _ = writeln!(so, "listening: {addr}");
        let _ = so.flush();
    }

    let mut sent: HashSet<(String, FactId)> = HashSet::new();
    loop {
        let mut active = false;
        active |= ingress_turn::<P>(idx, &listener);
        active |= egress_turn(idx, peers, &mut sent)?;
        if !active {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// Transport-in + ingress: accept pending connections, read frames to EOF, and
/// admit each one whose bytes `P::decode` accepts (Pass 1, persist). Validation
/// happens later, on read. Frames `P` rejects are volatile and dropped (§5).
fn ingress_turn<P: Projector>(idx: &SqliteIndex, listener: &TcpListener) -> bool {
    let mut did = false;
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                while let Ok(Some(frame)) = tcp::read_frame(&mut stream) {
                    if let Ok(item) = P::decode(&frame) {
                        if admit::<P>(item, now_ms(), idx).is_ok() {
                            did = true;
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }
    did
}

/// Egress: reconcile desired (all local facts) vs a volatile sent-set, pushing any
/// unsent facts to each peer. A failed connect just retries next turn — recurrence
/// is the liveness mechanism (§5), no retry record.
fn egress_turn(
    idx: &SqliteIndex,
    peers: &[String],
    sent: &mut HashSet<(String, FactId)>,
) -> Result<bool, String> {
    if peers.is_empty() {
        return Ok(false);
    }
    let facts = idx.all_facts()?;
    let mut did = false;
    for peer in peers {
        let frames: Vec<Vec<u8>> = facts
            .iter()
            .filter(|(id, _)| !sent.contains(&(peer.clone(), *id)))
            .map(|(_, bytes)| bytes.clone())
            .collect();
        if frames.is_empty() {
            continue;
        }
        if tcp::send_frames(peer, &frames).is_ok() {
            for (id, _) in &facts {
                sent.insert((peer.clone(), *id));
            }
            did = true;
        }
    }
    Ok(did)
}

fn stringify<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
