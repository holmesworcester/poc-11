//! Core: proof-targeted generic machinery. Current implementation files are
//! labeled `_unproven` until their invariants move into executable Verus kernels.
//! Public aliases preserve the existing API while making proof status visible in
//! the source tree.
//!
//! Invariant checklist (Verus):
//! Invariant owner: core module shape.
//! - [ ] This module has no fact, validity, or storage behavior of its own.
//! - [ ] Proof status stays visible: behavior-bearing core files keep `_unproven`
//!       until executable Verus proof covers their invariants.
//! - [ ] Unsuffixed core files are either proven executable code or thin wrappers
//!       around proven executable code.
pub mod admit_unproven;
pub mod effects_unproven;
pub mod engine_unproven;
pub mod index_unproven;
pub mod item_unproven;
pub mod offer_unproven;
pub mod play_unproven;
pub mod projector_unproven;
pub mod runtime_unproven;
pub mod turn_unproven;
pub mod typestate_unproven;

pub use admit_unproven as admit;
pub use effects_unproven as effects;
pub use engine_unproven as engine;
pub use index_unproven as index;
pub use item_unproven as item;
pub use offer_unproven as offer;
pub use play_unproven as play;
pub use projector_unproven as projector;
pub use runtime_unproven as runtime;
pub use turn_unproven as turn;
pub use typestate_unproven as typestate;
