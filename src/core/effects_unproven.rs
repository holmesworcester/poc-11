//! Effect request/result types for the deterministic core turn. These are still
//! unproven, but they make the storage boundary explicit.
//!
//! Invariant checklist (Verus):
//! - [ ] Every `EffectRequest` is a pure request for bytes or asserted-edge
//!       owner ids; it cannot itself create memory, validity, or context.
//! - [ ] Every successful `EffectResult` is interpreted only by the verified
//!       turn/application function.
//! - [ ] Missing facts, empty query results, and effect errors cannot create
//!       validated state.
//! - [ ] Storage results are discovery data only; they must pass canonical
//!       decode/admission before any projection step can depend on them.
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
