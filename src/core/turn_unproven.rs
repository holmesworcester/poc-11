//! Deterministic engine turn skeleton. This is the staging surface for a later
//! unsuffixed `turn`: state plus one input produces state plus either a requested
//! helper effect, an internal projection step, or idle.
//!
//! Invariant checklist (Verus):
//! - [ ] `turn` is deterministic for a given engine state.
//! - [ ] `turn` emits at most one effect request or performs at most one internal
//!       projection step.
//! - [ ] Effect requests are selected only from queued work already present in
//!       `EngineState`.
//! - [ ] `apply_effect` interprets loaded facts only through canonical admission
//!       and query results only as enqueue operations.
//! - [ ] `turn_with_storage` preserves the engine invariant for every successful
//!       step.
//! - [ ] `drain` safety follows by induction over repeated `turn_with_storage`
//!       steps; max-step exhaustion is a liveness failure, not a safety failure.
use super::effects::{EffectRequest, EffectResult};
use super::engine::{EngineState, Storage};
use super::item::FactId;
use super::projector::Projector;
use super::typestate::Validity;

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
    match turn(engine)? {
        TurnOutcome::Effect(request) => {
            let result = perform_effect(storage, request)?;
            apply_effect(engine, result)?;
            Ok(true)
        }
        TurnOutcome::Projected { .. } => Ok(true),
        TurnOutcome::Idle => Ok(false),
    }
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
