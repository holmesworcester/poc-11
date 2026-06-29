//! Link authoring adapter. The command kernel is a future proof target; this file
//! remains unproven while it performs storage admission directly.
//!
//! Invariant checklist (Verus):
//! - [ ] Authoring copies the requested `prev` parameter exactly into the link:
//!       `None` for roots, `Some(parent_id)` for children.
//! - [ ] After the root/domain model lands, authoring copies the requested
//!       root/domain id parameter exactly into child links and omits it for roots.
//! - [ ] Authored content is deterministic from command inputs that should affect
//!       the fact id.
//! - [ ] The authored link is admitted only through core admission, so persisted
//!       bytes and asserted edges match link extraction.
//! - [ ] Authoring return values such as depth/root are report data only; they do
//!       not establish validity.
use crate::core::admit::admit;
use crate::core::index::Index;
use crate::core::item::FactId;

use super::api_unproven::walk;
use super::project_unproven::{Link, LinkProjector};

pub struct Authored {
    pub id: FactId,
    pub depth: u64,
    pub root: FactId,
}

/// Author + admit a link. `content` carries `at` (+ optional label) so distinct
/// authorings get distinct ids, even with no prev (independents must not collide).
pub fn author(
    idx: &dyn Index,
    at: u64,
    prev: Option<FactId>,
    label: &str,
) -> Result<Authored, String> {
    let mut content = at.to_le_bytes().to_vec();
    content.extend_from_slice(label.as_bytes());
    let id = admit::<LinkProjector>(Link { content, prev }, at, idx)?.id();
    let w = walk(idx, id)?;
    Ok(Authored {
        id,
        depth: w.depth,
        root: w.root,
    })
}
