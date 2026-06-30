use linktoy::core::engine::EngineState;
use linktoy::core::item::fact_id;
use linktoy::core::projector::Projector;
use linktoy::core::typestate::Validity;
use linktoy::facts::link::project_unproven::{
    core_to_fact_id, extract_link_core, fact_id_to_core, link_core_for,
    link_emitted_fact_count_core, maybe_fact_id_to_core, project_link_core, projected_report_core,
    LinkCore, MaybeStatementCore, ValidityCore,
};
use linktoy::facts::link::{
    link_edges, link_id, link_project_validity, valid_link_key, Link, LinkProjector,
};

fn id(label: &[u8]) -> [u8; 32] {
    fact_id(label)
}

#[test]
fn fact_id_core_conversion_round_trips_runtime_ids() {
    let source = id(b"round-trip-id");
    assert_eq!(core_to_fact_id(fact_id_to_core(source)), source);
}

#[test]
fn canonical_link_codec_round_trips_accepted_bytes_and_ids() {
    let root_id = id(b"root-id");
    let samples = [
        Link {
            content: b"root".to_vec(),
            prev: None,
            root: None,
        },
        Link {
            content: b"child".to_vec(),
            prev: Some(id(b"parent-id")),
            root: Some(root_id),
        },
        Link {
            content: b"malformed".to_vec(),
            prev: None,
            root: Some(root_id),
        },
    ];

    for sample in samples {
        let bytes = LinkProjector::encode(&sample);
        let decoded = LinkProjector::decode(&bytes).expect("encoded link should decode");
        assert_eq!(decoded, sample);
        assert_eq!(LinkProjector::encode(&decoded), bytes);
        assert_eq!(link_id(&decoded), fact_id(&bytes));
    }
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
fn verified_report_kernel_root_is_complete_self() {
    let root_id = id(b"root-id");
    let report = projected_report_core(
        LinkCore {
            self_id: fact_id_to_core(root_id),
            prev: maybe_fact_id_to_core(None),
            root: maybe_fact_id_to_core(None),
        },
        ValidityCore::Valid,
        false,
        false,
        fact_id_to_core(id(b"ignored-parent-root")),
        0,
        0,
        0,
    );

    assert!(report.complete);
    assert_eq!(core_to_fact_id(report.root), root_id);
    assert_eq!(report.depth, 0);
    assert_eq!(report.length, 1);
    assert_eq!(report.ids_len, 1);
    assert_eq!(core_to_fact_id(report.head), root_id);
}

#[test]
fn verified_extraction_runtime_root_offer_matches_edges() {
    let root = Link {
        content: b"root".to_vec(),
        prev: None,
        root: None,
    };
    let root_id = link_id(&root);

    let extraction = extract_link_core(link_core_for(root_id, root.prev, root.root));
    match extraction.offer {
        MaybeStatementCore::Some(statement) => {
            assert_eq!(core_to_fact_id(statement.link_id), root_id);
            assert_eq!(core_to_fact_id(statement.root_id), root_id);
        }
        MaybeStatementCore::None => panic!("root extraction must assert self-root offer"),
    }
    assert_eq!(extraction.need, MaybeStatementCore::None);

    let edges = link_edges(&root);
    assert_eq!(edges.len(), 1);
    assert!(edges[0].is_offer());
    assert_eq!(edges[0].key, valid_link_key(root_id, root_id));
}

#[test]
fn verified_report_kernel_child_requires_complete_same_root_parent() {
    let child_id = id(b"child-id");
    let parent_id = id(b"parent-id");
    let root_id = id(b"root-id");
    let other_root_id = id(b"other-root-id");
    let child = LinkCore {
        self_id: fact_id_to_core(child_id),
        prev: maybe_fact_id_to_core(Some(parent_id)),
        root: maybe_fact_id_to_core(Some(root_id)),
    };

    let complete_same_root = projected_report_core(
        child,
        ValidityCore::Valid,
        true,
        true,
        fact_id_to_core(root_id),
        2,
        3,
        3,
    );
    assert!(complete_same_root.complete);
    assert_eq!(core_to_fact_id(complete_same_root.root), root_id);
    assert_eq!(complete_same_root.depth, 3);
    assert_eq!(complete_same_root.length, 4);
    assert_eq!(complete_same_root.ids_len, 4);
    assert_eq!(core_to_fact_id(complete_same_root.head), child_id);

    let missing_parent = projected_report_core(
        child,
        ValidityCore::Valid,
        false,
        false,
        fact_id_to_core(root_id),
        2,
        3,
        3,
    );
    assert!(!missing_parent.complete);
    assert_eq!(missing_parent.length, 1);
    assert_eq!(missing_parent.ids_len, 1);

    let incomplete_parent = projected_report_core(
        child,
        ValidityCore::Valid,
        true,
        false,
        fact_id_to_core(root_id),
        2,
        3,
        3,
    );
    assert!(!incomplete_parent.complete);

    let wrong_root = projected_report_core(
        child,
        ValidityCore::Valid,
        true,
        true,
        fact_id_to_core(other_root_id),
        2,
        3,
        3,
    );
    assert!(!wrong_root.complete);
}

#[test]
fn verified_link_projection_emits_no_raw_facts() {
    assert_eq!(link_emitted_fact_count_core(), 0);
}

#[test]
fn link_projector_runtime_projection_keeps_state_owned_and_emits_no_facts() {
    let root = Link {
        content: b"root".to_vec(),
        prev: None,
        root: None,
    };
    let root_id = link_id(&root);
    let mut engine = EngineState::<LinkProjector>::new();

    assert_eq!(engine.admit_item(root), root_id);
    assert_eq!(engine.project_one(root_id).unwrap(), Some(Validity::Valid));

    assert_eq!(
        engine.mem.len(),
        1,
        "link projection must not emit raw facts"
    );
    assert_eq!(
        engine.projector_state.seen.get(&root_id),
        Some(&Validity::Valid)
    );
    let projected = engine
        .projector_state
        .projected
        .get(&root_id)
        .expect("projection should write the root's own read-model entry");
    assert!(projected.complete);
    assert_eq!(projected.root, root_id);
    assert_eq!(projected.ids, vec![root_id]);
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
fn verified_extraction_runtime_child_asserts_same_root_need_and_offer() {
    let parent_id = id(b"parent-id");
    let root_id = id(b"root-id");
    let child = Link {
        content: b"child".to_vec(),
        prev: Some(parent_id),
        root: Some(root_id),
    };
    let child_id = link_id(&child);

    let extraction = extract_link_core(link_core_for(child_id, child.prev, child.root));
    match extraction.offer {
        MaybeStatementCore::Some(statement) => {
            assert_eq!(core_to_fact_id(statement.link_id), child_id);
            assert_eq!(core_to_fact_id(statement.root_id), root_id);
        }
        MaybeStatementCore::None => panic!("child extraction must assert self-root offer"),
    }
    match extraction.need {
        MaybeStatementCore::Some(statement) => {
            assert_eq!(core_to_fact_id(statement.link_id), parent_id);
            assert_eq!(core_to_fact_id(statement.root_id), root_id);
        }
        MaybeStatementCore::None => panic!("child extraction must assert parent-root need"),
    }

    let edges = link_edges(&child);
    assert_eq!(edges.len(), 2);
    assert!(edges[0].is_offer());
    assert_eq!(edges[0].key, valid_link_key(child_id, root_id));
    assert!(edges[1].is_need());
    assert_eq!(edges[1].key, valid_link_key(parent_id, root_id));
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
    assert!(link_edges(&malformed).is_empty());
}
