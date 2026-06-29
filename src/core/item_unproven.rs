//! Items and content addressing. A *fact* is a durable, content-addressed item:
//! its id is the hash of its canonical bytes (mirrors poc-10 `fact_id`).
//!
//! Invariant checklist (Verus):
//! - [ ] `FactId` is exactly a fixed 32-byte content address.
//! - [ ] All fact ids used by proven code are derived from canonical bytes.
//! - [ ] The proof treats `fact_id` as deterministic and collision-free for
//!       accepted canonical bytes.
//! - [ ] Hex parsing/formatting never participates in validity; it is only an
//!       app-facing representation boundary.

pub type FactId = [u8; 32];

pub use crate::helpers::crypto_unproven::fact_id;
pub use crate::helpers::hex_unproven::{from_hex, to_hex};
