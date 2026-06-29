//! Cryptographic helper boundary. Proofs assume deterministic content hashes and
//! exclude collisions rather than proving blake3 itself.
use crate::core::item::FactId;

/// Content id = blake3 of canonical bytes.
pub fn fact_id(bytes: &[u8]) -> FactId {
    *blake3::hash(bytes).as_bytes()
}

/// Fixed-width role id for proof-facing edge addresses.
pub fn role_id(role: &str) -> FactId {
    *blake3::hash(role.as_bytes()).as_bytes()
}
