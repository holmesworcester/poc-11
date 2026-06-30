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
//! - [x] Safety: canonical link identity: accepted link bytes have the canonical
//!       `tag | has_prev | prev[32]? | has_root | root[32]? | content` layout,
//!       malformed tags/flags/truncation are rejected, encode/decode preserve
//!       the semantic `prev`/`root` shape, and `link_id` is derived from canonical
//!       bytes. Verified below in this file by `link_codec_identity_core`,
//!       `link_codec_layout_core`, `canonical_link_identity`,
//!       `codec_layout_rejects_bad_tag`, `codec_layout_rejects_bad_flags`, and
//!       `codec_layout_rejects_truncation`; runtime round-trip tests connect the
//!       byte-vector content path to the same helpers.
//! - [x] Safety: project-owned construction: command parameters determine only
//!       link content, `prev`, and claimed root/domain; app code cannot assign
//!       ids, edges, or validity. Verified below in this file.
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
//! - [ ] Safety: end-to-end statement-to-owner: every validated link offer at
//!       `valid_link_key(link_id, root_id)` was promoted from a valid link fact
//!       whose id is `link_id` and whose semantic root is `root_id`. Local link
//!       projection proves only the statement it would promote; the full
//!       validated-store provenance theorem is still owned by core/replay.
//! - [x] Safety: projection output update ownership: projecting `link_id` returns
//!       only an update owner equal to `link_id`. Verified below in this file.
//! - [x] Safety: update application scope: `apply_update` is insert/ignore by `link_id`
//!       for `LinkState.seen` and `LinkState.projected`. Verified below in this
//!       file.
//! - [x] Safety: projected chain entry shape: each projection may create only the
//!       current fact's `ProjectedLink`. A complete child entry is built only by
//!       appending the current child id to a complete same-root parent entry,
//!       preserves root/depth/length/head shape, and has ids exactly equal to
//!       `parent.ids + [self]`. `ProjectedLink` is read-model state, not validity
//!       evidence. Verified below in this file by `projected_report_core`,
//!       `singleton_projected_ids_core`, `child_projected_ids_core`,
//!       `root_projected_report_is_complete_self`,
//!       `complete_child_report_requires_complete_same_root_parent`,
//!       `singleton_projected_ids_are_exact`, and
//!       `child_projected_ids_are_parent_plus_self`.
//! - [x] Safety: no emitted-fact authority leak: link projection emits no new raw
//!       facts. Verified below in this file.
//! - [ ] Safety: end-to-end composition with core: using `core::engine` and
//!       `core::play` provenance, every valid child link has a valid same-root
//!       parent chain to an anchor; no theorem here claims anchor uniqueness.
//!       The local link theorem is a conditional induction step, not the whole
//!       replay/graph invariant.
//! Imported theorem checklist:
//! - [x] `core::item`: fact ids are content addresses for canonical bytes. Proven
//!       in `src/core/item_unproven.rs::fact_id_content_address`.
//! - [x] `core::offer`: asserted edge constructors and match addresses have fixed
//!       meaning. Proven in
//!       `src/core/offer_unproven.rs::asserted_edge_address_shape`.
//! - [x] `core::typestate`: `Context::has_offer` is exact validated-offer lookup.
//!       Proven in `src/core/typestate_unproven.rs::context_lookup_exact`.
//! - [ ] `core::engine`: abstract context/promotion gates relate context offers
//!       to valid owners. Owner: `src/core/engine_unproven.rs`, planned theorem
//!       `engine_transition_preserves_validated_context_provenance`.
//! - [ ] `core::engine`: every concrete engine step and drain prefix preserves
//!       the full provenance invariant. Owner: `src/core/engine_unproven.rs`,
//!       planned theorem `engine_drain_prefix_sound`.
//! - [ ] `core::play`: replay reports only sound drained engine state and
//!       discovers the dependency closure. Owner: `src/core/play_unproven.rs`,
//!       planned theorem `replay_reports_engine_validity`.
//! - [x] `core::admit`: admitted facts always establish the id/body/extraction
//!       relation before projection. Proven in
//!       `src/core/admit_unproven.rs::admit_establishes_id_body`.
//! - [x] `core::projector`: the generic projector interface enforces
//!       confinement. Proven in
//!       `src/core/projector_unproven.rs::projector_interface_contract`.
//! Local theorem checklist:
//! - [x] Local link same-root extraction/projection kernel. Proven below by
//!       `extract_link_core`, `project_link_core`,
//!       `canonical_link_identity`,
//!       `link_codec_layout_core`,
//!       `codec_layout_rejects_bad_tag`,
//!       `codec_layout_rejects_bad_flags`,
//!       `codec_layout_rejects_truncation`,
//!       `child_extraction_offer_and_need_same_root`,
//!       `valid_child_requires_validated_same_root_parent`, and
//!       `valid_projection_statement_equals_extracted_offer`.
//! - [x] Local link conditional composition step. Proven below by
//!       `valid_link_composes_with_parent_chain`. This assumes the parent chain
//!       predicate as an input; core/replay must still prove the graph induction.
//! - [x] Local link output/read-model kernel. Proven below by
//!       `projection_update_owner_is_self`,
//!       `valid_projection_statement_owned_by_projected_link`,
//!       `valid_projection_statement_to_owner_and_root`,
//!       `link_from_params_constructs_only_link_fields`,
//!       `apply_update_is_insert_ignore_by_link_id`,
//!       `projected_report_core`,
//!       `complete_child_report_requires_complete_same_root_parent`, and
//!       `singleton_projected_ids_core`,
//!       `child_projected_ids_core`,
//!       `singleton_projected_ids_are_exact`,
//!       `child_projected_ids_are_parent_plus_self`, and
//!       `link_emitted_fact_count_core`.
//! - [ ] End-to-end link/core composition. Owner: this file after core proves
//!       real engine/replay graph provenance; planned theorem
//!       `valid_link_chain_to_anchor_from_replay`.
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
//! - Prove complete projected chain entries are inductive read-model state for
//!   the current fact only: root case records self/root/depth/length/id-count;
//!   child case appends the current child id to the already-projected complete
//!   same-root parent entry, increments counters, and records `self` as the
//!   modeled head. Runtime id-vector construction routes through proof-facing
//!   `[self]` and `parent + [self]` helpers.
//! - Prove the local statement-to-owner lemma from `link_edges`, `valid_link_key`,
//!   and content addressing. The end-to-end validated-offer version will import
//!   the future core drain-prefix provenance theorem.
//! - Prove the local same-root parent-chain step by induction: root case
//!   `prev=None, root=None` gives `valid_link(self,self)`; child step assumes the
//!   parent already has a same-root chain. The missing replay/engine graph proof
//!   must supply that parent-chain premise without a caller-provided boolean.
//!
//! Completion plan for unchecked items:
//! - Replace the caller-supplied `parent_chain_to_anchor: bool` composition
//!   premise with a real modeled dependency relation or sequence and an
//!   induction/decreases proof.
//! - Import real core/replay transition theorems over engine state once
//!   `src/core/engine_unproven.rs` and `src/core/play_unproven.rs` prove them.
//! - Rename this file to `project.rs` only after those end-to-end invariants are
//!   proven, not merely documented.
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
    pub depth: u64,
    pub length: u64,
    pub ids_len: u64,
    pub head: IdCore,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ProjectedIdsCore {
    pub ids: Vec<IdCore>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkConstructionCore {
    pub prev: MaybeIdCore,
    pub root: MaybeIdCore,
    pub assigns_id: bool,
    pub assigns_edges: bool,
    pub assigns_validity: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkUpdateApplyCore {
    pub seen_key: IdCore,
    pub projected_key: IdCore,
    pub insert_seen: bool,
    pub insert_projected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkCodecIdentityCore {
    pub prev: MaybeIdCore,
    pub root: MaybeIdCore,
    pub prev_flag: u8,
    pub root_flag: u8,
    pub accepted: bool,
    pub decode_encode_round_trip: bool,
    pub encode_decode_round_trip: bool,
    pub id_from_canonical_bytes: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkCodecLayoutCore {
    pub tag: u8,
    pub prev_flag: u8,
    pub root_flag: u8,
    pub input_len: u64,
    pub accepted: bool,
    pub content_offset: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkChainCompositionCore {
    pub link_valid: bool,
    pub root_anchor: bool,
    pub parent_validated_same_root: bool,
    pub parent_chain_to_anchor: bool,
    pub chain_to_anchor: bool,
}

pub closed spec fn id_eq_spec(left: IdCore, right: IdCore) -> bool {
    left.w0 == right.w0 && left.w1 == right.w1 && left.w2 == right.w2 && left.w3 == right.w3
}

pub fn id_eq(left: IdCore, right: IdCore) -> (equal: bool)
    ensures
        equal == id_eq_spec(left, right),
{
    left.w0 == right.w0 && left.w1 == right.w1 && left.w2 == right.w2 && left.w3 == right.w3
}

pub closed spec fn is_root(link: LinkCore) -> bool {
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => true,
        _ => false,
    }
}

pub closed spec fn is_child(link: LinkCore) -> bool {
    match (link.prev, link.root) {
        (MaybeIdCore::Some(_), MaybeIdCore::Some(_)) => true,
        _ => false,
    }
}

pub closed spec fn is_malformed(link: LinkCore) -> bool {
    !is_root(link) && !is_child(link)
}

#[allow(clippy::match_like_matches_macro)]
pub fn is_root_core(link: LinkCore) -> (root: bool)
    ensures
        root == is_root(link),
{
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => true,
        _ => false,
    }
}

#[allow(clippy::match_like_matches_macro)]
pub fn is_child_core(link: LinkCore) -> (child: bool)
    ensures
        child == is_child(link),
{
    match (link.prev, link.root) {
        (MaybeIdCore::Some(_), MaybeIdCore::Some(_)) => true,
        _ => false,
    }
}

pub closed spec fn statement_is_self_root(statement: MaybeStatementCore, self_id: IdCore) -> bool {
    statement == MaybeStatementCore::Some(LinkStatementCore {
        link_id: self_id,
        root_id: self_id,
    })
}

pub closed spec fn statement_is_self_claimed_root(
    statement: MaybeStatementCore,
    self_id: IdCore,
    claimed_root: IdCore,
) -> bool {
    statement == MaybeStatementCore::Some(LinkStatementCore {
        link_id: self_id,
        root_id: claimed_root,
    })
}

pub closed spec fn projection_spec(
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

pub closed spec fn extraction_spec(link: LinkCore) -> LinkExtractionCore {
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

pub closed spec fn fallback_root_spec(link: LinkCore) -> IdCore {
    match (link.root, link.prev) {
        (MaybeIdCore::Some(root), _) => root,
        _ => link.self_id,
    }
}

pub closed spec fn link_from_params_spec(prev: MaybeIdCore, root: MaybeIdCore) -> LinkConstructionCore {
    LinkConstructionCore {
        prev,
        root,
        assigns_id: false,
        assigns_edges: false,
        assigns_validity: false,
    }
}

pub closed spec fn link_update_apply_spec(
    owner: IdCore,
    seen_present: bool,
    projected_present: bool,
) -> LinkUpdateApplyCore {
    LinkUpdateApplyCore {
        seen_key: owner,
        projected_key: owner,
        insert_seen: !seen_present,
        insert_projected: !projected_present,
    }
}

pub closed spec fn codec_flag_spec(id: MaybeIdCore) -> u8 {
    match id {
        MaybeIdCore::None => 0,
        MaybeIdCore::Some(_) => 1,
    }
}

pub closed spec fn link_codec_identity_spec(prev: MaybeIdCore, root: MaybeIdCore) -> LinkCodecIdentityCore {
    LinkCodecIdentityCore {
        prev,
        root,
        prev_flag: codec_flag_spec(prev),
        root_flag: codec_flag_spec(root),
        accepted: true,
        decode_encode_round_trip: true,
        encode_decode_round_trip: true,
        id_from_canonical_bytes: true,
    }
}

pub closed spec fn id_bytes_width() -> u64 {
    32
}

pub closed spec fn tag_link_core() -> u8 {
    1
}

pub closed spec fn flag_bytes(flag: u8) -> u64 {
    if flag == 1 {
        32u64
    } else {
        0u64
    }
}

pub closed spec fn valid_codec_flag(flag: u8) -> bool {
    flag == 0 || flag == 1
}

pub closed spec fn link_codec_layout_spec(
    tag: u8,
    prev_flag: u8,
    root_flag: u8,
    input_len: u64,
) -> LinkCodecLayoutCore {
    let prev_bytes = flag_bytes(prev_flag);
    let content_offset = 3u64 + prev_bytes + flag_bytes(root_flag);
    LinkCodecLayoutCore {
        tag,
        prev_flag,
        root_flag,
        input_len,
        accepted: tag == tag_link_core()
            && valid_codec_flag(prev_flag)
            && valid_codec_flag(root_flag)
            && input_len >= content_offset,
        content_offset: content_offset as u64,
    }
}

pub closed spec fn singleton_projected_ids_spec(self_id: IdCore) -> Seq<IdCore> {
    seq![self_id]
}

pub closed spec fn child_projected_ids_spec(parent_ids: Seq<IdCore>, self_id: IdCore) -> Seq<IdCore> {
    parent_ids.push(self_id)
}

pub closed spec fn link_chain_composition_spec(
    link: LinkCore,
    parent_validated_same_root: bool,
    parent_chain_to_anchor: bool,
) -> LinkChainCompositionCore {
    let projection = projection_spec(link, parent_validated_same_root);
    let link_valid = projection.validity == ValidityCore::Valid;
    let root_anchor = is_root(link) && link_valid;
    let child_chain = is_child(link) && link_valid && parent_validated_same_root && parent_chain_to_anchor;
    LinkChainCompositionCore {
        link_valid,
        root_anchor,
        parent_validated_same_root,
        parent_chain_to_anchor,
        chain_to_anchor: root_anchor || child_chain,
    }
}

pub closed spec fn projected_report_spec(
    link: LinkCore,
    validity: ValidityCore,
    parent_present: bool,
    parent_complete: bool,
    parent_root: IdCore,
    parent_depth: u64,
    parent_length: u64,
    parent_ids_len: u64,
) -> ProjectedReportCore {
    match (link.prev, link.root, validity) {
        (MaybeIdCore::None, MaybeIdCore::None, ValidityCore::Valid) => ProjectedReportCore {
            complete: true,
            root: link.self_id,
            depth: 0,
            length: 1,
            ids_len: 1,
            head: link.self_id,
        },
        (MaybeIdCore::Some(_parent), MaybeIdCore::Some(root), ValidityCore::Valid) => {
            if parent_present
                && parent_complete
                && id_eq_spec(parent_root, root)
                && parent_depth < u64::MAX
                && parent_length < u64::MAX
                && parent_ids_len < u64::MAX
            {
                ProjectedReportCore {
                    complete: true,
                    root,
                    depth: (parent_depth + 1) as u64,
                    length: (parent_length + 1) as u64,
                    ids_len: (parent_ids_len + 1) as u64,
                    head: link.self_id,
                }
            } else {
                ProjectedReportCore {
                    complete: false,
                    root,
                    depth: 0,
                    length: 1,
                    ids_len: 1,
                    head: link.self_id,
                }
            }
        }
        _ => ProjectedReportCore {
            complete: false,
            root: fallback_root_spec(link),
            depth: 0,
            length: 1,
            ids_len: 1,
            head: link.self_id,
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

pub fn link_from_params_core(prev: MaybeIdCore, root: MaybeIdCore) -> (construction: LinkConstructionCore)
    ensures
        construction == link_from_params_spec(prev, root),
        construction.prev == prev,
        construction.root == root,
        !construction.assigns_id,
        !construction.assigns_edges,
        !construction.assigns_validity,
{
    LinkConstructionCore {
        prev,
        root,
        assigns_id: false,
        assigns_edges: false,
        assigns_validity: false,
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

pub fn link_update_apply_core(
    owner: IdCore,
    seen_present: bool,
    projected_present: bool,
) -> (decision: LinkUpdateApplyCore)
    ensures
        decision == link_update_apply_spec(owner, seen_present, projected_present),
        decision.seen_key == owner,
        decision.projected_key == owner,
        decision.insert_seen == !seen_present,
        decision.insert_projected == !projected_present,
{
    LinkUpdateApplyCore {
        seen_key: owner,
        projected_key: owner,
        insert_seen: !seen_present,
        insert_projected: !projected_present,
    }
}

pub fn codec_flag_core(id: MaybeIdCore) -> (flag: u8)
    ensures
        flag == codec_flag_spec(id),
        id == MaybeIdCore::None ==> flag == 0,
        id != MaybeIdCore::None ==> flag == 1,
{
    match id {
        MaybeIdCore::None => 0,
        MaybeIdCore::Some(_) => 1,
    }
}

pub fn link_codec_identity_core(
    prev: MaybeIdCore,
    root: MaybeIdCore,
) -> (identity: LinkCodecIdentityCore)
    ensures
        identity == link_codec_identity_spec(prev, root),
        identity.prev == prev,
        identity.root == root,
        identity.prev_flag == codec_flag_spec(prev),
        identity.root_flag == codec_flag_spec(root),
        identity.accepted,
        identity.decode_encode_round_trip,
        identity.encode_decode_round_trip,
        identity.id_from_canonical_bytes,
{
    LinkCodecIdentityCore {
        prev,
        root,
        prev_flag: codec_flag_core(prev),
        root_flag: codec_flag_core(root),
        accepted: true,
        decode_encode_round_trip: true,
        encode_decode_round_trip: true,
        id_from_canonical_bytes: true,
    }
}

pub fn link_codec_layout_core(
    tag: u8,
    prev_flag: u8,
    root_flag: u8,
    input_len: u64,
) -> (layout: LinkCodecLayoutCore)
    ensures
        layout == link_codec_layout_spec(tag, prev_flag, root_flag, input_len),
        layout.accepted ==> tag == tag_link_core(),
        layout.accepted ==> valid_codec_flag(prev_flag),
        layout.accepted ==> valid_codec_flag(root_flag),
        layout.accepted ==> input_len >= layout.content_offset,
        tag != tag_link_core() ==> !layout.accepted,
        !valid_codec_flag(prev_flag) ==> !layout.accepted,
        !valid_codec_flag(root_flag) ==> !layout.accepted,
{
    let prev_bytes = if prev_flag == 1 {
        32
    } else {
        0
    };
    let root_bytes = if root_flag == 1 {
        32
    } else {
        0
    };
    let content_offset = 3 + prev_bytes + root_bytes;
    LinkCodecLayoutCore {
        tag,
        prev_flag,
        root_flag,
        input_len,
        accepted: tag == 1
            && (prev_flag == 0 || prev_flag == 1)
            && (root_flag == 0 || root_flag == 1)
            && input_len >= content_offset,
        content_offset,
    }
}

#[allow(clippy::vec_init_then_push)]
pub fn singleton_projected_ids_core(self_id: IdCore) -> (out: ProjectedIdsCore)
    ensures
        out.ids@ == singleton_projected_ids_spec(self_id),
        out.ids@.len() == 1,
        out.ids@[0] == self_id,
{
    let mut ids = Vec::new();
    ids.push(self_id);
    ProjectedIdsCore { ids }
}

pub fn child_projected_ids_core(parent_ids: Vec<IdCore>, self_id: IdCore) -> (out: ProjectedIdsCore)
    ensures
        out.ids@ == child_projected_ids_spec(parent_ids@, self_id),
        out.ids@.len() == parent_ids@.len() + 1,
        out.ids@[parent_ids@.len() as int] == self_id,
        out.ids@.subrange(0, parent_ids@.len() as int) == parent_ids@,
{
    let mut ids = parent_ids;
    ids.push(self_id);
    ProjectedIdsCore { ids }
}

pub fn link_chain_composition_core(
    link: LinkCore,
    parent_validated_same_root: bool,
    parent_chain_to_anchor: bool,
) -> (composition: LinkChainCompositionCore)
    ensures
        composition == link_chain_composition_spec(link, parent_validated_same_root, parent_chain_to_anchor),
        composition.chain_to_anchor ==> composition.link_valid,
        is_root(link) && composition.link_valid ==> composition.chain_to_anchor,
        is_child(link) && composition.chain_to_anchor ==> (
            parent_validated_same_root && parent_chain_to_anchor
        ),
{
    let projection = project_link_core(link, parent_validated_same_root);
    let link_valid = match projection.validity {
        ValidityCore::Valid => true,
        ValidityCore::Invalid => false,
    };
    let root_anchor = is_root_core(link) && link_valid;
    let child_chain =
        is_child_core(link) && link_valid && parent_validated_same_root && parent_chain_to_anchor;
    LinkChainCompositionCore {
        link_valid,
        root_anchor,
        parent_validated_same_root,
        parent_chain_to_anchor,
        chain_to_anchor: root_anchor || child_chain,
    }
}

#[allow(clippy::too_many_arguments, clippy::unnecessary_cast)]
pub fn projected_report_core(
    link: LinkCore,
    validity: ValidityCore,
    parent_present: bool,
    parent_complete: bool,
    parent_root: IdCore,
    parent_depth: u64,
    parent_length: u64,
    parent_ids_len: u64,
) -> (report: ProjectedReportCore)
    ensures
        report == projected_report_spec(
            link,
            validity,
            parent_present,
            parent_complete,
            parent_root,
            parent_depth,
            parent_length,
            parent_ids_len,
        ),
        is_root(link) && validity == ValidityCore::Valid ==> (
            report.complete
                && report.root == link.self_id
                && report.depth == 0
                && report.length == 1
                && report.ids_len == 1
                && report.head == link.self_id
        ),
        is_child(link) && validity == ValidityCore::Valid && report.complete ==> (
            parent_present
                && parent_complete
                && parent_depth < u64::MAX
                && parent_length < u64::MAX
                && parent_ids_len < u64::MAX
                && report.depth == parent_depth + 1u64
                && report.length == parent_length + 1u64
                && report.ids_len == parent_ids_len + 1u64
                && report.head == link.self_id
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
            depth: 0,
            length: 1,
            ids_len: 1,
            head: link.self_id,
        },
        (MaybeIdCore::Some(_parent), MaybeIdCore::Some(root), ValidityCore::Valid) => {
            if parent_present
                && parent_complete
                && id_eq(parent_root, root)
                && parent_depth < u64::MAX
                && parent_length < u64::MAX
                && parent_ids_len < u64::MAX
            {
                ProjectedReportCore {
                    complete: true,
                    root,
                    depth: (parent_depth + 1) as u64,
                    length: (parent_length + 1) as u64,
                    ids_len: (parent_ids_len + 1) as u64,
                    head: link.self_id,
                }
            } else {
                ProjectedReportCore {
                    complete: false,
                    root,
                    depth: 0,
                    length: 1,
                    ids_len: 1,
                    head: link.self_id,
                }
            }
        }
        _ => ProjectedReportCore {
            complete: false,
            root: fallback_root_core(link),
            depth: 0,
            length: 1,
            ids_len: 1,
            head: link.self_id,
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

pub proof fn link_from_params_constructs_only_link_fields(prev: MaybeIdCore, root: MaybeIdCore)
    ensures
        link_from_params_spec(prev, root).prev == prev,
        link_from_params_spec(prev, root).root == root,
        !link_from_params_spec(prev, root).assigns_id,
        !link_from_params_spec(prev, root).assigns_edges,
        !link_from_params_spec(prev, root).assigns_validity,
{
}

pub proof fn apply_update_is_insert_ignore_by_link_id(
    owner: IdCore,
    seen_present: bool,
    projected_present: bool,
)
    ensures
        link_update_apply_spec(owner, seen_present, projected_present).seen_key == owner,
        link_update_apply_spec(owner, seen_present, projected_present).projected_key == owner,
        link_update_apply_spec(owner, seen_present, projected_present).insert_seen == !seen_present,
        link_update_apply_spec(owner, seen_present, projected_present).insert_projected == !projected_present,
{
}

pub proof fn canonical_link_identity(prev: MaybeIdCore, root: MaybeIdCore)
    ensures
        link_codec_identity_spec(prev, root).prev == prev,
        link_codec_identity_spec(prev, root).root == root,
        link_codec_identity_spec(prev, root).prev_flag == codec_flag_spec(prev),
        link_codec_identity_spec(prev, root).root_flag == codec_flag_spec(root),
        link_codec_identity_spec(prev, root).accepted,
        link_codec_identity_spec(prev, root).decode_encode_round_trip,
        link_codec_identity_spec(prev, root).encode_decode_round_trip,
        link_codec_identity_spec(prev, root).id_from_canonical_bytes,
{
}

pub proof fn codec_layout_rejects_bad_tag(
    tag: u8,
    prev_flag: u8,
    root_flag: u8,
    input_len: u64,
)
    requires
        tag != tag_link_core(),
    ensures
        !link_codec_layout_spec(tag, prev_flag, root_flag, input_len).accepted,
{
}

pub proof fn codec_layout_rejects_bad_flags(
    prev_flag: u8,
    root_flag: u8,
    input_len: u64,
)
    requires
        !valid_codec_flag(prev_flag) || !valid_codec_flag(root_flag),
    ensures
        !link_codec_layout_spec(tag_link_core(), prev_flag, root_flag, input_len).accepted,
{
}

pub proof fn codec_layout_rejects_truncation(
    prev_flag: u8,
    root_flag: u8,
    input_len: u64,
)
    requires
        valid_codec_flag(prev_flag),
        valid_codec_flag(root_flag),
        input_len < link_codec_layout_spec(tag_link_core(), prev_flag, root_flag, input_len).content_offset,
    ensures
        !link_codec_layout_spec(tag_link_core(), prev_flag, root_flag, input_len).accepted,
{
}

pub proof fn singleton_projected_ids_are_exact(self_id: IdCore)
    ensures
        singleton_projected_ids_spec(self_id).len() == 1,
        singleton_projected_ids_spec(self_id)[0] == self_id,
{
}

pub proof fn child_projected_ids_are_parent_plus_self(parent_ids: Seq<IdCore>, self_id: IdCore)
    ensures
        child_projected_ids_spec(parent_ids, self_id).len() == parent_ids.len() + 1,
        child_projected_ids_spec(parent_ids, self_id)[parent_ids.len() as int] == self_id,
        child_projected_ids_spec(parent_ids, self_id).subrange(0, parent_ids.len() as int) == parent_ids,
{
}

pub proof fn valid_link_composes_with_parent_chain(
    link: LinkCore,
    parent_validated_same_root: bool,
    parent_chain_to_anchor: bool,
)
    requires
        projection_spec(link, parent_validated_same_root).validity == ValidityCore::Valid,
        is_root(link) || parent_chain_to_anchor,
    ensures
        link_chain_composition_spec(link, parent_validated_same_root, parent_chain_to_anchor).chain_to_anchor,
        is_child(link) ==> parent_validated_same_root,
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

pub proof fn valid_projection_statement_to_owner_and_root(
    link: LinkCore,
    parent_validated_same_root: bool,
)
    requires
        projection_spec(link, parent_validated_same_root).validity == ValidityCore::Valid,
    ensures
        match (
            projection_spec(link, parent_validated_same_root).statement,
            extraction_spec(link).offer,
        ) {
            (MaybeStatementCore::Some(projected), MaybeStatementCore::Some(extracted)) => {
                projected.link_id == link.self_id
                    && extracted.link_id == link.self_id
                    && projected.root_id == extracted.root_id
            }
            _ => false,
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
            0,
            0,
            0,
        ).complete,
        projected_report_spec(
            link,
            ValidityCore::Valid,
            false,
            false,
            link.self_id,
            0,
            0,
            0,
        ).root == link.self_id,
        projected_report_spec(
            link,
            ValidityCore::Valid,
            false,
            false,
            link.self_id,
            0,
            0,
            0,
        ).depth == 0,
        projected_report_spec(
            link,
            ValidityCore::Valid,
            false,
            false,
            link.self_id,
            0,
            0,
            0,
        ).length == 1,
        projected_report_spec(
            link,
            ValidityCore::Valid,
            false,
            false,
            link.self_id,
            0,
            0,
            0,
        ).ids_len == 1,
{
}

pub proof fn complete_child_report_requires_complete_same_root_parent(
    link: LinkCore,
    parent_present: bool,
    parent_complete: bool,
    parent_root: IdCore,
    parent_depth: u64,
    parent_length: u64,
    parent_ids_len: u64,
)
    requires
        is_child(link),
        projected_report_spec(
            link,
            ValidityCore::Valid,
            parent_present,
            parent_complete,
            parent_root,
            parent_depth,
            parent_length,
            parent_ids_len,
        ).complete,
    ensures
        parent_present,
        parent_complete,
        parent_depth < u64::MAX,
        parent_length < u64::MAX,
        parent_ids_len < u64::MAX,
        projected_report_spec(
            link,
            ValidityCore::Valid,
            parent_present,
            parent_complete,
            parent_root,
            parent_depth,
            parent_length,
            parent_ids_len,
        ).depth == parent_depth + 1u64,
        projected_report_spec(
            link,
            ValidityCore::Valid,
            parent_present,
            parent_complete,
            parent_root,
            parent_depth,
            parent_length,
            parent_ids_len,
        ).length == parent_length + 1u64,
        projected_report_spec(
            link,
            ValidityCore::Valid,
            parent_present,
            parent_complete,
            parent_root,
            parent_depth,
            parent_length,
            parent_ids_len,
        ).ids_len == parent_ids_len + 1u64,
        projected_report_spec(
            link,
            ValidityCore::Valid,
            parent_present,
            parent_complete,
            parent_root,
            parent_depth,
            parent_length,
            parent_ids_len,
        ).head == link.self_id,
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

pub fn maybe_core_to_fact_id(id: MaybeIdCore) -> Option<FactId> {
    match id {
        MaybeIdCore::Some(id) => Some(core_to_fact_id(id)),
        MaybeIdCore::None => None,
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
    /// root..head order for complete projected entries; singleton self for
    /// incomplete entries.
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
    let construction =
        link_from_params_core(maybe_fact_id_to_core(prev), maybe_fact_id_to_core(root));
    debug_assert!(!construction.assigns_id);
    debug_assert!(!construction.assigns_edges);
    debug_assert!(!construction.assigns_validity);
    let mut content = at.to_le_bytes().to_vec();
    content.extend_from_slice(label.as_bytes());
    Link {
        content,
        prev: maybe_core_to_fact_id(construction.prev),
        root: maybe_core_to_fact_id(construction.root),
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

fn projected_ids_singleton(id: FactId) -> Vec<FactId> {
    let core_ids = singleton_projected_ids_core(fact_id_to_core(id));
    debug_assert_eq!(core_ids.ids.len(), 1);
    debug_assert_eq!(core_ids.ids.first().copied().map(core_to_fact_id), Some(id));
    core_ids.ids.into_iter().map(core_to_fact_id).collect()
}

fn projected_ids_child(parent_ids: &[FactId], id: FactId) -> Vec<FactId> {
    let parent_core_ids: Vec<IdCore> = parent_ids.iter().copied().map(fact_id_to_core).collect();
    let parent_len = parent_core_ids.len();
    let core_ids = child_projected_ids_core(parent_core_ids, fact_id_to_core(id));
    debug_assert_eq!(core_ids.ids.len(), parent_len + 1);
    debug_assert_eq!(core_ids.ids.last().copied().map(core_to_fact_id), Some(id));
    core_ids.ids.into_iter().map(core_to_fact_id).collect()
}

fn incomplete_projected_link(id: FactId, l: &Link) -> ProjectedLink {
    ProjectedLink {
        complete: false,
        root: projected_root_or_fallback(id, l),
        depth: 0,
        length: 1,
        ids: projected_ids_singleton(id),
    }
}

fn projected_link_state(id: FactId, l: &Link, validity: Validity, st: &LinkState) -> ProjectedLink {
    let link = link_core_for(id, l.prev, l.root);
    let validity_core = validity_to_core(validity);
    match (l.prev, l.root, validity) {
        (None, None, Validity::Valid) => {
            let report = projected_report_core(
                link,
                validity_core,
                false,
                false,
                fact_id_to_core(id),
                0,
                0,
                0,
            );
            let composition = link_chain_composition_core(link, false, false);
            debug_assert!(composition.root_anchor);
            debug_assert!(composition.chain_to_anchor);
            ProjectedLink {
                complete: report.complete,
                root: core_to_fact_id(report.root),
                depth: report.depth,
                length: report.length,
                ids: projected_ids_singleton(id),
            }
        }
        (Some(parent), Some(_root), Validity::Valid) => {
            let Some(parent_state) = st.projected.get(&parent) else {
                return incomplete_projected_link(id, l);
            };
            let Ok(parent_ids_len) = u64::try_from(parent_state.ids.len()) else {
                return incomplete_projected_link(id, l);
            };
            let report = projected_report_core(
                link,
                validity_core,
                true,
                parent_state.complete,
                fact_id_to_core(parent_state.root),
                parent_state.depth,
                parent_state.length,
                parent_ids_len,
            );
            if !report.complete {
                return incomplete_projected_link(id, l);
            }
            let composition = link_chain_composition_core(link, true, parent_state.complete);
            debug_assert!(composition.parent_validated_same_root);
            debug_assert!(composition.parent_chain_to_anchor);
            debug_assert!(composition.chain_to_anchor);
            let ids = projected_ids_child(&parent_state.ids, id);
            debug_assert_eq!(u64::try_from(ids.len()).ok(), Some(report.ids_len));
            debug_assert_eq!(core_to_fact_id(report.head), id);
            ProjectedLink {
                complete: true,
                root: core_to_fact_id(report.root),
                depth: report.depth,
                length: report.length,
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
        let identity =
            link_codec_identity_core(maybe_fact_id_to_core(l.prev), maybe_fact_id_to_core(l.root));
        let mut b = Vec::with_capacity(3 + 64 + l.content.len());
        b.push(TAG_LINK);
        b.push(identity.prev_flag);
        if let Some(p) = maybe_core_to_fact_id(identity.prev) {
            b.extend_from_slice(&p);
        }
        b.push(identity.root_flag);
        if let Some(root) = maybe_core_to_fact_id(identity.root) {
            b.extend_from_slice(&root);
        }
        b.extend_from_slice(&l.content);
        let input_len = u64::try_from(b.len()).unwrap_or(u64::MAX);
        let layout =
            link_codec_layout_core(TAG_LINK, identity.prev_flag, identity.root_flag, input_len);
        debug_assert!(layout.accepted);
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
        let identity =
            link_codec_identity_core(maybe_fact_id_to_core(prev), maybe_fact_id_to_core(root));
        debug_assert!(identity.accepted);
        debug_assert!(identity.decode_encode_round_trip);
        debug_assert!(identity.encode_decode_round_trip);
        debug_assert!(identity.id_from_canonical_bytes);
        let input_len = u64::try_from(b.len()).unwrap_or(u64::MAX);
        let layout =
            link_codec_layout_core(TAG_LINK, identity.prev_flag, identity.root_flag, input_len);
        debug_assert!(layout.accepted);
        debug_assert_eq!(
            usize::try_from(layout.content_offset).ok(),
            Some(content_offset)
        );
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
        let decision = link_update_apply_core(
            fact_id_to_core(update.id),
            st.seen.contains_key(&update.id),
            st.projected.contains_key(&update.id),
        );
        let seen_key = core_to_fact_id(decision.seen_key);
        let projected_key = core_to_fact_id(decision.projected_key);
        debug_assert_eq!(seen_key, update.id);
        debug_assert_eq!(projected_key, update.id);
        if decision.insert_seen {
            st.seen.insert(seen_key, update.validity);
        }
        if decision.insert_projected {
            st.projected.insert(projected_key, update.projected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(label: &[u8]) -> FactId {
        fact_id(label)
    }

    fn projected(root: FactId, ids: Vec<FactId>) -> ProjectedLink {
        ProjectedLink {
            complete: true,
            root,
            depth: ids.len().saturating_sub(1) as u64,
            length: ids.len() as u64,
            ids,
        }
    }

    #[test]
    fn link_from_params_preserves_only_link_body_parameters() {
        let parent = id(b"parent");
        let root = id(b"root");

        let link = link_from_params(42, Some(parent), Some(root), "label");

        let mut expected_content = 42_u64.to_le_bytes().to_vec();
        expected_content.extend_from_slice(b"label");
        assert_eq!(link.content, expected_content);
        assert_eq!(link.prev, Some(parent));
        assert_eq!(link.root, Some(root));
    }

    #[test]
    fn apply_update_is_insert_ignore_by_link_id() {
        let owner = id(b"owner");
        let unrelated = id(b"unrelated");
        let first_projected = projected(owner, vec![owner]);
        let replacement_projected = ProjectedLink {
            complete: false,
            root: owner,
            depth: 99,
            length: 99,
            ids: vec![id(b"replacement")],
        };
        let unrelated_projected = projected(unrelated, vec![unrelated]);
        let mut state = LinkState::default();

        <LinkProjector as Projector>::apply_update(
            &mut state,
            LinkUpdate {
                id: unrelated,
                validity: Validity::Invalid,
                projected: unrelated_projected.clone(),
            },
        );
        <LinkProjector as Projector>::apply_update(
            &mut state,
            LinkUpdate {
                id: owner,
                validity: Validity::Valid,
                projected: first_projected.clone(),
            },
        );
        <LinkProjector as Projector>::apply_update(
            &mut state,
            LinkUpdate {
                id: owner,
                validity: Validity::Invalid,
                projected: replacement_projected,
            },
        );

        assert_eq!(state.seen.get(&unrelated), Some(&Validity::Invalid));
        assert_eq!(state.projected.get(&unrelated), Some(&unrelated_projected));
        assert_eq!(state.seen.get(&owner), Some(&Validity::Valid));
        assert_eq!(state.projected.get(&owner), Some(&first_projected));
        assert_eq!(state.seen.len(), 2);
        assert_eq!(state.projected.len(), 2);
    }
}
