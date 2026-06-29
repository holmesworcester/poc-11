//! Pass 1 (admission). [`admit`] is the durable path for new/incoming facts:
//! running it extracts syntactic edges, persists them (`Asserted`), and flushes
//! durable bytes. Replay of already-stored facts uses the queue engine's
//! read-only storage admission path instead, indexing decoded facts in memory
//! without writing bytes or edges back to persistence.
use super::index::Index;
use super::item::{fact_id, FactId};
use super::projector::Projector;

/// A Pass-1 token. The fields are private, so no projector or emitted-fact path
/// can fabricate one outside the core admission/play modules.
pub struct Admitted<I> {
    item: I,
    id: FactId,
}

impl<I> Admitted<I> {
    pub(crate) fn from_parts(item: I, id: FactId) -> Self {
        Self { item, id }
    }

    pub fn item(&self) -> &I {
        &self.item
    }
    pub fn id(&self) -> FactId {
        self.id
    }
}

/// Admit one new/incoming item: extract → persist edges (Asserted) → flush bytes
/// if durable. Idempotent writes make repeated network/local admission safe, but
/// replay does not call this for facts already loaded from storage.
pub fn admit<P: Projector>(
    item: P::Item,
    ts: u64,
    idx: &dyn Index,
) -> Result<Admitted<P::Item>, String> {
    let bytes = P::encode(&item);
    let id = fact_id(&bytes);
    let edges = P::extract(&item);
    idx.insert_asserted(id, &edges, ts)?;
    if P::durable(&item) {
        idx.flush_fact(id, &bytes, ts)?;
    }
    Ok(Admitted { item, id })
}
