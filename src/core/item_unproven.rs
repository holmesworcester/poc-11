//! Items and content addressing. A *fact* is a durable, content-addressed item:
//! its id is the hash of its canonical bytes (mirrors poc-10 `fact_id`).
//!
//! Invariant checklist (Verus):
//! Owned invariant: fact-id meaning.
//! - [ ] A fact id is the content address of canonical fact bytes.
//! - [ ] Crypto assumption: two different canonical byte strings do not have the
//!       same fact id, and hashing the same bytes is deterministic.
//! - [ ] Hex is display/input syntax only; it is never evidence of validity,
//!       ownership, or authority.
//! - [ ] Other modules may depend on this theorem, but should prove only that
//!       they preserve the id/body relation at their own boundary.

pub type FactId = [u8; 32];

pub use crate::helpers::crypto_unproven::fact_id;
pub use crate::helpers::hex_unproven::{from_hex, to_hex};
