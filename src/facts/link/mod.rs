//! Link fact family, kept in poc-10-style family-directory shape.
pub mod api_unproven;
pub mod author_unproven;
pub mod cli_unproven;
pub mod project_unproven;

pub use api_unproven::{chain_report, Report};
pub use author_unproven::{author, Authored};
pub use project_unproven::{
    link_edges, link_id, link_project_validity, Link, LinkProjector, LinkState, LINK, TAG_LINK,
};
