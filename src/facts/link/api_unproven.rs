//! Link read/report helpers. These are app-facing and storage-backed, so they are
//! unproven until the storage/result contract is moved behind verified effects.
//!
//! Fact-family contract (do not weaken):
//! - Scope: observation/report layer only.
//! - Allowed here: run core replay for the requested fact, read the
//!   projector-maintained `LinkState` produced by that replay, and format report
//!   data.
//! - Forbidden here: fact construction, admission, storage writes, direct projector
//!   execution, direct persisted-byte chain walking, creation of `Validity`,
//!   creation of `Context`, and creation of `Offer<Validated>`.
//! - Report fields are observations. They are not proof witnesses and must not be
//!   used as inputs to core validity or link projection theorems.
//!
//! Invariant checklist (Verus):
//! Owned invariant: link reporting boundary.
//! - [ ] Safety: reports are observations for users; they are never authority for
//!       projection or future validation.
//! - [ ] Safety: report fields are read from projector-maintained `LinkState`
//!       after replay; this module does not compute them by walking persisted
//!       bytes.
//! - [ ] Safety: missing requested facts return `present=false`; malformed facts
//!       return a replay/decode error before any report can be produced.
//! - [ ] Safety: `complete` means replay projected the requested head valid and
//!       the link projector state contains a complete same-root chain report for
//!       that head.
//! - [ ] Safety: reporting code does not construct, admit, project, or create
//!       validated context.
//! Imported theorem checklist:
//! - [x] `facts::link::project`: `LinkState.projected` is updated only by link
//!       projection and records complete same-root chains. Proven in
//!       `src/facts/link/project.rs::complete_child_report_requires_complete_same_root_parent`
//!       and `src/facts/link/project.rs::apply_update_is_insert_ignore_by_link_id`.
//! - [x] `core::play`: replay soundness gives the validity result and projected
//!       state for `complete`. Proven in
//!       `src/core/play_unproven.rs::replay_reports_engine_validity`.
//! - [x] `core::index`: storage reads are untrusted observations. Proven in
//!       `src/core/index_unproven.rs::index_lookup_discovery_only`.
//! Proof strategy:
//! - Prove `chain_report` calls replay first, then reads the requested head's
//!   report from `LinkState.projected`.
//! - Prove `chain_report.complete` is true only when replay returns the head as
//!   valid and the projector-maintained report is complete.
//! - Prove report fields are returned as display/report data and never fed back
//!   into core projection.
use crate::core::index::Index;
use crate::core::item::FactId;
use crate::core::play::Replay;
use crate::core::typestate::Validity;

use super::project::LinkProjector;

/// A user-facing view of the projected chain ending at `head`.
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
    let mut replay = Replay::<LinkProjector>::new(idx);
    let Some(validity) = replay.play_if_present(head)? else {
        return Ok(Report {
            present: false,
            complete: false,
            root: head,
            depth: 0,
            length: 0,
            ids: vec![],
        });
    };
    let Some(projected) = replay.engine.projector_state.projected.get(&head) else {
        return Ok(Report {
            present: true,
            complete: false,
            root: head,
            depth: 0,
            length: 1,
            ids: vec![head],
        });
    };

    Ok(Report {
        present: true,
        complete: validity == Validity::Valid && projected.complete,
        root: projected.root,
        depth: projected.depth,
        length: projected.length,
        ids: projected.ids.clone(),
    })
}
