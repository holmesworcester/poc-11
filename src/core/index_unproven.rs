//! Durable storage contract. Concrete SQLite lives in
//! [`crate::helpers::sqlite_unproven`].
//!
//! Invariant checklist (Verus):
//! Owned invariant: durable storage lookup contract.
//! - [ ] Safety: loading a fact returns only bytes stored for the requested fact
//!       id; the engine rechecks content addressing before admitting them to
//!       memory.
//! - [ ] Safety: need/offer queries return only owners with stored asserted edges
//!       at the requested direction and match address.
//! - [ ] Safety: stored asserted edges remain discovery hints; storage never
//!       creates `Validity`, `Context`, or `Offer<Validated>`.
//! - [ ] Safety: window selection is only a replay seed choice; validity cannot
//!       depend on recency, ordering, or inclusion in the window.
//! Imported theorem checklist:
//! - [ ] `core::item`: callers can recheck loaded bytes against fact ids. Owner:
//!       `src/core/item_unproven.rs`, planned theorem `fact_id_content_address`.
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
