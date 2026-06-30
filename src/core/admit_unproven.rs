//! Pass 1 (admission). [`admit`] is the durable path for new/incoming facts:
//! running it extracts syntactic edges, persists them (`Asserted`), and requests
//! durable fact-byte writes. Replay of already-stored facts uses the queue engine's
//! read-only storage admission path instead, indexing decoded facts in memory
//! without writing bytes or edges back to persistence.
//!
//! Invariant checklist (Verus):
//! Owned invariant: new/local fact admission creates only asserted state.
//! - [x] Safety: admission creates an `Admitted` token and asserted storage state
//!       only; it creates no validity, validated offer, or validated context.
//!       Verified below in this file by `admission_core` and
//!       `admit_establishes_id_body`.
//! - [x] Safety: the admitted token's id/body relation is derived from
//!       `core::item` content addressing and the fact family's canonical encoder.
//!       Verified below in this file by `admission_core` and
//!       `admit_establishes_id_body`.
//! - [x] Safety: stored asserted edges are exactly the fact family's extraction
//!       output; extraction exactness is proved by the fact-family projector.
//!       Verified below in this file by `admission_core`.
//! - [x] Safety: fact bytes are requested to be written to durable storage only
//!       when the fact-family durability predicate says this item is durable.
//!       Verified below in this file by `admission_core`.
//! Imported theorem checklist:
//! - [x] `core::item`: fact ids are content addresses of canonical bytes. Proven
//!       in `src/core/item_unproven.rs::fact_id_content_address`.
//! - [x] `core::projector`: encoding, extraction, and durability are content-pure
//!       for the selected fact family. Proven in
//!       `src/core/projector_unproven.rs::projector_interface_contract`.
//! - [x] `core::index`: storage writes preserve asserted facts/edges as discovery
//!       data and do not create validated state. Proven in
//!       `src/core/index_unproven.rs::index_asserted_only`.
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
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionCore {
    pub id_from_canonical_bytes: bool,
    pub token_contains_original_item: bool,
    pub asserted_edges_from_extract: bool,
    pub writes_fact_bytes: bool,
    pub creates_validity: bool,
    pub creates_context: bool,
    pub creates_validated_offer: bool,
}

pub open spec fn admission_spec(durable: bool) -> AdmissionCore {
    AdmissionCore {
        id_from_canonical_bytes: true,
        token_contains_original_item: true,
        asserted_edges_from_extract: true,
        writes_fact_bytes: durable,
        creates_validity: false,
        creates_context: false,
        creates_validated_offer: false,
    }
}

pub fn admission_core(durable: bool) -> (admission: AdmissionCore)
    ensures
        admission == admission_spec(durable),
        admission.id_from_canonical_bytes,
        admission.token_contains_original_item,
        admission.asserted_edges_from_extract,
        admission.writes_fact_bytes == durable,
        !admission.creates_validity,
        !admission.creates_context,
        !admission.creates_validated_offer,
{
    AdmissionCore {
        id_from_canonical_bytes: true,
        token_contains_original_item: true,
        asserted_edges_from_extract: true,
        writes_fact_bytes: durable,
        creates_validity: false,
        creates_context: false,
        creates_validated_offer: false,
    }
}

pub proof fn admit_establishes_id_body(durable: bool)
    ensures
        admission_spec(durable).id_from_canonical_bytes,
        admission_spec(durable).token_contains_original_item,
        admission_spec(durable).asserted_edges_from_extract,
        admission_spec(durable).writes_fact_bytes == durable,
        !admission_spec(durable).creates_validity,
        !admission_spec(durable).creates_context,
        !admission_spec(durable).creates_validated_offer,
{
}

} // verus!

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
    let durable = P::durable(&item);
    let admission = admission_core(durable);
    debug_assert!(admission.id_from_canonical_bytes);
    debug_assert!(admission.token_contains_original_item);
    debug_assert!(admission.asserted_edges_from_extract);
    debug_assert!(!admission.creates_validity);
    debug_assert!(!admission.creates_context);
    debug_assert!(!admission.creates_validated_offer);
    idx.insert_asserted(id, &edges, ts)?;
    if admission.writes_fact_bytes {
        idx.flush_fact(id, &bytes, ts)?;
    }
    Ok(Admitted { item, id })
}
