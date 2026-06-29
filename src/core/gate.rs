//! Pure core readiness and projection-plan gate. Runtime code calls these
//! functions as ordinary Rust; `cargo-verus verify` checks the bodies against
//! their contracts.
//!
//! Invariant checklist (Verus):
//! Owned invariant: pure readiness and projection-plan gate.
//! - [ ] Safety: `fact_ready_core` is true exactly when every asserted need has a
//!       matching validated offer in the supplied abstract context.
//! - [ ] Safety: `project_fact_core` marks an abstract fact valid only when the
//!       fact-family projector returned valid and readiness holds.
//! - [ ] Safety: a valid projection plan promotes only offers and fields copied
//!       from the projected fact under that fact's owner.
//! - [ ] Safety: an invalid projection plan promotes nothing.
//! Imported theorems:
//! - None beyond Verus/Vstd sequence and equality reasoning; this is the pure
//!   gate other core proofs import.
//! Proof strategy:
//! - Prove address equality and context membership against executable scans.
//! - Prove readiness by induction over the fact's needs.
//! - Prove promoted offer/field vectors are constructed by copying only from the
//!   projected fact and assigning the projected fact id as owner.
//! - Prove `project_fact_core` combines the projector decision with readiness and
//!   returns empty promoted vectors on invalid plans.

use vstd::prelude::*;

