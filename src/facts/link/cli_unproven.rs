//! Link CLI formatting helpers. Parsing/formatting remains unproven; it delegates
//! fact-family behavior to the link author/report/project modules.
use std::collections::HashSet;

use crate::core::index::Index;
use crate::core::item::FactId;
use crate::core::play::replay;
use crate::helpers::hex_unproven::to_hex;

use super::api_unproven::chain_report;
use super::author_unproven::author;
use super::project_unproven::LinkProjector;

pub fn link_lines(
    idx: &dyn Index,
    at: u64,
    prev: Option<FactId>,
    label: &str,
) -> Result<Vec<String>, String> {
    let a = author(idx, at, prev, label)?;
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
