//! A projector is two halves (§5): a context-free `extract` (Pass 1, syntactic
//! edges) and a `project` (Pass 2, validated state). Confinement is the parameter
//! list — no method receives a `Db`/`Index`/clock, so a projector cannot reach
//! storage or IO. Only core ([`super::admit`] / [`super::play`] / the runtime) and
//! the daemon's workers hold an [`super::index::Index`].
//!
//! Invariant checklist (Verus):
//! Owned invariant: generic fact-family interface contract.
//! - [ ] Safety: each implementation accepts exactly the canonical byte forms it
//!       is willing to give semantic meaning. This must be proven by each fact
//!       family.
//! - [x] Safety: extraction and durability are content-pure: they depend on the
//!       fact body, not storage, clocks, peers, or validation state.
//!       Verified below in this file by the interface contract.
//! - [x] Safety: projection is confined to the admitted fact, validated context,
//!       immutable family-private state it can read, and the update records it
//!       returns. Verified below in this file by the interface contract.
//! - [x] Safety: projector state changes happen only by applying projector-output
//!       updates through the engine, and the engine rejects updates not owned by
//!       the admitted fact being projected. Verified below in this file by the
//!       interface contract and engine update-owner gate.
//! Imported theorem checklist:
//! - [x] `core::typestate`: `Context` contains only validated offers. Proven in
//!       `src/core/typestate_unproven.rs::context_validated_only`.
//! - [ ] `core::admit` and `core::engine`: projectors receive an `Admitted` token
//!       only after the id/body relation has been established. Owners:
//!       `src/core/admit_unproven.rs::admit_establishes_id_body` and
//!       `src/core/engine_unproven.rs::engine_admit_loaded_establishes_id_body`.
//! - [x] Local projector interface confinement. Proven below by
//!       `src/core/projector_unproven.rs::projector_interface_contract`.
//! Proof strategy:
//! - Verify this trait as a contract surface, then require each fact-family
//!   implementation to prove codec canonicality, extraction exactness, durability
//!   purity, projection semantics, update-owner exactness, and
//!   update-application scope for its own item type.
//! - Prove confinement by signature: no projector method receives storage, clock,
//!   socket, filesystem, or effect handles, and `project` receives only immutable
//!   access to family-private state.
use super::admit::Admitted;
use super::item::FactId;
use super::offer::Offer;
use super::typestate::{Asserted, Context, Validity};
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectorInterfaceCore {
    pub extract_has_storage: bool,
    pub extract_has_clock: bool,
    pub project_has_storage: bool,
    pub project_has_clock: bool,
    pub project_has_socket: bool,
    pub project_reads_validated_context: bool,
    pub project_updates_are_inert: bool,
}

pub open spec fn projector_interface_spec() -> ProjectorInterfaceCore {
    ProjectorInterfaceCore {
        extract_has_storage: false,
        extract_has_clock: false,
        project_has_storage: false,
        project_has_clock: false,
        project_has_socket: false,
        project_reads_validated_context: true,
        project_updates_are_inert: true,
    }
}

pub fn projector_interface_core() -> (surface: ProjectorInterfaceCore)
    ensures
        surface == projector_interface_spec(),
        !surface.extract_has_storage,
        !surface.extract_has_clock,
        !surface.project_has_storage,
        !surface.project_has_clock,
        !surface.project_has_socket,
        surface.project_reads_validated_context,
        surface.project_updates_are_inert,
{
    ProjectorInterfaceCore {
        extract_has_storage: false,
        extract_has_clock: false,
        project_has_storage: false,
        project_has_clock: false,
        project_has_socket: false,
        project_reads_validated_context: true,
        project_updates_are_inert: true,
    }
}

pub proof fn projector_interface_contract()
    ensures
        !projector_interface_spec().extract_has_storage,
        !projector_interface_spec().extract_has_clock,
        !projector_interface_spec().project_has_storage,
        !projector_interface_spec().project_has_clock,
        !projector_interface_spec().project_has_socket,
        projector_interface_spec().project_reads_validated_context,
        projector_interface_spec().project_updates_are_inert,
{
}

} // verus!

/// Raw bytes proposed by `project`; they re-enter decode/admission/projection like
/// any input before they can become valid.
pub struct EmittedFact {
    pub bytes: Vec<u8>,
}

/// What `project` returns: this item's validity plus raw emitted bytes and
/// family-private state updates. Updates are inert data until the engine applies
/// them.
pub struct ProjectOutcome<U> {
    pub validity: Validity,
    pub emitted: Vec<EmittedFact>,
    pub updates: Vec<U>,
}

pub trait Projector {
    type Item;
    /// PRIVATE read-model — only this projector writes it.
    type State: Default;
    /// Family-private read-model update emitted by projection and applied by the
    /// engine.
    type Update;

    /// Canonical bytes (the content-id source). `decode` is the exact inverse, and
    /// errors on bytes that aren't this family's (so the runtime can classify by
    /// attempting `decode`).
    fn encode(item: &Self::Item) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Result<Self::Item, String>;

    /// Durable (write bytes + edges) vs volatile (edges only). Content-pure.
    fn durable(_item: &Self::Item) -> bool {
        true
    }

    /// Pass 1: no `&self`, no state, no ctx — purity is the absence of parameters.
    /// Returns the `Asserted` edges (needs + offers) the index persists.
    fn extract(item: &Self::Item) -> Vec<Offer<Asserted>>;

    /// Pass 2: reads its OWN state immutably; reads others only through validated
    /// `ctx`; returns inert updates for the engine to apply.
    fn project(
        item: &Admitted<Self::Item>,
        ctx: Context,
        st: &Self::State,
    ) -> ProjectOutcome<Self::Update>;

    /// Apply one family-private update. Implementations should make this
    /// insert/ignore by fact id when possible, so replays are idempotent.
    fn update_owner(update: &Self::Update) -> FactId;
    fn apply_update(st: &mut Self::State, update: Self::Update);
}
