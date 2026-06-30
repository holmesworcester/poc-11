//! Pass 1 (admission). [`admit`] is the durable path for new/incoming facts:
//! running it extracts syntactic edges, persists them (`Asserted`), and requests
//! durable fact-byte writes. Replay of already-stored facts uses the queue engine's
//! read-only storage admission path instead, indexing decoded facts in memory
//! without writing bytes or edges back to persistence.
//!
//! Invariant checklist (Verus):
//! Owned invariant: new/local fact admission creates only asserted state.
//! - [ ] Safety: admission creates an `Admitted` token and asserted storage state
//!       only; it creates no validity, validated offer, or validated context.
//! - [ ] Safety: the admitted token's id/body relation is derived from
//!       `core::item` content addressing and the fact family's canonical encoder.
//! - [ ] Safety: stored asserted edges are exactly the fact family's extraction
//!       output; extraction exactness is proved by the fact-family projector.
//! - [ ] Safety: fact bytes are requested to be written to durable storage only
//!       when the fact-family durability predicate says this item is durable.
//! Imported theorem checklist:
//! - [x] `core::item`: fact ids are content addresses of canonical bytes. Proven
//!       in `src/core/item_unproven.rs::fact_id_content_address`.
//! - [ ] `core::projector`: encoding, extraction, and durability are content-pure
//!       for the selected fact family. Owner: `src/core/projector_unproven.rs`,
//!       planned theorem `projector_interface_contract`.
//! - [ ] `core::index`: storage writes preserve asserted facts/edges as discovery
//!       data and do not create validated state. Owner:
//!       `src/core/index_unproven.rs`, planned theorem `index_asserted_only`.
//! Proof strategy:
//! - Symbolically execute `admit`: compute bytes, compute id from bytes, compute
//!   asserted edges from the item, request asserted-edge persistence, and request
//!   a durable byte write only under `P::durable`.
//! - Prove the returned token contains exactly the original item and computed id.
//! - Prove by type inspection that this function constructs no `Validity`,
//!   `Context`, or `Offer<Validated>`.
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
    pub(in crate::core) fn from_engine_memory(item: I, id: FactId) -> Self {
        Self { item, id }
    }

    pub fn item(&self) -> &I {
        &self.item
    }
    pub fn id(&self) -> FactId {
        self.id
    }
}

/// Admit one new/incoming item: extract → persist edges (Asserted) → request a
/// durable byte write if durable. Idempotent writes make repeated network/local
/// admission safe, but replay does not call this for facts already loaded from
/// storage.
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
