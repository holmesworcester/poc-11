//! Link fact family, kept in poc-10-style family-directory shape.
//!
//! Fact-family contract (do not weaken):
//! - `project` is the only link module that defines link semantics: codec,
//!   extraction, projection, root/domain meaning, and link-specific theorems.
//! - `api` is only an observation/report layer. It may call replay and read the
//!   resulting projector-owned state, but it must not construct, admit, project,
//!   or walk persisted fact bytes directly.
//! - `cli` is the unproven app adapter. It may call project-owned deterministic
//!   constructors and core admission, but it must not define link semantics or
//!   create proof evidence.
//! - This contract is part of the proof plan. Do not weaken or move these
//!   responsibilities without updating the proof plan and the source-contract
//!   tests in `tests/documentation.rs`.
//!
//! Invariant checklist (Verus):
//! Owned invariant: link family module shape.
//! - [ ] Safety: all link-specific meaning lives in `project_unproven` until
//!       byte codec, exact report vector contents, and end-to-end core/replay
//!       composition are proved and the file can use the unsuffixed name.
//! - [ ] Safety: app/report modules cannot define link semantics or proof
//!       evidence.
//! - [ ] Safety: this module is re-export-only; it adds no behavior to prove.
//! Imported theorem checklist:
//! - [ ] `facts::link::project_unproven` owns link-family semantics and has
//!       local proven kernels, but still owns pending proof gaps. Owner:
//!       `src/facts/link/project_unproven.rs`.
//! - [ ] `facts::link::api_unproven` and `facts::link::cli_unproven` own their
//!       adapter/reporting local invariants. Owners:
//!       `src/facts/link/api_unproven.rs`, and
//!       `src/facts/link/cli_unproven.rs`.
//! Proof strategy:
//! - Prove by source inspection/contract test that this file contains only module
//!   declarations and re-exports.
//! - Prove no functions or data constructors are defined here.
pub mod api_unproven;
pub mod cli_unproven;
pub mod project_unproven;

pub use api_unproven::{chain_report, Report};
pub use project_unproven::{
    link_edges, link_from_params, link_id, link_project_validity, link_semantic_root,
    valid_link_key, Link, LinkProjector, LinkState, ProjectedLink, LINK, TAG_LINK,
};
