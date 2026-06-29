//! Link read/report helpers. These are app-facing and storage-backed, so they are
//! unproven until the storage/result contract is moved behind verified effects.
use crate::core::index::Index;
use crate::core::item::FactId;
use crate::core::play::replay;
use crate::core::projector::Projector;
use crate::core::typestate::Validity;

use super::project_unproven::LinkProjector;

/// A validated view of the chain ending at `head`.
pub struct Report {
    pub present: bool,
    pub complete: bool,
    pub root: FactId,
    pub depth: u64,
    pub length: u64,
    /// root..head order (the present contiguous run from head).
    pub ids: Vec<FactId>,
}

pub fn chain_report(idx: &dyn Index, head: FactId) -> Result<Report, String> {
    let w = walk(idx, head)?;
    let complete = if w.present {
        let memo = replay::<LinkProjector>(idx, &[head])?;
        w.reached_root && matches!(memo.get(&head), Some(Validity::Valid))
    } else {
        false
    };
    Ok(Report {
        present: w.present,
        complete,
        root: w.root,
        depth: w.depth,
        length: w.length,
        ids: w.ids,
    })
}

pub(crate) struct Walk {
    pub present: bool,
    pub root: FactId,
    pub depth: u64,
    pub length: u64,
    pub reached_root: bool,
    pub ids: Vec<FactId>,
}

/// Read-only walk of the `prev` chain from `head` down. Pure read (no writes).
pub(crate) fn walk(idx: &dyn Index, head: FactId) -> Result<Walk, String> {
    let mut ids = vec![];
    let mut cur = Some(head);
    let mut present = false;
    let mut reached_root = false;
    while let Some(id) = cur {
        let Some(bytes) = idx.load_fact(&id)? else {
            break;
        };
        if id == head {
            present = true;
        }
        ids.push(id);
        cur = match LinkProjector::decode(&bytes)?.prev {
            None => {
                reached_root = true;
                None
            }
            Some(p) => Some(p),
        };
    }
    let (root, depth, length) = if ids.is_empty() {
        (head, 0, 0)
    } else {
        (*ids.last().unwrap(), ids.len() as u64 - 1, ids.len() as u64)
    };
    ids.reverse();
    Ok(Walk {
        present,
        root,
        depth,
        length,
        reached_root,
        ids,
    })
}
