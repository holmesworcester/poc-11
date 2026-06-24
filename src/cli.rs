//! The `lk` app layer: parse `--db/--at COMMAND`, dispatch to the protocol's link
//! commands, and run the core runtime for `start`. This is the composition root —
//! the only place that knows both `core` and a `protocol` family — mirroring
//! poc-10's split (generic core hosting + protocol command adapters). Output is
//! `field: value` lines; read commands exit 0 even when an id is absent, so a
//! polling test keeps polling.
use std::collections::HashSet;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::index::{Index, SqliteIndex};
use crate::core::item::{from_hex, to_hex, FactId};
use crate::core::play::replay;
use crate::core::runtime;
use crate::protocol::link::{self, LinkProjector};

/// Parsed `lk [--db PATH] [--at MS] COMMAND [ARGS...]`.
struct Parsed {
    db: String,
    at: Option<u64>,
    command: String,
    rest: Vec<String>,
}

pub fn run(args: &[String]) -> i32 {
    let p = match parse(args) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return 2;
        }
    };
    // The daemon never returns lines: it prints `listening:` and loops.
    if p.command == "start" {
        return match start(&p) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        };
    }
    match handle(&p) {
        Ok(lines) => {
            let mut out = std::io::stdout();
            for l in &lines {
                let _ = writeln!(out, "{l}");
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

fn handle(p: &Parsed) -> Result<Vec<String>, String> {
    let idx = SqliteIndex::open(&p.db)?;
    match p.command.as_str() {
        "link" => cmd_link(&idx, p),
        "chain" => cmd_chain(&idx, p),
        "count" => cmd_count(&idx),
        "replay" => cmd_replay(&idx, p),
        other => Err(format!("unknown command: {other}")),
    }
}

fn cmd_link(idx: &SqliteIndex, p: &Parsed) -> Result<Vec<String>, String> {
    let at = p.at.unwrap_or_else(now_ms);
    let prev = match flag(&p.rest, "--prev") {
        Some(h) => Some(from_hex(&h).ok_or("bad --prev hex")?),
        None => None,
    };
    let label = flag(&p.rest, "--label").unwrap_or_default();
    let a = link::author(idx, at, prev, &label)?;
    Ok(vec![
        format!("link_id: {}", to_hex(&a.id)),
        format!(
            "prev_id: {}",
            prev.map(|p| to_hex(&p))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("depth: {}", a.depth),
        format!("root_id: {}", to_hex(&a.root)),
    ])
}

fn cmd_chain(idx: &SqliteIndex, p: &Parsed) -> Result<Vec<String>, String> {
    let head = from_hex(p.rest.first().ok_or("usage: chain <id>")?).ok_or("bad id hex")?;
    let r = link::chain_report(idx, head)?;
    let mut lines = vec![
        format!("present: {}", r.present),
        format!("complete: {}", r.complete),
        format!("target_id: {}", to_hex(&head)),
        format!("root_id: {}", to_hex(&r.root)),
        format!("depth: {}", r.depth),
        format!("length: {}", r.length),
    ];
    if r.present {
        lines.push(format!(
            "chain: {}",
            r.ids.iter().map(to_hex).collect::<Vec<_>>().join(" ")
        ));
    }
    Ok(lines)
}

fn cmd_count(idx: &SqliteIndex) -> Result<Vec<String>, String> {
    Ok(vec![
        format!("link_facts: {}", idx.total_facts()?),
        format!("edges: {}", idx.total_edges()?),
    ])
}

fn cmd_replay(idx: &SqliteIndex, p: &Parsed) -> Result<Vec<String>, String> {
    let window: usize = flag(&p.rest, "--window")
        .ok_or("usage: replay --window N")?
        .parse()
        .map_err(|_| "bad --window".to_string())?;
    let total = idx.total_facts()?;
    let seeds = idx.window(window)?;
    let seed_set: HashSet<FactId> = seeds.iter().copied().collect();

    let memo = replay::<LinkProjector>(idx, &seeds)?;
    let mut projected: Vec<FactId> = memo.keys().copied().collect();
    projected.sort();
    let pulled = projected
        .iter()
        .filter(|id| !seed_set.contains(*id))
        .count();

    let mut seed_sorted = seeds.clone();
    seed_sorted.sort();
    Ok(vec![
        format!("window: {window}"),
        format!("total_facts: {total}"),
        format!("seed_count: {}", seeds.len()),
        format!("projected_count: {}", projected.len()),
        format!("pulled_in_count: {pulled}"),
        format!(
            "seed: {}",
            seed_sorted.iter().map(to_hex).collect::<Vec<_>>().join(" ")
        ),
        format!(
            "projected: {}",
            projected.iter().map(to_hex).collect::<Vec<_>>().join(" ")
        ),
    ])
}

fn start(p: &Parsed) -> Result<(), String> {
    let (ip, port) = two_after(&p.rest, "--listen")
        .ok_or("usage: start --listen IP PORT [--peer IP PORT]...")?;
    let peers = peers_from(&p.rest);
    let idx = SqliteIndex::open(&p.db)?;
    runtime::serve::<LinkProjector>(&idx, &format!("{ip}:{port}"), &peers)
}

// ---- argument parsing ----

fn parse(args: &[String]) -> Result<Parsed, String> {
    let mut db = None;
    let mut at = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--db" => {
                db = Some(arg(args, i + 1, "--db")?);
                i += 2;
            }
            "--at" => {
                at = Some(
                    arg(args, i + 1, "--at")?
                        .parse()
                        .map_err(|_| "bad --at".to_string())?,
                );
                i += 2;
            }
            _ => break,
        }
    }
    let command = args.get(i).ok_or("missing command")?.clone();
    let rest = args.get(i + 1..).unwrap_or(&[]).to_vec();
    Ok(Parsed {
        db: db.ok_or("missing --db PATH")?,
        at,
        command,
        rest,
    })
}

fn arg(args: &[String], i: usize, name: &str) -> Result<String, String> {
    args.get(i)
        .cloned()
        .ok_or_else(|| format!("{name} needs a value"))
}

fn flag(rest: &[String], name: &str) -> Option<String> {
    rest.iter()
        .position(|a| a == name)
        .and_then(|i| rest.get(i + 1).cloned())
}

fn two_after(rest: &[String], name: &str) -> Option<(String, String)> {
    let i = rest.iter().position(|a| a == name)?;
    Some((rest.get(i + 1)?.clone(), rest.get(i + 2)?.clone()))
}

fn peers_from(rest: &[String]) -> Vec<String> {
    let mut peers = vec![];
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == "--peer" {
            if let (Some(ip), Some(port)) = (rest.get(i + 1), rest.get(i + 2)) {
                peers.push(format!("{ip}:{port}"));
                i += 3;
                continue;
            }
        }
        i += 1;
    }
    peers
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
