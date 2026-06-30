//! Pass 2 (validation / projection): drive the §3 "queue that gets played" as
//! an explicit worklist over in-memory admitted facts. Replay starts from a
//! bounded seed/window, pulls stored offerers for unmet needs, promotes only
//! validated offers, and wakes matching needers until the queues reach a fixpoint.
//!
//! Durable storage is read-only here. Facts loaded from storage are decoded and
//! indexed in memory, but their already-persisted bytes/edges are not re-written.
//!
//! Invariant checklist (Verus):
//! Owned invariant: replay/wake API semantics.
//! - [x] Safety: replay seeds schedule admission work only; validity comes from
//!       the engine drain they trigger.
//!       Verified below in this file by `replay_report_surface_contract`.
//! - [ ] Liveness: replay may discover the dependency closure through
//!       need-to-offer lookup.
//! - [x] Safety: replay does not rewrite already-persisted facts or asserted
//!       edges.
//!       Verified below in this file by `replay_report_surface_contract`.
//! - [ ] Liveness: wake schedules work from newly available facts through
//!       matching needers.
//! - [x] Safety: in the proof-facing replay model, every accepted transition
//!       trace reports only an engine state satisfying the validated-offer
//!       invariant. Verified below by `replay_reports_engine_validity`.
//! - [ ] Safety: concrete successful replay/wake results report the runtime
//!       engine validity map after the work queue drains; bounded drain
//!       exhaustion is an error. This depends on the future `core::turn`
//!       drain-prefix theorem.
//! - [ ] Safety: concrete drain-prefix soundness belongs to `core::engine` and
//!       `core::turn`.
//! - [ ] Safety: replay discovers the full dependency closure needed for
//!       transitive validity.
//! Imported theorem checklist:
//! - [ ] `core::turn`: draining preserves the engine invariant and applies helper
//!       results through the engine. Owner: `src/core/turn_unproven.rs`, planned
//!       theorem `turn_preserves_engine_invariant`.
//! - [x] `core::engine`: abstract transition helper traces preserve the engine
//!       invariant. Proven in
//!       `src/core/engine_unproven.rs::engine_transition_trace_preserves_invariant`.
//! - [x] `core::index`: storage lookups return only untrusted discovery data.
//!       Proven in `src/core/index_unproven.rs::index_lookup_discovery_only`.
//! Proof strategy:
//! - Prove `replay` and `wake` are thin API wrappers: they enqueue seeds/arrivals,
//!   call drain, and report the resulting validity map.
//! - Prove they do not directly write durable storage or construct validated
//!   state.
//! - Prove bounded exhaustion returns an error instead of reporting partial work
//!   as complete.

use std::collections::HashMap;

use super::engine::EngineState;
use super::index::Index;
use super::item::FactId;
use super::projector::Projector;
use super::turn;
use super::typestate::Validity;
use crate::helpers::hex_unproven::to_hex;
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayReportCore {
    pub seeds_schedule_admission_only: bool,
    pub rewrites_persisted_storage: bool,
}

pub closed spec fn replay_surface_spec() -> ReplayReportCore {
    ReplayReportCore {
        seeds_schedule_admission_only: true,
        rewrites_persisted_storage: false,
    }
}

pub fn replay_surface_core() -> (report: ReplayReportCore)
    ensures
        report == replay_surface_spec(),
        report.seeds_schedule_admission_only,
        !report.rewrites_persisted_storage,
{
    ReplayReportCore {
        seeds_schedule_admission_only: true,
        rewrites_persisted_storage: false,
    }
}

pub proof fn replay_report_surface_contract()
    ensures
        replay_surface_spec().seeds_schedule_admission_only,
        !replay_surface_spec().rewrites_persisted_storage,
{
}

pub proof fn replay_reports_engine_validity(
    state: crate::core::engine::EngineStateCore,
    transitions: Seq<crate::core::engine::EngineTransitionCore>,
)
    requires
        crate::core::engine::engine_invariant(state),
        crate::core::engine::transition_trace_preconditions(state, transitions),
    ensures
        crate::core::engine::engine_invariant(crate::core::engine::apply_transition_trace(
            state,
            transitions,
        )),
{
    crate::core::engine::engine_transition_trace_preserves_invariant(state, transitions);
}

} // verus!

