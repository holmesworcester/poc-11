//! Items and content addressing. A *fact* is a durable, content-addressed item:
//! its id is the hash of its canonical bytes (mirrors poc-10 `fact_id`).
//!
//! Invariant checklist (Verus):
//! Owned invariant: fact-id meaning.
//! - [ ] Safety: a fact id is the content address of canonical fact bytes.
//! - [ ] Safety: crypto assumption: two different canonical byte strings do not
//!       have the same fact id, and hashing the same bytes is deterministic.
//! Imported theorem checklist:
//! - [x] No imported theorem required. This file is the root assumption for
//!       content-addressed identity; the local planned proof target is
//!       `src/core/item_unproven.rs::fact_id_content_address`.
//! Proof strategy:
//! - Model `FactId` as a 32-byte value and `fact_id(bytes)` as an uninterpreted,
//!   deterministic, collision-resistant function over canonical byte strings.
//! - Treat hex parsing/formatting as an app-boundary representation of a
//!   `FactId`; any theorem that needs identity uses the 32-byte id, not the
//!   string representation.

pub type FactId = [u8; 32];

pub use crate::helpers::crypto_unproven::fact_id;
pub use crate::helpers::hex_unproven::{from_hex, to_hex};
