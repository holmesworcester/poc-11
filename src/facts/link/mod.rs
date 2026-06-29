//! Link fact family, kept in poc-10-style family-directory shape.
//!
//! Invariant checklist (Verus):
//! - [ ] This module remains re-export-only and contains no link semantics.
//! - [ ] Link semantics stay in `project`; authoring/reporting/CLI files may call
//!       it but must not duplicate projection rules.
//! - [ ] The family directory remains the locus for all link-specific proof
//!       obligations.
pub mod api_unproven;
pub mod author_unproven;
pub mod cli_unproven;
pub mod project_unproven;

pub use api_unproven::{chain_report, Report};
pub use author_unproven::{author, Authored};
pub use project_unproven::{
    link_edges, link_id, link_project_validity, Link, LinkProjector, LinkState, LINK, TAG_LINK,
};
