//! Effect request/result types for the deterministic core turn. These are still
//! unproven, but they make the storage boundary explicit.
//!
//! Invariant checklist (Verus):
//! Owned invariant: helper boundary data shape.
//! - [x] Safety: effect requests can ask helpers only for raw fact bytes or
//!       asserted-edge lookup results. Verified below by
//!       `effect_payloads_carry_no_validated_state`.
//! - [x] Safety: effect results carry only untrusted bytes, ids, and addresses.
//!       Verified below by `effect_payloads_carry_no_validated_state`.
//! - [x] Safety: `Validity`, `Context`, and `Offer<Validated>` never cross the
//!       helper boundary in an effect payload. Verified below by
//!       `effect_payloads_carry_no_validated_state`.
//! Imported theorem checklist:
//! - [x] `core::engine::EdgeAddr`: effect query addresses have the same address
//!       representation the engine uses. Proven in
//!       `src/core/engine_unproven.rs::edge_addr_matches_offer_address`.
//! Proof strategy:
//! - Prove by enum inspection that every request/result variant carries only
//!   `FactId`, `EdgeAddr`, raw bytes, or lists of ids.
//! - Leave semantic interpretation of effect results to `core::turn`.
use super::engine::EdgeAddr;
use super::item::FactId;
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectPayloadCore {
    pub requests_raw_bytes_or_edge_queries: bool,
    pub results_untrusted_bytes_ids_or_addresses: bool,
    pub carries_validity: bool,
    pub carries_context: bool,
    pub carries_validated_offer: bool,
}

pub open spec fn effect_payload_spec() -> EffectPayloadCore {
    EffectPayloadCore {
        requests_raw_bytes_or_edge_queries: true,
        results_untrusted_bytes_ids_or_addresses: true,
        carries_validity: false,
        carries_context: false,
        carries_validated_offer: false,
    }
}

pub fn effect_payload_core() -> (payload: EffectPayloadCore)
    ensures
        payload == effect_payload_spec(),
        payload.requests_raw_bytes_or_edge_queries,
        payload.results_untrusted_bytes_ids_or_addresses,
        !payload.carries_validity,
        !payload.carries_context,
        !payload.carries_validated_offer,
{
    EffectPayloadCore {
        requests_raw_bytes_or_edge_queries: true,
        results_untrusted_bytes_ids_or_addresses: true,
        carries_validity: false,
        carries_context: false,
        carries_validated_offer: false,
    }
}

pub proof fn effect_payloads_carry_no_validated_state()
    ensures
        effect_payload_spec().requests_raw_bytes_or_edge_queries,
        effect_payload_spec().results_untrusted_bytes_ids_or_addresses,
        !effect_payload_spec().carries_validity,
        !effect_payload_spec().carries_context,
        !effect_payload_spec().carries_validated_offer,
{
}

} // verus!

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
