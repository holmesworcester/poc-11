use linktoy::core::item::fact_id;
use linktoy::core::typestate::Validity;
use linktoy::facts::link::project::{
    fact_id_to_core, maybe_fact_id_to_core, project_link_core, LinkCore, MaybeStatementCore,
    ValidityCore,
};
use linktoy::facts::link::{link_project_validity, Link};

fn id(label: &[u8]) -> [u8; 32] {
    fact_id(label)
}

#[test]
fn verified_kernel_runtime_root_shape_matches_link_projector() {
    let root = Link {
        content: b"root".to_vec(),
        prev: None,
        root: None,
    };
    let root_id = id(b"root-id");

    let projection = project_link_core(
        LinkCore {
            self_id: fact_id_to_core(root_id),
            prev: maybe_fact_id_to_core(root.prev),
            root: maybe_fact_id_to_core(root.root),
        },
        false,
    );

    assert_eq!(projection.validity, ValidityCore::Valid);
    assert_eq!(projection.update_owner, fact_id_to_core(root_id));
    match projection.statement {
        MaybeStatementCore::Some(statement) => {
            assert_eq!(statement.link_id, fact_id_to_core(root_id));
            assert_eq!(statement.root_id, fact_id_to_core(root_id));
        }
        MaybeStatementCore::None => panic!("root projection must emit self-root statement"),
    }
    assert_eq!(link_project_validity(&root, false), Validity::Valid);
}

#[test]
fn verified_kernel_runtime_child_preserves_claimed_root() {
    let self_id = id(b"child-id");
    let parent_id = id(b"parent-id");
    let root_id = id(b"root-id");
    let child = Link {
        content: b"child".to_vec(),
        prev: Some(parent_id),
        root: Some(root_id),
    };

    let projection = project_link_core(
        LinkCore {
            self_id: fact_id_to_core(self_id),
            prev: maybe_fact_id_to_core(child.prev),
            root: maybe_fact_id_to_core(child.root),
        },
        true,
    );

    assert_eq!(projection.validity, ValidityCore::Valid);
    assert_eq!(projection.update_owner, fact_id_to_core(self_id));
    match projection.statement {
        MaybeStatementCore::Some(statement) => {
            assert_eq!(statement.link_id, fact_id_to_core(self_id));
            assert_eq!(statement.root_id, fact_id_to_core(root_id));
        }
        MaybeStatementCore::None => panic!("valid child projection must emit child-root statement"),
    }
    assert_eq!(link_project_validity(&child, true), Validity::Valid);
}

#[test]
fn verified_kernel_runtime_child_without_parent_context_is_invalid() {
    let self_id = id(b"child-id");
    let parent_id = id(b"parent-id");
    let root_id = id(b"root-id");
    let child = Link {
        content: b"child".to_vec(),
        prev: Some(parent_id),
        root: Some(root_id),
    };

    let projection = project_link_core(
        LinkCore {
            self_id: fact_id_to_core(self_id),
            prev: maybe_fact_id_to_core(child.prev),
            root: maybe_fact_id_to_core(child.root),
        },
        false,
    );

    assert_eq!(projection.validity, ValidityCore::Invalid);
    assert_eq!(projection.statement, MaybeStatementCore::None);
    assert_eq!(link_project_validity(&child, false), Validity::Invalid);
}

#[test]
fn verified_kernel_runtime_malformed_link_shape_is_invalid() {
    let self_id = id(b"bad-id");
    let malformed = Link {
        content: b"bad".to_vec(),
        prev: None,
        root: Some(id(b"root-id")),
    };

    let projection = project_link_core(
        LinkCore {
            self_id: fact_id_to_core(self_id),
            prev: maybe_fact_id_to_core(malformed.prev),
            root: maybe_fact_id_to_core(malformed.root),
        },
        true,
    );

    assert_eq!(projection.validity, ValidityCore::Invalid);
    assert_eq!(projection.update_owner, fact_id_to_core(self_id));
    assert_eq!(projection.statement, MaybeStatementCore::None);
    assert_eq!(link_project_validity(&malformed, true), Validity::Invalid);
}
