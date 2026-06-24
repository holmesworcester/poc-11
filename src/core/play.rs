//! Pass 2 (validation / projection): the §3 "queue that gets played" realized as
//! demand-driven recursion — the call stack IS the queue. To project an item,
//! ensure everything it depends on is projected first (resolve its needs to
//! offerers via the index, pulling in old facts), then `project` it, then play
//! anything it emits. `memo` makes each item project once; `on_stack` locates a
//! suppression/stratification cycle at the exact offending item.
use std::collections::{HashMap, HashSet};

use super::admit::admit;
use super::index::Index;
use super::item::{to_hex, FactId};
use super::offer::{Key, Offer, Role, Scope};
use super::projector::Projector;
use super::typestate::{Context, Validated, Validity};

pub struct Replay<'a, P: Projector> {
    idx: &'a dyn Index,
    /// The projector's private read-model, built this pass.
    pub state: P::State,
    /// item → result, this pass; "in memo" == "fully projected".
    pub memo: HashMap<FactId, Validity>,
    /// cycle detection.
    on_stack: HashSet<FactId>,
    /// promoted offers — the validated-context bus.
    validated: Vec<Offer<Validated>>,
    /// synthetic ts for the idempotent re-admits this pass performs.
    next_ts: u64,
}

impl<'a, P: Projector> Replay<'a, P> {
    pub fn new(idx: &'a dyn Index) -> Self {
        Self {
            idx,
            state: P::State::default(),
            memo: HashMap::new(),
            on_stack: HashSet::new(),
            validated: Vec::new(),
            next_ts: 0,
        }
    }

    /// Play one §3 queue item, recursing into its context first.
    pub fn play(&mut self, id: FactId) -> Result<Validity, String> {
        if let Some(v) = self.memo.get(&id) {
            return Ok(*v);
        }
        if !self.on_stack.insert(id) {
            return Err(format!("SuppressionCycle at {}", to_hex(&id)));
        }

        // resolve(addr) → Admitted: load the body and re-admit (idempotent).
        let bytes = self
            .idx
            .load_fact(&id)?
            .ok_or_else(|| format!("missing body {}", to_hex(&id)))?;
        self.next_ts += 1;
        let admitted = admit::<P>(P::decode(&bytes)?, self.next_ts, self.idx)?;
        let edges = P::extract(admitted.item());

        // Context first: for each need, play its offerers (pulling old facts in).
        for need in edges.iter().copied().filter(|o| o.is_need()) {
            for provider in self.idx.offers_for_key(need.role, need.scope, &need.key)? {
                self.play(provider)?;
            }
        }

        // Project with validated context now ready.
        let out = P::project(&admitted, self.collect(&edges), &mut self.state);

        // Promote this item's offers to Validated iff it projected valid.
        if out.validity == Validity::Valid {
            for offer in edges.iter().copied().filter(|o| o.is_offer()) {
                self.validated.push(offer.validate());
            }
        }

        // Emitted facts re-enter the pipeline (the link emits none).
        for ef in out.emitted {
            self.next_ts += 1;
            let a = admit::<P>(P::decode(&ef.bytes)?, self.next_ts, self.idx)?;
            self.play(a.id())?;
        }

        self.on_stack.remove(&id);
        self.memo.insert(id, out.validity);
        Ok(out.validity)
    }

    /// Build `Context` from the validated offers this item's needs name.
    fn collect(&self, edges: &[Offer<super::typestate::Asserted>]) -> Context {
        let mut offers = vec![];
        for need in edges.iter().filter(|o| o.is_need()) {
            for v in self
                .validated
                .iter()
                .filter(|v| v.role == need.role && v.key == need.key)
            {
                offers.push(*v);
            }
        }
        Context::from(offers)
    }
}

/// One Pass-2 run from a bounded seed. Fresh memo each call: confluence makes the
/// outer order irrelevant. Returns the projected set (the observable).
pub fn replay<P: Projector>(
    idx: &dyn Index,
    seeds: &[FactId],
) -> Result<HashMap<FactId, Validity>, String> {
    let mut r = Replay::<P>::new(idx);
    for s in seeds {
        r.play(*s)?;
    }
    Ok(r.memo)
}

/// Live offer→need wake (§5 "re-demand wavefront"): the forward dual of `replay`.
/// A newly-available fact can validate facts that NEED what it offers, and those,
/// once present, wake *their* needers — one hop per level. We discover that
/// affected set by following offer→need links through the index (the reverse key),
/// then re-derive it in one fresh confluent pass.
pub fn wake<P: Projector>(
    idx: &dyn Index,
    arrived: FactId,
) -> Result<HashMap<FactId, Validity>, String> {
    let mut affected = vec![arrived];
    let mut seen: HashSet<FactId> = HashSet::new();
    seen.insert(arrived);
    let mut queue = vec![arrived];
    while let Some(f) = queue.pop() {
        for (role, scope, key) in offered_keys::<P>(idx, f)? {
            for needer in idx.needs_for_key(role, scope, &key)? {
                if seen.insert(needer) {
                    affected.push(needer);
                    queue.push(needer);
                }
            }
        }
    }
    replay::<P>(idx, &affected)
}

/// The (role, scope, key) of every offer `f` makes — its syntactic offers, used to
/// find who needs `f`. Validity-free, so the affected set may slightly
/// over-approximate; re-deriving an unaffected fact is a harmless no-op.
fn offered_keys<P: Projector>(
    idx: &dyn Index,
    f: FactId,
) -> Result<Vec<(Role, Scope, Key)>, String> {
    let bytes = idx
        .load_fact(&f)?
        .ok_or_else(|| format!("missing body {}", to_hex(&f)))?;
    let item = P::decode(&bytes)?;
    Ok(P::extract(&item)
        .into_iter()
        .filter(|o| o.is_offer())
        .map(|o| (o.role, o.scope, o.key))
        .collect())
}
