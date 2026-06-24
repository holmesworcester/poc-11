//! The `link` item family + its projector, plus the family's CLI helpers. A link
//! carries an optional `prev` → a chain link0 <- link1 <- .... `extract` emits an
//! OFFER on the link's own id (derived from content) and, if it has a parent, a
//! NEED on `prev`. `project` makes a link valid iff its parent is valid (a root is
//! valid by itself). The chain's transitive validity is the Stage-1 Verus target.
use std::collections::BTreeMap;

use crate::core::admit::{admit, Admitted};
use crate::core::index::{Index, SqliteIndex};
use crate::core::item::{fact_id, FactId};
use crate::core::offer::{Key, Offer, Role};
use crate::core::play::replay;
use crate::core::projector::{ProjectOutcome, Projector};
use crate::core::typestate::{Asserted, Context, Validity};

/// Wire tag distinguishing a link fact from other frames on the network.
pub const TAG_LINK: u8 = 0x01;
/// The single match namespace.
pub const LINK: Role = Role("link");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub content: Vec<u8>,
    pub prev: Option<FactId>,
}

/// The projector's private read-model: id → validity.
#[derive(Default)]
pub struct LinkState {
    pub seen: BTreeMap<FactId, Validity>,
}

pub struct LinkProjector;

impl Projector for LinkProjector {
    type Item = Link;
    type State = LinkState;

    // Canonical layout: tag | has_prev | prev[32]? | content.
    fn encode(l: &Link) -> Vec<u8> {
        let mut b = Vec::with_capacity(2 + 32 + l.content.len());
        b.push(TAG_LINK);
        match &l.prev {
            Some(p) => {
                b.push(1);
                b.extend_from_slice(p);
            }
            None => b.push(0),
        }
        b.extend_from_slice(&l.content);
        b
    }

    fn decode(b: &[u8]) -> Result<Link, String> {
        if b.first() != Some(&TAG_LINK) {
            return Err("not a link fact".to_string());
        }
        match b.get(1) {
            Some(0) => Ok(Link {
                prev: None,
                content: b[2..].to_vec(),
            }),
            Some(1) => {
                let p: FactId = b
                    .get(2..34)
                    .ok_or("truncated prev")?
                    .try_into()
                    .map_err(|_| "bad prev".to_string())?;
                Ok(Link {
                    prev: Some(p),
                    content: b[34..].to_vec(),
                })
            }
            _ => Err("bad has_prev byte".to_string()),
        }
    }

    fn extract(l: &Link) -> Vec<Offer<Asserted>> {
        // The self-id is a pure function of content (the closure rule, §5).
        let id = fact_id(&Self::encode(l));
        let mut edges = vec![Offer::offer(LINK, Key(id))];
        if let Some(p) = l.prev {
            edges.push(Offer::need(LINK, Key(p)));
        }
        edges
    }

    fn project(item: &Admitted<Link>, ctx: Context, st: &mut LinkState) -> ProjectOutcome {
        let validity = match item.item().prev {
            None => Validity::Valid,
            Some(p) => {
                if ctx.has_offer(LINK, &Key(p)) {
                    Validity::Valid
                } else {
                    Validity::Invalid
                }
            }
        };
        st.seen.insert(item.id(), validity);
        ProjectOutcome {
            validity,
            emitted: vec![],
        }
    }
}

// ---- family CLI helpers (used by the app layer `crate::cli`) ----

pub struct Authored {
    pub id: FactId,
    pub depth: u64,
    pub root: FactId,
}

/// Author + admit a link. `content` carries `at` (+ optional label) so distinct
/// authorings get distinct ids, even with no prev (independents must not collide).
pub fn author(
    idx: &SqliteIndex,
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

/// A validated view of the chain ending at `head`.
pub struct Report {
    pub present: bool,
    pub complete: bool,
    pub root: FactId,
    pub depth: u64,
    pub length: u64,
    /// root..head order (the present contiguous run from head).
    pub ids: Vec<FactId>,
}

pub fn chain_report(idx: &SqliteIndex, head: FactId) -> Result<Report, String> {
    let w = walk(idx, head)?;
    let complete = if w.present {
        let memo = replay::<LinkProjector>(idx, &[head])?;
        w.reached_root && matches!(memo.get(&head), Some(Validity::Valid))
    } else {
        false
    };
    Ok(Report {
        present: w.present,
        complete,
        root: w.root,
        depth: w.depth,
        length: w.length,
        ids: w.ids,
    })
}

struct Walk {
    present: bool,
    root: FactId,
    depth: u64,
    length: u64,
    reached_root: bool,
    ids: Vec<FactId>,
}

/// Read-only walk of the `prev` chain from `head` down. Pure read (no writes).
fn walk(idx: &SqliteIndex, head: FactId) -> Result<Walk, String> {
    let mut ids = vec![];
    let mut cur = Some(head);
    let mut present = false;
    let mut reached_root = false;
    while let Some(id) = cur {
        let Some(bytes) = idx.load_fact(&id)? else {
            break;
        };
        if id == head {
            present = true;
        }
        ids.push(id);
        cur = match LinkProjector::decode(&bytes)?.prev {
            None => {
                reached_root = true;
                None
            }
            Some(p) => Some(p),
        };
    }
    let (root, depth, length) = if ids.is_empty() {
        (head, 0, 0)
    } else {
        (*ids.last().unwrap(), ids.len() as u64 - 1, ids.len() as u64)
    };
    ids.reverse();
    Ok(Walk {
        present,
        root,
        depth,
        length,
        reached_root,
        ids,
    })
}
