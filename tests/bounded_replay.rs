//! The decisive proof of the model (CLI black-box). A bounded "limited 10 fact"
//! replay pulls in a full dependency chain even though only the newest 10 were
//! seeded (Test A), but pulls in NO independent facts outside the window (Test B).
//! Identical totals (25 facts, window 10) make B a real control for A.
mod cli_harness;

use cli_harness::*;
use std::collections::HashSet;

fn ids_in(line: &str) -> HashSet<String> {
    line.split_whitespace().map(str::to_string).collect()
}

/// Admit a chain of `n` links (each `--prev` the last), `--at i` for determinism.
/// Returns ids oldest..newest (index 0 = root).
fn build_chain(db: &str, n: usize) -> Vec<String> {
    let mut ids = Vec::new();
    let mut prev: Option<String> = None;
    let mut root: Option<String> = None;
    for i in 1..=n {
        let at = i.to_string();
        let mut args = vec!["--db", db, "--at", &at, "link"];
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
    ids
}

/// Admit `n` independent links (no `--prev`). Returns ids in admission order.
fn build_independents(db: &str, n: usize) -> Vec<String> {
    (1..=n)
        .map(|i| {
            let at = i.to_string();
            line_value(
                &assert_success(lk_cli(&["--db", db, "--at", &at, "link"])),
                "link_id",
            )
        })
        .collect()
}

#[test]
fn cli_requires_explicit_root_for_child_links() {
    let tmp = tempfile::tempdir().unwrap();
    let db = temp_db(&tmp, "explicit-root.db");
    let root = line_value(
        &assert_success(lk_cli(&["--db", &db, "--at", "1", "link"])),
        "link_id",
    );

    let missing_root = lk_cli(&["--db", &db, "--at", "2", "link", "--prev", &root]);
    assert!(!missing_root.status.success());
    assert!(String::from_utf8_lossy(&missing_root.stderr).contains("child link requires --root"));

    let root_with_root = lk_cli(&["--db", &db, "--at", "3", "link", "--root", &root]);
    assert!(!root_with_root.status.success());
    assert!(
        String::from_utf8_lossy(&root_with_root.stderr).contains("root link must not pass --root")
    );
}

#[test]
fn bounded_replay_pulls_full_chain_closure_across_window() {
    let tmp = tempfile::tempdir().unwrap();
    let db = temp_db(&tmp, "chain.db");
    let ids = build_chain(&db, 25);

    let out = assert_success(lk_cli(&["--db", &db, "replay", "--window", "10"]));
    assert_eq!(line_value(&out, "total_facts"), "25");
    assert_eq!(line_value(&out, "seed_count"), "10"); // window strictly < total
    assert_eq!(line_value(&out, "projected_count"), "25"); // full closure
    assert_eq!(line_value(&out, "pulled_in_count"), "15");

    let seed = ids_in(&line_value(&out, "seed"));
    let projected = ids_in(&line_value(&out, "projected"));
    // The 15 oldest were NOT seeded but ARE projected => pulled across the window.
    for old in &ids[0..15] {
        assert!(!seed.contains(old), "old link must be out of window: {old}");
        assert!(
            projected.contains(old),
            "old link must be pulled into closure: {old}"
        );
    }

    // The head resolves the whole way back to the root.
    let head = assert_success(lk_cli(&["--db", &db, "chain", &ids[24]]));
    assert_eq!(line_value(&head, "complete"), "true");
    assert_eq!(line_value(&head, "length"), "25");
    assert_eq!(line_value(&head, "root_id"), ids[0]);
}

#[test]
fn bounded_replay_does_not_pull_independent_facts() {
    let tmp = tempfile::tempdir().unwrap();
    let db = temp_db(&tmp, "indep.db");
    let ids = build_independents(&db, 25);

    let out = assert_success(lk_cli(&["--db", &db, "replay", "--window", "10"]));
    assert_eq!(line_value(&out, "total_facts"), "25");
    assert_eq!(line_value(&out, "seed_count"), "10");
    assert_eq!(line_value(&out, "projected_count"), "10"); // ONLY the window
    assert_eq!(line_value(&out, "pulled_in_count"), "0");

    let projected = ids_in(&line_value(&out, "projected"));
    for old in &ids[0..15] {
        assert!(
            !projected.contains(old),
            "independent old link must NOT be pulled: {old}"
        );
    }
    for win in &ids[15..25] {
        assert!(
            projected.contains(win),
            "windowed link must be projected: {win}"
        );
    }
}
