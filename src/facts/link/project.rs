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
//! POLICY. A link is valid iff:
//!   1. CODEC. Its bytes decode canonically to exactly one `Link`, and its id is
//!      derived from those canonical bytes.
//!   2. SHAPE. It is either a root, a child, or malformed.
//!   3. EXTRACT. Roots assert `valid_link(self,self)`; children assert
//!      `valid_link(self,root)` and need `valid_link(parent,root)`; malformed
//!      links assert nothing.
//!   4. CONTEXT. A child may validate only from exact validated parent/root
//!      context.
//!   5. PROJECT. A valid projection promotes only its own statement and emits no
//!      raw facts.
//!   6. STATE. Projection updates only this link id's read-model entry.
//!   7. COMPOSE. The local child step composes with core/replay provenance for
//!      supplied proof-facing same-root chains.
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
//! - [x] Safety: canonical link codec bridge: `LinkProjector::encode` delegates
//!       to an executable Verus byte builder for
//!       `tag | has_prev | prev[32]? | has_root | root[32]? | content`, the
//!       proof-facing byte sequence preserves prev/root/content segments, and
//!       `LinkProjector::decode` delegates acceptance and prev/root/content
//!       byte segmentation to the executable Verus decoder before converting
//!       32-byte id segments into runtime `FactId`s. Verified below by
//!       `link_encode_bytes_core`, `link_decode_header_core`,
//!       `link_decode_bytes_core`,
//!       `canonical_link_bytes_round_trip`, `codec_flag_core`,
//!       `link_codec_layout_core`, `decode_header_accepts_only_canonical_layout`,
//!       `codec_layout_rejects_bad_tag`,
//!       `codec_layout_rejects_bad_flags`, and `codec_layout_rejects_truncation`.
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
//! - [x] Safety: proof-facing statement-to-owner: every validated link offer at
//!       `valid_link_key(link_id, root_id)` was promoted from a valid link fact
//!       whose owner is `link_id` and whose asserted offer address is the same
//!       link/root statement. Verified below by
//!       `validated_link_offer_statement_to_owner_from_engine`, importing the
//!       core engine provenance theorem.
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
//! - [x] Safety: proof-facing chain preservation with core: using `core::engine`
//!       and `core::play` provenance, a supplied valid same-root link chain and
//!       its validated statements remain backed by a replay state satisfying the
//!       engine invariant; no theorem here claims anchor uniqueness or derives
//!       chain existence from engine state alone.
//!       Verified below by `root_link_chain_to_anchor`,
//!       `child_extends_link_chain`, and
//!       `replay_preserves_supplied_link_chain_to_anchor`.
//! Imported theorem checklist:
//! - [x] `core::item`: fact ids are content addresses for canonical bytes. Proven
//!       in `src/core/item_unproven.rs::fact_id_content_address`.
//! - [x] `core::offer`: asserted edge constructors and match addresses have fixed
//!       meaning. Proven in
//!       `src/core/offer_unproven.rs::asserted_edge_address_shape`.
//! - [x] `core::typestate`: `Context::has_offer` is exact validated-offer lookup.
//!       Proven in `src/core/typestate_unproven.rs::context_lookup_exact`.
//! - [x] `core::engine`: proof-facing context/promotion gates relate context
//!       offers to valid owners. Proven in
//!       `src/core/engine_unproven.rs::engine_transition_preserves_validated_context_provenance`
//!       and `src/core/engine_unproven.rs::engine_transition_trace_preserves_invariant`.
//! - [x] `core::engine`: the proof-facing engine model exposes statement
//!       provenance for a validated offer. Proven in
//!       `src/core/engine_unproven.rs::engine_validated_offer_for_has_valid_owner`.
//! - [x] `core::play`: proof-facing replay traces preserve engine validity.
//!       Proven in `src/core/play_unproven.rs::replay_reports_engine_validity`.
//! Local theorem checklist:
//! - [x] Local link same-root extraction/projection kernel. Proven below by
//!       `extract_link_core`, `project_link_core`,
//!       `codec_flag_core`,
//!       `link_codec_layout_core`,
//!       `codec_layout_rejects_bad_tag`,
//!       `codec_layout_rejects_bad_flags`,
//!       `codec_layout_rejects_truncation`,
//!       `child_extraction_offer_and_need_same_root`,
//!       `valid_child_requires_validated_same_root_parent`, and
//!       `valid_projection_statement_equals_extracted_offer`.
//! - [x] Local link sequence composition step. Proven below by
//!       `root_link_chain_to_anchor`, `child_extends_link_chain`, and
//!       `replay_preserves_supplied_link_chain_to_anchor`.
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
//! - [x] Link/core supplied-chain preservation over the proof-facing replay
//!       model. Proven below by
//!       `replay_preserves_supplied_link_chain_to_anchor`.
//! Proof strategy:
//! - Prove the executable canonical byte builder preserves prev/root/content
//!   segments and prove rejection cases for tag/flag/truncation.
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
//! - Prove the statement-to-owner lemma from `link_edges`, `valid_link_key`,
//!   content addressing, and the core engine validated-offer provenance theorem.
//! - Prove the same-root parent-chain step over a concrete sequence: root case
//!   `prev=None, root=None` gives `valid_link(self,self)`; child step extends an
//!   existing sequence only when the child names the previous head and preserves
//!   the same root/domain id.
use std::collections::BTreeMap;

