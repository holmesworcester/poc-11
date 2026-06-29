//! Link codec, extraction, and projection. These belong together because versioned
//! byte interpretation is part of the meaning that extraction/projection prove.
//!
//! A link carries an optional `prev` -> a chain link0 <- link1 <- .... `extract`
//! emits an OFFER on the link's own id and, if it has a parent, a NEED on `prev`.
//! `project` makes a link valid iff its parent is valid; a root is valid by itself.
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
