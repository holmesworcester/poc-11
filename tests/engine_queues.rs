use linktoy::core::admit::admit;
use linktoy::core::engine::EngineState;
use linktoy::core::index::SqliteIndex;
use linktoy::core::item::{fact_id, FactId};
use linktoy::core::projector::Projector;
use linktoy::core::typestate::Validity;
use linktoy::protocol::link::{Link, LinkProjector, LINK};

fn temp_db() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("engine.db").display().to_string();
    (dir, path)
}

fn link(label: &str, prev: Option<FactId>) -> Link {
    Link {
        content: label.as_bytes().to_vec(),
        prev,
    }
}

fn link_id(l: &Link) -> FactId {
    fact_id(&LinkProjector::encode(l))
}

fn store(idx: &SqliteIndex, item: Link, ts: u64) -> FactId {
    admit::<LinkProjector>(item, ts, idx).unwrap().id()
}

fn assert_valid(engine: &EngineState<LinkProjector>, id: FactId) {
    assert_eq!(engine.validity.get(&id), Some(&Validity::Valid));
}

fn assert_validated_offer_provenance(engine: &EngineState<LinkProjector>) {
    for vo in &engine.validated {
        assert_eq!(
            engine.validity.get(&vo.owner),
            Some(&Validity::Valid),
            "validated offer must come from a valid owner"
        );
        assert_eq!(vo.offer.role, LINK);
        assert_eq!(vo.offer.key.0, vo.owner);
    }
}

#[test]
fn demand_for_head_pulls_stored_parent_chain_into_memory() {
    let (_dir, db) = temp_db();
    let idx = SqliteIndex::open(&db).unwrap();

    let root = link("root", None);
    let root_id = store(&idx, root, 1);
    let mid = link("mid", Some(root_id));
    let mid_id = store(&idx, mid, 2);
    let head = link("head", Some(mid_id));
    let head_id = store(&idx, head, 3);

    let mut engine = EngineState::<LinkProjector>::new();
    engine.enqueue_admit(head_id);
    let steps = engine.drain(&idx, 100).unwrap();

    assert!(steps > 0);
    assert_eq!(engine.mem.len(), 3);
    assert_valid(&engine, root_id);
    assert_valid(&engine, mid_id);
    assert_valid(&engine, head_id);
    assert_eq!(engine.pending_admit_len(), 0);
    assert_eq!(engine.pending_project_len(), 0);
    assert_eq!(engine.pending_query_len(), 0);
    assert_validated_offer_provenance(&engine);
}

#[test]
fn later_in_memory_parent_admission_wakes_stored_child() {
    let (_dir, db) = temp_db();
    let idx = SqliteIndex::open(&db).unwrap();

    let root = link("root", None);
    let root_id = link_id(&root);
    let child_id = store(&idx, link("child", Some(root_id)), 1);

    let mut engine = EngineState::<LinkProjector>::new();
    engine.enqueue_admit(child_id);
    engine.drain(&idx, 100).unwrap();

    assert_eq!(engine.mem.len(), 1);
    assert_eq!(engine.validity.get(&child_id), Some(&Validity::Invalid));

    let admitted_root = engine.admit_item(root);
    assert_eq!(admitted_root, root_id);
    engine.drain(&idx, 100).unwrap();

    assert_eq!(engine.mem.len(), 2);
    assert_valid(&engine, root_id);
    assert_valid(&engine, child_id);
    assert_eq!(engine.pending_admit_len(), 0);
    assert_eq!(engine.pending_project_len(), 0);
    assert_eq!(engine.pending_query_len(), 0);
    assert_validated_offer_provenance(&engine);
}
