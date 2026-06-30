//! Link CLI formatting helpers. Parsing/formatting and command admission remain
//! unproven; semantic link construction lives in `project_unproven` until the
//! full link/core composition proof lands.
//!
//! Fact-family contract (do not weaken):
//! - Scope: app adapter only: call deterministic link construction, call core
//!   admission, call report/replay helpers, pass explicit constructor
//!   parameters chosen by the app/frontend, and format returned data.
//! - Forbidden here: defining link constructors, defining codec/extraction/
//!   projection rules, interpreting root/domain semantics, creating `Validity`,
//!   creating `Context`, or creating `Offer<Validated>`.
//! - CLI output is display text. It is not proof evidence.
//!
//! Invariant checklist (Verus):
//! Owned invariant: link CLI adapter boundary.
//! - [ ] Safety: CLI input is not proof evidence; it only chooses parameters for the
//!       project-owned link constructor.
//! - [ ] Safety: CLI admission goes through core admission; it never writes fact
//!       bytes or asserted edges directly.
//! - [ ] Safety: displayed ids, roots, depths, and completeness are reports only;
//!       they do not affect future validity.
//! - [ ] Safety: CLI code cannot construct `Validity`, `Context`, or
//!       `Offer<Validated>`.
//! Imported theorem checklist:
//! - [x] `facts::link::project_unproven`: `link_from_params` owns link construction
//!       semantics. Proven in
//!       `src/facts/link/project_unproven.rs::link_from_params_constructs_only_link_fields`.
//! - [x] `core::admit`: admission creates only asserted state. Proven in
//!       `src/core/admit_unproven.rs::admit_establishes_id_body`.
//! - [ ] `facts::link::api`: reports are observations of projected state. Owner:
//!       `src/facts/link/api_unproven.rs::chain_report`, planned theorem
//!       `chain_report_observation_only`.
//! - [ ] `core::play`: replay reports drained engine validity. Owner:
//!       `src/core/play_unproven.rs`, planned theorem
//!       `replay_reports_engine_validity`.
//! Proof strategy:
//! - Prove CLI functions delegate construction to `link_from_params`, admission
//!   to `core::admit`, and report fields to `chain_report`/`replay`.
//! - Prove no CLI function calls storage write methods directly or constructs
//!   validity/context/validated offers.
//! - Prove formatted strings are output-only and are not consumed by projection.
use std::collections::HashSet;

use crate::core::admit::admit;
use crate::core::index::Index;
use crate::core::item::FactId;
use crate::core::play::replay;
use crate::helpers::hex_unproven::to_hex;

use super::api_unproven::chain_report;
use super::project_unproven::{link_from_params, LinkProjector};

pub fn link_lines(
    idx: &dyn Index,
    at: u64,
    prev: Option<FactId>,
    root: Option<FactId>,
    label: &str,
) -> Result<Vec<String>, String> {
    let id = admit::<LinkProjector>(link_from_params(at, prev, root, label), at, idx)?.id();
    let r = chain_report(idx, id)?;
    Ok(vec![
        format!("link_id: {}", to_hex(&id)),
        format!(
            "prev_id: {}",
            prev.map(|p| to_hex(&p))
                .unwrap_or_else(|| "none".to_string())
        ),
        format!("depth: {}", r.depth),
        format!("root_id: {}", to_hex(&r.root)),
    ])
}

pub fn chain_lines(idx: &dyn Index, head: FactId) -> Result<Vec<String>, String> {
    let r = chain_report(idx, head)?;
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

pub fn count_lines(idx: &dyn Index) -> Result<Vec<String>, String> {
    Ok(vec![
        format!("link_facts: {}", idx.total_facts()?),
        format!("edges: {}", idx.total_edges()?),
    ])
}

pub fn replay_lines(idx: &dyn Index, window: usize) -> Result<Vec<String>, String> {
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
