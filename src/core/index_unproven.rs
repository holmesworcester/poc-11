//! Durable storage contract. Concrete SQLite lives in
//! [`crate::helpers::sqlite_unproven`].
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