verus! {

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidityCore {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bytes32Core {
    pub w0: u64,
    pub w1: u64,
    pub w2: u64,
    pub w3: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeAddrCore {
    pub role: Bytes32Core,
    pub scope: u64,
    pub key: Bytes32Core,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldCore {
    pub name: u64,
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AdmittedFactCore {
    pub id: Bytes32Core,
    pub needs: Vec<EdgeAddrCore>,
    pub offers: Vec<EdgeAddrCore>,
    pub fields: Vec<FieldCore>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedOfferCore {
    pub owner: Bytes32Core,
    pub addr: EdgeAddrCore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedFieldCore {
    pub owner: Bytes32Core,
    pub field: FieldCore,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ProjectionPlanCore {
    pub valid: bool,
    pub promoted_offers: Vec<ValidatedOfferCore>,
    pub promoted_fields: Vec<ValidatedFieldCore>,
}

pub open spec fn bytes32_eq_spec(left: Bytes32Core, right: Bytes32Core) -> bool {
    left.w0 == right.w0 && left.w1 == right.w1 && left.w2 == right.w2 && left.w3 == right.w3
}

pub open spec fn edge_addr_eq_spec(left: EdgeAddrCore, right: EdgeAddrCore) -> bool {
    bytes32_eq_spec(left.role, right.role)
        && left.scope == right.scope
        && bytes32_eq_spec(left.key, right.key)
}

pub fn bytes32_eq(left: Bytes32Core, right: Bytes32Core) -> (equal: bool)
    ensures
        equal == bytes32_eq_spec(left, right),
{
    left.w0 == right.w0 && left.w1 == right.w1 && left.w2 == right.w2 && left.w3 == right.w3
}

pub fn edge_addr_eq(left: EdgeAddrCore, right: EdgeAddrCore) -> (equal: bool)
    ensures
        equal == edge_addr_eq_spec(left, right),
{
    bytes32_eq(left.role, right.role)
        && left.scope == right.scope
        && bytes32_eq(left.key, right.key)
}

pub open spec fn has_validated_offer_spec(ctx: Seq<ValidatedOfferCore>, addr: EdgeAddrCore) -> bool {
    exists|i: int| #![auto] 0 <= i < ctx.len() && edge_addr_eq_spec(ctx[i].addr, addr)
}

pub open spec fn all_needs_satisfied_spec(
    needs: Seq<EdgeAddrCore>,
    ctx: Seq<ValidatedOfferCore>,
) -> bool {
    forall|i: int| 0 <= i < needs.len() ==> has_validated_offer_spec(ctx, #[trigger] needs[i])
}

pub open spec fn promoted_offers_match_fact(
    fact: AdmittedFactCore,
    promoted: Seq<ValidatedOfferCore>,
) -> bool {
    promoted.len() == fact.offers@.len()
        && (forall|i: int| 0 <= i < promoted.len() ==>
            #[trigger] promoted[i].owner == fact.id
                && edge_addr_eq_spec(promoted[i].addr, fact.offers@[i]))
}

pub open spec fn promoted_fields_match_fact(
    fact: AdmittedFactCore,
    promoted: Seq<ValidatedFieldCore>,
) -> bool {
    promoted.len() == fact.fields@.len()
        && (forall|i: int| 0 <= i < promoted.len() ==>
            #[trigger] promoted[i].owner == fact.id && promoted[i].field == fact.fields@[i])
}

pub open spec fn projection_plan_sound(
    fact: AdmittedFactCore,
    ctx: Seq<ValidatedOfferCore>,
    decision: ValidityCore,
    plan: ProjectionPlanCore,
) -> bool {
    plan.valid == (decision == ValidityCore::Valid && all_needs_satisfied_spec(fact.needs@, ctx))
        && (plan.valid ==> promoted_offers_match_fact(fact, plan.promoted_offers@))
        && (plan.valid ==> promoted_fields_match_fact(fact, plan.promoted_fields@))
        && (!plan.valid ==> plan.promoted_offers@.len() == 0)
        && (!plan.valid ==> plan.promoted_fields@.len() == 0)
}

pub fn has_validated_offer(ctx: &Vec<ValidatedOfferCore>, addr: EdgeAddrCore) -> (found: bool)
    ensures
        found == has_validated_offer_spec(ctx@, addr),
{
    let mut i: usize = 0;
    while i < ctx.len()
        invariant
            0 <= i <= ctx.len(),
            ctx@.len() == ctx.len() as int,
            forall|k: int| #![auto] 0 <= k < i ==> !edge_addr_eq_spec(ctx@[k].addr, addr),
        decreases ctx.len() - i,
    {
        if edge_addr_eq(ctx[i].addr, addr) {
            assert(has_validated_offer_spec(ctx@, addr));
            return true;
        }
        i += 1;
    }
    assert(!has_validated_offer_spec(ctx@, addr));
    false
}

pub fn all_needs_satisfied(
    needs: &Vec<EdgeAddrCore>,
    ctx: &Vec<ValidatedOfferCore>,
) -> (ready: bool)
    ensures
        ready == all_needs_satisfied_spec(needs@, ctx@),
{
    let mut i: usize = 0;
    while i < needs.len()
        invariant
            0 <= i <= needs.len(),
            needs@.len() == needs.len() as int,
            forall|k: int| #![auto] 0 <= k < i ==> has_validated_offer_spec(ctx@, needs@[k]),
        decreases needs.len() - i,
    {
        if !has_validated_offer(ctx, needs[i]) {
            assert(!all_needs_satisfied_spec(needs@, ctx@));
            return false;
        }
        i += 1;
    }
    assert(all_needs_satisfied_spec(needs@, ctx@));
    true
}

pub fn fact_ready_core(
    fact: &AdmittedFactCore,
    ctx: &Vec<ValidatedOfferCore>,
) -> (ready: bool)
    ensures
        ready == all_needs_satisfied_spec(fact.needs@, ctx@),
{
    all_needs_satisfied(&fact.needs, ctx)
}

pub fn promoted_offers_for(fact: &AdmittedFactCore) -> (promoted: Vec<ValidatedOfferCore>)
    ensures
        promoted_offers_match_fact(*fact, promoted@),
{
    let mut promoted: Vec<ValidatedOfferCore> = Vec::new();
    let mut i: usize = 0;
    while i < fact.offers.len()
        invariant
            0 <= i <= fact.offers.len(),
            fact.offers@.len() == fact.offers.len() as int,
            promoted@.len() == i,
            forall|k: int| 0 <= k < promoted@.len() ==>
                #[trigger] promoted@[k].owner == fact.id
                    && edge_addr_eq_spec(promoted@[k].addr, fact.offers@[k]),
        decreases fact.offers.len() - i,
    {
        let ghost before = promoted@;
        let offer_addr = fact.offers[i];
        promoted.push(ValidatedOfferCore {
            owner: fact.id,
            addr: offer_addr,
        });
        assert(promoted@.len() == before.len() + 1);
        assert(before.len() == i);
        assert(promoted@[i as int].owner == fact.id);
        assert(edge_addr_eq_spec(promoted@[i as int].addr, offer_addr));
        assert(edge_addr_eq_spec(offer_addr, fact.offers@[i as int]));
        assert forall|k: int| 0 <= k < promoted@.len() implies
            #[trigger] promoted@[k].owner == fact.id
                && edge_addr_eq_spec(promoted@[k].addr, fact.offers@[k])
        by {
            if k < before.len() {
                assert(promoted@[k] == before[k]);
            } else {
                assert(k == i as int);
            }
        }
        i += 1;
    }
    assert(i == fact.offers.len());
    assert(promoted@.len() == fact.offers@.len());
    assert(promoted_offers_match_fact(*fact, promoted@));
    promoted
}

pub fn promoted_fields_for(fact: &AdmittedFactCore) -> (promoted: Vec<ValidatedFieldCore>)
    ensures
        promoted_fields_match_fact(*fact, promoted@),
{
    let mut promoted: Vec<ValidatedFieldCore> = Vec::new();
    let mut i: usize = 0;
    while i < fact.fields.len()
        invariant
            0 <= i <= fact.fields.len(),
            fact.fields@.len() == fact.fields.len() as int,
            promoted@.len() == i,
            forall|k: int| 0 <= k < promoted@.len() ==>
                #[trigger] promoted@[k].owner == fact.id && promoted@[k].field == fact.fields@[k],
        decreases fact.fields.len() - i,
    {
        let ghost before = promoted@;
        let field = fact.fields[i];
        promoted.push(ValidatedFieldCore {
            owner: fact.id,
            field,
        });
        assert(promoted@.len() == before.len() + 1);
        assert(before.len() == i);
        assert(promoted@[i as int].owner == fact.id);
        assert(promoted@[i as int].field == field);
        assert(field == fact.fields@[i as int]);
        assert forall|k: int| 0 <= k < promoted@.len() implies
            #[trigger] promoted@[k].owner == fact.id && promoted@[k].field == fact.fields@[k]
        by {
            if k < before.len() {
                assert(promoted@[k] == before[k]);
            } else {
                assert(k == i as int);
            }
        }
        i += 1;
    }
    assert(i == fact.fields.len());
    assert(promoted@.len() == fact.fields@.len());
    assert(promoted_fields_match_fact(*fact, promoted@));
    promoted
}

pub fn project_fact_core(
    fact: &AdmittedFactCore,
    ctx: &Vec<ValidatedOfferCore>,
    decision: ValidityCore,
) -> (plan: ProjectionPlanCore)
    ensures
        projection_plan_sound(*fact, ctx@, decision, plan),
{
    let ready = all_needs_satisfied(&fact.needs, ctx);
    let decision_valid = match decision {
        ValidityCore::Valid => true,
        ValidityCore::Invalid => false,
    };
    if decision_valid && ready {
        ProjectionPlanCore {
            valid: true,
            promoted_offers: promoted_offers_for(fact),
            promoted_fields: promoted_fields_for(fact),
        }
    } else {
        ProjectionPlanCore {
            valid: false,
            promoted_offers: Vec::new(),
            promoted_fields: Vec::new(),
        }
    }
}

} // verus!
