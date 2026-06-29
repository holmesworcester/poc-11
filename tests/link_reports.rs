use std::collections::HashMap;

use linktoy::core::index::Index;
use linktoy::core::item::{fact_id, FactId};
use linktoy::core::offer::{Key, Offer, Role, Scope};
use linktoy::core::projector::Projector;
use linktoy::core::typestate::Asserted;
use linktoy::facts::link::{chain_report, Link, LinkProjector};

struct ReportIndex {
    facts: HashMap<FactId, Vec<u8>>,
}

impl ReportIndex {
    fn with_fact(id: FactId, bytes: Vec<u8>) -> Self {
        let mut facts = HashMap::new();
        facts.insert(id, bytes);
        Self { facts }
    }
}

impl Index for ReportIndex {
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

    fn offers_for_key(
        &self,
        _role: Role,
        _scope: Scope,
        _key: &Key,
    ) -> Result<Vec<FactId>, String> {
        Ok(vec![])
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
fn chain_report_missing_parent_is_incomplete_observation() {
    let missing_parent = [9; 32];
    let child = Link {
        content: b"child".to_vec(),
        prev: Some(missing_parent),
        root: Some(missing_parent),
    };
    let child_bytes = LinkProjector::encode(&child);
    let child_id = fact_id(&child_bytes);
    let idx = ReportIndex::with_fact(child_id, child_bytes);

    let report = chain_report(&idx, child_id).unwrap();

    assert!(report.present);
    assert!(!report.complete);
    assert_eq!(report.length, 1);
    assert_eq!(report.ids, vec![child_id]);
}

#[test]
fn chain_report_malformed_fact_returns_error_without_report() {
    let bad_bytes = vec![0xFF, 0x00, 0x00];
    let bad_id = fact_id(&bad_bytes);
    let idx = ReportIndex::with_fact(bad_id, bad_bytes);

    let err = match chain_report(&idx, bad_id) {
        Ok(_) => panic!("malformed fact should not produce a report"),
        Err(err) => err,
    };

    assert!(err.contains("not a link fact"));
}
