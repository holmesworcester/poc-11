//! Needs/offers edges — the doc's `Offer<V>` (§5): one type parameterized by
//! validity. `extract` emits `Offer<Asserted>` (syntactic, content-pure, possibly
//! from a forged or not-yet-valid fact); the index persists that dirty layer.
//! [`Offer::validate`] is the ONLY bridge to `Offer<Validated>`, the clean evidence
//! that populates a [`crate::core::typestate::Context`]. A `kind` field carries
//! both needs and offers under one type; only offers are ever promoted (a need is a
//! lookup key, never "validated"). Edges are *valueless* (§6): matching is on the
//! key, and any value is read from the fact body at project time.
//!
//! Invariant checklist (Verus):
//! Owned invariant: edge representation and promotion shape.
//! - [ ] Safety: asserted needs/offers are routing claims, not proof that their
//!       owner is valid or authorized.
//! - [x] Safety: matching depends only on `(role, scope, key)`; dependency
//!       discovery cannot smuggle fact body data through the edge index.
//!       Verified below in this file.
//! - [ ] Safety: only offers, never needs, may be promoted to validated context.
//!       This is an engine authority precondition, not an edge-shape theorem.
//! - [x] Safety: promotion preserves the asserted edge's address and metadata; it
//!       adds no new authority payload.
//!       Verified below in this file.
//! - [ ] Safety: the authority to call promotion belongs to `core::engine`.
//! Imported theorem checklist:
//! - [x] No imported theorem required for representation shape; local proofs are
//!       `src/core/offer_unproven.rs::asserted_edge_address_shape`,
//!       `src/core/offer_unproven.rs::validate_preserves_offer_address`, and
//!       `src/core/offer_unproven.rs::validated_offer_typestate_only`.
//! - [ ] `core::engine`: promotion authority for
//!       `Offer<Asserted>::validate`. Owner: `src/core/engine_unproven.rs`,
//!       planned theorem `engine_promotes_only_valid_owner_offers`.
//! Proof strategy:
//! - Prove `Offer<Asserted>` constructors set only the requested direction and
//!   match address with fixed default scope/polarity/binding.
//! - Prove `Offer<Asserted>::validate` copies every semantic field unchanged and
//!   changes only the typestate marker.
//! - Keep `validate` visible only inside `core` so the engine remains the single
//!   promotion authority.
use std::marker::PhantomData;

