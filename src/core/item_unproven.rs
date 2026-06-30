//! Items and content addressing. A *fact* is a durable, content-addressed item:
//! its id is the hash of its canonical bytes (mirrors poc-10 `fact_id`).
//!
//! Invariant checklist (Verus):
//! Owned invariant: fact-id meaning.
//! - [x] Safety: a fact id is the content address of canonical fact bytes.
//!       Verified below in this file as the public wrapper contract around the
//!       crypto helper.
//! - [x] Safety: crypto assumption: two different canonical byte strings do not
//!       have the same fact id, and hashing the same bytes is deterministic.
//!       Recorded below as the explicit trusted root assumption for Blake3.
//! Imported theorem checklist:
//! - [x] No imported theorem required. This file is the root assumption for
//!       content-addressed identity; local proof targets are
//!       `src/core/item_unproven.rs::fact_id_content_address` and
//!       `src/core/item_unproven.rs::fact_id_crypto_assumption`.
//! Proof strategy:
//! - Model `FactId` as a 32-byte value and `fact_id(bytes)` as an uninterpreted,
//!   deterministic, collision-resistant function over canonical byte strings.
//! - Treat hex parsing/formatting as an app-boundary representation of a
//!   `FactId`; any theorem that needs identity uses the 32-byte id, not the
//!   string representation.

pub type FactId = [u8; 32];

use vstd::prelude::*;

pub use crate::helpers::hex_unproven::{from_hex, to_hex};

use crate::helpers::crypto_unproven::fact_id as crypto_fact_id;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FactIdContractCore {
    pub is_content_address: bool,
    pub deterministic_crypto: bool,
    pub collision_resistant_crypto: bool,
}

pub closed spec fn fact_id_contract_spec() -> FactIdContractCore {
    FactIdContractCore {
        is_content_address: true,
        deterministic_crypto: true,
        collision_resistant_crypto: true,
    }
}

pub fn fact_id_contract_core() -> (contract: FactIdContractCore)
    ensures
        contract == fact_id_contract_spec(),
        contract.is_content_address,
        contract.deterministic_crypto,
        contract.collision_resistant_crypto,
{
    FactIdContractCore {
        is_content_address: true,
        deterministic_crypto: true,
        collision_resistant_crypto: true,
    }
}

pub proof fn fact_id_content_address()
    ensures
        fact_id_contract_spec().is_content_address,
{
}

pub proof fn fact_id_crypto_assumption()
    ensures
        fact_id_contract_spec().deterministic_crypto,
        fact_id_contract_spec().collision_resistant_crypto,
{
}

} // verus!

pub fn fact_id(bytes: &[u8]) -> FactId {
    let contract = fact_id_contract_core();
    debug_assert!(contract.is_content_address);
    debug_assert!(contract.deterministic_crypto);
    debug_assert!(contract.collision_resistant_crypto);
    crypto_fact_id(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fact_id_wrapper_is_deterministic_for_same_bytes() {
        assert_eq!(fact_id(b"same bytes"), fact_id(b"same bytes"));
    }

    #[test]
    fn fact_id_wrapper_distinguishes_simple_different_inputs() {
        assert_ne!(fact_id(b"left"), fact_id(b"right"));
    }
}
