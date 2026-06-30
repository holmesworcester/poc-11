//! Queue-oriented in-memory engine model. Durable storage remains behind
//! [`Storage`]; this module owns the proof-facing state split:
//!
//! - `to_admit`: load/decode/index facts into memory.
//! - `to_project`: validate already-admitted facts.
//! - need queries: pull stored offerers for newly indexed needs.
//! - offer queries: wake stored/local needers for newly validated offers.
//!
//! Projection returns family-private read-model updates, but promotion is checked
//! directly against this engine's running state: the engine may promote asserted
//! offers and admit emitted facts only for the same fact whose projector returned
//! `Valid` after all asserted needs were satisfied by validated context.
//!
//! Invariant checklist (Verus):
//! Owned invariant: validated-context provenance and ongoing engine safety.
//! - [ ] Safety: every runtime in-memory fact is paired with the id derived from
//!       its canonical bytes before the engine hands it to a projector as an
//!       `Admitted` token.
//! - [ ] Safety: storage lookup results are discovery hints only; they cannot
//!       mark a fact valid or promote an offer.
//! - [ ] Safety: a projector is called only after every asserted need has a
//!       matching validated offer; it receives only validated offers whose
//!       addresses match needs asserted by the fact being projected.
//! - [x] Safety: in the proof-facing transition model, every validated offer is
//!       owned by a fact already projected valid and was asserted by that same
//!       owner. Verified below by `engine_transition_trace_preserves_invariant`.
//! - [ ] Safety: family-private projector state is not authority: a projector may
//!       return state updates only for the fact being projected, and the engine
//!       promotes offers and emitted facts only after readiness and projector
//!       validity are both established.
//! - [x] Safety: in the proof-facing transition model, one owner contributes at
//!       most one validated offer for a given match address. Verified below by
//!       `engine_transition_trace_preserves_invariant`.
//! - [x] Safety: in the proof-facing transition model, raw bytes returned in
//!       `ProjectOutcome.emitted` do not inherit authority from the emitting fact;
//!       they re-enter the admission queue. Verified below by
//!       `emitted_raw_fact_reenters_admission_queue`.
//! - [x] Safety: every proof-facing admit/query/project/promote/emit transition
//!       preserves these invariants, so every modeled transition prefix is sound.
//!       Verified below by `engine_single_transition_preserves_invariant` and
//!       `engine_transition_trace_preserves_invariant`.
//! - [ ] Safety: the concrete runtime `EngineState` HashMap/HashSet/VecDeque
//!       implementation refines the proof-facing transition model.
//! Imported theorem checklist:
//! - [x] `core::item`: fact ids identify canonical bytes. Proven in
//!       `src/core/item_unproven.rs::fact_id_content_address`.
//! - [x] `core::offer`: asserted-to-validated promotion preserves edge address
//!       and metadata. Proven in
//!       `src/core/offer_unproven.rs::validate_preserves_offer_address`.
//! - [x] `core::typestate`: `Context` contains only validated offers and exact
//!       match lookup has no storage/body access. Proven in
//!       `src/core/typestate_unproven.rs::context_validated_only` and
//!       `src/core/typestate_unproven.rs::context_lookup_exact`.
//! - [x] Local engine promotion/context gate shape. Proven below by
//!       `src/core/engine_unproven.rs::engine_transition_preserves_validated_context_provenance`
//!       and `src/core/engine_unproven.rs::engine_transition_trace_preserves_invariant`.
//! - [x] `core::projector`: the selected fact family supplies canonical codec,
//!       extraction, durability, projection contracts, and emitted bytes as raw
//!       `EmittedFact` payloads. Proven in
//!       `src/core/projector_unproven.rs::projector_interface_contract`.
//! Proof strategy:
//! - Maintain the proof model and state predicate over memory facts, asserted
//!   edges, validity, validated offers, promoted offer keys, and queues.
//! - Prove each proof-facing transition preserves the predicate: in-memory
//!   admission, storage load result, need-query result, projection, raw
//!   emitted-byte admission, and offer-query result. The load/query transitions
//!   may enqueue additional ids or addresses to inspect, but they do not mutate
//!   validity or validated offers.
//! - For projection, prove readiness first, build context only from matching
//!   validated offers, run the projector, reject any update whose owner is not the
//!   projected fact, apply returned family-private updates through
//!   `P::apply_update`, and promote asserted offers only when projector validity
//!   is `Valid`.
//! - Prove modeled drain safety by induction over transition steps; this is now
//!   the `engine_transition_trace_preserves_invariant` theorem. The remaining
//!   open work is proving the concrete runtime queues/maps refine this model.
//!   Prove completeness or liveness separately from safety.

