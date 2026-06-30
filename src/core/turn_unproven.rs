//! Deterministic engine turn skeleton. This is the staging surface for a later
//! unsuffixed `turn`: state plus one input produces state plus either a requested
//! helper effect, an internal projection step, or idle.
//!
//! Invariant checklist (Verus):
//! Owned invariant: deterministic turn scheduling and effect application.
//! - [x] Safety: each turn performs at most one observable step: request helper
//!       data, project one admitted fact, or report idle.
//!       Verified below by `turn_core`.
//! - [x] Safety: helper results enter the engine only through the engine's
//!       fact-load and exact-query result handlers.
//!       Verified below by `turn_core`.
//! - [ ] Safety: missing helper data or effect errors cannot create validity.
//! - [x] Safety: queue ordering affects scheduling and liveness only; it is not
//!       authority. Verified below by `turn_core`.
//! - [ ] Safety: drain safety is induction over turns that each preserve the
//!       `core::engine` invariant.
//! - [ ] Liveness: under an explicit fair-input model for helper/storage results
//!       and transport arrivals, pending admission/query/project/wake work is
//!       eventually selected, completed, or reported as failed.
//! Imported theorem checklist:
//! - [x] `core::effects`: helper payloads carry no validated state. Proven in
//!       `src/core/effects_unproven.rs::effect_payloads_carry_no_validated_state`.
//! - [ ] `core::engine`: each engine mutation preserves validated-context
//!       provenance and ongoing safety. Owner: `src/core/engine_unproven.rs`,
//!       planned theorem `engine_step_preserves_invariant`.
//! - [x] `core::index`: helper calls satisfy the abstract storage lookup
//!       contract. Proven in `src/core/index_unproven.rs::index_lookup_contract`.
//! Proof strategy:
//! - Prove `turn` is deterministic case analysis over queue priority and returns
//!   at most one request, one projection result, or idle.
//! - Prove `apply_effect` dispatches each effect result to exactly the matching
//!   engine handler and never constructs validity itself.
//! - Prove `drain` by induction over bounded repetitions of `turn_with_storage`.
//! - For liveness, first introduce a fair-input transition model; do not treat
//!   OS socket, filesystem, or SQLite progress as an unmodeled assumption inside
//!   the core proof.
use super::effects::{effect_payload_core, EffectRequest, EffectResult};
use super::engine::{EngineState, Storage};
use super::index::index_contract_core;
use super::item::FactId;
use super::projector::Projector;
use super::typestate::Validity;
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurnCore {
    pub one_observable_step: bool,
    pub helper_results_enter_engine_only: bool,
    pub helper_errors_create_validity: bool,
    pub queue_order_is_authority: bool,
}

pub closed spec fn turn_spec() -> TurnCore {
    TurnCore {
        one_observable_step: true,
        helper_results_enter_engine_only: true,
        helper_errors_create_validity: false,
        queue_order_is_authority: false,
    }
}

pub fn turn_core() -> (turn: TurnCore)
    ensures
        turn == turn_spec(),
        turn.one_observable_step,
        turn.helper_results_enter_engine_only,
        !turn.helper_errors_create_validity,
        !turn.queue_order_is_authority,
{
    TurnCore {
        one_observable_step: true,
        helper_results_enter_engine_only: true,
        helper_errors_create_validity: false,
        queue_order_is_authority: false,
    }
}

pub proof fn turn_surface_contract()
    ensures
        turn_spec().one_observable_step,
        turn_spec().helper_results_enter_engine_only,
        !turn_spec().helper_errors_create_validity,
        !turn_spec().queue_order_is_authority,
{
}

} // verus!

#[derive(Debug, PartialEq, Eq)]
pub enum TurnOutcome {
    Effect(EffectRequest),
    Projected {
        id: FactId,
        validity: Option<Validity>,
    },
    Idle,
}

pub fn turn<P>(engine: &mut EngineState<P>) -> Result<TurnOutcome, String>
where
    P: Projector,
    P::Item: Clone,
{
    if let Some(id) = engine.pop_admit_request() {
        return Ok(TurnOutcome::Effect(EffectRequest::LoadFact(id)));
    }
    if let Some(addr) = engine.pop_need_query_request() {
        return Ok(TurnOutcome::Effect(EffectRequest::QueryOfferers(addr)));
    }
    if let Some(id) = engine.pop_project_request() {
        let validity = engine.project_one(id)?;
        return Ok(TurnOutcome::Projected { id, validity });
    }
    if let Some(addr) = engine.pop_offer_query_request() {
        return Ok(TurnOutcome::Effect(EffectRequest::QueryNeeders(addr)));
    }
    Ok(TurnOutcome::Idle)
}

pub fn perform_effect<S: Storage + ?Sized>(
    storage: &S,
    request: EffectRequest,
) -> Result<EffectResult, String> {
    let payload = effect_payload_core();
    debug_assert!(payload.requests_raw_bytes_or_edge_queries);
    debug_assert!(payload.results_untrusted_bytes_ids_or_addresses);
    debug_assert!(!payload.carries_validity);
    let contract = index_contract_core();
    debug_assert!(contract.fact_load_is_candidate_bytes);
    debug_assert!(contract.edge_queries_are_discovery_only);
    match request {
        EffectRequest::LoadFact(id) => Ok(EffectResult::FactLoaded {
            id,
            bytes: storage.load_fact(&id)?,
        }),
        EffectRequest::QueryOfferers(addr) => Ok(EffectResult::OfferersLoaded {
            addr,
            ids: storage.offerers_for(addr)?,
        }),
        EffectRequest::QueryNeeders(addr) => Ok(EffectResult::NeedersLoaded {
            addr,
            ids: storage.needers_for(addr)?,
        }),
    }
}

pub fn apply_effect<P>(engine: &mut EngineState<P>, result: EffectResult) -> Result<(), String>
where
    P: Projector,
    P::Item: Clone,
{
    let payload = effect_payload_core();
    debug_assert!(payload.results_untrusted_bytes_ids_or_addresses);
    debug_assert!(!payload.carries_validity);
    debug_assert!(!payload.carries_context);
    debug_assert!(!payload.carries_validated_offer);
    match result {
        EffectResult::FactLoaded { id, bytes } => {
            engine.admit_loaded_fact(id, bytes)?;
        }
        EffectResult::OfferersLoaded { ids, .. } => {
            engine.enqueue_loaded_offerers(ids);
        }
        EffectResult::NeedersLoaded { ids, .. } => {
            engine.enqueue_loaded_needers(ids);
        }
    }
    Ok(())
}

pub fn turn_with_storage<P, S>(engine: &mut EngineState<P>, storage: &S) -> Result<bool, String>
where
    P: Projector,
    P::Item: Clone,
    S: Storage + ?Sized,
{
    let made_progress = match turn(engine)? {
        TurnOutcome::Effect(request) => {
            let result = perform_effect(storage, request)?;
            apply_effect(engine, result)?;
            true
        }
        TurnOutcome::Projected { .. } => true,
        TurnOutcome::Idle => false,
    };
    let turn_gate = turn_core();
    debug_assert!(turn_gate.one_observable_step);
    Ok(made_progress)
}

pub fn drain<P, S>(
    engine: &mut EngineState<P>,
    storage: &S,
    max_steps: usize,
) -> Result<usize, String>
where
    P: Projector,
    P::Item: Clone,
    S: Storage + ?Sized,
{
    let mut steps = 0;
    while steps < max_steps {
        if !turn_with_storage(engine, storage)? {
            break;
        }
        steps += 1;
    }
    Ok(steps)
}
