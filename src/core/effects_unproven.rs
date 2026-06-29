//! Effect request/result types for the deterministic core turn. These are still
//! unproven, but they make the storage boundary explicit.
//!
//! Invariant checklist (Verus):
//! Owned invariant: helper boundary data shape.
//! - [ ] Safety: effect requests can ask helpers only for raw fact bytes or
//!       asserted-edge lookup results.
//! - [ ] Safety: effect results carry only untrusted bytes, ids, and addresses.
//! - [ ] Safety: `Validity`, `Context`, and `Offer<Validated>` never cross the
//!       helper boundary in an effect payload.
//! Imported theorems:
//! - `core::engine::EdgeAddr`: effect query addresses have the same address
//!   representation the engine uses.
//! Proof strategy:
//! - Prove by enum inspection that every request/result variant carries only
//!   `FactId`, `EdgeAddr`, raw bytes, or lists of ids.
//! - Leave semantic interpretation of effect results to `core::turn`.
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
