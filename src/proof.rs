// Stage-0 placeholder proof. Proves the Verus pipeline runs end to end before
// the real link-projector proofs land in Stage 1. Verified standalone by
// scripts/run_verus.sh (`verus --crate-type=lib src/proof.rs`); this file is
// deliberately NOT in the cargo module tree, so `cargo build`/`cargo test`
// never see `verus!` syntax.
#![allow(unused)]
use vstd::prelude::*;

verus! {
    // If Verus can discharge this, the toolchain + runner are wired correctly.
    proof fn pipeline_ok() {
        assert(1 + 1 == 2);
    }
}
