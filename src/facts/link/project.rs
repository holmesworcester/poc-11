//! Verus-verified executable kernel for link projection semantics.
//!
//! The unproven Rust wrapper still owns byte decoding, hashing, `Context` lookup,
//! and read-model materialization. It delegates the user-facing validity decision
//! and the emitted validated-link statement shape to this kernel.
//!
//! Invariant checklist (Verus):
//! Owned invariant: verified same-root link projection kernel.
//! - [x] Safety: a root link projects valid only as `valid_link(self,self)`.
//! - [x] Safety: a malformed `prev`/`root` shape projects invalid and emits no
//!       validated link statement.
//! - [x] Safety: a child link projects valid only when its same-root parent
//!       statement is present in validated context.
//! - [x] Safety: a valid child emits only `valid_link(self, claimed_root)`, so
//!       projection preserves the root/domain it required from the parent.
//! - [x] Safety: every projection update produced by this kernel is owned by the
//!       projected link id.
//! Imported theorems:
//! - `core::typestate`: the runtime boolean supplied as
//!   `parent_validated_same_root` is produced by an exact validated-context lookup.
//! - `core::engine`: validated context entries are promoted only from valid
//!   owners.
//! Proof strategy:
//! - Verify an executable `project_link_core` function over a proof-friendly link
//!   shape.
//! - Prove the root, malformed, valid-child, and update-owner cases directly from
//!   the function's postcondition.
//! - Have `project_unproven` call this function, so the runtime link projector
//!   uses the verified validity/update-statement decision.

