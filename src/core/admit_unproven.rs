//! Pass 1 (admission). [`admit`] is the durable path for new/incoming facts:
//! running it extracts syntactic edges, persists them (`Asserted`), and flushes
//! durable bytes. Replay of already-stored facts uses the queue engine's
//! read-only storage admission path instead, indexing decoded facts in memory
//! without writing bytes or edges back to persistence.
//!
//! Invariant checklist (Verus):
//! Owned invariant: new/local fact admission creates only asserted state.
//! - [ ] Admission creates an `Admitted` token and asserted storage state only; it
//!       creates no validity, validated offer, or validated context.
//! - [ ] The admitted token's id/body relation is derived from `core::item`
//!       content addressing and the fact family's canonical encoder.
//! - [ ] Stored asserted edges are exactly the fact family's extraction output;
//!       extraction exactness is proved by the fact-family projector.
//! - [ ] Fact bytes are flushed only when the fact-family durability predicate
//!       says this item is durable.
//! - [ ] Any non-storage admission path inside core must preserve the same
//!       id/body relation before projection.
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
    pub(in crate::core) fn from_parts(item: I, id: FactId) -> Self {
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