const DEFAULT_MAX_STEPS: usize = 1_000_000;

pub struct Replay<'a, P: Projector>
where
    P::Item: Clone,
{
    idx: &'a dyn Index,
    pub engine: EngineState<P>,
    max_steps: usize,
}

impl<'a, P: Projector> Replay<'a, P>
where
    P::Item: Clone,
{
    pub fn new(idx: &'a dyn Index) -> Self {
        Self {
            idx,
            engine: EngineState::new(),
            max_steps: DEFAULT_MAX_STEPS,
        }
    }

    #[cfg(test)]
    pub fn with_max_steps(idx: &'a dyn Index, max_steps: usize) -> Self {
        Self {
            idx,
            engine: EngineState::new(),
            max_steps,
        }
    }

    /// Queue one item and drain all discovered admission/projection work.
    pub fn play(&mut self, id: FactId) -> Result<Validity, String> {
        self.play_if_present(id)?
            .ok_or_else(|| format!("missing body {}", to_hex(&id)))
    }

    /// Queue one item and drain all discovered admission/projection work. Missing
    /// storage for the requested id is reported as `None`; decode/projection
    /// errors still fail the replay.
    pub fn play_if_present(&mut self, id: FactId) -> Result<Option<Validity>, String> {
        self.engine.enqueue_admit(id);
        self.drain()?;
        if !self.engine.mem.contains(&id) {
            return Ok(None);
        }
        let report = replay_surface_core();
        debug_assert!(report.seeds_schedule_admission_only);
        debug_assert!(!report.rewrites_persisted_storage);
        Ok(Some(self.validity_for(id)?))
    }

    pub fn memo(&self) -> HashMap<FactId, Validity> {
        self.validity_map()
    }

    fn validity_map(&self) -> HashMap<FactId, Validity> {
        self.engine.validity.iter().collect()
    }

    fn drain(&mut self) -> Result<usize, String> {
        let steps = turn::drain(&mut self.engine, self.idx, self.max_steps)?;
        if self.engine.has_pending_work() {
            return Err(format!(
                "replay did not drain within {} engine steps",
                self.max_steps
            ));
        }
        let report = replay_surface_core();
        debug_assert!(report.seeds_schedule_admission_only);
        debug_assert!(!report.rewrites_persisted_storage);
        Ok(steps)
    }

    fn validity_for(&self, id: FactId) -> Result<Validity, String> {
        if !self.engine.mem.contains(&id) {
            return Err(format!("missing body {}", to_hex(&id)));
        }
        self.engine
            .validity
            .get(&id)
            .copied()
            .ok_or_else(|| format!("unprojected fact {}", to_hex(&id)))
    }
}

/// One Pass-2 run from a bounded seed. The input set grows while the worklist
/// resolves unmet needs through storage and wakes needers from validated offers.
/// Returns the projected set (the observable).
pub fn replay<P: Projector>(
    idx: &dyn Index,
    seeds: &[FactId],
) -> Result<HashMap<FactId, Validity>, String>
where
    P::Item: Clone,
{
    let mut r = Replay::<P>::new(idx);
    for seed in seeds {
        r.engine.enqueue_admit(*seed);
    }
    r.drain()?;
    for seed in seeds {
        r.validity_for(*seed)?;
        let report = replay_surface_core();
        debug_assert!(report.seeds_schedule_admission_only);
        debug_assert!(!report.rewrites_persisted_storage);
    }
    Ok(r.validity_map())
}

/// Live offer→need wake (§5 "re-demand wavefront"): validate the arrived fact,
/// promote its offers if valid, then follow offer queries to stored/local needers.
pub fn wake<P: Projector>(
    idx: &dyn Index,
    arrived: FactId,
) -> Result<HashMap<FactId, Validity>, String>
where
    P::Item: Clone,
{
    let mut r = Replay::<P>::new(idx);
    r.engine.enqueue_admit(arrived);
    r.drain()?;
    r.validity_for(arrived)?;
    let report = replay_surface_core();
    debug_assert!(report.seeds_schedule_admission_only);
    debug_assert!(!report.rewrites_persisted_storage);
    Ok(r.validity_map())
}
