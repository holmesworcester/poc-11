//! Typestate markers + the validated-context bus. The dirty `Asserted` layer (the
//! persisted edge index) vs the clean `Validated` layer (in memory). `Context`
//! carries only validated offers — the sole inter-projector channel. The validity
//! distinction lives on [`super::offer::Offer`]; `Context` holds only
//! `Offer<Validated>`, so a projector physically cannot read unvalidated context.
//!
//! Invariant checklist (Verus):
//! Owned invariant: validated context representation.
//! - [ ] `Context` can contain only `Offer<Validated>`.
//! - [ ] Unvalidated persisted edges cannot be placed in `Context`.
//! - [ ] `has_offer` answers only whether an exact validated match address is
//!       present; it does not inspect fact bodies or storage.
//! - [ ] Provenance of context contents belongs to `core::engine`.
use super::offer::{Key, Offer, Role};

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
        Self { offers }
    }

    /// "Is the offer on `key` validated?" — for a link, "is my parent valid?".
    pub fn has_offer(&self, role: Role, key: &Key) -> bool {
        self.offers.iter().any(|o| o.role == role && &o.key == key)
    }
}

/// Origin gate for unsigned local signals (§5). Present for completeness; the link
/// fact uses neither variant. `Local` is unforgeable outside the engine.
pub struct LocalToken(());

pub enum Origin {
    Network([u8; 32]),
    Local(LocalToken),
}
