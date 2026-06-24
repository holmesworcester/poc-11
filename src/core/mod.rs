//! Core: the protocol-agnostic **runtime and playback** (poc-10's core/protocol
//! division, carried into poc-11). Core supplies content addressing, the
//! `Offer<V>` typestate, the `Projector` trait, admission (Pass 1), the persisted
//! index, playback (`play`, Pass 2), transport, and the daemon runtime. It must
//! NOT know any item family — those live in [`crate::protocol`].
pub mod admit;
pub mod index;
pub mod item;
pub mod net;
pub mod offer;
pub mod play;
pub mod projector;
pub mod runtime;
pub mod typestate;
