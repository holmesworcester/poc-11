//! Link codec, extraction, and projection. These belong together because versioned
//! byte interpretation is part of the meaning that extraction/projection prove.
//!
//! A link carries an optional `prev` -> a chain link0 <- link1 <- .... `extract`
//! emits an OFFER on the link's own id and, if it has a parent, a NEED on `prev`.
//! `project` makes a link valid iff its parent is valid; a root is valid by itself.
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
//! - Any future root/domain field must be interpreted here. App admission may
//!   only pass explicit parameters into `link_from_params`.
//!
//! Invariant checklist (Verus):
//! Owned invariant: link-family semantics and its `Projector` implementation.
//! - [ ] Canonical link identity: accepted link bytes decode to exactly one
//!       semantic `Link`, re-encode to the same bytes, and derive the link id from
//!       those bytes.
//! - [ ] Project-owned construction: command parameters determine only link
//!       content and `prev`; app code cannot assign ids, edges, roots, or validity.
//! - [ ] Extraction honesty: a link asserts exactly its self-offer and, for a
//!       child, exactly the need for its declared parent.
//! - [ ] Starter validity rule: a root (`prev=None`) is valid; a child is valid
//!       exactly when validated context contains its parent offer.
//! - [ ] No state authority leak: starter projection records only this link's
//!       validity and emits no new facts.
//! - [ ] Composition with core: using `core::engine` validated-context
//!       provenance, every valid child link has a valid parent chain to an
//!       anchor; no theorem here claims anchor uniqueness.
//! Imported theorems:
//! - `core::item`: fact ids are content addresses for canonical bytes.
//! - `core::offer`: asserted edge constructors and match addresses have fixed
//!   meaning.
//! - `core::typestate`: `Context::has_offer` is exact validated-offer lookup.
//! - `core::engine`: context offers have valid owners.
//! - `core::projector`: the generic projector interface enforces confinement.
//! Proof strategy:
//! - Prove codec round trips and rejection cases for the current tag/prev/content
//!   layout.
//! - Prove `link_from_params` constructs only `content` and `prev`, leaving id,
//!   edges, and validity to core/projector paths.
//! - Prove `extract` is exactly `link_edges`: one self-offer plus one parent need
//!   iff `prev` is present.
//! - Prove `project` implements `link_project_validity`, writes only this link's
//!   validity into `LinkState`, and emits no facts.
//! - Compose with the engine provenance theorem to prove parent-chain
//!   transitivity for the current prev-only model. Root/domain preservation is a
//!   future proof after the link shape carries a root/domain id.
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
}

/// The projector's private read-model: id → validity.
#[derive(Default)]
pub struct LinkState {
    pub seen: BTreeMap<FactId, Validity>,
}

pub struct LinkProjector;

/// Deterministic constructor from command parameters to the typed link fact.
pub fn link_from_params(at: u64, prev: Option<FactId>, label: &str) -> Link {
    let mut content = at.to_le_bytes().to_vec();
    content.extend_from_slice(label.as_bytes());
    Link { content, prev }
}

pub fn link_id(l: &Link) -> FactId {
    fact_id(&LinkProjector::encode(l))
}

pub fn link_edges(l: &Link) -> Vec<Offer<Asserted>> {
    let mut edges = vec![Offer::offer(LINK, Key(link_id(l)))];
    if let Some(p) = l.prev {
        edges.push(Offer::need(LINK, Key(p)));
    }
    edges
}

pub fn link_project_validity(prev: Option<FactId>, parent_validated: bool) -> Validity {
    match prev {
        None => Validity::Valid,
        Some(_) if parent_validated => Validity::Valid,
        Some(_) => Validity::Invalid,
    }
}

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
        link_edges(l)
    }

    fn project(item: &Admitted<Link>, ctx: Context, st: &mut LinkState) -> ProjectOutcome {
        let parent_validated = item
            .item()
            .prev
            .is_some_and(|p| ctx.has_offer(LINK, &Key(p)));
        let validity = link_project_validity(item.item().prev, parent_validated);
        st.seen.insert(item.id(), validity);
        ProjectOutcome {
            validity,
            emitted: vec![],
        }
    }
}
