//! Verus-verified executable core for poc-11 projection.
//!
//! This crate is a normal path dependency of `linktoy`: tests and runtime
//! adapters can call these functions as ordinary Rust. `cargo-verus verify`
//! checks the function bodies against their contracts.

use vstd::prelude::*;

verus! {

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidityCore {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldCore {
    pub name: u64,
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AdmittedFactCore {
    pub id: u64,
    pub needs: Vec<u64>,
    pub offers: Vec<u64>,
    pub fields: Vec<FieldCore>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedOfferCore {
    pub owner: u64,
    pub key: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedFieldCore {
    pub owner: u64,
    pub field: FieldCore,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ProjectionPlanCore {
    pub valid: bool,
    pub promoted_offers: Vec<ValidatedOfferCore>,
    pub promoted_fields: Vec<ValidatedFieldCore>,
}

pub open spec fn has_validated_offer_spec(ctx: Seq<ValidatedOfferCore>, key: u64) -> bool {
    exists|i: int| #![auto] 0 <= i < ctx.len() && ctx[i].key == key
}

pub open spec fn all_needs_satisfied_spec(needs: Seq<u64>, ctx: Seq<ValidatedOfferCore>) -> bool {
    forall|i: int| 0 <= i < needs.len() ==> has_validated_offer_spec(ctx, #[trigger] needs[i])
}

pub open spec fn promoted_offers_match_fact(
    fact: AdmittedFactCore,
    promoted: Seq<ValidatedOfferCore>,
) -> bool {
    promoted.len() == fact.offers@.len()
        && (forall|i: int| 0 <= i < promoted.len() ==>
            #[trigger] promoted[i].owner == fact.id && promoted[i].key == fact.offers@[i])
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

pub fn has_validated_offer(ctx: &Vec<ValidatedOfferCore>, key: u64) -> (found: bool)
    ensures
        found == has_validated_offer_spec(ctx@, key),
{
    let mut i: usize = 0;
    while i < ctx.len()
        invariant
            0 <= i <= ctx.len(),
            ctx@.len() == ctx.len() as int,
            forall|k: int| #![auto] 0 <= k < i ==> ctx@[k].key != key,
        decreases ctx.len() - i,
    {
        if ctx[i].key == key {
            assert(has_validated_offer_spec(ctx@, key));
            return true;
        }
        i += 1;
    }
    assert(!has_validated_offer_spec(ctx@, key));
    false
}

pub fn all_needs_satisfied(
    needs: &Vec<u64>,
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
                #[trigger] promoted@[k].owner == fact.id && promoted@[k].key == fact.offers@[k],
        decreases fact.offers.len() - i,
    {
        let ghost before = promoted@;
        let offer_key = fact.offers[i];
        promoted.push(ValidatedOfferCore {
            owner: fact.id,
            key: offer_key,
        });
        assert(promoted@.len() == before.len() + 1);
        assert(before.len() == i);
        assert(promoted@[i as int].owner == fact.id);
        assert(promoted@[i as int].key == offer_key);
        assert(offer_key == fact.offers@[i as int]);
        assert forall|k: int| 0 <= k < promoted@.len() implies
            #[trigger] promoted@[k].owner == fact.id && promoted@[k].key == fact.offers@[k]
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
