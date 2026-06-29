//! Effect request/result types for the deterministic core turn. These are still
//! unproven, but they make the storage boundary explicit.
//!
//! Invariant checklist (Verus):
//! - [ ] Helper effects can request only raw facts or asserted-edge lookups; they
//!       cannot request validated state.
//! - [ ] Helper results are untrusted observations until the turn layer feeds
//!       them through decode, admission, or query-result handling.
//! - [ ] Missing, malformed, or failed helper results cannot create validity.
//! - [ ] No effect request or result can carry `Validity`, `Context`, or
//!       `Offer<Validated>` across the helper boundary.
use super::engine::EdgeAddr;
use super::item::FactId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectRequest {
    LoadFact(FactId),
    QueryOfferers(EdgeAddr),
    QueryNeeders(EdgeAddr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectResult {
    FactLoaded { id: FactId, bytes: Option<Vec<u8>> },
    OfferersLoaded { addr: EdgeAddr, ids: Vec<FactId> },
    NeedersLoaded { addr: EdgeAddr, ids: Vec<FactId> },
}