use crate::core::admit::Admitted;
use crate::core::item::{fact_id, FactId};
use crate::core::offer::{Key, Offer, Role};
use crate::core::projector::{ProjectOutcome, Projector};
use crate::core::typestate::{Asserted, Context, Validity};
use vstd::prelude::*;

// 1. Runtime Surface.
//
// These are the public runtime nouns the rest of the file explains and proves:
// `Link` is the semantic fact, `ProjectedLink` is the family-owned read model,
// and `LinkProjector` is the only runtime path that can validate/update links.

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

/// The projector's private read-model: id -> projected chain entry.
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

// 2. Proof Vocabulary.
//
// These proof-facing nouns mirror the runtime surface with small, explicit
// shapes that Verus can reason about directly. The later sections prove the
// runtime functions by delegating through these kernels.

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
pub enum LinkShapeCore {
    Root,
    Child(IdCore, IdCore),
    Malformed,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParentReportCore {
    pub present: bool,
    pub complete: bool,
    pub root: IdCore,
    pub depth: u64,
    pub length: u64,
    pub ids_len: u64,
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
pub struct LinkCodecLayoutCore {
    pub tag: u8,
    pub prev_flag: u8,
    pub root_flag: u8,
    pub input_len: u64,
    pub accepted: bool,
    pub content_offset: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LinkDecodedBytesCore {
    pub layout: LinkCodecLayoutCore,
    pub prev_present: bool,
    pub prev_bytes: Vec<u8>,
    pub root_present: bool,
    pub root_bytes: Vec<u8>,
    pub content: Vec<u8>,
}

// 3. Shape Predicates And Statement Helpers.
//
// These helpers define the three semantic branches used everywhere below:
// root, child, and malformed. Later extraction/projection/report sections all
// reduce to these branch predicates.

pub closed spec fn id_eq_spec(left: IdCore, right: IdCore) -> bool {
    left.w0 == right.w0 && left.w1 == right.w1 && left.w2 == right.w2 && left.w3 == right.w3
}

pub fn id_eq(left: IdCore, right: IdCore) -> (equal: bool)
    ensures
        equal == id_eq_spec(left, right),
{
    left.w0 == right.w0 && left.w1 == right.w1 && left.w2 == right.w2 && left.w3 == right.w3
}

pub closed spec fn link_shape_spec(link: LinkCore) -> LinkShapeCore {
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => LinkShapeCore::Root,
        (MaybeIdCore::Some(parent), MaybeIdCore::Some(root)) => LinkShapeCore::Child(parent, root),
        _ => LinkShapeCore::Malformed,
    }
}

pub fn link_shape_core(link: LinkCore) -> (shape: LinkShapeCore)
    ensures
        shape == link_shape_spec(link),
{
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => LinkShapeCore::Root,
        (MaybeIdCore::Some(parent), MaybeIdCore::Some(root)) => LinkShapeCore::Child(parent, root),
        _ => LinkShapeCore::Malformed,
    }
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

pub closed spec fn link_statement_spec(link_id: IdCore, root_id: IdCore) -> MaybeStatementCore {
    MaybeStatementCore::Some(LinkStatementCore { link_id, root_id })
}

pub closed spec fn no_link_statement_spec() -> MaybeStatementCore {
    MaybeStatementCore::None
}

pub fn link_statement_core(link_id: IdCore, root_id: IdCore) -> (statement: MaybeStatementCore)
    ensures
        statement == link_statement_spec(link_id, root_id),
{
    MaybeStatementCore::Some(LinkStatementCore { link_id, root_id })
}

pub closed spec fn statement_is_self_root(statement: MaybeStatementCore, self_id: IdCore) -> bool {
    statement == link_statement_spec(self_id, self_id)
}

pub closed spec fn statement_is_self_claimed_root(
    statement: MaybeStatementCore,
    self_id: IdCore,
    claimed_root: IdCore,
) -> bool {
    statement == link_statement_spec(self_id, claimed_root)
}

pub closed spec fn zero_id_core() -> IdCore {
    IdCore {
        w0: 0,
        w1: 0,
        w2: 0,
        w3: 0,
    }
}

pub closed spec fn chain_head_id(chain: Seq<LinkCore>) -> IdCore {
    if chain.len() == 0 {
        zero_id_core()
    } else {
        chain[chain.len() - 1].self_id
    }
}

pub closed spec fn engine_id_for_link_id(id: IdCore) -> crate::core::engine::EngineIdCore {
    crate::core::engine::EngineIdCore {
        w0: id.w0,
        w1: id.w1,
        w2: id.w2,
        w3: id.w3,
    }
}

pub closed spec fn link_statement_addr_core(
    link_id: IdCore,
    root_id: IdCore,
) -> crate::core::engine::EngineAddrCore {
    crate::core::engine::EngineAddrCore {
        role: 1,
        scope: 0,
        key_subject: engine_id_for_link_id(link_id),
        key_domain: engine_id_for_link_id(root_id),
    }
}

// 4. Projection Validity Model.
//
// A root is valid by itself. A child is valid only when the caller supplies the
// exact validated same-root parent context. Malformed shapes are invalid.

pub closed spec fn projection_spec(
    link: LinkCore,
    parent_validated_same_root: bool,
) -> LinkProjectionCore {
    match link_shape_spec(link) {
        LinkShapeCore::Root => LinkProjectionCore {
            validity: ValidityCore::Valid,
            update_owner: link.self_id,
            statement: link_statement_spec(link.self_id, link.self_id),
        },
        LinkShapeCore::Child(_parent, root) => {
            if parent_validated_same_root {
                LinkProjectionCore {
                    validity: ValidityCore::Valid,
                    update_owner: link.self_id,
                    statement: link_statement_spec(link.self_id, root),
                }
            } else {
                LinkProjectionCore {
                    validity: ValidityCore::Invalid,
                    update_owner: link.self_id,
                    statement: no_link_statement_spec(),
                }
            }
        }
        LinkShapeCore::Malformed => LinkProjectionCore {
            validity: ValidityCore::Invalid,
            update_owner: link.self_id,
            statement: no_link_statement_spec(),
        },
    }
}

// 5. Extraction Model.
//
// Extraction is context-free. It names the one self statement a well-formed
// link may later promote and, for children, the exact parent/root statement it
// needs from validated context.

pub closed spec fn extraction_spec(link: LinkCore) -> LinkExtractionCore {
    match link_shape_spec(link) {
        LinkShapeCore::Root => LinkExtractionCore {
            offer: link_statement_spec(link.self_id, link.self_id),
            need: no_link_statement_spec(),
        },
        LinkShapeCore::Child(parent, root) => LinkExtractionCore {
            offer: link_statement_spec(link.self_id, root),
            need: link_statement_spec(parent, root),
        },
        LinkShapeCore::Malformed => LinkExtractionCore {
            offer: no_link_statement_spec(),
            need: no_link_statement_spec(),
        },
    }
}

// 6. Report Fallback Model.
//
// Incomplete reports are read-model observations only; they do not create
// validity evidence. The fallback root keeps display/report shape deterministic.

pub closed spec fn fallback_root_spec(link: LinkCore) -> IdCore {
    match (link.root, link.prev) {
        (MaybeIdCore::Some(root), _) => root,
        _ => link.self_id,
    }
}

// 7. Construction Proof Model.
//
// Construction can copy only caller-supplied link parameters into the typed
// fact. It cannot assign ids, edges, or validity.

pub closed spec fn link_from_params_spec(prev: MaybeIdCore, root: MaybeIdCore) -> LinkConstructionCore {
    LinkConstructionCore {
        prev,
        root,
        assigns_id: false,
        assigns_edges: false,
        assigns_validity: false,
    }
}

// 8. Update Application Model.
//
// Updates are owner-scoped and insert/ignore. Projection of one fact cannot
// overwrite another fact's projected state.

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

// 9. Canonical Codec Model.
//
// The byte layout is `tag | has_prev | prev[32]? | has_root | root[32]? |
// content`. These helpers model the accepted layout, rejection cases, and
// semantic flag shape used by runtime encode/decode.

pub closed spec fn codec_flag_spec(id: MaybeIdCore) -> u8 {
    match id {
        MaybeIdCore::None => 0,
        MaybeIdCore::Some(_) => 1,
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

pub closed spec fn missing_codec_flag() -> u8 {
    255
}

pub closed spec fn id_segment_width(present: bool) -> int {
    if present {
        32
    } else {
        0
    }
}

pub closed spec fn valid_optional_id_segment(present: bool, bytes: Seq<u8>) -> bool {
    !present || bytes.len() == 32
}

pub closed spec fn codec_flag_from_present(present: bool) -> u8 {
    if present {
        1
    } else {
        0
    }
}

pub closed spec fn optional_id_segment(present: bool, bytes: Seq<u8>) -> Seq<u8> {
    if present {
        bytes
    } else {
        Seq::empty()
    }
}

pub closed spec fn link_encoded_bytes_spec(
    prev_present: bool,
    prev_bytes: Seq<u8>,
    root_present: bool,
    root_bytes: Seq<u8>,
    content: Seq<u8>,
) -> Seq<u8> {
    seq![tag_link_core()]
        .add(seq![codec_flag_from_present(prev_present)])
        .add(optional_id_segment(prev_present, prev_bytes))
        .add(seq![codec_flag_from_present(root_present)])
        .add(optional_id_segment(root_present, root_bytes))
        .add(content)
}

pub closed spec fn link_encoded_content_offset_spec(
    prev_present: bool,
    root_present: bool,
) -> int {
    3 + id_segment_width(prev_present) + id_segment_width(root_present)
}

pub closed spec fn canonical_link_bytes_spec(
    bytes: Seq<u8>,
    prev_present: bool,
    prev_bytes: Seq<u8>,
    root_present: bool,
    root_bytes: Seq<u8>,
    content: Seq<u8>,
) -> bool {
    valid_optional_id_segment(prev_present, prev_bytes)
        && valid_optional_id_segment(root_present, root_bytes)
        && bytes == link_encoded_bytes_spec(
            prev_present,
            prev_bytes,
            root_present,
            root_bytes,
            content,
        )
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

pub closed spec fn link_decode_header_spec(bytes: Seq<u8>) -> LinkCodecLayoutCore {
    let input_len = bytes.len() as u64;
    if bytes.len() < 2 {
        LinkCodecLayoutCore {
            tag: if bytes.len() > 0 { bytes[0] } else { 0 },
            prev_flag: missing_codec_flag(),
            root_flag: missing_codec_flag(),
            input_len,
            accepted: false,
            content_offset: 0,
        }
    } else if bytes[0] != tag_link_core() || !valid_codec_flag(bytes[1]) {
        LinkCodecLayoutCore {
            tag: bytes[0],
            prev_flag: bytes[1],
            root_flag: missing_codec_flag(),
            input_len,
            accepted: false,
            content_offset: 0,
        }
    } else {
        let root_flag_offset = 2 + flag_bytes(bytes[1]) as int;
        if bytes.len() <= root_flag_offset {
            LinkCodecLayoutCore {
                tag: bytes[0],
                prev_flag: bytes[1],
                root_flag: missing_codec_flag(),
                input_len,
                accepted: false,
                content_offset: 0,
            }
        } else {
            link_codec_layout_spec(bytes[0], bytes[1], bytes[root_flag_offset], input_len)
        }
    }
}

pub closed spec fn singleton_projected_ids_spec(self_id: IdCore) -> Seq<IdCore> {
    seq![self_id]
}

pub closed spec fn child_projected_ids_spec(parent_ids: Seq<IdCore>, self_id: IdCore) -> Seq<IdCore> {
    parent_ids.push(self_id)
}

pub closed spec fn absent_parent_report_spec(root: IdCore) -> ParentReportCore {
    ParentReportCore {
        present: false,
        complete: false,
        root,
        depth: 0,
        length: 0,
        ids_len: 0,
    }
}

pub closed spec fn root_report_is_complete_self(report: ProjectedReportCore, link: LinkCore) -> bool {
    report.complete
        && report.root == link.self_id
        && report.depth == 0
        && report.length == 1
        && report.ids_len == 1
        && report.head == link.self_id
}

pub closed spec fn complete_child_report_extends_parent(
    report: ProjectedReportCore,
    parent: ParentReportCore,
    link: LinkCore,
) -> bool {
    parent.present
        && parent.complete
        && parent.depth < u64::MAX
        && parent.length < u64::MAX
        && parent.ids_len < u64::MAX
        && report.depth == parent.depth + 1u64
        && report.length == parent.length + 1u64
        && report.ids_len == parent.ids_len + 1u64
        && report.head == link.self_id
        && match link.root {
            MaybeIdCore::Some(root) => id_eq_spec(parent.root, root),
            MaybeIdCore::None => false,
        }
}

// 10. Composition Model.
//
// Link ancestry is a concrete sequence: the first element is an anchor root and
// each later child names the previous head as `prev` and preserves the same
// root/domain id. This replaces the old caller-supplied parent-chain boolean.

pub closed spec fn link_chain_to_anchor(chain: Seq<LinkCore>, root: IdCore) -> bool
    decreases chain.len(),
{
    if chain.len() == 0 {
        false
    } else if chain.len() == 1 {
        is_root(chain[0])
            && chain[0].self_id == root
            && projection_spec(chain[0], false).validity == ValidityCore::Valid
    } else {
        let parent_chain = chain.subrange(0, chain.len() - 1);
        let child = chain[chain.len() - 1];
        link_chain_to_anchor(parent_chain, root)
            && is_child(child)
            && child.prev == MaybeIdCore::Some(chain_head_id(parent_chain))
            && child.root == MaybeIdCore::Some(root)
            && projection_spec(child, true).validity == ValidityCore::Valid
    }
}

pub closed spec fn chain_contains_validated_link_statements(
    state: crate::core::engine::EngineStateCore,
    chain: Seq<LinkCore>,
    root: IdCore,
) -> bool {
    forall |i: int|
        0 <= i < chain.len() ==>
            crate::core::engine::validated_offer_for(
                state.validated,
                engine_id_for_link_id(#[trigger] chain[i].self_id),
                link_statement_addr_core(chain[i].self_id, root),
            )
}

// 11. Projected Report Model.
//
// Complete child reports can be constructed only from complete same-root parent
// reports. All other cases produce a singleton incomplete observation.

pub closed spec fn projected_report_spec(
    link: LinkCore,
    validity: ValidityCore,
    parent: ParentReportCore,
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
            if parent.present
                && parent.complete
                && id_eq_spec(parent.root, root)
                && parent.depth < u64::MAX
                && parent.length < u64::MAX
                && parent.ids_len < u64::MAX
            {
                ProjectedReportCore {
                    complete: true,
                    root,
                    depth: (parent.depth + 1) as u64,
                    length: (parent.length + 1) as u64,
                    ids_len: (parent.ids_len + 1) as u64,
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

// 12. Report Helper Kernel.

pub fn fallback_root_core(link: LinkCore) -> (root: IdCore)
    ensures
        root == fallback_root_spec(link),
{
    match (link.root, link.prev) {
        (MaybeIdCore::Some(root), _) => root,
        _ => link.self_id,
    }
}

// 13. Construction Kernel.

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

// 14. Extraction Kernel.

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
    match link_shape_core(link) {
        LinkShapeCore::Root => LinkExtractionCore {
            offer: link_statement_core(link.self_id, link.self_id),
            need: MaybeStatementCore::None,
        },
        LinkShapeCore::Child(parent, root) => LinkExtractionCore {
            offer: link_statement_core(link.self_id, root),
            need: link_statement_core(parent, root),
        },
        LinkShapeCore::Malformed => LinkExtractionCore {
            offer: MaybeStatementCore::None,
            need: MaybeStatementCore::None,
        },
    }
}

// 15. Update Application Kernel.

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

// 16. Codec Kernels.

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

pub fn link_decode_header_core(bytes: Vec<u8>) -> (layout: LinkCodecLayoutCore)
    ensures
        layout == link_decode_header_spec(bytes@),
        layout.accepted ==> layout.tag == tag_link_core(),
        layout.accepted ==> valid_codec_flag(layout.prev_flag),
        layout.accepted ==> valid_codec_flag(layout.root_flag),
        layout.accepted ==> layout.input_len >= layout.content_offset,
{
    let input_len = bytes.len() as u64;
    if bytes.len() < 2 {
        LinkCodecLayoutCore {
            tag: if !bytes.is_empty() { bytes[0] } else { 0 },
            prev_flag: 255,
            root_flag: 255,
            input_len,
            accepted: false,
            content_offset: 0,
        }
    } else if bytes[0] != 1 || !(bytes[1] == 0 || bytes[1] == 1) {
        LinkCodecLayoutCore {
            tag: bytes[0],
            prev_flag: bytes[1],
            root_flag: 255,
            input_len,
            accepted: false,
            content_offset: 0,
        }
    } else {
        let root_flag_offset: usize = if bytes[1] == 1 { 34 } else { 2 };
        if bytes.len() <= root_flag_offset {
            LinkCodecLayoutCore {
                tag: bytes[0],
                prev_flag: bytes[1],
                root_flag: 255,
                input_len,
                accepted: false,
                content_offset: 0,
            }
        } else {
            link_codec_layout_core(bytes[0], bytes[1], bytes[root_flag_offset], input_len)
        }
    }
}

#[allow(clippy::ptr_arg)]
pub fn copy_range_core(bytes: &Vec<u8>, start: usize, end: usize) -> (out: Vec<u8>)
    requires
        start <= end,
        end <= bytes.len(),
    ensures
        out@ == bytes@.subrange(start as int, end as int),
{
    let mut out = Vec::new();
    let mut i = start;
    while i < end
        invariant
            start <= i <= end,
            end <= bytes.len(),
            out@ == bytes@.subrange(start as int, i as int),
        decreases end - i
    {
        out.push(bytes[i]);
        i += 1;
    }
    out
}

pub fn link_decode_bytes_core(bytes: Vec<u8>) -> (decoded: LinkDecodedBytesCore)
    ensures
        decoded.layout == link_decode_header_spec(bytes@),
        decoded.layout.accepted ==> decoded.prev_present == (decoded.layout.prev_flag == 1),
        decoded.layout.accepted ==> decoded.root_present == (decoded.layout.root_flag == 1),
        decoded.layout.accepted && decoded.prev_present ==> decoded.prev_bytes@
            == bytes@.subrange(2, 34),
        decoded.layout.accepted && !decoded.prev_present ==> decoded.prev_bytes@ == Seq::<u8>::empty(),
        decoded.layout.accepted && decoded.root_present ==> decoded.root_bytes@
            == bytes@.subrange(if decoded.prev_present { 35 } else { 3 }, if decoded.prev_present { 67 } else { 35 }),
        decoded.layout.accepted && !decoded.root_present ==> decoded.root_bytes@ == Seq::<u8>::empty(),
        decoded.layout.accepted ==> decoded.content@
            == bytes@.subrange(decoded.layout.content_offset as int, bytes@.len() as int),
{
    let layout = link_decode_header_core(bytes.clone());
    if !layout.accepted {
        LinkDecodedBytesCore {
            layout,
            prev_present: false,
            prev_bytes: Vec::new(),
            root_present: false,
            root_bytes: Vec::new(),
            content: Vec::new(),
        }
    } else {
        let prev_present = layout.prev_flag == 1;
        let prev_bytes = if prev_present {
            assert(bytes.len() >= 34);
            copy_range_core(&bytes, 2, 34)
        } else {
            Vec::new()
        };

        let root_flag_offset: usize = if prev_present { 34 } else { 2 };
        let root_present = layout.root_flag == 1;
        let root_start = root_flag_offset + 1;
        let root_end = if root_present {
            root_start + 32
        } else {
            root_start
        };
        let root_bytes = if root_present {
            assert(bytes.len() >= root_end);
            copy_range_core(&bytes, root_start, root_end)
        } else {
            Vec::new()
        };

        let content_start = root_end;
        assert(layout.content_offset == content_start as u64);
        assert(bytes.len() >= content_start);
        let content = copy_range_core(&bytes, content_start, bytes.len());
        LinkDecodedBytesCore {
            layout,
            prev_present,
            prev_bytes,
            root_present,
            root_bytes,
            content,
        }
    }
}

pub fn append_bytes_core(prefix: Vec<u8>, bytes: Vec<u8>) -> (out: Vec<u8>)
    ensures
        out@ == prefix@.add(bytes@),
{
    let mut out = prefix;
    let mut i: usize = 0;
    while i < bytes.len()
        invariant
            i <= bytes.len(),
            out@ == prefix@.add(bytes@.subrange(0, i as int)),
        decreases bytes.len() - i
    {
        out.push(bytes[i]);
        i += 1;
    }
    out
}

pub fn link_encode_bytes_core(
    prev_present: bool,
    prev_bytes: Vec<u8>,
    root_present: bool,
    root_bytes: Vec<u8>,
    content: Vec<u8>,
) -> (out: Vec<u8>)
    requires
        valid_optional_id_segment(prev_present, prev_bytes@),
        valid_optional_id_segment(root_present, root_bytes@),
    ensures
        out@ == link_encoded_bytes_spec(
            prev_present,
            prev_bytes@,
            root_present,
            root_bytes@,
            content@,
        ),
        canonical_link_bytes_spec(
            out@,
            prev_present,
            prev_bytes@,
            root_present,
            root_bytes@,
            content@,
    ),
{
    let mut out = Vec::new();
    out.push(1);
    out.push(if prev_present { 1 } else { 0 });
    let prev_segment = if prev_present {
        prev_bytes
    } else {
        Vec::new()
    };
    out = append_bytes_core(out, prev_segment);
    out.push(if root_present { 1 } else { 0 });
    let root_segment = if root_present {
        root_bytes
    } else {
        Vec::new()
    };
    out = append_bytes_core(out, root_segment);
    append_bytes_core(out, content)
}

// 17. Projected Id Vector Kernels.

#[allow(clippy::vec_init_then_push)]
pub fn singleton_projected_ids_core(self_id: IdCore) -> (ids: Vec<IdCore>)
    ensures
        ids@ == singleton_projected_ids_spec(self_id),
        ids@.len() == 1,
        ids@[0] == self_id,
{
    let mut ids = Vec::new();
    ids.push(self_id);
    ids
}

pub fn child_projected_ids_core(parent_ids: Vec<IdCore>, self_id: IdCore) -> (ids: Vec<IdCore>)
    ensures
        ids@ == child_projected_ids_spec(parent_ids@, self_id),
        ids@.len() == parent_ids@.len() + 1,
        ids@[parent_ids@.len() as int] == self_id,
        ids@.subrange(0, parent_ids@.len() as int) == parent_ids@,
{
    let mut ids = parent_ids;
    ids.push(self_id);
    ids
}

// 18. Projected Report Kernel.

#[allow(clippy::unnecessary_cast)]
pub fn projected_report_core(
    link: LinkCore,
    validity: ValidityCore,
    parent: ParentReportCore,
) -> (report: ProjectedReportCore)
    ensures
        report == projected_report_spec(link, validity, parent),
        is_root(link) && validity == ValidityCore::Valid ==> root_report_is_complete_self(report, link),
        is_child(link) && validity == ValidityCore::Valid && report.complete
            ==> complete_child_report_extends_parent(report, parent, link),
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
            if parent.present
                && parent.complete
                && id_eq(parent.root, root)
                && parent.depth < u64::MAX
                && parent.length < u64::MAX
                && parent.ids_len < u64::MAX
            {
                ProjectedReportCore {
                    complete: true,
                    root,
                    depth: (parent.depth + 1) as u64,
                    length: (parent.length + 1) as u64,
                    ids_len: (parent.ids_len + 1) as u64,
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

// 19. Projection Validity Kernel.

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
    match link_shape_core(link) {
        LinkShapeCore::Root => LinkProjectionCore {
            validity: ValidityCore::Valid,
            update_owner: link.self_id,
            statement: link_statement_core(link.self_id, link.self_id),
        },
        LinkShapeCore::Child(_parent, root) => {
            if parent_validated_same_root {
                LinkProjectionCore {
                    validity: ValidityCore::Valid,
                    update_owner: link.self_id,
                    statement: link_statement_core(link.self_id, root),
                }
            } else {
                LinkProjectionCore {
                    validity: ValidityCore::Invalid,
                    update_owner: link.self_id,
                    statement: MaybeStatementCore::None,
                }
            }
        }
        LinkShapeCore::Malformed => LinkProjectionCore {
            validity: ValidityCore::Invalid,
            update_owner: link.self_id,
            statement: MaybeStatementCore::None,
        },
    }
}

// 20. Emitted-Fact Kernel.
//
// Link projection currently emits no raw facts; authority comes only from
// promoted offers for the projected owner.

pub fn link_emitted_fact_count_core() -> (count: usize)
    ensures
        count == 0,
{
    0
}

// 21. Projection Lemmas.

pub proof fn root_projection_emits_self_root(link: LinkCore)
    requires
        is_root(link),
    ensures
        projection_spec(link, false).validity == ValidityCore::Valid,
        statement_is_self_root(projection_spec(link, false).statement, link.self_id),
{
}

// 22. Output Ownership Lemmas.

pub proof fn projection_update_owner_is_self(link: LinkCore, parent_validated_same_root: bool)
    ensures
        projection_spec(link, parent_validated_same_root).update_owner == link.self_id,
{
}

// 23. Construction Lemma.

pub proof fn link_from_params_constructs_only_link_fields(prev: MaybeIdCore, root: MaybeIdCore)
    ensures
        link_from_params_spec(prev, root).prev == prev,
        link_from_params_spec(prev, root).root == root,
        !link_from_params_spec(prev, root).assigns_id,
        !link_from_params_spec(prev, root).assigns_edges,
        !link_from_params_spec(prev, root).assigns_validity,
{
}

// 24. Update Application Lemma.

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

// 25. Codec Lemmas.

pub proof fn canonical_link_bytes_round_trip(
    prev_present: bool,
    prev_bytes: Seq<u8>,
    root_present: bool,
    root_bytes: Seq<u8>,
    content: Seq<u8>,
)
    requires
        valid_optional_id_segment(prev_present, prev_bytes),
        valid_optional_id_segment(root_present, root_bytes),
    ensures
        canonical_link_bytes_spec(
            link_encoded_bytes_spec(
                prev_present,
                prev_bytes,
                root_present,
                root_bytes,
                content,
            ),
            prev_present,
            prev_bytes,
            root_present,
            root_bytes,
            content,
        ),
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

pub proof fn decode_header_accepts_only_canonical_layout(bytes: Seq<u8>)
    ensures
        link_decode_header_spec(bytes).accepted ==> link_decode_header_spec(bytes).tag == tag_link_core(),
        link_decode_header_spec(bytes).accepted ==> valid_codec_flag(link_decode_header_spec(bytes).prev_flag),
        link_decode_header_spec(bytes).accepted ==> valid_codec_flag(link_decode_header_spec(bytes).root_flag),
        link_decode_header_spec(bytes).accepted ==> link_decode_header_spec(bytes).input_len
            >= link_decode_header_spec(bytes).content_offset,
{
}

// 26. Projected Id Vector Lemmas.

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

// 27. Composition Lemmas.

pub proof fn root_link_chain_to_anchor(link: LinkCore)
    requires
        is_root(link),
        projection_spec(link, false).validity == ValidityCore::Valid,
    ensures
        link_chain_to_anchor(seq![link], link.self_id),
{
}

pub proof fn child_extends_link_chain(
    parent_chain: Seq<LinkCore>,
    child: LinkCore,
    root: IdCore,
)
    requires
        link_chain_to_anchor(parent_chain, root),
        is_child(child),
        child.prev == MaybeIdCore::Some(chain_head_id(parent_chain)),
        child.root == MaybeIdCore::Some(root),
        projection_spec(child, true).validity == ValidityCore::Valid,
    ensures
        link_chain_to_anchor(parent_chain.push(child), root),
{
    assert(parent_chain.len() > 0);
    assert(parent_chain.push(child).len() > 1);
    assert(parent_chain.push(child).subrange(0, parent_chain.push(child).len() - 1) =~= parent_chain);
    assert(parent_chain.push(child)[parent_chain.push(child).len() - 1] == child);
}

pub proof fn replay_preserves_supplied_link_chain_to_anchor(
    state: crate::core::engine::EngineStateCore,
    transitions: Seq<crate::core::engine::EngineTransitionCore>,
    chain: Seq<LinkCore>,
    root: IdCore,
)
    requires
        crate::core::engine::engine_invariant(state),
        crate::core::engine::transition_trace_preconditions(state, transitions),
        link_chain_to_anchor(chain, root),
        chain_contains_validated_link_statements(
            crate::core::engine::apply_transition_trace(state, transitions),
            chain,
            root,
        ),
    ensures
        crate::core::engine::engine_invariant(crate::core::engine::apply_transition_trace(
            state,
            transitions,
        )),
        link_chain_to_anchor(chain, root),
        chain_contains_validated_link_statements(
            crate::core::engine::apply_transition_trace(state, transitions),
            chain,
            root,
        ),
{
    crate::core::play::replay_reports_engine_validity(state, transitions);
}

// 28. Extraction Lemmas.

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

// 29. Projection Statement Lemmas.

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

pub proof fn validated_link_offer_statement_to_owner_from_engine(
    state: crate::core::engine::EngineStateCore,
    link_id: IdCore,
    root_id: IdCore,
)
    requires
        crate::core::engine::engine_invariant(state),
        crate::core::engine::validated_offer_for(
            state.validated,
            engine_id_for_link_id(link_id),
            link_statement_addr_core(link_id, root_id),
        ),
    ensures
        crate::core::engine::contains_id(state.valid, engine_id_for_link_id(link_id)),
        crate::core::engine::asserted_offer_for(
            state.asserted,
            engine_id_for_link_id(link_id),
            link_statement_addr_core(link_id, root_id),
        ),
{
    let owner = engine_id_for_link_id(link_id);
    let addr = link_statement_addr_core(link_id, root_id);
    crate::core::engine::engine_validated_offer_for_has_valid_owner(state, owner, addr);
}

// 30. Malformed Shape Lemmas.

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

// 31. Projected Report Lemmas.

pub proof fn root_projected_report_is_complete_self(link: LinkCore)
    requires
        is_root(link),
    ensures
        root_report_is_complete_self(
            projected_report_spec(link, ValidityCore::Valid, absent_parent_report_spec(link.self_id)),
            link,
        ),
{
}

pub proof fn complete_child_report_requires_complete_same_root_parent(
    link: LinkCore,
    parent: ParentReportCore,
)
    requires
        is_child(link),
        projected_report_spec(link, ValidityCore::Valid, parent).complete,
    ensures
        complete_child_report_extends_parent(
            projected_report_spec(link, ValidityCore::Valid, parent),
            parent,
            link,
        ),
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

// 32. Runtime Construction.
//
// Primary runtime function: `link_from_params`.
// Proof handlers: `link_from_params_spec`, `link_from_params_core`, and
// `link_from_params_constructs_only_link_fields`.

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

// 33. Runtime Canonical Codec.
//
// Primary runtime functions: `LinkProjector::encode`, `LinkProjector::decode`,
// and `link_id`.
// Proof handlers: `codec_flag_*`, `link_codec_layout_*`, and the codec layout
// rejection lemmas.

pub fn link_id(l: &Link) -> FactId {
    fact_id(&LinkProjector::encode(l))
}

// 34. Runtime Extraction.
//
// Primary runtime functions: `link_edges`, `link_semantic_root`, and
// `valid_link_key`.
// Proof handlers: `extraction_spec`, `extract_link_core`,
// `child_extraction_offer_and_need_same_root`, and
// `malformed_extraction_is_empty`.

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

// 35. Runtime Projection Validity.
//
// Primary runtime functions: `LinkProjector::project`, `link_project_decision`,
// and `link_project_validity`.
// Proof handlers: `projection_spec`, `project_link_core`, and the root/child/
// malformed projection lemmas.

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

// 36. Runtime Output And Read Model.
//
// Primary runtime functions: `projected_link_state`,
// `incomplete_projected_link`, `LinkProjector::update_owner`, and
// `LinkProjector::apply_update`.
// Proof handlers: `projected_report_*`, `link_update_apply_*`,
// `singleton_projected_ids_*`, `child_projected_ids_*`, and
// `link_emitted_fact_count_core`.

fn projected_root_or_fallback(id: FactId, l: &Link) -> FactId {
    link_semantic_root(l).or(l.root).unwrap_or(id)
}

fn projected_ids_singleton(id: FactId) -> Vec<FactId> {
    let core_ids = singleton_projected_ids_core(fact_id_to_core(id));
    debug_assert_eq!(core_ids.len(), 1);
    debug_assert_eq!(core_ids.first().copied().map(core_to_fact_id), Some(id));
    core_ids.into_iter().map(core_to_fact_id).collect()
}

fn projected_ids_child(parent_ids: &[FactId], id: FactId) -> Vec<FactId> {
    let parent_core_ids: Vec<IdCore> = parent_ids.iter().copied().map(fact_id_to_core).collect();
    let parent_len = parent_core_ids.len();
    let core_ids = child_projected_ids_core(parent_core_ids, fact_id_to_core(id));
    debug_assert_eq!(core_ids.len(), parent_len + 1);
    debug_assert_eq!(core_ids.last().copied().map(core_to_fact_id), Some(id));
    core_ids.into_iter().map(core_to_fact_id).collect()
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
                ParentReportCore {
                    present: false,
                    complete: false,
                    root: fact_id_to_core(id),
                    depth: 0,
                    length: 0,
                    ids_len: 0,
                },
            );
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
                ParentReportCore {
                    present: true,
                    complete: parent_state.complete,
                    root: fact_id_to_core(parent_state.root),
                    depth: parent_state.depth,
                    length: parent_state.length,
                    ids_len: parent_ids_len,
                },
            );
            if !report.complete {
                return incomplete_projected_link(id, l);
            }
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

fn optional_id_bytes(id: Option<FactId>) -> Vec<u8> {
    id.map(|id| id.to_vec()).unwrap_or_default()
}

// 37. Projector Trait Wiring.
//
// The trait methods are the runtime entry points. Each method delegates to the
// sectioned helpers above so the implementation reads in the same order as the
// policy: codec, extraction, projection, and owner-scoped state update.

impl Projector for LinkProjector {
    type Item = Link;
    type State = LinkState;
    type Update = LinkUpdate;

    // Canonical layout: tag | has_prev | prev[32]? | has_root | root[32]? | content.
    fn encode(l: &Link) -> Vec<u8> {
        let prev_flag = codec_flag_core(maybe_fact_id_to_core(l.prev));
        let root_flag = codec_flag_core(maybe_fact_id_to_core(l.root));
        let b = link_encode_bytes_core(
            l.prev.is_some(),
            optional_id_bytes(l.prev),
            l.root.is_some(),
            optional_id_bytes(l.root),
            l.content.clone(),
        );
        let input_len = u64::try_from(b.len()).unwrap_or(u64::MAX);
        let layout = link_codec_layout_core(TAG_LINK, prev_flag, root_flag, input_len);
        debug_assert!(layout.accepted);
        b
    }

    fn decode(b: &[u8]) -> Result<Link, String> {
        if b.first() != Some(&TAG_LINK) {
            return Err("not a link fact".to_string());
        }
        let decoded = link_decode_bytes_core(b.to_vec());
        if !decoded.layout.accepted {
            return Err("malformed link codec".to_string());
        }
        let prev = if decoded.prev_present {
            let p: FactId = decoded
                .prev_bytes
                .as_slice()
                .try_into()
                .map_err(|_| "bad prev".to_string())?;
            Some(p)
        } else {
            None
        };
        let root = if decoded.root_present {
            let root: FactId = decoded
                .root_bytes
                .as_slice()
                .try_into()
                .map_err(|_| "bad root".to_string())?;
            Some(root)
        } else {
            None
        };
        debug_assert_eq!(
            codec_flag_core(maybe_fact_id_to_core(prev)),
            decoded.layout.prev_flag
        );
        debug_assert_eq!(
            codec_flag_core(maybe_fact_id_to_core(root)),
            decoded.layout.root_flag
        );
        Ok(Link {
            prev,
            root,
            content: decoded.content,
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

// 38. Runtime Bridge Helpers.
//
// Runtime code uses `[u8; 32]` fact ids and core typestates. The proof kernels
// use small proof-facing ids and enums. These conversions keep that boundary
// explicit and local to this file.

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
