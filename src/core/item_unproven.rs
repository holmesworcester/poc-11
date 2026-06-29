//! Items and content addressing. A *fact* is a durable, content-addressed item:
//! its id is the hash of its canonical bytes (mirrors poc-10 `fact_id`).

pub type FactId = [u8; 32];

pub use crate::helpers::crypto_unproven::fact_id;
pub use crate::helpers::hex_unproven::{from_hex, to_hex};
