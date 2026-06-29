//! Items and content addressing. A *fact* is a durable, content-addressed item:
//! its id is the blake3 hash of its canonical bytes (mirrors poc-10 `fact_id`).

pub type FactId = [u8; 32];

/// Content id = blake3 of the canonical bytes. Deterministic content addressing.
pub fn fact_id(bytes: &[u8]) -> FactId {
    *blake3::hash(bytes).as_bytes()
}

/// Lowercase-hex of a fact id (for `field: <hex>` CLI output and tests).
pub fn to_hex(id: &FactId) -> String {
    let mut s = String::with_capacity(64);
    for b in id {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Parse 64 hex chars back into a fact id.
pub fn from_hex(s: &str) -> Option<FactId> {
    let s = s.trim();
    if s.len() != 64 {
        return None;
    }
    let mut id = [0u8; 32];
    for (i, b) in id.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(id)
}
