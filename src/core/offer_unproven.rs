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
//! Invariant owner: edge representation and promotion shape.
//! - [ ] Asserted needs/offers are routing claims, not proof that their owner is
//!       valid or authorized.
//! - [ ] Matching depends only on `(role, scope, key)`; dependency discovery
//!       cannot smuggle fact body data through the edge index.
//! - [ ] Only offers, never needs, have a representation that can be promoted to
//!       validated context.
//! - [ ] Promotion preserves the asserted edge's address and metadata; it adds no
//!       new authority payload.
//! - [ ] The authority to call promotion belongs to `core::engine`.
use std::marker::PhantomData;

use super::typestate::{Asserted, Validated};

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
        Self {
            kind,
            role,
            scope: Scope::Local,
            key,
            polarity: Polarity::Additive,
            binding: Binding::Forward,
            _v: PhantomData,
        }
    }

    /// The ONLY bridge to the clean layer. The caller must have validated the
    /// owning item (its `project` returned `Valid`); Stage 1 turns that
    /// precondition into a Verus `requires`. The doc writes
    /// `validate(o) -> Option<Offer<Validated>>`; the Option lives one level up, in
    /// `project`'s `Validity`, so this per-offer step is infallible.
    pub(in crate::core) fn validate(self) -> Offer<Validated> {
        Offer {
            kind: self.kind,
            role: self.role,
            scope: self.scope,
            key: self.key,
            polarity: self.polarity,
            binding: self.binding,
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
