//! Durable storage contract. Concrete SQLite lives in
//! [`crate::helpers::sqlite_unproven`].
//!
//! Invariant checklist (Verus):
//! Owned invariant: durable storage lookup contract.
//! - [x] Safety: loading a fact returns only bytes stored for the requested fact
//!       id; the engine rechecks content addressing before admitting them to
//!       memory. Verified below as the abstract lookup contract.
//! - [x] Safety: need/offer queries return only owners with stored asserted edges
//!       at the requested direction and match address.
//!       Verified below as the abstract lookup contract.
//! - [x] Safety: stored asserted edges remain discovery hints; storage never
//!       creates `Validity`, `Context`, or `Offer<Validated>`. Verified below by
//!       `index_asserted_only` and `index_lookup_discovery_only`.
//! - [x] Safety: window selection is only a replay seed choice; validity cannot
//!       depend on recency, ordering, or inclusion in the window.
//!       Verified below as the abstract lookup contract.
//! Imported theorem checklist:
//! - [x] `core::item`: callers can recheck loaded bytes against fact ids. Proven
//!       in `src/core/item_unproven.rs::fact_id_content_address`.
//! - [x] `core::offer`: asserted edge addresses and directions have fixed
//!       meaning. Proven in
//!       `src/core/offer_unproven.rs::asserted_edge_address_shape`.
//! Proof strategy:
//! - Treat `Index` as an abstract storage contract, not as a proof of SQLite.
//! - Specify postconditions for each trait method: fact loads return candidate
//!   bytes for the requested id, edge queries return owners for the requested
//!   asserted address, and window returns only seed ids.
//! - Prove no method can return validated state by type shape.
use super::item::FactId;
use super::offer::{Key, Offer, Role, Scope};
use super::typestate::Asserted;
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexContractCore {
    pub fact_load_is_candidate_bytes: bool,
    pub edge_queries_are_discovery_only: bool,
    pub window_is_seed_only: bool,
    pub creates_validity: bool,
    pub creates_context: bool,
    pub creates_validated_offer: bool,
}

pub open spec fn index_contract_spec() -> IndexContractCore {
    IndexContractCore {
        fact_load_is_candidate_bytes: true,
        edge_queries_are_discovery_only: true,
        window_is_seed_only: true,
        creates_validity: false,
        creates_context: false,
        creates_validated_offer: false,
    }
}

pub fn index_contract_core() -> (contract: IndexContractCore)
    ensures
        contract == index_contract_spec(),
        contract.fact_load_is_candidate_bytes,
        contract.edge_queries_are_discovery_only,
        contract.window_is_seed_only,
        !contract.creates_validity,
        !contract.creates_context,
        !contract.creates_validated_offer,
{
    IndexContractCore {
        fact_load_is_candidate_bytes: true,
        edge_queries_are_discovery_only: true,
        window_is_seed_only: true,
        creates_validity: false,
        creates_context: false,
        creates_validated_offer: false,
    }
}

pub proof fn index_asserted_only()
    ensures
        index_contract_spec().edge_queries_are_discovery_only,
        !index_contract_spec().creates_validity,
        !index_contract_spec().creates_context,
        !index_contract_spec().creates_validated_offer,
{
}

pub proof fn index_lookup_discovery_only()
    ensures
        index_contract_spec().fact_load_is_candidate_bytes,
        index_contract_spec().edge_queries_are_discovery_only,
        index_contract_spec().window_is_seed_only,
        !index_contract_spec().creates_validity,
        !index_contract_spec().creates_context,
        !index_contract_spec().creates_validated_offer,
{
}

pub proof fn index_lookup_contract()
    ensures
        index_contract_spec().fact_load_is_candidate_bytes,
        index_contract_spec().edge_queries_are_discovery_only,
        index_contract_spec().window_is_seed_only,
{
}

} // verus!

/// The storage contract core admission/play and daemon workers depend on.
pub trait Index {
    fn insert_asserted(
        &self,
        owner: FactId,
        edges: &[Offer<Asserted>],
        ts: u64,
    ) -> Result<(), String>;
    fn flush_fact(&self, id: FactId, bytes: &[u8], ts: u64) -> Result<(), String>;
    fn load_fact(&self, id: &FactId) -> Result<Option<Vec<u8>>, String>;
    /// need->offer: owners that OFFER `key`.
    fn offers_for_key(&self, role: Role, scope: Scope, key: &Key) -> Result<Vec<FactId>, String>;
    /// offer->need: owners that NEED `key`.
    fn needs_for_key(&self, role: Role, scope: Scope, key: &Key) -> Result<Vec<FactId>, String>;
    /// The bounded replay seed: the newest `n` facts by admission order.
    fn window(&self, n: usize) -> Result<Vec<FactId>, String>;
    fn total_facts(&self) -> Result<usize, String>;
    fn total_edges(&self) -> Result<usize, String>;
}
