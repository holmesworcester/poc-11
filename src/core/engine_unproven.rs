//! Queue-oriented in-memory engine model. Durable storage remains behind
//! [`Storage`]; this module keeps the running state intentionally Vec-backed so
//! the Verus proof target is the same shape as the code we execute:
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
//! - [x] Safety: abstract transition helper coverage: every modeled validated
//!       offer is owned by a fact already projected valid and was asserted by
//!       that same owner. Verified below by
//!       `engine_transition_trace_preserves_invariant`. This does not complete
//!       the runtime transition proof.
//! - [ ] Safety: family-private projector state is not authority: a projector may
//!       return state updates only for the fact being projected, and the engine
//!       promotes offers and emitted facts only after readiness and projector
//!       validity are both established.
//! - [x] Safety: abstract transition helper coverage: one owner contributes at
//!       most one modeled validated offer for a given match address. Verified
//!       below by `engine_transition_trace_preserves_invariant`. This does not
//!       complete the runtime transition proof.
//! - [x] Safety: abstract transition helper coverage: raw bytes returned in
//!       `ProjectOutcome.emitted` do not inherit authority from the emitting
//!       fact; they re-enter the modeled admission queue. Verified below by
//!       `emitted_raw_fact_reenters_admission_queue`. This does not complete the
//!       runtime transition proof.
//! - [x] Safety: abstract transition helper coverage: recorded dependency edges
//!       have valid consumers, valid providers, and provider validated offers.
//!       Verified below by `record_dependency_preserves_invariant` and
//!       `engine_dependency_edge_has_valid_provider`. This does not complete the
//!       runtime transition proof.
//! - [x] Safety: every abstract admit/query/project/promote/emit transition
//!       preserves these modeled invariants, so every modeled transition prefix
//!       is sound. Verified below by `engine_single_transition_preserves_invariant`
//!       and `engine_transition_trace_preserves_invariant`. This does not
//!       complete the runtime transition proof.
//! - [x] Safety: runtime id queue scheduling uses Verus-verified bytewise fact-id
//!       equality and a Verus-verified Vec-backed unique-enqueue transition.
//!       Runtime `enqueue_admit` and `enqueue_project` call this kernel directly.
//!       Verified below by `runtime_fact_id_eq_core`,
//!       `runtime_queue_contains_id`, and `runtime_enqueue_id_core`.
//! - [ ] Safety: the concrete runtime `EngineState` Vec-backed implementation
//!       is the proof target for the transition invariant, without an
//!       unproved HashMap/HashSet/VecDeque refinement layer.
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
//! - Maintain the runtime Vec-backed state predicate over memory facts, asserted
//!   edges, validity, validated offers, recorded dependencies, promoted offer
//!   keys, and queues.
//! - Prove each runtime transition preserves the predicate: in-memory admission,
//!   storage load result, need-query result, projection, raw emitted-byte
//!   admission, and offer-query result. The load/query transitions may enqueue
//!   additional ids or addresses to inspect, but they do not mutate validity,
//!   validated offers, or recorded dependencies.
//! - For projection, prove readiness first, build context only from matching
//!   validated offers, run the projector, reject any update whose owner is not the
//!   projected fact, apply returned family-private updates through
//!   `P::apply_update`, and promote asserted offers only when projector validity
//!   is `Valid`.
//! - For dependency recording, prove every recorded consumer/provider pair is
//!   already valid and that the provider's offer was validated in the running
//!   state.
//! - Prove modeled drain safety by induction over transition steps; this is now
//!   the `engine_transition_trace_preserves_invariant` theorem. Runtime id queue
//!   enqueue has been moved onto a directly called Verus kernel. The remaining
//!   work is moving projection, promotion, dependency recording, and effect
//!   result transitions onto Vec-backed runtime transition helpers directly.
//!   Prove completeness or liveness separately from safety.

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineDependencyCore {
    pub consumer: EngineIdCore,
    pub provider: EngineIdCore,
    pub addr: EngineAddrCore,
}

