//! Protocol: item families and their projectors (poc-10's core/protocol division).
//! The toy has one family, `link`. Real poc-11 adds auth/content/connection/sync/
//! versioning families here, each owning its item type, encoding, and projector —
//! all depending on [`crate::core`], never the reverse.
pub mod link;
