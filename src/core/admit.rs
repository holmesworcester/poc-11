//! Pass 1 (admission). [`admit`] is the ONLY way to mint an [`Admitted<I>`]:
//! running it extracts the syntactic edges, persists them (`Asserted`), and
//! flushes durable bytes. Because `project`/`play` require an `Admitted`, Pass 2
//! is unreachable without Pass 1 — extract-before-project is a compile error.
use super::index::Index;
use super::item::{fact_id, FactId};
use super::projector::Projector;

/// A Pass-1 token. The fields are private, so no projector or emitted-fact path
/// can fabricate one outside this module.
pub struct Admitted<I> {
    item: I,
    id: FactId,
}

impl<I> Admitted<I> {
    pub fn item(&self) -> &I {
        &self.item
    }
    pub fn id(&self) -> FactId {
        self.id
    }
}

/// Admit one item: extract → persist edges (Asserted) → flush bytes if durable.
/// Idempotent (the index inserts are `INSERT OR IGNORE`), so re-admitting a stored
/// fact to obtain its token during `play` is safe.
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