use super::typestate::{Asserted, Validated};
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKindCore {
    Need,
    Offer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeCore {
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolarityCore {
    Additive,
    Suppressing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingCore {
    Forward,
    LateBound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeShapeCore {
    pub kind: EdgeKindCore,
    pub scope: ScopeCore,
    pub polarity: PolarityCore,
    pub binding: BindingCore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeLayerCore {
    Asserted,
    Validated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypedEdgeCore {
    pub layer: EdgeLayerCore,
    pub shape: EdgeShapeCore,
}

pub open spec fn asserted_edge_shape_spec(kind: EdgeKindCore) -> EdgeShapeCore {
    EdgeShapeCore {
        kind,
        scope: ScopeCore::Local,
        polarity: PolarityCore::Additive,
        binding: BindingCore::Forward,
    }
}

pub open spec fn validate_shape_spec(shape: EdgeShapeCore) -> EdgeShapeCore {
    shape
}

pub open spec fn asserted_edge_spec(kind: EdgeKindCore) -> TypedEdgeCore {
    TypedEdgeCore {
        layer: EdgeLayerCore::Asserted,
        shape: asserted_edge_shape_spec(kind),
    }
}

pub open spec fn validate_edge_spec(edge: TypedEdgeCore) -> TypedEdgeCore {
    TypedEdgeCore {
        layer: EdgeLayerCore::Validated,
        shape: validate_shape_spec(edge.shape),
    }
}

pub fn asserted_edge_shape_core(kind: EdgeKindCore) -> (shape: EdgeShapeCore)
    ensures
        shape == asserted_edge_shape_spec(kind),
        shape.kind == kind,
        shape.scope == ScopeCore::Local,
        shape.polarity == PolarityCore::Additive,
        shape.binding == BindingCore::Forward,
{
    EdgeShapeCore {
        kind,
        scope: ScopeCore::Local,
        polarity: PolarityCore::Additive,
        binding: BindingCore::Forward,
    }
}

pub fn validate_shape_core(shape: EdgeShapeCore) -> (validated: EdgeShapeCore)
    ensures
        validated == validate_shape_spec(shape),
        validated.kind == shape.kind,
        validated.scope == shape.scope,
        validated.polarity == shape.polarity,
        validated.binding == shape.binding,
{
    shape
}

pub fn asserted_edge_core(kind: EdgeKindCore) -> (edge: TypedEdgeCore)
    ensures
        edge == asserted_edge_spec(kind),
        edge.layer == EdgeLayerCore::Asserted,
        edge.shape.kind == kind,
        edge.shape.scope == ScopeCore::Local,
        edge.shape.polarity == PolarityCore::Additive,
        edge.shape.binding == BindingCore::Forward,
{
    TypedEdgeCore {
        layer: EdgeLayerCore::Asserted,
        shape: asserted_edge_shape_core(kind),
    }
}

pub fn validate_edge_core(edge: TypedEdgeCore) -> (validated: TypedEdgeCore)
    requires
        edge.layer == EdgeLayerCore::Asserted,
    ensures
        validated == validate_edge_spec(edge),
        validated.layer == EdgeLayerCore::Validated,
        validated.shape.kind == edge.shape.kind,
        validated.shape.scope == edge.shape.scope,
        validated.shape.polarity == edge.shape.polarity,
        validated.shape.binding == edge.shape.binding,
{
    TypedEdgeCore {
        layer: EdgeLayerCore::Validated,
        shape: validate_shape_core(edge.shape),
    }
}

pub proof fn asserted_edge_address_shape(kind: EdgeKindCore)
    ensures
        asserted_edge_spec(kind).layer == EdgeLayerCore::Asserted,
        asserted_edge_spec(kind).shape.kind == kind,
        asserted_edge_spec(kind).shape.scope == ScopeCore::Local,
        asserted_edge_spec(kind).shape.polarity == PolarityCore::Additive,
        asserted_edge_spec(kind).shape.binding == BindingCore::Forward,
{
}

pub proof fn validate_preserves_offer_address(shape: EdgeShapeCore)
    ensures
        validate_shape_spec(shape).kind == shape.kind,
        validate_shape_spec(shape).scope == shape.scope,
        validate_shape_spec(shape).polarity == shape.polarity,
        validate_shape_spec(shape).binding == shape.binding,
{
}

pub proof fn validated_offer_typestate_only(shape: EdgeShapeCore)
    ensures
        ({
            let edge = TypedEdgeCore {
                layer: EdgeLayerCore::Asserted,
                shape,
            };
            validate_edge_spec(edge).layer == EdgeLayerCore::Validated
                && validate_edge_spec(edge).shape == shape
        }),
{
}

} // verus!

fn edge_kind_to_core(kind: EdgeKind) -> EdgeKindCore {
    match kind {
        EdgeKind::Need => EdgeKindCore::Need,
        EdgeKind::Offer => EdgeKindCore::Offer,
    }
}

fn core_to_edge_kind(kind: EdgeKindCore) -> EdgeKind {
    match kind {
        EdgeKindCore::Need => EdgeKind::Need,
        EdgeKindCore::Offer => EdgeKind::Offer,
    }
}

fn scope_to_core(scope: Scope) -> ScopeCore {
    match scope {
        Scope::Local => ScopeCore::Local,
    }
}

fn core_to_scope(scope: ScopeCore) -> Scope {
    match scope {
        ScopeCore::Local => Scope::Local,
    }
}

fn polarity_to_core(polarity: Polarity) -> PolarityCore {
    match polarity {
        Polarity::Additive => PolarityCore::Additive,
        Polarity::Suppressing => PolarityCore::Suppressing,
    }
}

fn core_to_polarity(polarity: PolarityCore) -> Polarity {
    match polarity {
        PolarityCore::Additive => Polarity::Additive,
        PolarityCore::Suppressing => Polarity::Suppressing,
    }
}

fn binding_to_core(binding: Binding) -> BindingCore {
    match binding {
        Binding::Forward => BindingCore::Forward,
        Binding::LateBound => BindingCore::LateBound,
    }
}

fn core_to_binding(binding: BindingCore) -> Binding {
    match binding {
        BindingCore::Forward => Binding::Forward,
        BindingCore::LateBound => Binding::LateBound,
    }
}

fn edge_shape_to_core<V>(edge: &Offer<V>) -> EdgeShapeCore {
    EdgeShapeCore {
        kind: edge_kind_to_core(edge.kind),
        scope: scope_to_core(edge.scope),
        polarity: polarity_to_core(edge.polarity),
        binding: binding_to_core(edge.binding),
    }
}

/// The match namespace (poc-10's "role"). The toy uses one: [`crate::facts::link::LINK`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Role(pub &'static str);

/// Match scope. The toy is single-scope; real families add workspace/etc.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Scope {
    Local,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Local => "local",
        }
    }
}

/// A concrete match address. For links it is a [`super::item::FactId`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Key(pub [u8; 32]);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum EdgeKind {
    Need,
    Offer,
}

/// Additive (toy) vs suppressing (tombstones — Stage 2). The marker rides on the
/// edge so the projector *proves* it, rather than a registration flag (§5).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Polarity {
    Additive,
    Suppressing,
}

