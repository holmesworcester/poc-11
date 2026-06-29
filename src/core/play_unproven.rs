//! Pass 2 (validation / projection): drive the §3 "queue that gets played" as
//! an explicit worklist over in-memory admitted facts. Replay starts from a
//! bounded seed/window, pulls stored offerers for unmet needs, promotes only
//! validated offers, and wakes matching needers until the queues reach a fixpoint.
//!
//! Durable storage is read-only here. Facts loaded from storage are decoded and
//! indexed in memory, but their already-persisted bytes/edges are not re-written.
//!
//! Invariant checklist (Verus):
//! - [ ] Replay seeds only enqueue admission; they do not create validity.
//! - [ ] Replay and wake drive the same proven turn transition as live draining.
//! - [ ] Storage facts pulled during replay are admitted read-only through the
//!       engine's canonical load/decode path.
//! - [ ] The returned memo contains only facts projected by the engine.
//! - [ ] A successful replay proves each requested seed is admitted and projected.
//! - [ ] A step bound can stop liveness/completeness, but it cannot create an
//!       unsound valid fact.
//! - [ ] Wake is the offer-to-need direction: it validates the arrived fact, then
//!       relies on core offer-query turns to discover needers.

use std::collections::HashMap;

use super::engine::EngineState;
use super::index::Index;
use super::item::FactId;
use super::projector::Projector;
use super::turn;
use super::typestate::Validity;
use crate::helpers::hex_unproven::to_hex;

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
        self.engine.enqueue_admit(id);
        self.drain()?;
        self.validity_for(id)
    }

    pub fn memo(&self) -> &HashMap<FactId, Validity> {
        &self.engine.validity
    }

    fn drain(&mut self) -> Result<usize, String> {
        let steps = turn::drain(&mut self.engine, self.idx, self.max_steps)?;
        if self.engine.has_pending_work() {
            return Err(format!(
                "replay did not drain within {} engine steps",
                self.max_steps
            ));
        }
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
    }
    Ok(r.engine.validity)
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
    Ok(r.engine.validity)
}
