// Verus model for poc-11's queue-oriented link projector. This file is compiled
// standalone by scripts/run_verus.sh and intentionally stays out of cargo's
// module tree. Crypto and durable storage are abstract contracts here; the proof
// is over typed in-memory facts, needs/offers, validated offers, and projection.
#![allow(unused)]
use vstd::prelude::*;

verus! {

pub type Id = int;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Validity {
    Valid,
    Invalid,
}

#[derive(Copy, Clone)]
pub struct LinkSpec {
    pub id: Id,
    pub prev: Option<Id>,
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

pub proof fn link_projector_extracts_self_offer(link: LinkSpec)
    ensures link_self_offer(link) == (link.id, link.id)
{
}

pub proof fn link_projector_extracts_parent_need(link: LinkSpec, parent: Id)
    requires link.prev == Some(parent)
    ensures link_parent_need(link) == Some((link.id, parent))
{
}

pub proof fn link_projector_root_is_valid(link: LinkSpec)
    requires link.prev == Option::<Id>::None
    ensures link_project_validity(link.prev, false) == Validity::Valid
{
}

pub proof fn link_projector_child_valid_iff_parent_offer(link: LinkSpec, parent_validated: bool)
    requires link.prev != Option::<Id>::None
    ensures (link_project_validity(link.prev, parent_validated) == Validity::Valid) == parent_validated
{
}

pub open spec fn mem_index_admits_link(mem: MemIndex, link: LinkSpec) -> bool {
    mem.facts.contains(link.id)
        && mem.offers.contains(link_self_offer(link))
        && match link_parent_need(link) {
            None => true,
            Some(need) => mem.needs.contains(need),
        }
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

pub open spec fn engine_invariant(e: EngineState) -> bool {
    projection_context_sound(e.projection)
        && projected_offer_has_provenance(e.mem, e.projection)
}

pub open spec fn parent_offer_validated(proj: Projection, parent: Id) -> bool {
    proj.validated_offers.contains((parent, parent))
}

pub open spec fn link_can_project(proj: Projection, link: LinkSpec) -> bool {
    match link.prev {
        None => true,
        Some(parent) => parent_offer_validated(proj, parent),
    }
}

pub open spec fn promote_self_offer(proj: Projection, id: Id) -> Projection {
    Projection {
        valid: proj.valid.insert(id),
        validated_offers: proj.validated_offers.insert((id, id)),
    }
}

pub open spec fn project_link_projection(proj: Projection, link: LinkSpec) -> Projection {
    if link_can_project(proj, link) {
        promote_self_offer(proj, link.id)
    } else {
        proj
    }
}

pub open spec fn project_link(e: EngineState, link: LinkSpec) -> EngineState {
    EngineState {
        mem: e.mem,
        projection: project_link_projection(e.projection, link),
        to_admit: e.to_admit,
        to_project: e.to_project,
        need_queries: e.need_queries,
        offer_queries: e.offer_queries,
    }
}

pub proof fn promotion_preserves_context_sound(proj: Projection, id: Id)
    requires projection_context_sound(proj)
    ensures projection_context_sound(promote_self_offer(proj, id))
{
    assert forall|owner: Id, key: Id|
        promote_self_offer(proj, id).validated_offers.contains((owner, key))
            implies promote_self_offer(proj, id).valid.contains(owner)
    by {
        if (owner, key) == (id, id) {
            assert(owner == id);
        } else {
            assert(proj.validated_offers.contains((owner, key)));
            assert(proj.valid.contains(owner));
        }
    }
}

pub proof fn promotion_records_offer_provenance(mem: MemIndex, proj: Projection, id: Id)
    requires
        projected_offer_has_provenance(mem, proj),
        mem.offers.contains((id, id)),
    ensures projected_offer_has_provenance(mem, promote_self_offer(proj, id))
{
    assert forall|owner: Id, key: Id|
        promote_self_offer(proj, id).validated_offers.contains((owner, key))
            implies #[trigger] mem.offers.contains((owner, key))
    by {
        if (owner, key) == (id, id) {
        } else {
            assert(proj.validated_offers.contains((owner, key)));
            assert(mem.offers.contains((owner, key)));
        }
    }
}

pub proof fn project_link_preserves_engine_invariant(e: EngineState, link: LinkSpec)
    requires
        engine_invariant(e),
        mem_index_admits_link(e.mem, link),
    ensures engine_invariant(project_link(e, link))
{
    if link_can_project(e.projection, link) {
        promotion_preserves_context_sound(e.projection, link.id);
        promotion_records_offer_provenance(e.mem, e.projection, link.id);
    }
}

pub proof fn valid_projection_promotes_only_valid_offer(proj: Projection, link: LinkSpec)
    requires
        projection_context_sound(proj),
        link_can_project(proj, link),
    ensures
        promote_self_offer(proj, link.id).valid.contains(link.id),
        promote_self_offer(proj, link.id).validated_offers.contains((link.id, link.id)),
        projection_context_sound(promote_self_offer(proj, link.id)),
{
    promotion_preserves_context_sound(proj, link.id);
}

pub proof fn project_link_preserves_valid_fact(proj: Projection, link: LinkSpec, id: Id)
    requires proj.valid.contains(id)
    ensures project_link_projection(proj, link).valid.contains(id)
{
}

pub proof fn project_chain_preserves_valid_fact(proj: Projection, chain: Seq<LinkSpec>, id: Id)
    requires proj.valid.contains(id)
    ensures project_chain(proj, chain).valid.contains(id)
    decreases chain.len()
{
    if chain.len() == 0 {
    } else {
        let first = chain[0];
        let after_first = project_link_projection(proj, first);
        project_link_preserves_valid_fact(proj, first, id);
        project_chain_preserves_valid_fact(after_first, chain.subrange(1, chain.len() as int), id);
    }
}

pub open spec fn chain_connected_from(chain: Seq<LinkSpec>, proj: Projection) -> bool {
    if chain.len() == 0 {
        true
    } else {
        link_can_project(proj, chain[0])
            && (forall|i: int| 1 <= i < chain.len() ==>
                #[trigger] chain[i].prev == Some(chain[i - 1].id))
    }
}

pub open spec fn all_admitted(mem: MemIndex, chain: Seq<LinkSpec>) -> bool {
    forall|i: int| 0 <= i < chain.len() ==> mem_index_admits_link(mem, #[trigger] chain[i])
}

pub open spec fn project_chain(proj: Projection, chain: Seq<LinkSpec>) -> Projection
    decreases chain.len()
{
    if chain.len() == 0 {
        proj
    } else {
        let first = chain[0];
        let after_first = project_link_projection(proj, first);
        project_chain(after_first, chain.subrange(1, chain.len() as int))
    }
}

pub proof fn project_chain_preserves_context_sound(proj: Projection, chain: Seq<LinkSpec>)
    requires projection_context_sound(proj)
    ensures projection_context_sound(project_chain(proj, chain))
    decreases chain.len()
{
    if chain.len() == 0 {
    } else {
        let first = chain[0];
        let after_first = project_link_projection(proj, first);
        if link_can_project(proj, first) {
            promotion_preserves_context_sound(proj, first.id);
        }
        project_chain_preserves_context_sound(after_first, chain.subrange(1, chain.len() as int));
    }
}

pub proof fn project_chain_validates_connected_chain(proj: Projection, chain: Seq<LinkSpec>)
    requires
        projection_context_sound(proj),
        chain_connected_from(chain, proj),
    ensures
        forall|i: int| 0 <= i < chain.len() ==>
            project_chain(proj, chain).valid.contains(#[trigger] chain[i].id),
        projection_context_sound(project_chain(proj, chain)),
    decreases chain.len()
{
    if chain.len() == 0 {
        project_chain_preserves_context_sound(proj, chain);
    } else {
        let first = chain[0];
        let after_first = project_link_projection(proj, first);
        let tail = chain.subrange(1, chain.len() as int);

        if link_can_project(proj, first) {
            valid_projection_promotes_only_valid_offer(proj, first);
        }
        assert(after_first.valid.contains(first.id));
        assert(after_first.validated_offers.contains((first.id, first.id)));
        assert(projection_context_sound(after_first));

        if tail.len() > 0 {
            assert(tail[0] == chain[1]);
            assert(chain[1].prev == Some(chain[0].id));
            assert(chain[0].id == first.id);
            assert(parent_offer_validated(after_first, first.id));
            assert(link_can_project(after_first, tail[0]));
            assert forall|k: int| 1 <= k < tail.len() implies
                #[trigger] tail[k].prev == Some(tail[k - 1].id)
            by {
                assert(tail[k] == chain[k + 1]);
                assert(tail[k - 1] == chain[k]);
                assert(chain[k + 1].prev == Some(chain[k].id));
            }
        }
        assert(chain_connected_from(tail, after_first));
        project_chain_validates_connected_chain(after_first, tail);

        assert forall|i: int| 0 <= i < chain.len() implies
            #[trigger] project_chain(proj, chain).valid.contains(chain[i].id)
        by {
            if i == 0 {
                assert(chain[i].id == first.id);
                project_chain_preserves_valid_fact(after_first, tail, first.id);
            } else {
                assert(0 <= i - 1 < tail.len());
                assert(tail[i - 1] == chain[i]);
            }
        }
        project_chain_preserves_context_sound(proj, chain);
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
            project_chain(
                Projection { valid: Set::empty(), validated_offers: Set::empty() },
                chain
            ).valid.contains(#[trigger] chain[i].id),
{
    let empty = Projection { valid: Set::empty(), validated_offers: Set::empty() };
    assert(projection_context_sound(empty));
    assert(chain_connected_from(chain, empty));
    project_chain_validates_connected_chain(empty, chain);
}

} // verus!
