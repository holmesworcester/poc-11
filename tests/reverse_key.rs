//! Lib-level tests for the two cases the CLI can't easily set up:
//!  - Test D: out-of-order arrival, driven by the ENGINE. A child admitted before
//!    its parent is Invalid; once the parent arrives, `play::wake` follows the
//!    reverse key (offer→need) and re-derives the affected set, validating the
//!    child (§3 Fact 2 / the cascade).
//!  - Cycle guard: a fabricated suppression cycle (impossible to author honestly,
//!    since `prev` is in the hash) must raise a located `SuppressionCycle`. It runs
//!    against an in-memory fake `Index` so the real content-addressed SQLite path
//!    keeps its `id == hash(bytes)` invariant.
use std::collections::HashMap;

use linktoy::core::admit::admit;
use linktoy::core::index::{Index, SqliteIndex};
use linktoy::core::item::{fact_id, FactId};
use linktoy::core::offer::{Key, Offer, Role, Scope};
use linktoy::core::play::{replay, wake};
use linktoy::core::projector::Projector;
use linktoy::core::typestate::{Asserted, Validity};
use linktoy::protocol::link::{Link, LinkProjector, LINK};

fn open(dir: &tempfile::TempDir, name: &str) -> SqliteIndex {
    SqliteIndex::open(dir.path().join(name).to_str().unwrap()).unwrap()
}

#[test]
fn out_of_order_child_before_parent_wakes_via_engine() {
    let tmp = tempfile::tempdir().unwrap();
    let idx = open(&tmp, "ooo.db");

    // The parent's id is content-addressed, so we can name it before authoring it.
    let root = Link {
        content: b"root".to_vec(),
        prev: None,
    };
    let root_id = fact_id(&LinkProjector::encode(&root));
    let child = Link {
        content: b"child".to_vec(),
        prev: Some(root_id),
    };

    // Admit the CHILD first — its parent does not exist yet.
    let cid = admit::<LinkProjector>(child, 1, &idx).unwrap().id();
    let m1 = replay::<LinkProjector>(&idx, &[cid]).unwrap();
    assert_eq!(
        m1.get(&cid),
        Some(&Validity::Invalid),
        "orphan child must be invalid"
    );

    // The index is reverse-keyed: the child's need is findable by the parent's key.
    assert_eq!(
        idx.needs_for_key(LINK, Scope::Local, &Key(root_id))
            .unwrap(),
        vec![cid]
    );

    // Admit the parent; the ENGINE's offer→need wake re-derives the affected set.
    let rid = admit::<LinkProjector>(root, 2, &idx).unwrap().id();
    assert_eq!(rid, root_id);
    let woken = wake::<LinkProjector>(&idx, root_id).unwrap();
    assert_eq!(woken.get(&root_id), Some(&Validity::Valid));
    assert_eq!(
        woken.get(&cid),
        Some(&Validity::Valid),
        "child validates once parent wakes it"
    );
}

/// In-memory `Index` used only to fabricate a suppression cycle (which honest,
/// content-addressed facts cannot form). It returns crafted bodies and offerers;
/// the writes are no-ops.
struct FakeIndex {
    facts: HashMap<FactId, Vec<u8>>,
    offerers: HashMap<[u8; 32], Vec<FactId>>,
}

impl Index for FakeIndex {
    fn insert_asserted(
        &self,
        _owner: FactId,
        _edges: &[Offer<Asserted>],
        _ts: u64,
    ) -> Result<(), String> {
        Ok(())
    }
    fn flush_fact(&self, _id: FactId, _bytes: &[u8], _ts: u64) -> Result<(), String> {
        Ok(())
    }
    fn load_fact(&self, id: &FactId) -> Result<Option<Vec<u8>>, String> {
        Ok(self.facts.get(id).cloned())
    }
    fn offers_for_key(&self, _role: Role, _scope: Scope, key: &Key) -> Result<Vec<FactId>, String> {
        Ok(self.offerers.get(&key.0).cloned().unwrap_or_default())
    }
    fn needs_for_key(&self, _role: Role, _scope: Scope, _key: &Key) -> Result<Vec<FactId>, String> {
        Ok(vec![])
    }
    fn window(&self, _n: usize) -> Result<Vec<FactId>, String> {
        Ok(vec![])
    }
    fn total_facts(&self) -> Result<usize, String> {
        Ok(self.facts.len())
    }
    fn total_edges(&self) -> Result<usize, String> {
        Ok(0)
    }
}

#[test]
fn fabricated_suppression_cycle_is_located() {
    let id_a = [0xAAu8; 32];
    let id_b = [0xBBu8; 32];

    // Bodies point at each other; each fabricated id offers itself, so the needs
    // resolve into a cycle a -> b -> a.
    let mut facts = HashMap::new();
    facts.insert(
        id_a,
        LinkProjector::encode(&Link {
            content: vec![1],
            prev: Some(id_b),
        }),
    );
    facts.insert(
        id_b,
        LinkProjector::encode(&Link {
            content: vec![2],
            prev: Some(id_a),
        }),
    );
    let mut offerers = HashMap::new();
    offerers.insert(id_a, vec![id_a]);
    offerers.insert(id_b, vec![id_b]);
    let idx = FakeIndex { facts, offerers };

    let err = replay::<LinkProjector>(&idx, &[id_a]).unwrap_err();
    assert!(
        err.contains("SuppressionCycle"),
        "expected located cycle, got: {err}"
    );
}
