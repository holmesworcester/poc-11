//! Test C (real two-process TCP round-trip): admit a chain on daemon A, which
//! pushes it to daemon B over a real socket. B must end up with the whole chain
//! AND reconstruct + validate its closure — not merely store the bytes.
mod cli_harness;

use cli_harness::*;
use std::thread::sleep;
use std::time::{Duration, Instant};

fn wait_for_count(db: &str, expected: usize, timeout: Duration) {
    let start = Instant::now();
    loop {
        let out = assert_success(lk_cli(&["--db", db, "count"]));
        if line_value(&out, "link_facts") == expected.to_string() {
            return;
        }
        assert!(
            start.elapsed() <= timeout,
            "timeout: link_facts != {expected} on {db}\n{out}"
        );
        sleep(Duration::from_millis(100));
    }
}

#[test]
fn link_chain_travels_over_tcp_and_projects_on_peer() {
    let tmp = tempfile::tempdir().unwrap();
    let a = temp_db(&tmp, "a.db");
    let b = temp_db(&tmp, "b.db");
    let a_port = free_port();
    let b_port = free_port();

    // B listens; A listens and pushes everything it has to B.
    let _bd = spawn_daemon(&b, b_port, &[]);
    let _ad = spawn_daemon(&a, a_port, &[b_port]);

    // Admit a chain of 8 on A via separate CLI processes (the daemon reads a.db
    // through WAL and ships new facts to B as they appear).
    let mut prev: Option<String> = None;
    let mut root: Option<String> = None;
    let mut ids = Vec::new();
    for i in 1..=8 {
        let at = i.to_string();
        let mut args = vec!["--db", a.as_str(), "--at", &at, "link"];
        if let Some(p) = &prev {
            args.push("--prev");
            args.push(p);
            args.push("--root");
            args.push(root.as_ref().unwrap());
        }
        let id = line_value(&assert_success(lk_cli(&args)), "link_id");
        if root.is_none() {
            root = Some(id.clone());
        }
        prev = Some(id.clone());
        ids.push(id);
    }

    // Liveness: bytes crossed the socket and were admitted on B.
    wait_for_count(&b, 8, Duration::from_secs(60));

    // Correctness: B reconstructed + validated the full closure.
    let head = assert_success(lk_cli(&["--db", &b, "chain", &ids[7]]));
    assert_eq!(line_value(&head, "complete"), "true");
    assert_eq!(line_value(&head, "length"), "8");
    assert_eq!(line_value(&head, "root_id"), ids[0]);
}