use std::collections::{HashMap, HashSet, VecDeque};

use super::admit::Admitted;
use super::index::Index;
use super::item::{fact_id, FactId};
use super::offer::{Key, Offer, Role, Scope};
use super::projector::{projector_interface_core, Projector};
use super::typestate::{Asserted, Context, Validated, Validity};
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineIdCore {
    pub w0: u64,
    pub w1: u64,
    pub w2: u64,
    pub w3: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineAddrCore {
    pub role: u64,
    pub scope: u64,
    pub key_subject: EngineIdCore,
    pub key_domain: EngineIdCore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineEdgeKindCore {
    Need,
    Offer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineEdgeCore {
    pub owner: EngineIdCore,
    pub addr: EngineAddrCore,
    pub kind: EngineEdgeKindCore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineValidatedOfferCore {
    pub owner: EngineIdCore,
    pub addr: EngineAddrCore,
}

pub struct EngineStateCore {
    pub admitted: Seq<EngineIdCore>,
    pub asserted: Seq<EngineEdgeCore>,
    pub valid: Seq<EngineIdCore>,
    pub validated: Seq<EngineValidatedOfferCore>,
    pub to_admit: Seq<EngineIdCore>,
    pub to_project: Seq<EngineIdCore>,
    pub need_queries: Seq<EngineAddrCore>,
    pub offer_queries: Seq<EngineAddrCore>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineTransitionCore {
    EnqueueAdmit(EngineIdCore),
    AdmitCanonicalFact(EngineIdCore),
    IndexAssertedEdge(EngineEdgeCore),
    QueryResultEnqueue(EngineIdCore),
    ProjectValid(EngineIdCore),
    PromoteOffer(EngineIdCore, EngineAddrCore),
    EmitRawFact(EngineIdCore),
}

pub closed spec fn contains_id(ids: Seq<EngineIdCore>, id: EngineIdCore) -> bool {
    exists |i: int| 0 <= i < ids.len() && ids[i] == id
}

pub closed spec fn asserted_offer_for(
    asserted: Seq<EngineEdgeCore>,
    owner: EngineIdCore,
    addr: EngineAddrCore,
) -> bool {
    exists |i: int|
        0 <= i < asserted.len()
            && asserted[i].owner == owner
            && asserted[i].addr == addr
            && asserted[i].kind == EngineEdgeKindCore::Offer
}

pub closed spec fn validated_offer_for(
    validated: Seq<EngineValidatedOfferCore>,
    owner: EngineIdCore,
    addr: EngineAddrCore,
) -> bool {
    exists |i: int| 0 <= i < validated.len() && validated[i].owner == owner && validated[i].addr == addr
}

pub closed spec fn validated_offer_provenance(state: EngineStateCore) -> bool {
    forall |i: int|
        0 <= i < state.validated.len() ==>
            contains_id(state.valid, #[trigger] state.validated[i].owner)
                && asserted_offer_for(
                    state.asserted,
                    state.validated[i].owner,
                    state.validated[i].addr,
                )
}

pub closed spec fn promoted_offer_unique_per_owner_addr(state: EngineStateCore) -> bool {
    forall |i: int, j: int|
        0 <= i < state.validated.len() && 0 <= j < state.validated.len() && i != j ==>
            !(#[trigger] state.validated[i] == #[trigger] state.validated[j]
                || state.validated[i].owner == state.validated[j].owner
                && state.validated[i].addr == state.validated[j].addr)
}

pub closed spec fn engine_invariant(state: EngineStateCore) -> bool {
    validated_offer_provenance(state) && promoted_offer_unique_per_owner_addr(state)
}

pub proof fn contains_id_push_preserves_existing(
    ids: Seq<EngineIdCore>,
    id: EngineIdCore,
    pushed: EngineIdCore,
)
    requires
        contains_id(ids, id),
    ensures
        contains_id(ids.push(pushed), id),
{
    let i = choose |i: int| 0 <= i < ids.len() && ids[i] == id;
    assert(ids.push(pushed)[i] == ids[i]);
}

pub proof fn asserted_offer_push_preserves_existing(
    asserted: Seq<EngineEdgeCore>,
    owner: EngineIdCore,
    addr: EngineAddrCore,
    pushed: EngineEdgeCore,
)
    requires
        asserted_offer_for(asserted, owner, addr),
    ensures
        asserted_offer_for(asserted.push(pushed), owner, addr),
{
    let i = choose |i: int|
        0 <= i < asserted.len()
            && asserted[i].owner == owner
            && asserted[i].addr == addr
            && asserted[i].kind == EngineEdgeKindCore::Offer;
    assert(asserted.push(pushed)[i] == asserted[i]);
}

pub proof fn validated_offer_push_adds_offer(
    validated: Seq<EngineValidatedOfferCore>,
    owner: EngineIdCore,
    addr: EngineAddrCore,
)
    ensures
        validated_offer_for(validated.push(EngineValidatedOfferCore { owner, addr }), owner, addr),
{
    let i = validated.len() as int;
    assert(validated.push(EngineValidatedOfferCore { owner, addr })[i] == EngineValidatedOfferCore { owner, addr });
}

pub closed spec fn empty_engine_state() -> EngineStateCore {
    EngineStateCore {
        admitted: Seq::empty(),
        asserted: Seq::empty(),
        valid: Seq::empty(),
        validated: Seq::empty(),
        to_admit: Seq::empty(),
        to_project: Seq::empty(),
        need_queries: Seq::empty(),
        offer_queries: Seq::empty(),
    }
}

pub closed spec fn state_enqueue_admit(
    state: EngineStateCore,
    id: EngineIdCore,
) -> EngineStateCore {
    EngineStateCore {
        admitted: state.admitted,
        asserted: state.asserted,
        valid: state.valid,
        validated: state.validated,
        to_admit: state.to_admit.push(id),
        to_project: state.to_project,
        need_queries: state.need_queries,
        offer_queries: state.offer_queries,
    }
}

pub closed spec fn state_admit_canonical_fact(
    state: EngineStateCore,
    id: EngineIdCore,
) -> EngineStateCore {
    EngineStateCore {
        admitted: state.admitted.push(id),
        asserted: state.asserted,
        valid: state.valid,
        validated: state.validated,
        to_admit: state.to_admit,
        to_project: state.to_project.push(id),
        need_queries: state.need_queries,
        offer_queries: state.offer_queries,
    }
}

pub closed spec fn state_index_asserted_edge(
    state: EngineStateCore,
    edge: EngineEdgeCore,
) -> EngineStateCore {
    EngineStateCore {
        admitted: state.admitted,
        asserted: state.asserted.push(edge),
        valid: state.valid,
        validated: state.validated,
        to_admit: state.to_admit,
        to_project: state.to_project,
        need_queries: if edge.kind == EngineEdgeKindCore::Need {
            state.need_queries.push(edge.addr)
        } else {
            state.need_queries
        },
        offer_queries: state.offer_queries,
    }
}

pub closed spec fn state_query_result_enqueue(
    state: EngineStateCore,
    id: EngineIdCore,
) -> EngineStateCore {
    EngineStateCore {
        admitted: state.admitted,
        asserted: state.asserted,
        valid: state.valid,
        validated: state.validated,
        to_admit: state.to_admit.push(id),
        to_project: state.to_project.push(id),
        need_queries: state.need_queries,
        offer_queries: state.offer_queries,
    }
}

pub closed spec fn state_project_valid(
    state: EngineStateCore,
    id: EngineIdCore,
) -> EngineStateCore {
    EngineStateCore {
        admitted: state.admitted,
        asserted: state.asserted,
        valid: state.valid.push(id),
        validated: state.validated,
        to_admit: state.to_admit,
        to_project: state.to_project,
        need_queries: state.need_queries,
        offer_queries: state.offer_queries,
    }
}

pub closed spec fn state_promote_offer(
    state: EngineStateCore,
    owner: EngineIdCore,
    addr: EngineAddrCore,
) -> EngineStateCore {
    EngineStateCore {
        admitted: state.admitted,
        asserted: state.asserted,
        valid: state.valid,
        validated: state.validated.push(EngineValidatedOfferCore { owner, addr }),
        to_admit: state.to_admit,
        to_project: state.to_project,
        need_queries: state.need_queries,
        offer_queries: state.offer_queries.push(addr),
    }
}

pub closed spec fn state_emit_raw_fact(
    state: EngineStateCore,
    id: EngineIdCore,
) -> EngineStateCore {
    state_enqueue_admit(state, id)
}

pub closed spec fn transition_precondition(
    state: EngineStateCore,
    transition: EngineTransitionCore,
) -> bool {
    match transition {
        EngineTransitionCore::PromoteOffer(owner, addr) => {
            contains_id(state.valid, owner)
                && asserted_offer_for(state.asserted, owner, addr)
                && !validated_offer_for(state.validated, owner, addr)
        }
        _ => true,
    }
}

pub closed spec fn apply_transition(
    state: EngineStateCore,
    transition: EngineTransitionCore,
) -> EngineStateCore {
    match transition {
        EngineTransitionCore::EnqueueAdmit(id) => state_enqueue_admit(state, id),
        EngineTransitionCore::AdmitCanonicalFact(id) => state_admit_canonical_fact(state, id),
        EngineTransitionCore::IndexAssertedEdge(edge) => state_index_asserted_edge(state, edge),
        EngineTransitionCore::QueryResultEnqueue(id) => state_query_result_enqueue(state, id),
        EngineTransitionCore::ProjectValid(id) => state_project_valid(state, id),
        EngineTransitionCore::PromoteOffer(owner, addr) => state_promote_offer(state, owner, addr),
        EngineTransitionCore::EmitRawFact(id) => state_emit_raw_fact(state, id),
    }
}

pub closed spec fn transition_trace_preconditions(
    state: EngineStateCore,
    transitions: Seq<EngineTransitionCore>,
) -> bool
    decreases transitions.len(),
{
    if transitions.len() == 0 {
        true
    } else {
        let transition = transitions[0];
        transition_precondition(state, transition)
            && transition_trace_preconditions(
                apply_transition(state, transition),
                transitions.subrange(1, transitions.len() as int),
            )
    }
}

pub closed spec fn apply_transition_trace(
    state: EngineStateCore,
    transitions: Seq<EngineTransitionCore>,
) -> EngineStateCore
    decreases transitions.len(),
{
    if transitions.len() == 0 {
        state
    } else {
        let transition = transitions[0];
        apply_transition_trace(
            apply_transition(state, transition),
            transitions.subrange(1, transitions.len() as int),
        )
    }
}

pub proof fn empty_engine_state_satisfies_invariant()
    ensures
        engine_invariant(empty_engine_state()),
{
}

pub proof fn enqueue_admit_preserves_invariant(state: EngineStateCore, id: EngineIdCore)
    requires
        engine_invariant(state),
    ensures
        engine_invariant(state_enqueue_admit(state, id)),
{
}

pub proof fn admit_canonical_fact_preserves_invariant(state: EngineStateCore, id: EngineIdCore)
    requires
        engine_invariant(state),
    ensures
        engine_invariant(state_admit_canonical_fact(state, id)),
{
}

pub proof fn index_asserted_edge_preserves_invariant(state: EngineStateCore, edge: EngineEdgeCore)
    requires
        engine_invariant(state),
    ensures
        engine_invariant(state_index_asserted_edge(state, edge)),
{
    assert forall |i: int| 0 <= i < state.validated.len() implies
        contains_id(state_index_asserted_edge(state, edge).valid, #[trigger] state_index_asserted_edge(state, edge).validated[i].owner)
            && asserted_offer_for(
                state_index_asserted_edge(state, edge).asserted,
                state_index_asserted_edge(state, edge).validated[i].owner,
                state_index_asserted_edge(state, edge).validated[i].addr,
            )
    by {
        assert(contains_id(state.valid, state.validated[i].owner));
        assert(asserted_offer_for(state.asserted, state.validated[i].owner, state.validated[i].addr));
        asserted_offer_push_preserves_existing(
            state.asserted,
            state.validated[i].owner,
            state.validated[i].addr,
            edge,
        );
    }
}

pub proof fn query_result_enqueue_preserves_invariant(state: EngineStateCore, id: EngineIdCore)
    requires
        engine_invariant(state),
    ensures
        engine_invariant(state_query_result_enqueue(state, id)),
{
}

pub proof fn project_valid_preserves_invariant(state: EngineStateCore, id: EngineIdCore)
    requires
        engine_invariant(state),
    ensures
        engine_invariant(state_project_valid(state, id)),
{
    assert forall |i: int| 0 <= i < state.validated.len() implies
        contains_id(state_project_valid(state, id).valid, #[trigger] state_project_valid(state, id).validated[i].owner)
            && asserted_offer_for(
                state_project_valid(state, id).asserted,
                state_project_valid(state, id).validated[i].owner,
                state_project_valid(state, id).validated[i].addr,
            )
    by {
        assert(contains_id(state.valid, state.validated[i].owner));
        contains_id_push_preserves_existing(state.valid, state.validated[i].owner, id);
        assert(asserted_offer_for(state.asserted, state.validated[i].owner, state.validated[i].addr));
    }
}

pub proof fn promote_offer_preserves_invariant(
    state: EngineStateCore,
    owner: EngineIdCore,
    addr: EngineAddrCore,
)
    requires
        engine_invariant(state),
        contains_id(state.valid, owner),
        asserted_offer_for(state.asserted, owner, addr),
        !validated_offer_for(state.validated, owner, addr),
    ensures
        engine_invariant(state_promote_offer(state, owner, addr)),
{
}

pub proof fn emitted_raw_fact_reenters_admission_queue(
    state: EngineStateCore,
    id: EngineIdCore,
)
    requires
        engine_invariant(state),
    ensures
        engine_invariant(state_emit_raw_fact(state, id)),
{
}

pub proof fn engine_single_transition_preserves_invariant(
    state: EngineStateCore,
    transition: EngineTransitionCore,
)
    requires
        engine_invariant(state),
        transition_precondition(state, transition),
    ensures
        engine_invariant(apply_transition(state, transition)),
{
    match transition {
        EngineTransitionCore::EnqueueAdmit(id) => {
            enqueue_admit_preserves_invariant(state, id);
        }
        EngineTransitionCore::AdmitCanonicalFact(id) => {
            admit_canonical_fact_preserves_invariant(state, id);
        }
        EngineTransitionCore::IndexAssertedEdge(edge) => {
            index_asserted_edge_preserves_invariant(state, edge);
        }
        EngineTransitionCore::QueryResultEnqueue(id) => {
            query_result_enqueue_preserves_invariant(state, id);
        }
        EngineTransitionCore::ProjectValid(id) => {
            project_valid_preserves_invariant(state, id);
        }
        EngineTransitionCore::PromoteOffer(owner, addr) => {
            promote_offer_preserves_invariant(state, owner, addr);
        }
        EngineTransitionCore::EmitRawFact(id) => {
            emitted_raw_fact_reenters_admission_queue(state, id);
        }
    }
}

pub proof fn engine_transition_trace_preserves_invariant(
    state: EngineStateCore,
    transitions: Seq<EngineTransitionCore>,
)
    requires
        engine_invariant(state),
        transition_trace_preconditions(state, transitions),
    ensures
        engine_invariant(apply_transition_trace(state, transitions)),
    decreases transitions.len(),
{
    if transitions.len() > 0 {
        let transition = transitions[0];
        let tail = transitions.subrange(1, transitions.len() as int);
        engine_single_transition_preserves_invariant(state, transition);
        engine_transition_trace_preserves_invariant(apply_transition(state, transition), tail);
    }
}

pub proof fn engine_transition_preserves_validated_context_provenance(
    state: EngineStateCore,
    owner: EngineIdCore,
    addr: EngineAddrCore,
)
    requires
        engine_invariant(state),
        contains_id(state.valid, owner),
        asserted_offer_for(state.asserted, owner, addr),
        !validated_offer_for(state.validated, owner, addr),
    ensures
        engine_invariant(state_promote_offer(state, owner, addr)),
        validated_offer_provenance(state_promote_offer(state, owner, addr)),
{
    promote_offer_preserves_invariant(state, owner, addr);
    assert(state_promote_offer(state, owner, addr).validated
        == state.validated.push(EngineValidatedOfferCore { owner, addr }));
    validated_offer_push_adds_offer(state.validated, owner, addr);
    let i = state.validated.len() as int;
    assert(0 <= i < state_promote_offer(state, owner, addr).validated.len());
    assert(state_promote_offer(state, owner, addr).validated[i]
        == EngineValidatedOfferCore { owner, addr });
    assert(validated_offer_for(state_promote_offer(state, owner, addr).validated, owner, addr));
}

pub proof fn engine_promotes_only_valid_owner_offers(
    state: EngineStateCore,
    owner: EngineIdCore,
    addr: EngineAddrCore,
)
    requires
        engine_invariant(state),
        contains_id(state.valid, owner),
        asserted_offer_for(state.asserted, owner, addr),
        !validated_offer_for(state.validated, owner, addr),
    ensures
        contains_id(state_promote_offer(state, owner, addr).valid, owner),
        asserted_offer_for(state_promote_offer(state, owner, addr).asserted, owner, addr),
        validated_offer_for(state_promote_offer(state, owner, addr).validated, owner, addr),
        engine_invariant(state_promote_offer(state, owner, addr)),
{
    promote_offer_preserves_invariant(state, owner, addr);
    assert(state_promote_offer(state, owner, addr).validated
        == state.validated.push(EngineValidatedOfferCore { owner, addr }));
    validated_offer_push_adds_offer(state.validated, owner, addr);
    let i = state.validated.len() as int;
    assert(0 <= i < state_promote_offer(state, owner, addr).validated.len());
    assert(state_promote_offer(state, owner, addr).validated[i]
        == EngineValidatedOfferCore { owner, addr });
    assert(validated_offer_for(state_promote_offer(state, owner, addr).validated, owner, addr));
}

pub proof fn engine_context_offers_have_valid_owners(
    state: EngineStateCore,
    i: int,
)
    requires
        engine_invariant(state),
        0 <= i < state.validated.len(),
    ensures
        contains_id(state.valid, state.validated[i].owner),
        asserted_offer_for(state.asserted, state.validated[i].owner, state.validated[i].addr),
{
}

pub proof fn engine_validated_offer_for_has_valid_owner(
    state: EngineStateCore,
    owner: EngineIdCore,
    addr: EngineAddrCore,
)
    requires
        engine_invariant(state),
        validated_offer_for(state.validated, owner, addr),
    ensures
        contains_id(state.valid, owner),
        asserted_offer_for(state.asserted, owner, addr),
{
    let i = choose |i: int|
        0 <= i < state.validated.len()
            && state.validated[i].owner == owner
            && state.validated[i].addr == addr;
    assert(0 <= i < state.validated.len());
    assert(state.validated[i].owner == owner);
    assert(state.validated[i].addr == addr);
    engine_context_offers_have_valid_owners(state, i);
}

} // verus!

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct EdgeAddr {
    pub role: Role,
    pub scope: Scope,
    pub key: Key,
}

impl EdgeAddr {
    fn from_offer<V>(offer: &Offer<V>) -> Self {
        Self {
            role: offer.role,
            scope: offer.scope,
            key: offer.key,
        }
    }
}

/// Durable lookup contract used by the in-memory engine. SQLite is one
/// implementation; the proof assumes this contract, not the SQL implementation.
pub trait Storage {
    fn load_fact(&self, id: &FactId) -> Result<Option<Vec<u8>>, String>;
    fn offerers_for(&self, addr: EdgeAddr) -> Result<Vec<FactId>, String>;
    fn needers_for(&self, addr: EdgeAddr) -> Result<Vec<FactId>, String>;
}

impl<T: Index + ?Sized> Storage for T {
    fn load_fact(&self, id: &FactId) -> Result<Option<Vec<u8>>, String> {
        Index::load_fact(self, id)
    }

    fn offerers_for(&self, addr: EdgeAddr) -> Result<Vec<FactId>, String> {
        self.offers_for_key(addr.role, addr.scope, &addr.key)
    }

    fn needers_for(&self, addr: EdgeAddr) -> Result<Vec<FactId>, String> {
        self.needs_for_key(addr.role, addr.scope, &addr.key)
    }
}

pub struct MemIndex<P: Projector> {
    facts: HashMap<FactId, P::Item>,
    edges: HashMap<FactId, Vec<Offer<Asserted>>>,
    offers: HashMap<EdgeAddr, HashSet<FactId>>,
    needs: HashMap<EdgeAddr, HashSet<FactId>>,
}

impl<P: Projector> Default for MemIndex<P> {
    fn default() -> Self {
        Self {
            facts: HashMap::new(),
            edges: HashMap::new(),
            offers: HashMap::new(),
            needs: HashMap::new(),
        }
    }
}

impl<P: Projector> MemIndex<P> {
    pub fn contains(&self, id: &FactId) -> bool {
        self.facts.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    fn item(&self, id: &FactId) -> Option<&P::Item> {
        self.facts.get(id)
    }

    fn edges(&self, id: &FactId) -> Option<&[Offer<Asserted>]> {
        self.edges.get(id).map(Vec::as_slice)
    }

    fn insert(&mut self, id: FactId, item: P::Item, edges: Vec<Offer<Asserted>>) -> bool {
        if self.facts.contains_key(&id) {
            return false;
        }
        for edge in &edges {
            let addr = EdgeAddr::from_offer(edge);
            if edge.is_offer() {
                self.offers.entry(addr).or_default().insert(id);
            } else if edge.is_need() {
                self.needs.entry(addr).or_default().insert(id);
            }
        }
        self.facts.insert(id, item);
        self.edges.insert(id, edges);
        true
    }

    fn offerers(&self, addr: EdgeAddr) -> Vec<FactId> {
        self.offers
            .get(&addr)
            .map(|owners| owners.iter().copied().collect())
            .unwrap_or_default()
    }

    fn needers(&self, addr: EdgeAddr) -> Vec<FactId> {
        self.needs
            .get(&addr)
            .map(|owners| owners.iter().copied().collect())
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
pub struct ValidatedOffer {
    pub owner: FactId,
    pub offer: Offer<Validated>,
}

pub struct EngineState<P: Projector> {
    pub mem: MemIndex<P>,
    pub projector_state: P::State,
    pub validity: HashMap<FactId, Validity>,
    pub validated: Vec<ValidatedOffer>,
    validated_by_addr: HashMap<EdgeAddr, Vec<ValidatedOffer>>,
    promoted_offers: HashSet<(FactId, EdgeAddr)>,
    to_admit: VecDeque<FactId>,
    to_project: VecDeque<FactId>,
    need_queries: VecDeque<EdgeAddr>,
    offer_queries: VecDeque<EdgeAddr>,
    queued_admit: HashSet<FactId>,
    queued_project: HashSet<FactId>,
    queued_need_queries: HashSet<EdgeAddr>,
    queued_offer_queries: HashSet<EdgeAddr>,
}

impl<P: Projector> Default for EngineState<P> {
    fn default() -> Self {
        Self {
            mem: MemIndex::default(),
            projector_state: P::State::default(),
            validity: HashMap::new(),
            validated: Vec::new(),
            validated_by_addr: HashMap::new(),
            promoted_offers: HashSet::new(),
            to_admit: VecDeque::new(),
            to_project: VecDeque::new(),
            need_queries: VecDeque::new(),
            offer_queries: VecDeque::new(),
            queued_admit: HashSet::new(),
            queued_project: HashSet::new(),
            queued_need_queries: HashSet::new(),
            queued_offer_queries: HashSet::new(),
        }
    }
}

impl<P: Projector> EngineState<P>
where
    P::Item: Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue_admit(&mut self, id: FactId) {
        if self.queued_admit.insert(id) {
            self.to_admit.push_back(id);
        }
    }

    pub fn enqueue_project(&mut self, id: FactId) {
        if self.queued_project.insert(id) {
            self.to_project.push_back(id);
        }
    }

    pub fn pending_admit_len(&self) -> usize {
        self.to_admit.len()
    }

    pub fn pending_project_len(&self) -> usize {
        self.to_project.len()
    }

    pub fn pending_query_len(&self) -> usize {
        self.need_queries.len() + self.offer_queries.len()
    }

    /// Admit an already-decoded local item into memory. This is the non-storage
    /// path for facts that should not be written to durable storage by this pass.
    pub fn admit_item(&mut self, item: P::Item) -> FactId {
        let id = fact_id(&P::encode(&item));
        self.index_item(id, item);
        id
    }

    /// Load a content-addressed fact from storage, decode it, and index its
    /// asserted needs/offers in memory. This deliberately does not call
    /// `admit`: storage already owns the persisted bytes and asserted edge rows.
    pub fn admit_from_storage<S: Storage + ?Sized>(
        &mut self,
        id: FactId,
        storage: &S,
    ) -> Result<bool, String> {
        let bytes = storage.load_fact(&id)?;
        self.admit_loaded_fact(id, bytes)
    }

    pub fn admit_loaded_fact(
        &mut self,
        id: FactId,
        bytes: Option<Vec<u8>>,
    ) -> Result<bool, String> {
        if self.mem.contains(&id) {
            return Ok(false);
        }
        let Some(bytes) = bytes else {
            return Ok(false);
        };
        if fact_id(&bytes) != id {
            return Err("storage returned bytes whose hash does not match id".to_string());
        }
        let item = P::decode(&bytes)?;
        if P::encode(&item) != bytes {
            return Err("storage returned non-canonical bytes".to_string());
        }
        self.index_item(id, item);
        Ok(true)
    }

    fn index_item(&mut self, id: FactId, item: P::Item) {
        let edges = P::extract(&item);
        if !self.mem.insert(id, item, edges.clone()) {
            self.enqueue_project_if_not_valid(id);
            return;
        }
        self.enqueue_project(id);
        for need in edges.iter().filter(|edge| edge.is_need()) {
            self.enqueue_need_query(EdgeAddr::from_offer(need));
        }
    }

    pub fn project_one(&mut self, id: FactId) -> Result<Option<Validity>, String> {
        if self.validity.get(&id) == Some(&Validity::Valid) {
            return Ok(Some(Validity::Valid));
        }
        let Some(item) = self.mem.item(&id).cloned() else {
            self.enqueue_admit(id);
            return Ok(None);
        };
        let Some(edges) = self.mem.edges(&id).map(|edges| edges.to_vec()) else {
            self.enqueue_admit(id);
            return Ok(None);
        };

        for need in edges.iter().filter(|edge| edge.is_need()) {
            let addr = EdgeAddr::from_offer(need);
            for provider in self.mem.offerers(addr) {
                if !self.validity.contains_key(&provider) {
                    self.enqueue_project(provider);
                }
            }
            if !self.has_validated_offer(addr) {
                self.enqueue_need_query(addr);
            }
        }

        if !self.needs_satisfied(&edges) {
            self.validity.insert(id, Validity::Invalid);
            return Ok(Some(Validity::Invalid));
        }

        let admitted = Admitted::from_engine_memory(item, id);
        let interface = projector_interface_core();
        debug_assert!(!interface.project_has_storage);
        debug_assert!(!interface.project_has_clock);
        debug_assert!(!interface.project_has_socket);
        debug_assert!(interface.project_reads_validated_context);
        debug_assert!(interface.project_updates_are_inert);
        let out = P::project(&admitted, self.collect(&edges), &self.projector_state);
        let effective_validity = out.validity;
        let updates = out.updates;
        for update in &updates {
            if P::update_owner(update) != id {
                return Err("projector returned state update for a different fact".to_string());
            }
        }

        self.validity.insert(id, effective_validity);
        for update in updates {
            P::apply_update(&mut self.projector_state, update);
        }

        if effective_validity == Validity::Valid {
            for offer in edges.iter().copied().filter(|edge| edge.is_offer()) {
                let addr = EdgeAddr::from_offer(&offer);
                let first_promotion = self.promoted_offers.insert((id, addr));
                if !first_promotion {
                    continue;
                }
                let validated = ValidatedOffer {
                    owner: id,
                    offer: offer.validate(),
                };
                self.validated.push(validated);
                self.validated_by_addr
                    .entry(addr)
                    .or_default()
                    .push(validated);
                for needer in self.mem.needers(addr) {
                    self.enqueue_project_if_not_valid(needer);
                }
                self.enqueue_offer_query(addr);
            }
        }

        if effective_validity == Validity::Valid {
            for emitted in out.emitted {
                let id = fact_id(&emitted.bytes);
                let item = P::decode(&emitted.bytes)?;
                if P::encode(&item) != emitted.bytes {
                    return Err("projector emitted non-canonical bytes".to_string());
                }
                self.index_item(id, item);
            }
        }

        Ok(Some(effective_validity))
    }

    pub fn has_pending_work(&self) -> bool {
        !self.to_admit.is_empty()
            || !self.to_project.is_empty()
            || !self.need_queries.is_empty()
            || !self.offer_queries.is_empty()
    }

    fn enqueue_need_query(&mut self, addr: EdgeAddr) {
        if self.queued_need_queries.insert(addr) {
            self.need_queries.push_back(addr);
        }
    }

    fn enqueue_offer_query(&mut self, addr: EdgeAddr) {
        if self.queued_offer_queries.insert(addr) {
            self.offer_queries.push_back(addr);
        }
    }

    fn enqueue_project_if_unseen(&mut self, id: FactId) {
        if !self.validity.contains_key(&id) {
            self.enqueue_project(id);
        }
    }

    fn enqueue_project_if_not_valid(&mut self, id: FactId) {
        if self.validity.get(&id) != Some(&Validity::Valid) {
            self.enqueue_project(id);
        }
    }

    pub(crate) fn pop_admit_request(&mut self) -> Option<FactId> {
        let id = self.to_admit.pop_front()?;
        self.queued_admit.remove(&id);
        Some(id)
    }

    pub(crate) fn pop_need_query_request(&mut self) -> Option<EdgeAddr> {
        let addr = self.need_queries.pop_front()?;
        self.queued_need_queries.remove(&addr);
        Some(addr)
    }

    pub(crate) fn pop_project_request(&mut self) -> Option<FactId> {
        let id = self.to_project.pop_front()?;
        self.queued_project.remove(&id);
        Some(id)
    }

    pub(crate) fn pop_offer_query_request(&mut self) -> Option<EdgeAddr> {
        let addr = self.offer_queries.pop_front()?;
        self.queued_offer_queries.remove(&addr);
        Some(addr)
    }

    pub(crate) fn enqueue_loaded_offerers(&mut self, ids: Vec<FactId>) {
        for id in ids {
            self.enqueue_admit(id);
            self.enqueue_project_if_unseen(id);
        }
    }

    pub(crate) fn enqueue_loaded_needers(&mut self, ids: Vec<FactId>) {
        for id in ids {
            self.enqueue_admit(id);
            self.enqueue_project_if_not_valid(id);
        }
    }

    fn has_validated_offer(&self, addr: EdgeAddr) -> bool {
        self.validated_by_addr
            .get(&addr)
            .is_some_and(|offers| !offers.is_empty())
    }

    fn needs_satisfied(&self, edges: &[Offer<Asserted>]) -> bool {
        edges
            .iter()
            .filter(|edge| edge.is_need())
            .all(|need| self.has_validated_offer(EdgeAddr::from_offer(need)))
    }

    fn collect(&self, edges: &[Offer<Asserted>]) -> Context {
        let mut offers = vec![];
        for need in edges.iter().filter(|edge| edge.is_need()) {
            let addr = EdgeAddr::from_offer(need);
            for vo in self.validated_by_addr.get(&addr).into_iter().flatten() {
                debug_assert!(self.validity.get(&vo.owner) == Some(&Validity::Valid));
                debug_assert!(self
                    .promoted_offers
                    .contains(&(vo.owner, EdgeAddr::from_offer(&vo.offer))));
                offers.push(vo.offer);
            }
        }
        Context::from(offers)
    }
}
