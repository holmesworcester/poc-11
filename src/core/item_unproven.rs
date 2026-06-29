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
//! Imported theorems:
//! - None. This file is the root assumption for content-addressed identity.
//! Proof strategy:
//! - Model `FactId` as a 32-byte value and `fact_id(bytes)` as an uninterpreted,
//!   deterministic, collision-resistant function over canonical byte strings.
//! - Keep hex helpers out of proven paths; prove display/input round trips only
//!   as app-boundary tests, not as validity evidence.

pub type FactId = [u8; 32];

pub use crate::helpers::crypto_unproven::fact_id;
pub use crate::helpers::hex_unproven::{from_hex, to_hex};