use crate::core::item::FactId;
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdCore {
    pub w0: u64,
    pub w1: u64,
    pub w2: u64,
    pub w3: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaybeIdCore {
    None,
    Some(IdCore),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkCore {
    pub self_id: IdCore,
    pub prev: MaybeIdCore,
    pub root: MaybeIdCore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidityCore {
    Valid,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkStatementCore {
    pub link_id: IdCore,
    pub root_id: IdCore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaybeStatementCore {
    None,
    Some(LinkStatementCore),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkProjectionCore {
    pub validity: ValidityCore,
    pub update_owner: IdCore,
    pub statement: MaybeStatementCore,
}

pub open spec fn is_root(link: LinkCore) -> bool {
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => true,
        _ => false,
    }
}

pub open spec fn is_child(link: LinkCore) -> bool {
    match (link.prev, link.root) {
        (MaybeIdCore::Some(_), MaybeIdCore::Some(_)) => true,
        _ => false,
    }
}

pub open spec fn is_malformed(link: LinkCore) -> bool {
    !is_root(link) && !is_child(link)
}

pub open spec fn statement_is_self_root(statement: MaybeStatementCore, self_id: IdCore) -> bool {
    statement == MaybeStatementCore::Some(LinkStatementCore {
        link_id: self_id,
        root_id: self_id,
    })
}

pub open spec fn statement_is_self_claimed_root(
    statement: MaybeStatementCore,
    self_id: IdCore,
    claimed_root: IdCore,
) -> bool {
    statement == MaybeStatementCore::Some(LinkStatementCore {
        link_id: self_id,
        root_id: claimed_root,
    })
}

pub open spec fn projection_spec(
    link: LinkCore,
    parent_validated_same_root: bool,
) -> LinkProjectionCore {
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => LinkProjectionCore {
            validity: ValidityCore::Valid,
            update_owner: link.self_id,
            statement: MaybeStatementCore::Some(LinkStatementCore {
                link_id: link.self_id,
                root_id: link.self_id,
            }),
        },
        (MaybeIdCore::Some(_parent), MaybeIdCore::Some(root)) => {
            if parent_validated_same_root {
                LinkProjectionCore {
                    validity: ValidityCore::Valid,
                    update_owner: link.self_id,
                    statement: MaybeStatementCore::Some(LinkStatementCore {
                        link_id: link.self_id,
                        root_id: root,
                    }),
                }
            } else {
                LinkProjectionCore {
                    validity: ValidityCore::Invalid,
                    update_owner: link.self_id,
                    statement: MaybeStatementCore::None,
                }
            }
        }
        _ => LinkProjectionCore {
            validity: ValidityCore::Invalid,
            update_owner: link.self_id,
            statement: MaybeStatementCore::None,
        },
    }
}

pub fn project_link_core(
    link: LinkCore,
    parent_validated_same_root: bool,
) -> (projection: LinkProjectionCore)
    ensures
        projection == projection_spec(link, parent_validated_same_root),
        projection.update_owner == link.self_id,
        is_root(link) ==> (
            projection.validity == ValidityCore::Valid
                && statement_is_self_root(projection.statement, link.self_id)
        ),
        is_malformed(link) ==> (
            projection.validity == ValidityCore::Invalid
                && projection.statement == MaybeStatementCore::None
        ),
        projection.validity == ValidityCore::Valid && is_child(link) ==> parent_validated_same_root,
{
    match (link.prev, link.root) {
        (MaybeIdCore::None, MaybeIdCore::None) => LinkProjectionCore {
            validity: ValidityCore::Valid,
            update_owner: link.self_id,
            statement: MaybeStatementCore::Some(LinkStatementCore {
                link_id: link.self_id,
                root_id: link.self_id,
            }),
        },
        (MaybeIdCore::Some(_parent), MaybeIdCore::Some(root)) => {
            if parent_validated_same_root {
                LinkProjectionCore {
                    validity: ValidityCore::Valid,
                    update_owner: link.self_id,
                    statement: MaybeStatementCore::Some(LinkStatementCore {
                        link_id: link.self_id,
                        root_id: root,
                    }),
                }
            } else {
                LinkProjectionCore {
                    validity: ValidityCore::Invalid,
                    update_owner: link.self_id,
                    statement: MaybeStatementCore::None,
                }
            }
        }
        _ => LinkProjectionCore {
            validity: ValidityCore::Invalid,
            update_owner: link.self_id,
            statement: MaybeStatementCore::None,
        },
    }
}

pub proof fn root_projection_emits_self_root(link: LinkCore)
    requires
        is_root(link),
    ensures
        projection_spec(link, false).validity == ValidityCore::Valid,
        statement_is_self_root(projection_spec(link, false).statement, link.self_id),
{
}

pub proof fn malformed_projection_is_invalid(link: LinkCore, parent_validated_same_root: bool)
    requires
        is_malformed(link),
    ensures
        projection_spec(link, parent_validated_same_root).validity == ValidityCore::Invalid,
        projection_spec(link, parent_validated_same_root).statement == MaybeStatementCore::None,
{
}

pub proof fn valid_child_requires_validated_same_root_parent(
    link: LinkCore,
    parent_validated_same_root: bool,
)
    requires
        is_child(link),
        projection_spec(link, parent_validated_same_root).validity == ValidityCore::Valid,
    ensures
        parent_validated_same_root,
{
}

pub proof fn valid_child_preserves_claimed_root(
    self_id: IdCore,
    parent_id: IdCore,
    root_id: IdCore,
)
    ensures
        ({
            let link = LinkCore {
                self_id,
                prev: MaybeIdCore::Some(parent_id),
                root: MaybeIdCore::Some(root_id),
            };
            let projection = projection_spec(link, true);
            projection.validity == ValidityCore::Valid
                && projection.update_owner == self_id
                && statement_is_self_claimed_root(projection.statement, self_id, root_id)
        }),
{
}

pub proof fn invalid_child_emits_no_statement(
    self_id: IdCore,
    parent_id: IdCore,
    root_id: IdCore,
)
    ensures
        ({
            let link = LinkCore {
                self_id,
                prev: MaybeIdCore::Some(parent_id),
                root: MaybeIdCore::Some(root_id),
            };
            let projection = projection_spec(link, false);
            projection.validity == ValidityCore::Invalid
                && projection.statement == MaybeStatementCore::None
        }),
{
}

} // verus!

fn chunk_u64(id: &FactId, offset: usize) -> u64 {
    u64::from_le_bytes([
        id[offset],
        id[offset + 1],
        id[offset + 2],
        id[offset + 3],
        id[offset + 4],
        id[offset + 5],
        id[offset + 6],
        id[offset + 7],
    ])
}

pub fn fact_id_to_core(id: FactId) -> IdCore {
    IdCore {
        w0: chunk_u64(&id, 0),
        w1: chunk_u64(&id, 8),
        w2: chunk_u64(&id, 16),
        w3: chunk_u64(&id, 24),
    }
}

pub fn maybe_fact_id_to_core(id: Option<FactId>) -> MaybeIdCore {
    match id {
        Some(id) => MaybeIdCore::Some(fact_id_to_core(id)),
        None => MaybeIdCore::None,
    }
}

pub fn validity_from_core(validity: ValidityCore) -> crate::core::typestate::Validity {
    match validity {
        ValidityCore::Valid => crate::core::typestate::Validity::Valid,
        ValidityCore::Invalid => crate::core::typestate::Validity::Invalid,
    }
}