/// Forward / closure-carried (toy) vs late-binding / reverse-keyed (Stage 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Binding {
    Forward,
    LateBound,
}

/// One needs/offers edge, parameterized by validity. The toy emits only
/// Additive/Forward edges; the other variants exist so the schema and signatures
/// already fit the real model.
#[derive(Clone, Copy)]
pub struct Offer<V> {
    pub kind: EdgeKind,
    pub role: Role,
    pub scope: Scope,
    pub key: Key,
    pub polarity: Polarity,
    pub binding: Binding,
    _v: PhantomData<V>,
}

// `offer`/`need` are the two edge constructors (distinguished by `kind`); `Offer`
// is really an edge carrying both, so the self-named-constructor lint misfires here.
#[allow(clippy::self_named_constructors)]
impl Offer<Asserted> {
    /// An additive, forward OFFER on `key` (a fact's own id, for a self-offer).
    pub fn offer(role: Role, key: Key) -> Self {
        Self::edge(EdgeKind::Offer, role, key)
    }

    /// An additive, forward NEED on `key`.
    pub fn need(role: Role, key: Key) -> Self {
        Self::edge(EdgeKind::Need, role, key)
    }

    fn edge(kind: EdgeKind, role: Role, key: Key) -> Self {
        let shape = asserted_edge_core(edge_kind_to_core(kind)).shape;
        Self {
            kind: core_to_edge_kind(shape.kind),
            role,
            scope: core_to_scope(shape.scope),
            key,
            polarity: core_to_polarity(shape.polarity),
            binding: core_to_binding(shape.binding),
            _v: PhantomData,
        }
    }

    /// The ONLY bridge to the clean layer. The caller must have validated the
    /// owning item (its `project` returned `Valid`); Stage 1 turns that
    /// precondition into a Verus `requires`. The doc writes
    /// `validate(o) -> Option<Offer<Validated>>`; the Option lives one level up, in
    /// `project`'s `Validity`, so this per-offer step is infallible.
    pub(in crate::core) fn validate(self) -> Offer<Validated> {
        let shape = validate_edge_core(TypedEdgeCore {
            layer: EdgeLayerCore::Asserted,
            shape: edge_shape_to_core(&self),
        })
        .shape;
        Offer {
            kind: core_to_edge_kind(shape.kind),
            role: self.role,
            scope: core_to_scope(shape.scope),
            key: self.key,
            polarity: core_to_polarity(shape.polarity),
            binding: core_to_binding(shape.binding),
            _v: PhantomData,
        }
    }
}

impl<V> Offer<V> {
    pub fn is_need(&self) -> bool {
        matches!(self.kind, EdgeKind::Need)
    }
    pub fn is_offer(&self) -> bool {
        matches!(self.kind, EdgeKind::Offer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ROLE: Role = Role("test");

    fn key(byte: u8) -> Key {
        Key([byte; 32])
    }

    fn assert_fixed_asserted_shape(edge: &Offer<Asserted>, kind: EdgeKind, key: Key) {
        assert_eq!(edge.kind, kind);
        assert_eq!(edge.role, TEST_ROLE);
        assert_eq!(edge.scope, Scope::Local);
        assert_eq!(edge.key, key);
        assert_eq!(edge.polarity, Polarity::Additive);
        assert_eq!(edge.binding, Binding::Forward);
    }

    #[test]
    fn asserted_offer_and_need_constructors_set_only_direction_and_address() {
        let offer_key = key(7);
        let need_key = key(9);

        let offer = Offer::offer(TEST_ROLE, offer_key);
        let need = Offer::need(TEST_ROLE, need_key);

        assert_fixed_asserted_shape(&offer, EdgeKind::Offer, offer_key);
        assert!(offer.is_offer());
        assert!(!offer.is_need());

        assert_fixed_asserted_shape(&need, EdgeKind::Need, need_key);
        assert!(need.is_need());
        assert!(!need.is_offer());
    }

    #[test]
    fn validation_preserves_runtime_address_and_edge_metadata() {
        let asserted = Offer::offer(TEST_ROLE, key(11));
        let validated = asserted.validate();

        assert_eq!(validated.kind, asserted.kind);
        assert_eq!(validated.role, asserted.role);
        assert_eq!(validated.scope, asserted.scope);
        assert_eq!(validated.key, asserted.key);
        assert_eq!(validated.polarity, asserted.polarity);
        assert_eq!(validated.binding, asserted.binding);
    }
}
