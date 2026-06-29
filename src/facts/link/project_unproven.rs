//! Link codec, extraction, and projection. These belong together because versioned
//! byte interpretation is part of the meaning that extraction/projection prove.
//!
//! A link carries an optional `prev` and an optional root/domain id. Roots encode
//! `prev=None, root=None` and use their own fact id as semantic root. Children
//! encode `prev=Some(parent_id), root=Some(anchor_id)`. `extract` emits an OFFER
//! for `valid_link(self_id, root_id)` and, for a child, a NEED for
//! `valid_link(parent_id, root_id)`. `project` makes a child valid only when that
//! same-root parent statement is in validated context; a root is valid by itself.
//!
//! Fact-family contract (do not weaken):
//! - Scope: the only home for link semantics.
//! - Owned here: `Link`, `LinkState`, `LinkProjector`, link codec, link
//!   deterministic constructors, extraction, projection, root/domain
//!   interpretation, and link-specific theorems.
//! - Allowed dependency: core supplies `Admitted`, asserted/validated edge types,
//!   `Context`, and `Validity`; core proves validated-context provenance.
//! - Forbidden here: durable storage access, CLI/report formatting, network IO,
//!   SQLite, and command admission policy.
//! - The root/domain field is interpreted here. App admission may only pass
//!   explicit parameters into `link_from_params`.
//!
//! Invariant checklist (Verus):
//! Owned invariant: link-family semantics and its `Projector` implementation.
//! - [ ] Safety: canonical link identity: accepted link bytes decode to exactly
//!       one semantic `Link`, re-encode to the same bytes, and derive the link id
//!       from those bytes.
//! - [ ] Safety: project-owned construction: command parameters determine only
//!       link content, `prev`, and claimed root/domain; app code cannot assign
//!       ids, edges, or validity.
//! - [ ] Safety: parent naming: for any child `link.prev == Some(parent_id)`,
//!       extraction asserts exactly one need for `parent_id`; no other field or
//!       app input can choose the parent dependency.
//! - [ ] Safety: starter validity rule: a root (`prev=None`) is valid; a child is
//!       valid exactly when validated context contains the offer whose owner and
//!       key are the child's declared `parent_id`.
//! - [ ] Safety: same-root/domain preservation: a child is valid only when its
//!       claimed root/domain matches the validated parent statement it depends on,
//!       and the child's promoted self-offer carries that same root/domain.
//! - [ ] Safety: no state authority leak: starter projection records only this
//!       link's validity and emits no new facts.
//! - [ ] Safety: composition with core: using `core::engine` validated-context
//!       provenance, every valid child link has a valid same-root parent chain to
//!       an anchor; no theorem here claims anchor uniqueness.
//! Imported theorems:
//! - `core::item`: fact ids are content addresses for canonical bytes.
//! - `core::offer`: asserted edge constructors and match addresses have fixed
//!   meaning.
//! - `core::typestate`: `Context::has_offer` is exact validated-offer lookup.
//! - `core::engine`: context offers have valid owners.
//! - `core::projector`: the generic projector interface enforces confinement.
//! Proof strategy:
//! - Prove codec round trips and rejection cases for the current
//!   tag/prev/root/content layout.
//! - Prove `link_from_params` constructs only `content`, `prev`, and claimed
//!   root/domain, leaving id, edges, and validity to core/projector paths.
//! - Prove `extract` is exactly `link_edges`: roots offer
//!   `valid_link(self_id,self_id)`; children offer `valid_link(self_id,root_id)`
//!   and need `valid_link(prev,root_id)`.
//! - Prove `project` implements `link_project_validity`, writes only this link's
//!   validity into `LinkState`, and emits no facts.
//! - Compose with the engine provenance theorem to prove same-root parent-chain
//!   transitivity for the current root/domain model.
use std::collections::BTreeMap;

use crate::core::admit::Admitted;
use crate::core::item::{fact_id, FactId};
use crate::core::offer::{Key, Offer, Role};
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
    pub root: Option<FactId>,
}

