//! poc-11 "link toy": a minimal, functional proof-of-model for the in-memory
//! projection / bounded-replay design (see
//! `../poc-10/docs/research/in-memory-projection-bounded-replay.md`).
//!
//! poc-11 keeps poc-10's generic-core / fact-family division:
//!  - [`core`] — the protocol-agnostic runtime + playback: content addressing, the
//!    `Offer<V>` typestate, the `Projector` trait, admission (Pass 1), the persisted
//!    index, playback (`play`, Pass 2), transport, and the daemon runtime.
//!  - [`facts`] — fact families and their projectors (here, [`facts::link`]).
//!  - [`cli`] — the thin app layer wiring a fact family into the core runtime.
//!
//! The model in one screen: the durable side is a content-addressed fact log plus a
//! *syntactic* needs/offers index (`extract`, Pass 1). The in-memory side is
//! validated read-model state (`project`, Pass 2), rebuilt on demand by [`core::play`]
//! as an explicit admit/project/query worklist. Projectors are pure over
//! `(item, context)` and never receive a storage/IO handle.
pub mod cli_unproven;
pub mod core;
pub mod facts;
pub mod helpers;

pub use cli_unproven as cli;