pub struct EngineStateCore {
    pub admitted: Seq<EngineIdCore>,
    pub asserted: Seq<EngineEdgeCore>,
    pub valid: Seq<EngineIdCore>,
    pub validated: Seq<EngineValidatedOfferCore>,
    pub dependencies: Seq<EngineDependencyCore>,
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
    RecordDependency(EngineIdCore, EngineIdCore, EngineAddrCore),
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

pub closed spec fn dependency_edge_for(
    dependencies: Seq<EngineDependencyCore>,
    consumer: EngineIdCore,
    provider: EngineIdCore,
    addr: EngineAddrCore,
) -> bool {
    exists |i: int|
        0 <= i < dependencies.len()
            && dependencies[i].consumer == consumer
            && dependencies[i].provider == provider
            && dependencies[i].addr == addr
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

pub closed spec fn dependency_provenance(state: EngineStateCore) -> bool {
    forall |i: int|
        0 <= i < state.dependencies.len() ==>
            contains_id(state.valid, #[trigger] state.dependencies[i].consumer)
                && contains_id(state.valid, state.dependencies[i].provider)
                && validated_offer_for(
                    state.validated,
                    state.dependencies[i].provider,
                    state.dependencies[i].addr,
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
    validated_offer_provenance(state)
        && dependency_provenance(state)
        && promoted_offer_unique_per_owner_addr(state)
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

pub proof fn validated_offer_push_preserves_existing(
    validated: Seq<EngineValidatedOfferCore>,
    owner: EngineIdCore,
    addr: EngineAddrCore,
    pushed: EngineValidatedOfferCore,
)
    requires
        validated_offer_for(validated, owner, addr),
    ensures
        validated_offer_for(validated.push(pushed), owner, addr),
{
    let i = choose |i: int| 0 <= i < validated.len() && validated[i].owner == owner && validated[i].addr == addr;
    assert(validated.push(pushed)[i] == validated[i]);
}

pub proof fn dependency_push_preserves_existing(
    dependencies: Seq<EngineDependencyCore>,
    consumer: EngineIdCore,
    provider: EngineIdCore,
    addr: EngineAddrCore,
    pushed: EngineDependencyCore,
)
    requires
        dependency_edge_for(dependencies, consumer, provider, addr),
    ensures
        dependency_edge_for(dependencies.push(pushed), consumer, provider, addr),
{
    let i = choose |i: int|
        0 <= i < dependencies.len()
            && dependencies[i].consumer == consumer
            && dependencies[i].provider == provider
            && dependencies[i].addr == addr;
    assert(dependencies.push(pushed)[i] == dependencies[i]);
}

pub proof fn dependency_push_adds_dependency(
    dependencies: Seq<EngineDependencyCore>,
    consumer: EngineIdCore,
    provider: EngineIdCore,
    addr: EngineAddrCore,
)
    ensures
        dependency_edge_for(
            dependencies.push(EngineDependencyCore { consumer, provider, addr }),
            consumer,
            provider,
            addr,
        ),
{
    let i = dependencies.len() as int;
    assert(dependencies.push(EngineDependencyCore { consumer, provider, addr })[i]
        == EngineDependencyCore { consumer, provider, addr });
}

pub closed spec fn empty_engine_state() -> EngineStateCore {
    EngineStateCore {
        admitted: Seq::empty(),
        asserted: Seq::empty(),
        valid: Seq::empty(),
        validated: Seq::empty(),
        dependencies: Seq::empty(),
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
        dependencies: state.dependencies,
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
        dependencies: state.dependencies,
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
        dependencies: state.dependencies,
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
        dependencies: state.dependencies,
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
        dependencies: state.dependencies,
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
        dependencies: state.dependencies,
        to_admit: state.to_admit,
        to_project: state.to_project,
        need_queries: state.need_queries,
        offer_queries: state.offer_queries.push(addr),
    }
}

pub closed spec fn state_record_dependency(
    state: EngineStateCore,
    consumer: EngineIdCore,
    provider: EngineIdCore,
    addr: EngineAddrCore,
) -> EngineStateCore {
    EngineStateCore {
        admitted: state.admitted,
        asserted: state.asserted,
        valid: state.valid,
        validated: state.validated,
        dependencies: state.dependencies.push(EngineDependencyCore { consumer, provider, addr }),
        to_admit: state.to_admit,
        to_project: state.to_project,
        need_queries: state.need_queries,
        offer_queries: state.offer_queries,
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
        EngineTransitionCore::RecordDependency(consumer, provider, addr) => {
            contains_id(state.valid, consumer)
                && contains_id(state.valid, provider)
                && validated_offer_for(state.validated, provider, addr)
                && !dependency_edge_for(state.dependencies, consumer, provider, addr)
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
        EngineTransitionCore::RecordDependency(consumer, provider, addr) => {
            state_record_dependency(state, consumer, provider, addr)
        }
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
    assert forall |i: int| 0 <= i < state.dependencies.len() implies
        contains_id(
            state_project_valid(state, id).valid,
            #[trigger] state_project_valid(state, id).dependencies[i].consumer,
        )
            && contains_id(
                state_project_valid(state, id).valid,
                state_project_valid(state, id).dependencies[i].provider,
            )
            && validated_offer_for(
                state_project_valid(state, id).validated,
                state_project_valid(state, id).dependencies[i].provider,
                state_project_valid(state, id).dependencies[i].addr,
            )
    by {
        assert(state_project_valid(state, id).dependencies[i] == state.dependencies[i]);
        assert(contains_id(state.valid, state.dependencies[i].consumer));
        contains_id_push_preserves_existing(state.valid, state.dependencies[i].consumer, id);
        assert(contains_id(state.valid, state.dependencies[i].provider));
        contains_id_push_preserves_existing(state.valid, state.dependencies[i].provider, id);
        assert(validated_offer_for(
            state.validated,
            state.dependencies[i].provider,
            state.dependencies[i].addr,
        ));
    }
    assert forall |i: int, j: int|
        0 <= i < state_project_valid(state, id).validated.len()
            && 0 <= j < state_project_valid(state, id).validated.len()
            && i != j
        implies
            !(#[trigger] state_project_valid(state, id).validated[i] == #[trigger] state_project_valid(state, id).validated[j]
                || state_project_valid(state, id).validated[i].owner
                    == state_project_valid(state, id).validated[j].owner
                    && state_project_valid(state, id).validated[i].addr
                        == state_project_valid(state, id).validated[j].addr)
    by {
        assert(state_project_valid(state, id).validated == state.validated);
        assert(promoted_offer_unique_per_owner_addr(state));
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
    assert forall |i: int| 0 <= i < state.dependencies.len() implies
        contains_id(
            state_promote_offer(state, owner, addr).valid,
            #[trigger] state_promote_offer(state, owner, addr).dependencies[i].consumer,
        )
            && contains_id(
                state_promote_offer(state, owner, addr).valid,
                state_promote_offer(state, owner, addr).dependencies[i].provider,
            )
            && validated_offer_for(
                state_promote_offer(state, owner, addr).validated,
                state_promote_offer(state, owner, addr).dependencies[i].provider,
                state_promote_offer(state, owner, addr).dependencies[i].addr,
            )
    by {
        assert(state_promote_offer(state, owner, addr).dependencies[i] == state.dependencies[i]);
        assert(contains_id(state.valid, state.dependencies[i].consumer));
        assert(contains_id(state.valid, state.dependencies[i].provider));
        assert(validated_offer_for(
            state.validated,
            state.dependencies[i].provider,
            state.dependencies[i].addr,
        ));
        validated_offer_push_preserves_existing(
            state.validated,
            state.dependencies[i].provider,
            state.dependencies[i].addr,
            EngineValidatedOfferCore { owner, addr },
        );
    }
}

pub proof fn record_dependency_preserves_invariant(
    state: EngineStateCore,
    consumer: EngineIdCore,
    provider: EngineIdCore,
    addr: EngineAddrCore,
)
    requires
        engine_invariant(state),
        contains_id(state.valid, consumer),
        contains_id(state.valid, provider),
        validated_offer_for(state.validated, provider, addr),
        !dependency_edge_for(state.dependencies, consumer, provider, addr),
    ensures
        engine_invariant(state_record_dependency(state, consumer, provider, addr)),
{
    assert forall |i: int| 0 <= i < state_record_dependency(state, consumer, provider, addr).dependencies.len() implies
        contains_id(
            state_record_dependency(state, consumer, provider, addr).valid,
            #[trigger] state_record_dependency(state, consumer, provider, addr).dependencies[i].consumer,
        )
            && contains_id(
                state_record_dependency(state, consumer, provider, addr).valid,
                state_record_dependency(state, consumer, provider, addr).dependencies[i].provider,
            )
            && validated_offer_for(
                state_record_dependency(state, consumer, provider, addr).validated,
                state_record_dependency(state, consumer, provider, addr).dependencies[i].provider,
                state_record_dependency(state, consumer, provider, addr).dependencies[i].addr,
            )
    by {
        if i < state.dependencies.len() {
            assert(state_record_dependency(state, consumer, provider, addr).dependencies[i]
                == state.dependencies[i]);
            assert(contains_id(state.valid, state.dependencies[i].consumer));
            assert(contains_id(state.valid, state.dependencies[i].provider));
            assert(validated_offer_for(
                state.validated,
                state.dependencies[i].provider,
                state.dependencies[i].addr,
            ));
        } else {
            assert(i == state.dependencies.len());
            assert(state_record_dependency(state, consumer, provider, addr).dependencies[i]
                == EngineDependencyCore { consumer, provider, addr });
        }
    }
    dependency_push_adds_dependency(state.dependencies, consumer, provider, addr);
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
        EngineTransitionCore::RecordDependency(consumer, provider, addr) => {
            record_dependency_preserves_invariant(state, consumer, provider, addr);
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

pub proof fn engine_dependency_edge_has_valid_provider(
    state: EngineStateCore,
    consumer: EngineIdCore,
    provider: EngineIdCore,
    addr: EngineAddrCore,
)
    requires
        engine_invariant(state),
        dependency_edge_for(state.dependencies, consumer, provider, addr),
    ensures
        contains_id(state.valid, consumer),
        contains_id(state.valid, provider),
        validated_offer_for(state.validated, provider, addr),
{
    let i = choose |i: int|
        0 <= i < state.dependencies.len()
            && state.dependencies[i].consumer == consumer
            && state.dependencies[i].provider == provider
            && state.dependencies[i].addr == addr;
    assert(0 <= i < state.dependencies.len());
    assert(state.dependencies[i].consumer == consumer);
    assert(state.dependencies[i].provider == provider);
    assert(state.dependencies[i].addr == addr);
}

pub closed spec fn runtime_fact_id_eq(left: [u8; 32], right: [u8; 32]) -> bool {
    left[0] == right[0] && left[1] == right[1] && left[2] == right[2] && left[3] == right[3]
        && left[4] == right[4] && left[5] == right[5] && left[6] == right[6]
        && left[7] == right[7] && left[8] == right[8] && left[9] == right[9]
        && left[10] == right[10] && left[11] == right[11] && left[12] == right[12]
        && left[13] == right[13] && left[14] == right[14] && left[15] == right[15]
        && left[16] == right[16] && left[17] == right[17] && left[18] == right[18]
        && left[19] == right[19] && left[20] == right[20] && left[21] == right[21]
        && left[22] == right[22] && left[23] == right[23] && left[24] == right[24]
        && left[25] == right[25] && left[26] == right[26] && left[27] == right[27]
        && left[28] == right[28] && left[29] == right[29] && left[30] == right[30]
        && left[31] == right[31]
}

pub fn runtime_fact_id_eq_core(left: [u8; 32], right: [u8; 32]) -> (equal: bool)
    ensures
        equal == runtime_fact_id_eq(left, right),
{
    left[0] == right[0] && left[1] == right[1] && left[2] == right[2] && left[3] == right[3]
        && left[4] == right[4] && left[5] == right[5] && left[6] == right[6]
        && left[7] == right[7] && left[8] == right[8] && left[9] == right[9]
        && left[10] == right[10] && left[11] == right[11] && left[12] == right[12]
        && left[13] == right[13] && left[14] == right[14] && left[15] == right[15]
        && left[16] == right[16] && left[17] == right[17] && left[18] == right[18]
        && left[19] == right[19] && left[20] == right[20] && left[21] == right[21]
        && left[22] == right[22] && left[23] == right[23] && left[24] == right[24]
        && left[25] == right[25] && left[26] == right[26] && left[27] == right[27]
        && left[28] == right[28] && left[29] == right[29] && left[30] == right[30]
        && left[31] == right[31]
}

pub closed spec fn runtime_id_seq_contains(ids: Seq<[u8; 32]>, id: [u8; 32]) -> bool {
    exists |i: int| 0 <= i < ids.len() && runtime_fact_id_eq(ids[i], id)
}

pub proof fn runtime_fact_id_eq_reflexive(id: [u8; 32])
    ensures
        runtime_fact_id_eq(id, id),
{
}

pub proof fn runtime_id_seq_push_contains(ids: Seq<[u8; 32]>, id: [u8; 32])
    ensures
        runtime_id_seq_contains(ids.push(id), id),
{
    let i = ids.len() as int;
    runtime_fact_id_eq_reflexive(id);
    assert(ids.push(id)[i] == id);
    assert(runtime_fact_id_eq(ids.push(id)[i], id));
}

#[allow(clippy::ptr_arg)]
pub fn runtime_queue_contains_id(queue: &Vec<[u8; 32]>, id: [u8; 32]) -> (found: bool)
    ensures
        found == runtime_id_seq_contains(queue@, id),
{
    let mut i: usize = 0;
    while i < queue.len()
        invariant
            0 <= i <= queue.len(),
            forall |j: int| 0 <= j < i ==> !runtime_fact_id_eq(queue@[j], id),
        decreases queue.len() - i,
    {
        if runtime_fact_id_eq_core(queue[i], id) {
            assert(runtime_fact_id_eq(queue@[i as int], id));
            return true;
        }
        i += 1;
    }
    false
}

pub fn runtime_enqueue_id_core(queue: Vec<[u8; 32]>, id: [u8; 32]) -> (out: Vec<[u8; 32]>)
    ensures
        runtime_id_seq_contains(out@, id),
        runtime_id_seq_contains(queue@, id) ==> out@ == queue@,
        !runtime_id_seq_contains(queue@, id) ==> out@ == queue@.push(id),
{
    if runtime_queue_contains_id(&queue, id) {
        queue
    } else {
        let mut out = queue;
        out.push(id);
        proof {
            runtime_id_seq_push_contains(queue@, id);
        }
        out
    }
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

pub struct MemEntry<I> {
    pub id: FactId,
    pub item: I,
    pub edges: Vec<Offer<Asserted>>,
}

pub struct MemIndex<P: Projector> {
    entries: Vec<MemEntry<P::Item>>,
}

impl<P: Projector> Default for MemIndex<P> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<P: Projector> MemIndex<P> {
    pub fn contains(&self, id: &FactId) -> bool {
        self.entries
            .iter()
            .any(|entry| runtime_fact_id_eq_core(entry.id, *id))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn item(&self, id: &FactId) -> Option<&P::Item> {
        self.entries
            .iter()
            .find(|entry| runtime_fact_id_eq_core(entry.id, *id))
            .map(|entry| &entry.item)
    }

    fn edges(&self, id: &FactId) -> Option<&[Offer<Asserted>]> {
        self.entries
            .iter()
            .find(|entry| runtime_fact_id_eq_core(entry.id, *id))
            .map(|entry| entry.edges.as_slice())
    }

    fn insert(&mut self, id: FactId, item: P::Item, edges: Vec<Offer<Asserted>>) -> bool {
        if self.contains(&id) {
            return false;
        }
        self.entries.push(MemEntry { id, item, edges });
        true
    }

    fn offerers(&self, addr: EdgeAddr) -> Vec<FactId> {
        let mut ids = Vec::new();
        for entry in &self.entries {
            if entry
                .edges
                .iter()
                .any(|edge| edge.is_offer() && EdgeAddr::from_offer(edge) == addr)
                && !runtime_queue_contains_id(&ids, entry.id)
            {
                ids.push(entry.id);
            }
        }
        ids
    }

    fn needers(&self, addr: EdgeAddr) -> Vec<FactId> {
        let mut ids = Vec::new();
        for entry in &self.entries {
            if entry
                .edges
                .iter()
                .any(|edge| edge.is_need() && EdgeAddr::from_offer(edge) == addr)
                && !runtime_queue_contains_id(&ids, entry.id)
            {
                ids.push(entry.id);
            }
        }
        ids
    }
}

#[derive(Clone, Copy)]
pub struct ValidatedOffer {
    pub owner: FactId,
    pub offer: Offer<Validated>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecordedDependency {
    pub consumer: FactId,
    pub provider: FactId,
    pub addr: EdgeAddr,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ValidityEntry {
    pub id: FactId,
    pub validity: Validity,
}

#[derive(Clone, Default)]
pub struct ValidityIndex {
    entries: Vec<ValidityEntry>,
}

impl ValidityIndex {
    pub fn get(&self, id: &FactId) -> Option<&Validity> {
        self.entries
            .iter()
            .find(|entry| runtime_fact_id_eq_core(entry.id, *id))
            .map(|entry| &entry.validity)
    }

    pub fn insert(&mut self, id: FactId, validity: Validity) -> Option<Validity> {
        for entry in &mut self.entries {
            if runtime_fact_id_eq_core(entry.id, id) {
                let old = entry.validity;
                entry.validity = validity;
                return Some(old);
            }
        }
        self.entries.push(ValidityEntry { id, validity });
        None
    }

    pub fn contains_key(&self, id: &FactId) -> bool {
        self.entries
            .iter()
            .any(|entry| runtime_fact_id_eq_core(entry.id, *id))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (FactId, Validity)> + '_ {
        self.entries.iter().map(|entry| (entry.id, entry.validity))
    }

    pub fn keys(&self) -> impl Iterator<Item = FactId> + '_ {
        self.entries.iter().map(|entry| entry.id)
    }
}

pub struct EngineState<P: Projector> {
    pub mem: MemIndex<P>,
    pub projector_state: P::State,
    pub validity: ValidityIndex,
    pub validated: Vec<ValidatedOffer>,
    pub dependencies: Vec<RecordedDependency>,
    promoted_offers: Vec<(FactId, EdgeAddr)>,
    to_admit: Vec<FactId>,
    to_project: Vec<FactId>,
    need_queries: Vec<EdgeAddr>,
    offer_queries: Vec<EdgeAddr>,
}

impl<P: Projector> Default for EngineState<P> {
    fn default() -> Self {
        Self {
            mem: MemIndex::default(),
            projector_state: P::State::default(),
            validity: ValidityIndex::default(),
            validated: Vec::new(),
            dependencies: Vec::new(),
            promoted_offers: Vec::new(),
            to_admit: Vec::new(),
            to_project: Vec::new(),
            need_queries: Vec::new(),
            offer_queries: Vec::new(),
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
        self.to_admit = runtime_enqueue_id_core(std::mem::take(&mut self.to_admit), id);
    }

    pub fn enqueue_project(&mut self, id: FactId) {
        self.to_project = runtime_enqueue_id_core(std::mem::take(&mut self.to_project), id);
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
        if !runtime_fact_id_eq_core(fact_id(&bytes), id) {
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
            if !runtime_fact_id_eq_core(P::update_owner(update), id) {
                return Err("projector returned state update for a different fact".to_string());
            }
        }

        self.validity.insert(id, effective_validity);
        for update in updates {
            P::apply_update(&mut self.projector_state, update);
        }

        if effective_validity == Validity::Valid {
            self.record_dependencies(id, &edges);
            for offer in edges.iter().copied().filter(|edge| edge.is_offer()) {
                let addr = EdgeAddr::from_offer(&offer);
                if self.promoted_offers.iter().any(|(owner, promoted_addr)| {
                    runtime_fact_id_eq_core(*owner, id) && *promoted_addr == addr
                }) {
                    continue;
                }
                self.promoted_offers.push((id, addr));
                let validated = ValidatedOffer {
                    owner: id,
                    offer: offer.validate(),
                };
                self.validated.push(validated);
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
        if !self.need_queries.contains(&addr) {
            self.need_queries.push(addr);
        }
    }

    fn enqueue_offer_query(&mut self, addr: EdgeAddr) {
        if !self.offer_queries.contains(&addr) {
            self.offer_queries.push(addr);
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
        if self.to_admit.is_empty() {
            None
        } else {
            Some(self.to_admit.remove(0))
        }
    }

    pub(crate) fn pop_need_query_request(&mut self) -> Option<EdgeAddr> {
        if self.need_queries.is_empty() {
            None
        } else {
            Some(self.need_queries.remove(0))
        }
    }

    pub(crate) fn pop_project_request(&mut self) -> Option<FactId> {
        if self.to_project.is_empty() {
            None
        } else {
            Some(self.to_project.remove(0))
        }
    }

    pub(crate) fn pop_offer_query_request(&mut self) -> Option<EdgeAddr> {
        if self.offer_queries.is_empty() {
            None
        } else {
            Some(self.offer_queries.remove(0))
        }
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
        self.validated
            .iter()
            .any(|vo| EdgeAddr::from_offer(&vo.offer) == addr)
    }

    fn needs_satisfied(&self, edges: &[Offer<Asserted>]) -> bool {
        edges
            .iter()
            .filter(|edge| edge.is_need())
            .all(|need| self.has_validated_offer(EdgeAddr::from_offer(need)))
    }

    fn record_dependencies(&mut self, consumer: FactId, edges: &[Offer<Asserted>]) {
        for need in edges.iter().filter(|edge| edge.is_need()) {
            let addr = EdgeAddr::from_offer(need);
            let providers: Vec<_> = self
                .validated
                .iter()
                .copied()
                .filter(|vo| EdgeAddr::from_offer(&vo.offer) == addr)
                .collect();
            for vo in providers {
                debug_assert!(self.validity.get(&consumer) == Some(&Validity::Valid));
                debug_assert!(self.validity.get(&vo.owner) == Some(&Validity::Valid));
                debug_assert_eq!(EdgeAddr::from_offer(&vo.offer), addr);
                if !self.dependencies.iter().any(|dep| {
                    runtime_fact_id_eq_core(dep.consumer, consumer)
                        && runtime_fact_id_eq_core(dep.provider, vo.owner)
                        && dep.addr == addr
                }) {
                    self.dependencies.push(RecordedDependency {
                        consumer,
                        provider: vo.owner,
                        addr,
                    });
                }
            }
        }
    }

    fn collect(&self, edges: &[Offer<Asserted>]) -> Context {
        let mut offers = vec![];
        for need in edges.iter().filter(|edge| edge.is_need()) {
            let addr = EdgeAddr::from_offer(need);
            for vo in self
                .validated
                .iter()
                .filter(|vo| EdgeAddr::from_offer(&vo.offer) == addr)
            {
                debug_assert!(self.validity.get(&vo.owner) == Some(&Validity::Valid));
                debug_assert!(self.promoted_offers.iter().any(|(owner, promoted_addr)| {
                    runtime_fact_id_eq_core(*owner, vo.owner)
                        && *promoted_addr == EdgeAddr::from_offer(&vo.offer)
                }));
                offers.push(vo.offer);
            }
        }
        Context::from(offers)
    }
}
