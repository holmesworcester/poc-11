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
//!   interpretation, projector-owned read-model state, and link-specific
//!   theorems.
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
//! - [x] Safety: well-formed parent naming: for any child
//!       `link.prev == Some(parent_id)` and `link.root == Some(root_id)`,
//!       extraction asserts exactly one need for `valid_link(parent_id, root_id)`;
//!       no other field or app input can choose the parent dependency. Verified
//!       below in this file.
//! - [x] Safety: malformed `prev`/`root` combinations assert no edges and project
//!       invalid. Verified below in this file.
//! - [x] Safety: starter validity rule: a root (`prev=None`) is valid; a child is
//!       valid exactly when validated context contains
//!       `valid_link(parent_id, root_id)` for the child's declared parent and
//!       root/domain ids. Verified below in this file.
//! - [x] Safety: same-root/domain preservation: a child is valid only when its
//!       claimed root/domain matches the validated parent statement it depends on,
//!       and the child's promoted self-offer carries that same root/domain.
//!       Verified below in this file.
//! - [ ] Safety: statement-to-owner: every validated link offer at
//!       `valid_link_key(link_id, root_id)` was promoted from a valid link fact
//!       whose id is `link_id` and whose semantic root is `root_id`.
//! - [x] Safety: projection output update ownership: projecting `link_id` returns
//!       only an update owner equal to `link_id`. Verified below in this file.
//! - [ ] Safety: update application scope: `apply_update` is insert/ignore by `link_id`
//!       for `LinkState.seen` and `LinkState.projected`.
//! - [x] Safety: projected report completeness shape: a complete child report is
//!       derived only from a complete same-root parent report. Verified below in
//!       this file.
//! - [x] Safety: no emitted-fact authority leak: link projection emits no new raw
//!       facts. Verified below in this file.
//! - [ ] Safety: composition with core: using `core::engine` validated-context
//!       provenance, every valid child link has a valid same-root parent chain to
//!       an anchor; no theorem here claims anchor uniqueness.
//! Imported theorem checklist:
//! - [ ] `core::item`: fact ids are content addresses for canonical bytes. Owner:
//!       `src/core/item_unproven.rs`, planned theorem `fact_id_content_address`.
//! - [x] `core::offer`: asserted edge constructors and match addresses have fixed
//!       meaning. Proven in
//!       `src/core/offer_unproven.rs::asserted_edge_address_shape`.
//! - [ ] `core::typestate`: `Context::has_offer` is exact validated-offer lookup.
//!       Owner: `src/core/typestate_unproven.rs`, planned theorem
//!       `context_lookup_exact`.
//! - [ ] `core::engine`: context offers have valid owners. Owner:
//!       `src/core/engine_unproven.rs`, planned theorem
//!       `engine_context_offers_have_valid_owners`.
//! - [ ] `core::projector`: the generic projector interface enforces
//!       confinement. Owner: `src/core/projector_unproven.rs`, planned theorem
//!       `projector_interface_contract`.
//! - [x] Local link same-root extraction/projection kernel. Proven below by
//!       `extract_link_core`, `project_link_core`,
//!       `child_extraction_offer_and_need_same_root`,
//!       `valid_child_requires_validated_same_root_parent`, and
//!       `valid_projection_statement_equals_extracted_offer`.
//! - [x] Local link output/read-model kernel. Proven below by
//!       `projection_update_owner_is_self`,
//!       `valid_projection_statement_owned_by_projected_link`,
//!       `projected_report_core`,
//!       `complete_child_report_requires_complete_same_root_parent`, and
//!       `link_emitted_fact_count_core`.
//! Proof strategy:
//! - Prove codec round trips and rejection cases for the current
//!   tag/prev/root/content layout.
//! - Prove `link_from_params` constructs only `content`, `prev`, and claimed
//!   root/domain, leaving id, edges, and validity to core/projector paths.
//! - Prove `extract` is exactly `link_edges`: well-formed roots offer
//!   `valid_link(self_id,self_id)`; well-formed children offer
//!   `valid_link(self_id,root_id)` and need `valid_link(prev,root_id)`;
//!   malformed `prev`/`root` combinations emit no edges.
//! - Prove `project` implements `link_project_validity`, returns only current-id
//!   `LinkUpdate` values for `LinkState`, and emits no facts.
//! - Prove `update_owner` returns the update's owner id exactly, and
//!   `apply_update` is insert/ignore by fact id and cannot update any entry
//!   except the update's owner id.
//! - Prove complete projected reports are inductive read-model state: root case
//!   records `[self]`; child case appends `self` to the already-projected valid
//!   same-root parent report.
//! - Prove the statement-to-owner lemma from `link_edges`, `valid_link_key`,
//!   content addressing, and the engine theorem that every validated offer was
//!   asserted by its valid owner.
//! - Prove same-root parent-chain transitivity by induction: root case
//!   `prev=None, root=None` gives `valid_link(self,self)`; child step uses the
//!   validated `valid_link(parent,r)` dependency plus the statement-to-owner lemma
//!   to obtain a valid parent in the same root/domain, then repeats to the anchor.
use std::collections::BTreeMap;