/// The projector's private read-model: id → validity.
#[derive(Default)]
pub struct LinkState {
    pub seen: BTreeMap<FactId, Validity>,
}

pub struct LinkProjector;

/// Deterministic constructor from command parameters to the typed link fact.
pub fn link_from_params(at: u64, prev: Option<FactId>, root: Option<FactId>, label: &str) -> Link {
    let mut content = at.to_le_bytes().to_vec();
    content.extend_from_slice(label.as_bytes());
    Link {
        content,
        prev,
        root,
    }
}

pub fn link_id(l: &Link) -> FactId {
    fact_id(&LinkProjector::encode(l))
}

pub fn link_edges(l: &Link) -> Vec<Offer<Asserted>> {
    let Some(root) = link_semantic_root(l) else {
        return vec![];
    };
    let id = link_id(l);
    let mut edges = vec![Offer::offer(LINK, valid_link_key(id, root))];
    if let Some(parent) = l.prev {
        edges.push(Offer::need(LINK, valid_link_key(parent, root)));
    }
    edges
}

pub fn link_semantic_root(l: &Link) -> Option<FactId> {
    match (l.prev, l.root) {
        (None, None) => Some(link_id(l)),
        (Some(_), Some(root)) => Some(root),
        _ => None,
    }
}

pub fn valid_link_key(link_id: FactId, root_id: FactId) -> Key {
    let mut bytes = Vec::with_capacity(16 + 64);
    bytes.extend_from_slice(b"link.valid.v1\0");
    bytes.extend_from_slice(&link_id);
    bytes.extend_from_slice(&root_id);
    Key(fact_id(&bytes))
}

pub fn link_project_validity(l: &Link, parent_validated_same_root: bool) -> Validity {
    match (l.prev, l.root) {
        (None, None) => Validity::Valid,
        (Some(_), Some(_)) if parent_validated_same_root => Validity::Valid,
        _ => Validity::Invalid,
    }
}

impl Projector for LinkProjector {
    type Item = Link;
    type State = LinkState;

    // Canonical layout: tag | has_prev | prev[32]? | has_root | root[32]? | content.
    fn encode(l: &Link) -> Vec<u8> {
        let mut b = Vec::with_capacity(3 + 64 + l.content.len());
        b.push(TAG_LINK);
        match &l.prev {
            Some(p) => {
                b.push(1);
                b.extend_from_slice(p);
            }
            None => b.push(0),
        }
        match &l.root {
            Some(root) => {
                b.push(1);
                b.extend_from_slice(root);
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
        let (prev, offset) = match b.get(1) {
            Some(0) => (None, 2),
            Some(1) => {
                let p: FactId = b
                    .get(2..34)
                    .ok_or("truncated prev")?
                    .try_into()
                    .map_err(|_| "bad prev".to_string())?;
                (Some(p), 34)
            }
            _ => return Err("bad has_prev byte".to_string()),
        };
        let (root, content_offset) = match b.get(offset) {
            Some(0) => (None, offset + 1),
            Some(1) => {
                let root: FactId = b
                    .get(offset + 1..offset + 33)
                    .ok_or("truncated root")?
                    .try_into()
                    .map_err(|_| "bad root".to_string())?;
                (Some(root), offset + 33)
            }
            _ => return Err("bad has_root byte".to_string()),
        };
        Ok(Link {
            prev,
            root,
            content: b[content_offset..].to_vec(),
        })
    }

    fn extract(l: &Link) -> Vec<Offer<Asserted>> {
        link_edges(l)
    }

    fn project(item: &Admitted<Link>, ctx: Context, st: &mut LinkState) -> ProjectOutcome {
        let parent_validated_same_root = match (item.item().prev, item.item().root) {
            (Some(parent), Some(root)) => ctx.has_offer(LINK, &valid_link_key(parent, root)),
            _ => false,
        };
        let validity = link_project_validity(item.item(), parent_validated_same_root);
        st.seen.insert(item.id(), validity);
        ProjectOutcome {
            validity,
            emitted: vec![],
        }
    }
}
