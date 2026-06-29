// Verus model for poc-11's queue-oriented projection core. This file is
// compiled standalone by scripts/run_verus.sh and intentionally stays out of
// cargo's module tree. Crypto and durable storage are abstract contracts here;
// the proof is over typed in-memory facts, needs/offers, validated offers, and
// projection.
#![allow(unused)]
use vstd::prelude::*;

verus! {

pub type Id = int;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Validity {
    Valid,
    Invalid,
}

// -------------------------------------------------------------------------
// Generic positive projection calculus.
// -------------------------------------------------------------------------

pub struct SpecFact {
    pub id: Id,
    pub needs: Seq<Id>,
    pub offers: Seq<Id>,
}

pub struct MemIndex {
    pub facts: Set<Id>,
    pub offers: Set<(Id, Id)>, // (owner, key)
    pub needs: Set<(Id, Id)>,  // (owner, key)
}

pub struct Projection {
    pub valid: Set<Id>,
    pub validated_offers: Set<(Id, Id)>, // (owner, key)
}

pub struct EngineState {
    pub mem: MemIndex,
    pub projection: Projection,
    pub to_admit: Seq<Id>,
    pub to_project: Seq<Id>,
    pub need_queries: Seq<Id>,
    pub offer_queries: Seq<Id>,
}

pub enum EngineEvent {
    Admit(SpecFact),
    NeedQuery(Id),
    Project(SpecFact),
    OfferQuery(Id),
}

pub open spec fn mem_index_admits_fact(mem: MemIndex, fact: SpecFact) -> bool {
    mem.facts.contains(fact.id)
        && (forall|i: int| 0 <= i < fact.offers.len() ==>
            #[trigger] mem.offers.contains((fact.id, fact.offers[i])))
        && (forall|i: int| 0 <= i < fact.needs.len() ==>
            #[trigger] mem.needs.contains((fact.id, fact.needs[i])))
}

pub open spec fn mem_index_exact_fact_edges(mem: MemIndex, fact: SpecFact) -> bool {
    mem_index_admits_fact(mem, fact)
        && (forall|key: Id| #[trigger] mem.offers.contains((fact.id, key)) ==>
            exists|i: int| 0 <= i < fact.offers.len() && fact.offers[i] == key)
        && (forall|key: Id| #[trigger] mem.needs.contains((fact.id, key)) ==>
            exists|i: int| 0 <= i < fact.needs.len() && fact.needs[i] == key)
}

pub open spec fn same_mem(left: MemIndex, right: MemIndex) -> bool {
    left.facts =~= right.facts
        && left.offers =~= right.offers
        && left.needs =~= right.needs
}

pub open spec fn same_projection(left: Projection, right: Projection) -> bool {
    left.valid =~= right.valid
        && left.validated_offers =~= right.validated_offers
}

pub open spec fn mem_extends(old: MemIndex, new: MemIndex) -> bool {
    (forall|id: Id| #[trigger] old.facts.contains(id) ==> new.facts.contains(id))
        && (forall|owner: Id, key: Id|
            #[trigger] old.offers.contains((owner, key)) ==> new.offers.contains((owner, key)))
        && (forall|owner: Id, key: Id|
            #[trigger] old.needs.contains((owner, key)) ==> new.needs.contains((owner, key)))
}

pub open spec fn no_new_needs_for_valid_owners(old: MemIndex, new: MemIndex, proj: Projection) -> bool {
    forall|owner: Id, key: Id|
        #[trigger] new.needs.contains((owner, key)) && proj.valid.contains(owner) ==>
            old.needs.contains((owner, key))
}

pub open spec fn projection_context_sound(proj: Projection) -> bool {
    forall|owner: Id, key: Id|
        proj.validated_offers.contains((owner, key)) ==> proj.valid.contains(owner)
}

pub open spec fn projected_offer_has_provenance(mem: MemIndex, proj: Projection) -> bool {
    forall|owner: Id, key: Id|
        proj.validated_offers.contains((owner, key)) ==>
            #[trigger] mem.offers.contains((owner, key))
}

pub open spec fn valid_facts_have_memory_provenance(mem: MemIndex, proj: Projection) -> bool {
    forall|id: Id| #[trigger] proj.valid.contains(id) ==> mem.facts.contains(id)
}

pub open spec fn need_satisfied(proj: Projection, key: Id) -> bool {
    exists|owner: Id| proj.validated_offers.contains((owner, key))
}

pub open spec fn valid_facts_used_validated_context(mem: MemIndex, proj: Projection) -> bool {
    forall|owner: Id, key: Id|
        #[trigger] mem.needs.contains((owner, key)) && proj.valid.contains(owner) ==>
            need_satisfied(proj, key)
}

pub open spec fn seq_contains(xs: Seq<Id>, x: Id) -> bool {
    exists|i: int| 0 <= i < xs.len() && xs[i] == x
}

pub open spec fn project_queue_well_formed(e: EngineState) -> bool {
    forall|i: int| 0 <= i < e.to_project.len() ==>
        e.mem.facts.contains(#[trigger] e.to_project[i])
            || seq_contains(e.to_admit, e.to_project[i])
}

pub open spec fn mem_has_need_for(mem: MemIndex, key: Id) -> bool {
    exists|owner: Id| mem.needs.contains((owner, key))
}

pub open spec fn projection_has_validated_offer_for(proj: Projection, key: Id) -> bool {
    exists|owner: Id| proj.validated_offers.contains((owner, key))
}

pub open spec fn need_query_queue_well_formed(e: EngineState) -> bool {
    forall|i: int| 0 <= i < e.need_queries.len() ==>
        mem_has_need_for(e.mem, #[trigger] e.need_queries[i])
}

pub open spec fn offer_query_queue_well_formed(e: EngineState) -> bool {
    forall|i: int| 0 <= i < e.offer_queries.len() ==>
        projection_has_validated_offer_for(e.projection, #[trigger] e.offer_queries[i])
}

pub open spec fn queues_well_formed(e: EngineState) -> bool {
    project_queue_well_formed(e)
        && need_query_queue_well_formed(e)
        && offer_query_queue_well_formed(e)
}

pub open spec fn engine_invariant(e: EngineState) -> bool {
    projection_context_sound(e.projection)
        && projected_offer_has_provenance(e.mem, e.projection)
        && valid_facts_have_memory_provenance(e.mem, e.projection)
        && valid_facts_used_validated_context(e.mem, e.projection)
        && queues_well_formed(e)
}

pub open spec fn fact_can_project(proj: Projection, fact: SpecFact) -> bool {
    forall|i: int| 0 <= i < fact.needs.len() ==>
        need_satisfied(proj, #[trigger] fact.needs[i])
}

pub open spec fn admit_step(e: EngineState, fact: SpecFact, next: EngineState) -> bool {
    mem_extends(e.mem, next.mem)
        && mem_index_exact_fact_edges(next.mem, fact)
        && no_new_needs_for_valid_owners(e.mem, next.mem, e.projection)
        && same_projection(next.projection, e.projection)
        && queues_well_formed(next)
}

pub open spec fn query_step(e: EngineState, next: EngineState) -> bool {
    same_mem(next.mem, e.mem)
        && same_projection(next.projection, e.projection)
        && queues_well_formed(next)
}

pub open spec fn project_step(e: EngineState, fact: SpecFact, next: EngineState) -> bool {
    same_mem(next.mem, e.mem)
        && mem_index_exact_fact_edges(e.mem, fact)
        && same_projection(next.projection, project_fact_projection(e.projection, fact))
        && queues_well_formed(next)
}

pub open spec fn engine_step(e: EngineState, event: EngineEvent, next: EngineState) -> bool {
    match event {
        EngineEvent::Admit(fact) => admit_step(e, fact, next),
        EngineEvent::NeedQuery(_) => query_step(e, next),
        EngineEvent::Project(fact) => project_step(e, fact, next),
        EngineEvent::OfferQuery(_) => query_step(e, next),
    }
}

pub open spec fn engine_run(states: Seq<EngineState>, events: Seq<EngineEvent>) -> bool
    decreases events.len()
{
    if events.len() == 0 {
        states.len() == 1
    } else {
        states.len() == events.len() + 1
            && engine_step(states[0], events[0], states[1])
            && engine_run(
                states.subrange(1, states.len() as int),
                events.subrange(1, events.len() as int),
            )
    }
}

pub open spec fn promote_one_offer(proj: Projection, owner: Id, key: Id) -> Projection {
    Projection {
        valid: proj.valid.insert(owner),
        validated_offers: proj.validated_offers.insert((owner, key)),
    }
}

pub open spec fn promote_offers(proj: Projection, owner: Id, offers: Seq<Id>) -> Projection
    decreases offers.len()
{
    if offers.len() == 0 {
        Projection {
            valid: proj.valid.insert(owner),
            validated_offers: proj.validated_offers,
        }
    } else {
        let after_first = promote_one_offer(proj, owner, offers[0]);
        promote_offers(after_first, owner, offers.subrange(1, offers.len() as int))
    }
}

pub open spec fn project_fact_projection(proj: Projection, fact: SpecFact) -> Projection {
    if fact_can_project(proj, fact) {
        promote_offers(proj, fact.id, fact.offers)
    } else {
        proj
    }
}

pub open spec fn project_fact(e: EngineState, fact: SpecFact) -> EngineState {
    EngineState {
        mem: e.mem,
        projection: project_fact_projection(e.projection, fact),
        to_admit: e.to_admit,
        to_project: e.to_project,
        need_queries: e.need_queries,
        offer_queries: e.offer_queries,
    }
}

pub proof fn promote_one_preserves_context_sound(proj: Projection, owner: Id, key: Id)
    requires projection_context_sound(proj)
    ensures projection_context_sound(promote_one_offer(proj, owner, key))
{
    assert forall|o: Id, k: Id|
        promote_one_offer(proj, owner, key).validated_offers.contains((o, k))
            implies promote_one_offer(proj, owner, key).valid.contains(o)
    by {
        if (o, k) == (owner, key) {
            assert(o == owner);
        } else {
            assert(proj.validated_offers.contains((o, k)));
            assert(proj.valid.contains(o));
        }
    }
}

pub proof fn promote_one_records_offer_provenance(
    mem: MemIndex,
    proj: Projection,
    owner: Id,
    key: Id,
)
    requires
        projected_offer_has_provenance(mem, proj),
        mem.offers.contains((owner, key)),
    ensures projected_offer_has_provenance(mem, promote_one_offer(proj, owner, key))
{
    assert forall|o: Id, k: Id|
        promote_one_offer(proj, owner, key).validated_offers.contains((o, k))
            implies #[trigger] mem.offers.contains((o, k))
    by {
        if (o, k) == (owner, key) {
        } else {
            assert(proj.validated_offers.contains((o, k)));
            assert(mem.offers.contains((o, k)));
        }
    }
}

pub proof fn promote_offers_preserves_context_sound(
    proj: Projection,
    owner: Id,
    offers: Seq<Id>,
)
    requires projection_context_sound(proj)
    ensures projection_context_sound(promote_offers(proj, owner, offers))
    decreases offers.len()
{
    if offers.len() == 0 {
        assert forall|o: Id, k: Id|
            promote_offers(proj, owner, offers).validated_offers.contains((o, k))
                implies promote_offers(proj, owner, offers).valid.contains(o)
        by {
            assert(proj.validated_offers.contains((o, k)));
            assert(proj.valid.contains(o));
        }
    } else {
        let after_first = promote_one_offer(proj, owner, offers[0]);
        promote_one_preserves_context_sound(proj, owner, offers[0]);
        promote_offers_preserves_context_sound(
            after_first,
            owner,
            offers.subrange(1, offers.len() as int),
        );
    }
}

pub proof fn promote_offers_records_offer_provenance(
    mem: MemIndex,
    proj: Projection,
    owner: Id,
    offers: Seq<Id>,
)
    requires
        projected_offer_has_provenance(mem, proj),
        forall|i: int| 0 <= i < offers.len() ==>
            #[trigger] mem.offers.contains((owner, offers[i])),
    ensures projected_offer_has_provenance(mem, promote_offers(proj, owner, offers))
    decreases offers.len()
{
    if offers.len() == 0 {
        assert forall|o: Id, k: Id|
            promote_offers(proj, owner, offers).validated_offers.contains((o, k))
                implies #[trigger] mem.offers.contains((o, k))
        by {
            assert(proj.validated_offers.contains((o, k)));
            assert(mem.offers.contains((o, k)));
        }
    } else {
        let first = offers[0];
        let tail = offers.subrange(1, offers.len() as int);
        let after_first = promote_one_offer(proj, owner, first);
        assert(mem.offers.contains((owner, first)));
        promote_one_records_offer_provenance(mem, proj, owner, first);
        assert forall|i: int| 0 <= i < tail.len() implies
            #[trigger] mem.offers.contains((owner, tail[i]))
        by {
            assert(tail[i] == offers[i + 1]);
            assert(0 <= i + 1 < offers.len());
        }
        promote_offers_records_offer_provenance(mem, after_first, owner, tail);
    }
}

pub proof fn promote_offers_preserves_validated_offer(
    proj: Projection,
    owner: Id,
    offers: Seq<Id>,
    old_owner: Id,
    old_key: Id,
)
    requires proj.validated_offers.contains((old_owner, old_key))
    ensures promote_offers(proj, owner, offers).validated_offers.contains((old_owner, old_key))
    decreases offers.len()
{
    if offers.len() == 0 {
    } else {
        let after_first = promote_one_offer(proj, owner, offers[0]);
        assert(after_first.validated_offers.contains((old_owner, old_key)));
        promote_offers_preserves_validated_offer(
            after_first,
            owner,
            offers.subrange(1, offers.len() as int),
            old_owner,
            old_key,
        );
    }
}

pub proof fn promote_offers_preserves_need_satisfied(
    proj: Projection,
    owner: Id,
    offers: Seq<Id>,
    key: Id,
)
    requires need_satisfied(proj, key)
    ensures need_satisfied(promote_offers(proj, owner, offers), key)
{
    let provider = choose|provider: Id| proj.validated_offers.contains((provider, key));
    promote_offers_preserves_validated_offer(proj, owner, offers, provider, key);
}

pub proof fn promote_offers_new_valid_owner_source(
    proj: Projection,
    owner: Id,
    offers: Seq<Id>,
    id: Id,
)
    requires
        promote_offers(proj, owner, offers).valid.contains(id),
        !proj.valid.contains(id),
    ensures id == owner
    decreases offers.len()
{
    if offers.len() == 0 {
    } else {
        let after_first = promote_one_offer(proj, owner, offers[0]);
        if after_first.valid.contains(id) {
            assert(id == owner);
        } else {
            promote_offers_new_valid_owner_source(
                after_first,
                owner,
                offers.subrange(1, offers.len() as int),
                id,
            );
        }
    }
}

pub proof fn promote_offers_preserves_valid_memory_provenance(
    mem: MemIndex,
    proj: Projection,
    owner: Id,
    offers: Seq<Id>,
)
    requires
        valid_facts_have_memory_provenance(mem, proj),
        mem.facts.contains(owner),
    ensures valid_facts_have_memory_provenance(mem, promote_offers(proj, owner, offers))
{
    assert forall|id: Id|
        #[trigger] promote_offers(proj, owner, offers).valid.contains(id)
            implies mem.facts.contains(id)
    by {
        if proj.valid.contains(id) {
            assert(mem.facts.contains(id));
        } else {
            promote_offers_new_valid_owner_source(proj, owner, offers, id);
            assert(id == owner);
        }
    }
}

pub proof fn promote_offers_preserves_validated_context_for_valid_facts(
    mem: MemIndex,
    proj: Projection,
    fact: SpecFact,
)
    requires
        valid_facts_used_validated_context(mem, proj),
        mem_index_exact_fact_edges(mem, fact),
        fact_can_project(proj, fact),
    ensures valid_facts_used_validated_context(mem, promote_offers(proj, fact.id, fact.offers))
{
    assert forall|owner: Id, key: Id|
        #[trigger] mem.needs.contains((owner, key))
            && promote_offers(proj, fact.id, fact.offers).valid.contains(owner)
            implies need_satisfied(promote_offers(proj, fact.id, fact.offers), key)
    by {
        if proj.valid.contains(owner) {
            assert(need_satisfied(proj, key));
            promote_offers_preserves_need_satisfied(proj, fact.id, fact.offers, key);
        } else {
            promote_offers_new_valid_owner_source(proj, fact.id, fact.offers, owner);
            assert(owner == fact.id);
            assert(mem.needs.contains((fact.id, key)));
            assert(exists|i: int| 0 <= i < fact.needs.len() && fact.needs[i] == key);
            let i = choose|i: int| 0 <= i < fact.needs.len() && fact.needs[i] == key;
            assert(need_satisfied(proj, fact.needs[i]));
            assert(fact.needs[i] == key);
            assert(need_satisfied(proj, key));
            promote_offers_preserves_need_satisfied(proj, fact.id, fact.offers, key);
        }
    }
}

pub proof fn promote_offers_marks_owner_valid(proj: Projection, owner: Id, offers: Seq<Id>)
    ensures promote_offers(proj, owner, offers).valid.contains(owner)
    decreases offers.len()
{
    if offers.len() == 0 {
    } else {
        let after_first = promote_one_offer(proj, owner, offers[0]);
        promote_offers_marks_owner_valid(after_first, owner, offers.subrange(1, offers.len() as int));
    }
}

pub proof fn promote_offers_preserves_valid_fact(
    proj: Projection,
    owner: Id,
    offers: Seq<Id>,
    id: Id,
)
    requires proj.valid.contains(id)
    ensures promote_offers(proj, owner, offers).valid.contains(id)
    decreases offers.len()
{
    if offers.len() == 0 {
    } else {
        let after_first = promote_one_offer(proj, owner, offers[0]);
        promote_offers_preserves_valid_fact(
            after_first,
            owner,
            offers.subrange(1, offers.len() as int),
            id,
        );
    }
}

pub proof fn project_fact_preserves_valid_fact(proj: Projection, fact: SpecFact, id: Id)
    requires proj.valid.contains(id)
    ensures project_fact_projection(proj, fact).valid.contains(id)
{
    if fact_can_project(proj, fact) {
        promote_offers_preserves_valid_fact(proj, fact.id, fact.offers, id);
    }
}

pub proof fn project_schedule_preserves_valid_fact(
    proj: Projection,
    facts: Seq<SpecFact>,
    id: Id,
)
    requires proj.valid.contains(id)
    ensures project_schedule(proj, facts).valid.contains(id)
    decreases facts.len()
{
    if facts.len() == 0 {
    } else {
        let first = facts[0];
        let after_first = project_fact_projection(proj, first);
        project_fact_preserves_valid_fact(proj, first, id);
        project_schedule_preserves_valid_fact(after_first, facts.subrange(1, facts.len() as int), id);
    }
}

pub proof fn project_fact_preserves_queues_well_formed(e: EngineState, fact: SpecFact)
    requires queues_well_formed(e)
    ensures queues_well_formed(project_fact(e, fact))
{
    let pe = project_fact(e, fact);
    assert(project_queue_well_formed(pe)) by {
        assert forall|i: int| 0 <= i < pe.to_project.len() implies
            pe.mem.facts.contains(#[trigger] pe.to_project[i])
                || seq_contains(pe.to_admit, pe.to_project[i])
    by {
        if 0 <= i < pe.to_project.len() {
            let id = pe.to_project[i];
            assert(pe.to_project.len() == e.to_project.len());
            assert(0 <= i < e.to_project.len());
            assert(id == e.to_project[i]);
            assert(pe.to_admit =~= e.to_admit);
            assert(pe.mem.facts =~= e.mem.facts);
            if e.mem.facts.contains(id) {
                assert(pe.mem.facts.contains(id));
            } else {
                assert(seq_contains(e.to_admit, id));
                let j = choose|j: int| 0 <= j < e.to_admit.len() && e.to_admit[j] == id;
                assert(pe.to_admit[j] == id);
                assert(seq_contains(pe.to_admit, id));
            }
        }
    }
    }

    assert(need_query_queue_well_formed(pe)) by {
        assert forall|i: int| 0 <= i < pe.need_queries.len() implies
            mem_has_need_for(pe.mem, #[trigger] pe.need_queries[i])
    by {
        if 0 <= i < pe.need_queries.len() {
            let key = pe.need_queries[i];
            assert(pe.need_queries.len() == e.need_queries.len());
            assert(0 <= i < e.need_queries.len());
            assert(key == e.need_queries[i]);
            assert(exists|owner: Id| e.mem.needs.contains((owner, key)));
            let owner = choose|owner: Id| e.mem.needs.contains((owner, key));
            assert(pe.mem.needs =~= e.mem.needs);
            assert(pe.mem.needs.contains((owner, key)));
            assert(mem_has_need_for(pe.mem, key));
        }
    }
    }

    assert(offer_query_queue_well_formed(pe)) by {
        assert forall|i: int| 0 <= i < pe.offer_queries.len() implies
            projection_has_validated_offer_for(pe.projection, #[trigger] pe.offer_queries[i])
    by {
        if 0 <= i < pe.offer_queries.len() {
            let key = pe.offer_queries[i];
            assert(pe.offer_queries.len() == e.offer_queries.len());
            assert(0 <= i < e.offer_queries.len());
            assert(key == e.offer_queries[i]);
            assert(exists|owner: Id| e.projection.validated_offers.contains((owner, key)));
            let owner = choose|owner: Id| e.projection.validated_offers.contains((owner, key));
            if fact_can_project(e.projection, fact) {
                assert(pe.projection.validated_offers
                    =~= promote_offers(e.projection, fact.id, fact.offers).validated_offers);
                promote_offers_preserves_validated_offer(
                    e.projection,
                    fact.id,
                    fact.offers,
                    owner,
                    key,
                );
                assert(pe.projection.validated_offers.contains((owner, key)));
            } else {
                assert(pe.projection.validated_offers.contains((owner, key)));
            }
            assert(projection_has_validated_offer_for(pe.projection, key));
        }
    }
    }
    assert(queues_well_formed(pe));
}

pub proof fn project_fact_preserves_engine_invariant(e: EngineState, fact: SpecFact)
    requires
        engine_invariant(e),
        mem_index_exact_fact_edges(e.mem, fact),
    ensures engine_invariant(project_fact(e, fact))
{
    if fact_can_project(e.projection, fact) {
        promote_offers_preserves_context_sound(e.projection, fact.id, fact.offers);
        promote_offers_records_offer_provenance(e.mem, e.projection, fact.id, fact.offers);
        promote_offers_preserves_valid_memory_provenance(
            e.mem,
            e.projection,
            fact.id,
            fact.offers,
        );
        promote_offers_preserves_validated_context_for_valid_facts(e.mem, e.projection, fact);
    }
    project_fact_preserves_queues_well_formed(e, fact);
}

pub proof fn same_projection_preserves_need_satisfied(
    left: Projection,
    right: Projection,
    key: Id,
)
    requires
        same_projection(left, right),
        need_satisfied(left, key),
    ensures need_satisfied(right, key)
{
    let owner = choose|owner: Id| left.validated_offers.contains((owner, key));
    assert(right.validated_offers.contains((owner, key)));
}

pub proof fn same_state_preserves_engine_invariant(e: EngineState, next: EngineState)
    requires
        engine_invariant(e),
        same_mem(next.mem, e.mem),
        same_projection(next.projection, e.projection),
        queues_well_formed(next),
    ensures engine_invariant(next)
{
    assert forall|owner: Id, key: Id|
        next.projection.validated_offers.contains((owner, key))
            implies next.projection.valid.contains(owner)
    by {
        assert(e.projection.validated_offers.contains((owner, key)));
        assert(e.projection.valid.contains(owner));
    }

    assert forall|owner: Id, key: Id|
        next.projection.validated_offers.contains((owner, key))
            implies #[trigger] next.mem.offers.contains((owner, key))
    by {
        assert(e.projection.validated_offers.contains((owner, key)));
        assert(e.mem.offers.contains((owner, key)));
    }

    assert forall|id: Id|
        #[trigger] next.projection.valid.contains(id) implies next.mem.facts.contains(id)
    by {
        assert(e.projection.valid.contains(id));
        assert(e.mem.facts.contains(id));
    }

    assert forall|owner: Id, key: Id|
        #[trigger] next.mem.needs.contains((owner, key)) && next.projection.valid.contains(owner)
            implies need_satisfied(next.projection, key)
    by {
        assert(e.mem.needs.contains((owner, key)));
        assert(e.projection.valid.contains(owner));
        assert(need_satisfied(e.projection, key));
        same_projection_preserves_need_satisfied(e.projection, next.projection, key);
    }
}

pub proof fn admit_step_preserves_engine_invariant(
    e: EngineState,
    fact: SpecFact,
    next: EngineState,
)
    requires
        engine_invariant(e),
        admit_step(e, fact, next),
    ensures engine_invariant(next)
{
    assert forall|owner: Id, key: Id|
        next.projection.validated_offers.contains((owner, key))
            implies next.projection.valid.contains(owner)
    by {
        assert(e.projection.validated_offers.contains((owner, key)));
        assert(e.projection.valid.contains(owner));
    }

    assert forall|owner: Id, key: Id|
        next.projection.validated_offers.contains((owner, key))
            implies #[trigger] next.mem.offers.contains((owner, key))
    by {
        assert(e.projection.validated_offers.contains((owner, key)));
        assert(e.mem.offers.contains((owner, key)));
        assert(next.mem.offers.contains((owner, key)));
    }

    assert forall|id: Id|
        #[trigger] next.projection.valid.contains(id) implies next.mem.facts.contains(id)
    by {
        assert(e.projection.valid.contains(id));
        assert(e.mem.facts.contains(id));
        assert(next.mem.facts.contains(id));
    }

    assert forall|owner: Id, key: Id|
        #[trigger] next.mem.needs.contains((owner, key)) && next.projection.valid.contains(owner)
            implies need_satisfied(next.projection, key)
    by {
        assert(e.projection.valid.contains(owner));
        assert(e.mem.needs.contains((owner, key)));
        assert(need_satisfied(e.projection, key));
        same_projection_preserves_need_satisfied(e.projection, next.projection, key);
    }
}

pub proof fn project_step_preserves_engine_invariant(
    e: EngineState,
    fact: SpecFact,
    next: EngineState,
)
    requires
        engine_invariant(e),
        project_step(e, fact, next),
    ensures engine_invariant(next)
{
    let projected = project_fact(e, fact);
    project_fact_preserves_engine_invariant(e, fact);
    assert(same_mem(next.mem, projected.mem));
    assert(same_projection(next.projection, projected.projection));
    same_state_preserves_engine_invariant(projected, next);
}

pub proof fn engine_step_preserves_engine_invariant(
    e: EngineState,
    event: EngineEvent,
    next: EngineState,
)
    requires
        engine_invariant(e),
        engine_step(e, event, next),
    ensures engine_invariant(next)
{
    match event {
        EngineEvent::Admit(fact) => {
            admit_step_preserves_engine_invariant(e, fact, next);
        },
        EngineEvent::NeedQuery(_) => {
            same_state_preserves_engine_invariant(e, next);
        },
        EngineEvent::Project(fact) => {
            project_step_preserves_engine_invariant(e, fact, next);
        },
        EngineEvent::OfferQuery(_) => {
            same_state_preserves_engine_invariant(e, next);
        },
    }
}

pub proof fn engine_run_preserves_engine_invariant(
    states: Seq<EngineState>,
    events: Seq<EngineEvent>,
)
    requires
        engine_run(states, events),
        engine_invariant(states[0]),
    ensures
        forall|i: int| 0 <= i < states.len() ==>
            engine_invariant(#[trigger] states[i]),
    decreases events.len()
{
    if events.len() == 0 {
        assert(states.len() == 1);
    } else {
        assert(states.len() == events.len() + 1);
        engine_step_preserves_engine_invariant(states[0], events[0], states[1]);
        let tail_states = states.subrange(1, states.len() as int);
        let tail_events = events.subrange(1, events.len() as int);
        assert(tail_states[0] == states[1]);
        engine_run_preserves_engine_invariant(tail_states, tail_events);
        assert forall|i: int| 0 <= i < states.len() implies
            engine_invariant(#[trigger] states[i])
        by {
            if i == 0 {
            } else {
                assert(0 <= i - 1 < tail_states.len());
                assert(tail_states[i - 1] == states[i]);
            }
        }
    }
}

pub proof fn project_fact_when_ready_marks_valid(proj: Projection, fact: SpecFact)
    requires fact_can_project(proj, fact)
    ensures project_fact_projection(proj, fact).valid.contains(fact.id)
{
    promote_offers_marks_owner_valid(proj, fact.id, fact.offers);
}

pub open spec fn schedule_ready(proj: Projection, facts: Seq<SpecFact>) -> bool
    decreases facts.len()
{
    if facts.len() == 0 {
        true
    } else {
        fact_can_project(proj, facts[0])
            && schedule_ready(
                project_fact_projection(proj, facts[0]),
                facts.subrange(1, facts.len() as int),
            )
    }
}

pub open spec fn project_schedule(proj: Projection, facts: Seq<SpecFact>) -> Projection
    decreases facts.len()
{
    if facts.len() == 0 {
        proj
    } else {
        let after_first = project_fact_projection(proj, facts[0]);
        project_schedule(after_first, facts.subrange(1, facts.len() as int))
    }
}

pub proof fn project_schedule_preserves_context_sound(proj: Projection, facts: Seq<SpecFact>)
    requires projection_context_sound(proj)
    ensures projection_context_sound(project_schedule(proj, facts))
    decreases facts.len()
{
    if facts.len() == 0 {
    } else {
        let first = facts[0];
        let after_first = project_fact_projection(proj, first);
        if fact_can_project(proj, first) {
            promote_offers_preserves_context_sound(proj, first.id, first.offers);
        }
        project_schedule_preserves_context_sound(after_first, facts.subrange(1, facts.len() as int));
    }
}

pub proof fn ready_schedule_validates_all_facts(proj: Projection, facts: Seq<SpecFact>)
    requires
        projection_context_sound(proj),
        schedule_ready(proj, facts),
    ensures
        forall|i: int| 0 <= i < facts.len() ==>
            project_schedule(proj, facts).valid.contains(#[trigger] facts[i].id),
        projection_context_sound(project_schedule(proj, facts)),
    decreases facts.len()
{
    if facts.len() == 0 {
        project_schedule_preserves_context_sound(proj, facts);
    } else {
        let first = facts[0];
        let after_first = project_fact_projection(proj, first);
        let tail = facts.subrange(1, facts.len() as int);

        project_fact_when_ready_marks_valid(proj, first);
        assert(after_first.valid.contains(first.id));
        promote_offers_preserves_context_sound(proj, first.id, first.offers);
        assert(projection_context_sound(after_first));

        ready_schedule_validates_all_facts(after_first, tail);

        assert forall|i: int| 0 <= i < facts.len() implies
            #[trigger] project_schedule(proj, facts).valid.contains(facts[i].id)
        by {
            if i == 0 {
                assert(facts[i].id == first.id);
                project_schedule_preserves_valid_fact(after_first, tail, first.id);
            } else {
                assert(0 <= i - 1 < tail.len());
                assert(tail[i - 1] == facts[i]);
            }
        }
        project_schedule_preserves_context_sound(proj, facts);
    }
}

// -------------------------------------------------------------------------
// Link projector as an instance of the generic calculus.
// -------------------------------------------------------------------------

#[derive(Copy, Clone)]
pub struct LinkSpec {
    pub id: Id,
    pub prev: Option<Id>,
}

pub open spec fn link_self_offer(link: LinkSpec) -> (Id, Id) {
    (link.id, link.id)
}

pub open spec fn link_parent_need(link: LinkSpec) -> Option<(Id, Id)> {
    match link.prev {
        None => None,
        Some(parent) => Some((link.id, parent)),
    }
}

pub open spec fn link_project_validity(prev: Option<Id>, parent_validated: bool) -> Validity {
    match prev {
        None => Validity::Valid,
        Some(_) => if parent_validated { Validity::Valid } else { Validity::Invalid },
    }
}

pub open spec fn link_needs(link: LinkSpec) -> Seq<Id> {
    match link.prev {
        None => Seq::empty(),
        Some(parent) => Seq::empty().push(parent),
    }
}

pub open spec fn link_offers(link: LinkSpec) -> Seq<Id> {
    Seq::empty().push(link.id)
}

pub open spec fn link_fact(link: LinkSpec) -> SpecFact {
    SpecFact {
        id: link.id,
        needs: link_needs(link),
        offers: link_offers(link),
    }
}

pub open spec fn project_link_schedule(proj: Projection, chain: Seq<LinkSpec>) -> Projection
    decreases chain.len()
{
    if chain.len() == 0 {
        proj
    } else {
        let after_first = project_fact_projection(proj, link_fact(chain[0]));
        project_link_schedule(after_first, chain.subrange(1, chain.len() as int))
    }
}

pub proof fn link_projector_extracts_self_offer(link: LinkSpec)
    ensures
        link_self_offer(link) == (link.id, link.id),
        link_fact(link).offers.len() == 1,
        link_fact(link).offers[0] == link.id,
{
}

pub proof fn link_projector_extracts_parent_need(link: LinkSpec, parent: Id)
    requires link.prev == Some(parent)
    ensures
        link_parent_need(link) == Some((link.id, parent)),
        link_fact(link).needs.len() == 1,
        link_fact(link).needs[0] == parent,
{
}

pub proof fn link_projector_root_is_valid(link: LinkSpec)
    requires link.prev == Option::<Id>::None
    ensures
        link_project_validity(link.prev, false) == Validity::Valid,
        fact_can_project(Projection { valid: Set::empty(), validated_offers: Set::empty() }, link_fact(link)),
{
}

pub proof fn link_projector_child_valid_iff_parent_offer(link: LinkSpec, parent_validated: bool)
    requires link.prev != Option::<Id>::None
    ensures (link_project_validity(link.prev, parent_validated) == Validity::Valid) == parent_validated
{
}

pub open spec fn chain_connected_from(chain: Seq<LinkSpec>, proj: Projection) -> bool {
    if chain.len() == 0 {
        true
    } else {
        fact_can_project(proj, link_fact(chain[0]))
            && (forall|i: int| 1 <= i < chain.len() ==>
                #[trigger] chain[i].prev == Some(chain[i - 1].id))
    }
}

pub proof fn link_project_promotes_self_offer(proj: Projection, link: LinkSpec)
    requires fact_can_project(proj, link_fact(link))
    ensures
        project_fact_projection(proj, link_fact(link)).valid.contains(link.id),
        project_fact_projection(proj, link_fact(link)).validated_offers.contains((link.id, link.id)),
{
    let fact = link_fact(link);
    assert(fact.offers.len() == 1);
    assert(fact.offers[0] == link.id);
    project_fact_when_ready_marks_valid(proj, fact);
    let after_one = promote_one_offer(proj, link.id, link.id);
    assert(fact.offers.subrange(1, fact.offers.len() as int) == Seq::<Id>::empty());
    assert(after_one.validated_offers.contains((link.id, link.id)));
    assert(promote_offers(after_one, link.id, Seq::empty()).validated_offers.contains((link.id, link.id)));
}

pub proof fn project_link_schedule_preserves_valid_fact(
    proj: Projection,
    chain: Seq<LinkSpec>,
    id: Id,
)
    requires proj.valid.contains(id)
    ensures project_link_schedule(proj, chain).valid.contains(id)
    decreases chain.len()
{
    if chain.len() == 0 {
    } else {
        let first = chain[0];
        let after_first = project_fact_projection(proj, link_fact(first));
        project_fact_preserves_valid_fact(proj, link_fact(first), id);
        project_link_schedule_preserves_valid_fact(after_first, chain.subrange(1, chain.len() as int), id);
    }
}

pub proof fn project_link_schedule_preserves_context_sound(proj: Projection, chain: Seq<LinkSpec>)
    requires projection_context_sound(proj)
    ensures projection_context_sound(project_link_schedule(proj, chain))
    decreases chain.len()
{
    if chain.len() == 0 {
    } else {
        let first = chain[0];
        let after_first = project_fact_projection(proj, link_fact(first));
        if fact_can_project(proj, link_fact(first)) {
            promote_offers_preserves_context_sound(proj, first.id, link_fact(first).offers);
        }
        project_link_schedule_preserves_context_sound(after_first, chain.subrange(1, chain.len() as int));
    }
}

pub proof fn project_link_chain_validates_connected_chain(proj: Projection, chain: Seq<LinkSpec>)
    requires
        projection_context_sound(proj),
        chain_connected_from(chain, proj),
    ensures
        forall|i: int| 0 <= i < chain.len() ==>
            project_link_schedule(proj, chain).valid.contains(#[trigger] chain[i].id),
        projection_context_sound(project_link_schedule(proj, chain)),
    decreases chain.len()
{
    if chain.len() == 0 {
        project_link_schedule_preserves_context_sound(proj, chain);
    } else {
        let first = chain[0];
        let after_first = project_fact_projection(proj, link_fact(first));
        let tail = chain.subrange(1, chain.len() as int);

        assert(fact_can_project(proj, link_fact(first)));
        link_project_promotes_self_offer(proj, first);
        assert(after_first.valid.contains(first.id));
        assert(after_first.validated_offers.contains((first.id, first.id)));
        promote_offers_preserves_context_sound(proj, first.id, link_fact(first).offers);
        assert(projection_context_sound(after_first));

        if tail.len() > 0 {
            assert(tail[0] == chain[1]);
            assert(chain[1].prev == Some(chain[0].id));
            assert(chain[0].id == first.id);
            assert(need_satisfied(after_first, first.id));
            assert(fact_can_project(after_first, link_fact(tail[0])));
            assert forall|k: int| 1 <= k < tail.len() implies
                #[trigger] tail[k].prev == Some(tail[k - 1].id)
            by {
                assert(tail[k] == chain[k + 1]);
                assert(tail[k - 1] == chain[k]);
                assert(chain[k + 1].prev == Some(chain[k].id));
            }
        }
        assert(chain_connected_from(tail, after_first));
        project_link_chain_validates_connected_chain(after_first, tail);

        assert forall|i: int| 0 <= i < chain.len() implies
            #[trigger] project_link_schedule(proj, chain).valid.contains(chain[i].id)
        by {
            if i == 0 {
                assert(chain[i].id == first.id);
                project_link_schedule_preserves_valid_fact(after_first, tail, first.id);
            } else {
                assert(0 <= i - 1 < tail.len());
                assert(tail[i - 1] == chain[i]);
            }
        }
        project_link_schedule_preserves_context_sound(proj, chain);
    }
}

pub proof fn root_to_head_chain_projects_transitively(chain: Seq<LinkSpec>)
    requires
        chain.len() > 0,
        chain[0].prev == Option::<Id>::None,
        forall|i: int| 1 <= i < chain.len() ==>
            #[trigger] chain[i].prev == Some(chain[i - 1].id),
    ensures
        forall|i: int| 0 <= i < chain.len() ==>
            project_link_schedule(
                Projection { valid: Set::empty(), validated_offers: Set::empty() },
                chain,
            ).valid.contains(#[trigger] chain[i].id),
{
    let empty = Projection { valid: Set::empty(), validated_offers: Set::empty() };
    assert(projection_context_sound(empty));
    assert(chain_connected_from(chain, empty));
    project_link_chain_validates_connected_chain(empty, chain);
}

} // verus!