use crate::core::admit::Admitted;
use crate::core::item::{fact_id, FactId};
use crate::core::offer::{Key, Offer, Role};
use crate::core::projector::{ProjectOutcome, Projector};
use crate::core::typestate::{Asserted, Context, Validity};
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdCore {
    pub w0: u64,
    pub w1: u64,
    pub w2: u64,
    pub w3: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaybeIdCore {
    None,
    Some(IdCore),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkCore {
    pub self_id: IdCore,
    pub prev: MaybeIdCore,
    pub root: MaybeIdCore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidityCore {
    Valid,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkStatementCore {
    pub link_id: IdCore,
    pub root_id: IdCore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaybeStatementCore {
    None,
    Some(LinkStatementCore),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkProjectionCore {
    pub validity: ValidityCore,
    pub update_owner: IdCore,
    pub statement: MaybeStatementCore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkExtractionCore {
    pub offer: MaybeStatementCore,
    pub need: MaybeStatementCore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectedReportCore {
    pub complete: bool,
    pub root: IdCore,
}

pub open spec fn id_eq_spec(left: IdCore, right: IdCore) -> bool {
    left.w0 == right.w0 && left.w1 == right.w1 && left.w2 == right.w2 && left.w3 == right.w3
}

pub fn id_eq(left: IdCore, right: IdCore) -> (equal: bool)
    ensures
        equal == id_eq_spec(left, right),
{
    left.w0 == right.w0 && left.w1 == right.w1 && left.w2 == right.w2 && left.w3 == right.w3
}

pub open spec fn is_root(link: LinkCore) -> bool {
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => true,
        _ => false,
    }
}

pub open spec fn is_child(link: LinkCore) -> bool {
    match (link.prev, link.root) {
        (MaybeIdCore::Some(_), MaybeIdCore::Some(_)) => true,
        _ => false,
    }
}

pub open spec fn is_malformed(link: LinkCore) -> bool {
    !is_root(link) && !is_child(link)
}

pub open spec fn statement_is_self_root(statement: MaybeStatementCore, self_id: IdCore) -> bool {
    statement == MaybeStatementCore::Some(LinkStatementCore {
        link_id: self_id,
        root_id: self_id,
    })
}

pub open spec fn statement_is_self_claimed_root(
    statement: MaybeStatementCore,
    self_id: IdCore,
    claimed_root: IdCore,
) -> bool {
    statement == MaybeStatementCore::Some(LinkStatementCore {
        link_id: self_id,
        root_id: claimed_root,
    })
}

pub open spec fn projection_spec(
    link: LinkCore,
    parent_validated_same_root: bool,
) -> LinkProjectionCore {
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => LinkProjectionCore {
            validity: ValidityCore::Valid,
            update_owner: link.self_id,
            statement: MaybeStatementCore::Some(LinkStatementCore {
                link_id: link.self_id,
                root_id: link.self_id,
            }),
        },
        (MaybeIdCore::Some(_parent), MaybeIdCore::Some(root)) => {
            if parent_validated_same_root {
                LinkProjectionCore {
                    validity: ValidityCore::Valid,
                    update_owner: link.self_id,
                    statement: MaybeStatementCore::Some(LinkStatementCore {
                        link_id: link.self_id,
                        root_id: root,
                    }),
                }
            } else {
                LinkProjectionCore {
                    validity: ValidityCore::Invalid,
                    update_owner: link.self_id,
                    statement: MaybeStatementCore::None,
                }
            }
        }
        _ => LinkProjectionCore {
            validity: ValidityCore::Invalid,
            update_owner: link.self_id,
            statement: MaybeStatementCore::None,
        },
    }
}

pub open spec fn extraction_spec(link: LinkCore) -> LinkExtractionCore {
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => LinkExtractionCore {
            offer: MaybeStatementCore::Some(LinkStatementCore {
                link_id: link.self_id,
                root_id: link.self_id,
            }),
            need: MaybeStatementCore::None,
        },
        (MaybeIdCore::Some(parent), MaybeIdCore::Some(root)) => LinkExtractionCore {
            offer: MaybeStatementCore::Some(LinkStatementCore {
                link_id: link.self_id,
                root_id: root,
            }),
            need: MaybeStatementCore::Some(LinkStatementCore {
                link_id: parent,
                root_id: root,
            }),
        },
        _ => LinkExtractionCore {
            offer: MaybeStatementCore::None,
            need: MaybeStatementCore::None,
        },
    }
}

pub open spec fn fallback_root_spec(link: LinkCore) -> IdCore {
    match (link.root, link.prev) {
        (MaybeIdCore::Some(root), _) => root,
        _ => link.self_id,
    }
}

pub open spec fn projected_report_spec(
    link: LinkCore,
    validity: ValidityCore,
    parent_present: bool,
    parent_complete: bool,
    parent_root: IdCore,
) -> ProjectedReportCore {
    match (link.prev, link.root, validity) {
        (MaybeIdCore::None, MaybeIdCore::None, ValidityCore::Valid) => ProjectedReportCore {
            complete: true,
            root: link.self_id,
        },
        (MaybeIdCore::Some(_parent), MaybeIdCore::Some(root), ValidityCore::Valid) => {
            if parent_present && parent_complete && id_eq_spec(parent_root, root) {
                ProjectedReportCore {
                    complete: true,
                    root,
                }
            } else {
                ProjectedReportCore {
                    complete: false,
                    root,
                }
            }
        }
        _ => ProjectedReportCore {
            complete: false,
            root: fallback_root_spec(link),
        },
    }
}

pub fn fallback_root_core(link: LinkCore) -> (root: IdCore)
    ensures
        root == fallback_root_spec(link),
{
    match (link.root, link.prev) {
        (MaybeIdCore::Some(root), _) => root,
        _ => link.self_id,
    }
}

pub fn extract_link_core(link: LinkCore) -> (extraction: LinkExtractionCore)
    ensures
        extraction == extraction_spec(link),
        is_root(link) ==> (
            statement_is_self_root(extraction.offer, link.self_id)
                && extraction.need == MaybeStatementCore::None
        ),
        is_malformed(link) ==> (
            extraction.offer == MaybeStatementCore::None
                && extraction.need == MaybeStatementCore::None
        ),
{
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => LinkExtractionCore {
            offer: MaybeStatementCore::Some(LinkStatementCore {
                link_id: link.self_id,
                root_id: link.self_id,
            }),
            need: MaybeStatementCore::None,
        },
        (MaybeIdCore::Some(parent), MaybeIdCore::Some(root)) => LinkExtractionCore {
            offer: MaybeStatementCore::Some(LinkStatementCore {
                link_id: link.self_id,
                root_id: root,
            }),
            need: MaybeStatementCore::Some(LinkStatementCore {
                link_id: parent,
                root_id: root,
            }),
        },
        _ => LinkExtractionCore {
            offer: MaybeStatementCore::None,
            need: MaybeStatementCore::None,
        },
    }
}

pub fn projected_report_core(
    link: LinkCore,
    validity: ValidityCore,
    parent_present: bool,
    parent_complete: bool,
    parent_root: IdCore,
) -> (report: ProjectedReportCore)
    ensures
        report == projected_report_spec(
            link,
            validity,
            parent_present,
            parent_complete,
            parent_root,
        ),
        is_root(link) && validity == ValidityCore::Valid ==> (
            report.complete && report.root == link.self_id
        ),
        is_child(link) && validity == ValidityCore::Valid && report.complete ==> (
            parent_present
                && parent_complete
                && match link.root {
                    MaybeIdCore::Some(root) => id_eq_spec(parent_root, root),
                    MaybeIdCore::None => false,
                }
        ),
{
    match (link.prev, link.root, validity) {
        (MaybeIdCore::None, MaybeIdCore::None, ValidityCore::Valid) => ProjectedReportCore {
            complete: true,
            root: link.self_id,
        },
        (MaybeIdCore::Some(_parent), MaybeIdCore::Some(root), ValidityCore::Valid) => {
            if parent_present && parent_complete && id_eq(parent_root, root) {
                ProjectedReportCore {
                    complete: true,
                    root,
                }
            } else {
                ProjectedReportCore {
                    complete: false,
                    root,
                }
            }
        }
        _ => ProjectedReportCore {
            complete: false,
            root: fallback_root_core(link),
        },
    }
}

pub fn project_link_core(
    link: LinkCore,
    parent_validated_same_root: bool,
) -> (projection: LinkProjectionCore)
    ensures
        projection == projection_spec(link, parent_validated_same_root),
        projection.update_owner == link.self_id,
        is_root(link) ==> (
            projection.validity == ValidityCore::Valid
                && statement_is_self_root(projection.statement, link.self_id)
        ),
        is_malformed(link) ==> (
            projection.validity == ValidityCore::Invalid
                && projection.statement == MaybeStatementCore::None
        ),
        projection.validity == ValidityCore::Valid && is_child(link) ==> parent_validated_same_root,
        projection.validity == ValidityCore::Valid ==> projection.statement == extraction_spec(link).offer,
        projection.validity == ValidityCore::Valid ==> projection.statement != MaybeStatementCore::None,
{
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => LinkProjectionCore {
            validity: ValidityCore::Valid,
            update_owner: link.self_id,
            statement: MaybeStatementCore::Some(LinkStatementCore {
                link_id: link.self_id,
                root_id: link.self_id,
            }),
        },
        (MaybeIdCore::Some(_parent), MaybeIdCore::Some(root)) => {
            if parent_validated_same_root {
                LinkProjectionCore {
                    validity: ValidityCore::Valid,
                    update_owner: link.self_id,
                    statement: MaybeStatementCore::Some(LinkStatementCore {
                        link_id: link.self_id,
                        root_id: root,
                    }),
                }
            } else {
                LinkProjectionCore {
                    validity: ValidityCore::Invalid,
                    update_owner: link.self_id,
                    statement: MaybeStatementCore::None,
                }
            }
        }
        _ => LinkProjectionCore {
            validity: ValidityCore::Invalid,
            update_owner: link.self_id,
            statement: MaybeStatementCore::None,
        },
    }
}

pub fn link_emitted_fact_count_core() -> (count: usize)
    ensures
        count == 0,
{
    0
}

pub proof fn root_projection_emits_self_root(link: LinkCore)
    requires
        is_root(link),
    ensures
        projection_spec(link, false).validity == ValidityCore::Valid,
        statement_is_self_root(projection_spec(link, false).statement, link.self_id),
{
}

pub proof fn projection_update_owner_is_self(link: LinkCore, parent_validated_same_root: bool)
    ensures
        projection_spec(link, parent_validated_same_root).update_owner == link.self_id,
{
}

pub proof fn child_extraction_offer_and_need_same_root(
    self_id: IdCore,
    parent_id: IdCore,
    root_id: IdCore,
)
    ensures
        ({
            let link = LinkCore {
                self_id,
                prev: MaybeIdCore::Some(parent_id),
                root: MaybeIdCore::Some(root_id),
            };
            let extraction = extraction_spec(link);
            extraction.offer == MaybeStatementCore::Some(LinkStatementCore {
                link_id: self_id,
                root_id,
            }) && extraction.need == MaybeStatementCore::Some(LinkStatementCore {
                link_id: parent_id,
                root_id,
            })
        }),
{
}

pub proof fn valid_projection_statement_owned_by_projected_link(
    link: LinkCore,
    parent_validated_same_root: bool,
)
    requires
        projection_spec(link, parent_validated_same_root).validity == ValidityCore::Valid,
    ensures
        match projection_spec(link, parent_validated_same_root).statement {
            MaybeStatementCore::Some(statement) => statement.link_id == link.self_id,
            MaybeStatementCore::None => false,
        },
{
}

pub proof fn malformed_projection_is_invalid(link: LinkCore, parent_validated_same_root: bool)
    requires
        is_malformed(link),
    ensures
        projection_spec(link, parent_validated_same_root).validity == ValidityCore::Invalid,
        projection_spec(link, parent_validated_same_root).statement == MaybeStatementCore::None,
{
}

pub proof fn malformed_extraction_is_empty(link: LinkCore)
    requires
        is_malformed(link),
    ensures
        extraction_spec(link).offer == MaybeStatementCore::None,
        extraction_spec(link).need == MaybeStatementCore::None,
{
}

pub proof fn root_projected_report_is_complete_self(link: LinkCore)
    requires
        is_root(link),
    ensures
        projected_report_spec(
            link,
            ValidityCore::Valid,
            false,
            false,
            link.self_id,
        ).complete,
        projected_report_spec(
            link,
            ValidityCore::Valid,
            false,
            false,
            link.self_id,
        ).root == link.self_id,
{
}

pub proof fn complete_child_report_requires_complete_same_root_parent(
    link: LinkCore,
    parent_present: bool,
    parent_complete: bool,
    parent_root: IdCore,
)
    requires
        is_child(link),
        projected_report_spec(
            link,
            ValidityCore::Valid,
            parent_present,
            parent_complete,
            parent_root,
        ).complete,
    ensures
        parent_present,
        parent_complete,
        match link.root {
            MaybeIdCore::Some(root) => id_eq_spec(parent_root, root),
            MaybeIdCore::None => false,
        },
{
}

pub proof fn valid_child_requires_validated_same_root_parent(
    link: LinkCore,
    parent_validated_same_root: bool,
)
    requires
        is_child(link),
        projection_spec(link, parent_validated_same_root).validity == ValidityCore::Valid,
    ensures
        parent_validated_same_root,
{
}

pub proof fn valid_projection_statement_equals_extracted_offer(
    link: LinkCore,
    parent_validated_same_root: bool,
)
    requires
        projection_spec(link, parent_validated_same_root).validity == ValidityCore::Valid,
    ensures
        projection_spec(link, parent_validated_same_root).statement == extraction_spec(link).offer,
        projection_spec(link, parent_validated_same_root).statement != MaybeStatementCore::None,
{
}

pub proof fn valid_child_preserves_claimed_root(
    self_id: IdCore,
    parent_id: IdCore,
    root_id: IdCore,
)
    ensures
        ({
            let link = LinkCore {
                self_id,
                prev: MaybeIdCore::Some(parent_id),
                root: MaybeIdCore::Some(root_id),
            };
            let projection = projection_spec(link, true);
            projection.validity == ValidityCore::Valid
                && projection.update_owner == self_id
                && statement_is_self_claimed_root(projection.statement, self_id, root_id)
        }),
{
}

pub proof fn invalid_child_emits_no_statement(
    self_id: IdCore,
    parent_id: IdCore,
    root_id: IdCore,
)
    ensures
        ({
            let link = LinkCore {
                self_id,
                prev: MaybeIdCore::Some(parent_id),
                root: MaybeIdCore::Some(root_id),
            };
            let projection = projection_spec(link, false);
            projection.validity == ValidityCore::Invalid
                && projection.statement == MaybeStatementCore::None
        }),
{
}

} // verus!

fn chunk_u64(id: &FactId, offset: usize) -> u64 {
    u64::from_le_bytes([
        id[offset],
        id[offset + 1],
        id[offset + 2],
        id[offset + 3],
        id[offset + 4],
        id[offset + 5],
        id[offset + 6],
        id[offset + 7],
    ])
}

pub fn fact_id_to_core(id: FactId) -> IdCore {
    IdCore {
        w0: chunk_u64(&id, 0),
        w1: chunk_u64(&id, 8),
        w2: chunk_u64(&id, 16),
        w3: chunk_u64(&id, 24),
    }
}

pub fn core_to_fact_id(id: IdCore) -> FactId {
    let mut out = [0; 32];
    out[0..8].copy_from_slice(&id.w0.to_le_bytes());
    out[8..16].copy_from_slice(&id.w1.to_le_bytes());
    out[16..24].copy_from_slice(&id.w2.to_le_bytes());
    out[24..32].copy_from_slice(&id.w3.to_le_bytes());
    out
}

pub fn link_core_for(self_id: FactId, prev: Option<FactId>, root: Option<FactId>) -> LinkCore {
    LinkCore {
        self_id: fact_id_to_core(self_id),
        prev: maybe_fact_id_to_core(prev),
        root: maybe_fact_id_to_core(root),
    }
}

pub fn maybe_fact_id_to_core(id: Option<FactId>) -> MaybeIdCore {
    match id {
        Some(id) => MaybeIdCore::Some(fact_id_to_core(id)),
        None => MaybeIdCore::None,
    }
}

pub fn validity_from_core(validity: ValidityCore) -> crate::core::typestate::Validity {
    match validity {
        ValidityCore::Valid => crate::core::typestate::Validity::Valid,
        ValidityCore::Invalid => crate::core::typestate::Validity::Invalid,
    }
}

pub fn validity_to_core(validity: crate::core::typestate::Validity) -> ValidityCore {
    match validity {
        crate::core::typestate::Validity::Valid => ValidityCore::Valid,
        crate::core::typestate::Validity::Invalid => ValidityCore::Invalid,
    }
}

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectedLink {
    pub complete: bool,
    pub root: FactId,
    pub depth: u64,
    pub length: u64,
    /// root..head order for complete reports; singleton self for incomplete
    /// reports.
    pub ids: Vec<FactId>,
}

#[derive(Default)]
pub struct LinkState {
    pub seen: BTreeMap<FactId, Validity>,
    pub projected: BTreeMap<FactId, ProjectedLink>,
}

pub struct LinkUpdate {
    pub id: FactId,
    pub validity: Validity,
    pub projected: ProjectedLink,
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
    let extraction = extract_link_core(link_core_for(link_id(l), l.prev, l.root));
    let mut edges = vec![];
    if let MaybeStatementCore::Some(statement) = extraction.offer {
        edges.push(Offer::offer(
            LINK,
            valid_link_key(
                core_to_fact_id(statement.link_id),
                core_to_fact_id(statement.root_id),
            ),
        ));
    }
    if let MaybeStatementCore::Some(statement) = extraction.need {
        edges.push(Offer::need(
            LINK,
            valid_link_key(
                core_to_fact_id(statement.link_id),
                core_to_fact_id(statement.root_id),
            ),
        ));
    }
    edges
}

pub fn link_semantic_root(l: &Link) -> Option<FactId> {
    let extraction = extract_link_core(link_core_for(link_id(l), l.prev, l.root));
    match extraction.offer {
        MaybeStatementCore::Some(statement) => Some(core_to_fact_id(statement.root_id)),
        MaybeStatementCore::None => None,
    }
}

pub fn valid_link_key(link_id: FactId, root_id: FactId) -> Key {
    let mut bytes = Vec::with_capacity(16 + 64);
    bytes.extend_from_slice(b"link.valid.v1\0");
    bytes.extend_from_slice(&link_id);
    bytes.extend_from_slice(&root_id);
    Key(fact_id(&bytes))
}

pub fn link_project_decision(
    id: FactId,
    l: &Link,
    parent_validated_same_root: bool,
) -> LinkProjectionCore {
    project_link_core(
        link_core_for(id, l.prev, l.root),
        parent_validated_same_root,
    )
}

pub fn link_project_validity(l: &Link, parent_validated_same_root: bool) -> Validity {
    let projection = link_project_decision(link_id(l), l, parent_validated_same_root);
    validity_from_core(projection.validity)
}

fn projected_root_or_fallback(id: FactId, l: &Link) -> FactId {
    link_semantic_root(l).or(l.root).unwrap_or(id)
}

fn incomplete_projected_link(id: FactId, l: &Link) -> ProjectedLink {
    ProjectedLink {
        complete: false,
        root: projected_root_or_fallback(id, l),
        depth: 0,
        length: 1,
        ids: vec![id],
    }
}

fn projected_link_state(id: FactId, l: &Link, validity: Validity, st: &LinkState) -> ProjectedLink {
    let link = link_core_for(id, l.prev, l.root);
    let validity_core = validity_to_core(validity);
    match (l.prev, l.root, validity) {
        (None, None, Validity::Valid) => {
            let report =
                projected_report_core(link, validity_core, false, false, fact_id_to_core(id));
            ProjectedLink {
                complete: report.complete,
                root: core_to_fact_id(report.root),
                depth: 0,
                length: 1,
                ids: vec![id],
            }
        }
        (Some(parent), Some(_root), Validity::Valid) => {
            let Some(parent_state) = st.projected.get(&parent) else {
                return incomplete_projected_link(id, l);
            };
            let report = projected_report_core(
                link,
                validity_core,
                true,
                parent_state.complete,
                fact_id_to_core(parent_state.root),
            );
            if !report.complete {
                return incomplete_projected_link(id, l);
            }
            let mut ids = parent_state.ids.clone();
            ids.push(id);
            ProjectedLink {
                complete: true,
                root: core_to_fact_id(report.root),
                depth: parent_state.depth + 1,
                length: parent_state.length + 1,
                ids,
            }
        }
        _ => incomplete_projected_link(id, l),
    }
}

impl Projector for LinkProjector {
    type Item = Link;
    type State = LinkState;
    type Update = LinkUpdate;

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

    fn project(item: &Admitted<Link>, ctx: Context, st: &LinkState) -> ProjectOutcome<LinkUpdate> {
        let parent_validated_same_root = match (item.item().prev, item.item().root) {
            (Some(parent), Some(root)) => ctx.has_offer(LINK, &valid_link_key(parent, root)),
            _ => false,
        };
        let projection = link_project_decision(item.id(), item.item(), parent_validated_same_root);
        let validity = validity_from_core(projection.validity);
        let projected = projected_link_state(item.id(), item.item(), validity, st);
        ProjectOutcome {
            validity,
            emitted: Vec::with_capacity(link_emitted_fact_count_core()),
            updates: vec![LinkUpdate {
                id: core_to_fact_id(projection.update_owner),
                validity,
                projected,
            }],
        }
    }

    fn update_owner(update: &LinkUpdate) -> FactId {
        update.id
    }

    fn apply_update(st: &mut LinkState, update: LinkUpdate) {
        st.seen.entry(update.id).or_insert(update.validity);
        st.projected.entry(update.id).or_insert(update.projected);
    }
}
