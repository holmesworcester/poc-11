//! Link read/report helpers. These are app-facing and storage-backed, so they are
//! unproven until the storage/result contract is moved behind verified effects.
//!
//! Fact-family contract (do not weaken):
//! - Scope: observation/report layer only.
//! - Allowed here: read persisted bytes, decode links, follow declared `prev`
//!   references, and call core replay to compute report completeness.
//! - Forbidden here: fact construction, admission, storage writes, direct projector
//!   execution, creation of `Validity`, creation of `Context`, and creation of
//!   `Offer<Validated>`.
//! - Report fields are observations. They are not proof witnesses and must not be
//!   used as inputs to core validity or link projection theorems.
//!
//! Invariant checklist (Verus):
//! Owned invariant: link reporting boundary.
//! - [ ] Safety: reports are observations for users; they are never authority for
//!       projection or future validation.
//! - [ ] Safety: chain walking follows decoded `prev` links from persisted bytes
//!       and stops at the first missing or malformed fact.
//! - [ ] Safety: `complete` means all of: the structural walk reached an anchor,
//!       the head's projected root/domain equals that anchor, and replay validated
//!       the requested head.
//! - [ ] Safety: reporting code does not construct, admit, project, or create
//!       validated context.
//! Imported theorems:
//! - `facts::link::project`: link bytes decode to the link semantic shape.
//! - `core::play`: replay soundness gives the validity result for `complete`.
//! - `core::index`: storage reads are untrusted observations.
//! Proof strategy:
//! - Prove `walk` is read-only and follows only decoded `prev` pointers.
//! - Prove `chain_report.complete` is true only when the walk reaches `prev=None`,
//!   the structural anchor agrees with the head's projected root/domain, and
//!   replay reports the head valid.
//! - Prove report fields are returned as display/report data and never fed back
//!   into core projection.
use crate::core::index::Index;
use crate::core::item::FactId;
use crate::core::play::replay;
use crate::core::projector::Projector;
use crate::core::typestate::Validity;

use super::project_unproven::{link_semantic_root, LinkProjector};

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
        w.reached_root
            && w.projected_root == Some(w.root)
            && matches!(memo.get(&head), Some(Validity::Valid))
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
    pub projected_root: Option<FactId>,
    pub ids: Vec<FactId>,
}

/// Read-only walk of the `prev` chain from `head` down. Pure read (no writes).
pub(crate) fn walk(idx: &dyn Index, head: FactId) -> Result<Walk, String> {
    let mut ids = vec![];
    let mut cur = Some(head);
    let mut present = false;
    let mut reached_root = false;
    let mut projected_root = None;
    while let Some(id) = cur {
        let Some(bytes) = idx.load_fact(&id)? else {
            break;
        };
        if id == head {
            present = true;
        }
        let link = LinkProjector::decode(&bytes)?;
        if id == head {
            projected_root = link_semantic_root(&link);
        }
        ids.push(id);
        cur = match link.prev {
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
        projected_root,
        ids,
    })
}
