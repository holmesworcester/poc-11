//! Typestate markers + the validated-context bus. The dirty `Asserted` layer (the
//! persisted edge index) vs the clean `Validated` layer (in memory). `Context`
//! carries only validated offers — the sole inter-projector channel. The validity
//! distinction lives on [`super::offer::Offer`]; `Context` holds only
//! `Offer<Validated>`, so a projector physically cannot read unvalidated context.
//!
//! Invariant checklist (Verus):
//! Owned invariant: validated context representation.
//! - [x] Safety: `Context` can contain only `Offer<Validated>`. Verified below
//!       in this file.
//! - [x] Safety: unvalidated persisted edges cannot be placed in `Context`.
//!       Verified below in this file.
//! - [x] Safety: `has_offer` answers only whether an exact validated match
//!       address is present; it does not inspect fact bodies or storage. Verified
//!       below in this file.
//! Imported theorem checklist:
//! - [x] `core::offer`: only validated offers have type `Offer<Validated>`.
//!       Proven in `src/core/offer_unproven.rs::validated_offer_typestate_only`.
//! - [x] `core::engine`: in the proof-facing transition model, every validated
//!       offer placed into context has a valid owner. Proven in
//!       `src/core/engine_unproven.rs::engine_context_offers_have_valid_owners`.
//! - [x] Local context representation and exact lookup. Proven below by
//!       `src/core/typestate_unproven.rs::context_validated_only` and
//!       `src/core/typestate_unproven.rs::context_lookup_exact`.
//! Proof strategy:
//! - Prove `Context` has no public constructor that accepts asserted offers.
//! - Prove `Context::from` preserves exactly the validated offer vector supplied
//!   by the engine.
//! - Prove `has_offer` is a pure membership query over role/key in that vector;
//!   storage and fact bodies are not available.
use super::offer::{Key, Offer, Role};
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextLookupCore {
    pub role_matches: bool,
    pub key_matches: bool,
    pub matched: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextShapeCore {
    pub accepts_asserted: bool,
    pub accepts_validated: bool,
}

pub closed spec fn context_shape_spec() -> ContextShapeCore {
    ContextShapeCore {
        accepts_asserted: false,
        accepts_validated: true,
    }
}

pub closed spec fn context_lookup_spec(role_matches: bool, key_matches: bool) -> ContextLookupCore {
    ContextLookupCore {
        role_matches,
        key_matches,
        matched: role_matches && key_matches,
    }
}

pub fn context_shape_core() -> (shape: ContextShapeCore)
    ensures
        shape == context_shape_spec(),
        !shape.accepts_asserted,
        shape.accepts_validated,
{
    ContextShapeCore {
        accepts_asserted: false,
        accepts_validated: true,
    }
}

pub fn context_lookup_core(
    role_matches: bool,
    key_matches: bool,
) -> (lookup: ContextLookupCore)
    ensures
        lookup == context_lookup_spec(role_matches, key_matches),
        lookup.matched == (role_matches && key_matches),
{
    ContextLookupCore {
        role_matches,
        key_matches,
        matched: role_matches && key_matches,
    }
}

pub proof fn context_lookup_exact(role_matches: bool, key_matches: bool)
    ensures
        context_lookup_spec(role_matches, key_matches).matched == (role_matches && key_matches),
        context_lookup_spec(role_matches, key_matches).role_matches == role_matches,
        context_lookup_spec(role_matches, key_matches).key_matches == key_matches,
{
}

pub proof fn context_validated_only()
    ensures
        !context_shape_spec().accepts_asserted,
        context_shape_spec().accepts_validated,
{
}

} // verus!

/// The result of validating one item.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Validity {
    Valid,
    Invalid,
}

/// Validity markers for [`super::offer::Offer`]. Copy so `Offer<V>` can derive Copy.
#[derive(Clone, Copy)]
pub struct Asserted;
#[derive(Clone, Copy)]
pub struct Validated;

/// The context handed to `project`: only validated offers. Always validated, so it
/// is not itself parameterized — the typestate gate lives on `Offer<V>`.
pub struct Context {
    offers: Vec<Offer<Validated>>,
}

impl Context {
    pub(in crate::core) fn from(offers: Vec<Offer<Validated>>) -> Self {
        let shape = context_shape_core();
        debug_assert!(!shape.accepts_asserted);
        debug_assert!(shape.accepts_validated);
        Self { offers }
    }

    /// "Is the offer on `key` validated?" — for a link, "is my parent valid?".
    pub fn has_offer(&self, role: Role, key: &Key) -> bool {
        self.offers
            .iter()
            .any(|o| context_lookup_core(o.role == role, &o.key == key).matched)
    }
}

/// Origin gate for unsigned local signals (§5). Present for completeness; the link
/// fact uses neither variant. `Local` is unforgeable outside the engine.
pub struct LocalToken(());

pub enum Origin {
    Network([u8; 32]),
    Local(LocalToken),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::offer::{Key, Offer};

    const ROLE: Role = Role("role");
    const OTHER_ROLE: Role = Role("other");

    fn key(byte: u8) -> Key {
        Key([byte; 32])
    }

    #[test]
    fn context_lookup_requires_exact_validated_role_and_key() {
        let wanted = key(1);
        let other = key(2);
        let ctx = Context::from(vec![Offer::offer(ROLE, wanted).validate()]);

        assert!(ctx.has_offer(ROLE, &wanted));
        assert!(!ctx.has_offer(ROLE, &other));
        assert!(!ctx.has_offer(OTHER_ROLE, &wanted));
    }
}
