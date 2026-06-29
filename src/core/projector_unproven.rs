//! A projector is two halves (§5): a context-free `extract` (Pass 1, syntactic
//! edges) and a `project` (Pass 2, validated state). Confinement is the parameter
//! list — no method receives a `Db`/`Index`/clock, so a projector cannot reach
//! storage or IO. Only core ([`super::admit`] / [`super::play`] / the runtime) and
//! the daemon's workers hold an [`super::index::Index`].
//!
//! Invariant checklist (Verus):
//! - [ ] Each projector proves `decode(encode(item)) == item` for its semantic
//!       item type.
//! - [ ] Each projector rejects malformed or foreign-family bytes.
//! - [ ] Each projector proves accepted bytes are canonical:
//!       `encode(decode(bytes)) == bytes`.
//! - [ ] `durable(item)` is deterministic and depends only on the item.
//! - [ ] `extract(item)` is context-free and exact for all addresses the
//!       projector may later read.
//! - [ ] `project(admitted, ctx, state)` reads other facts only through validated
//!       `ctx`.
//! - [ ] `project` mutates only its private projector state and returns explicit
//!       emitted facts/effects.
//! - [ ] Any emitted fact bytes are canonical for their route before core admits
//!       them.
use super::admit::Admitted;
use super::offer::Offer;
use super::typestate::{Asserted, Context, Validity};

/// A fact emitted by `project`; it re-enters `admit` + `play` like any input.
pub struct EmittedFact {
    pub bytes: Vec<u8>,
}

/// What `project` returns: this item's validity plus any emitted facts.
pub struct ProjectOutcome {
    pub validity: Validity,
    pub emitted: Vec<EmittedFact>,
}

pub trait Projector {
    type Item;
    /// PRIVATE read-model — only this projector writes it.
    type State: Default;

    /// Canonical bytes (the content-id source). `decode` is the exact inverse, and
    /// errors on bytes that aren't this family's (so the runtime can classify by
    /// attempting `decode`).
    fn encode(item: &Self::Item) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Result<Self::Item, String>;

    /// Durable (flush bytes + edges) vs volatile (edges only). Content-pure.
    fn durable(_item: &Self::Item) -> bool {
        true
    }

    /// Pass 1: no `&self`, no state, no ctx — purity is the absence of parameters.
    /// Returns the `Asserted` edges (needs + offers) the index persists.
    fn extract(item: &Self::Item) -> Vec<Offer<Asserted>>;

    /// Pass 2: `&mut` to its OWN state; reads others only through validated `ctx`.
    fn project(item: &Admitted<Self::Item>, ctx: Context, st: &mut Self::State) -> ProjectOutcome;
}
