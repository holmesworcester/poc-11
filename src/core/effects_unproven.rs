//! Effect request/result types for the deterministic core turn. These are still
//! unproven, but they make the storage boundary explicit.
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
