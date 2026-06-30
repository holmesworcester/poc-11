use linktoy::core::admit::admit;
use linktoy::core::admit::Admitted;
use linktoy::core::effects::{EffectRequest, EffectResult};
use linktoy::core::engine::{EdgeAddr, EngineState, Storage};
use linktoy::core::item::{fact_id, FactId};
use linktoy::core::offer::{Key, Offer, Role, Scope};
use linktoy::core::projector::{EmittedFact, ProjectOutcome, Projector};
use linktoy::core::turn::{self, TurnOutcome};
use linktoy::core::typestate::{Asserted, Context, Validity};
use linktoy::facts::link::{valid_link_key, Link, LinkProjector, LINK};
use linktoy::helpers::sqlite_unproven::SqliteIndex;

fn temp_db() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("engine.db").display().to_string();
    (dir, path)
}

fn link(label: &str, prev: Option<FactId>, root: Option<FactId>) -> Link {
    Link {
        content: label.as_bytes().to_vec(),
        prev,
        root,
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
    }
}

fn assert_projected_link_report(
    engine: &EngineState<LinkProjector>,
    id: FactId,
    root: FactId,
    ids: &[FactId],
) {
    let report = engine
        .projector_state
        .projected
        .get(&id)
        .unwrap_or_else(|| panic!("missing projected report for {id:?}"));
    assert!(report.complete);
    assert_eq!(report.root, root);
    assert_eq!(report.depth, ids.len() as u64 - 1);
    assert_eq!(report.length, ids.len() as u64);
    assert_eq!(report.ids, ids);
}

fn assert_recorded_dependency(
    engine: &EngineState<LinkProjector>,
    consumer: FactId,
    provider: FactId,
    root: FactId,
) {
    let addr = EdgeAddr {
        role: LINK,
        scope: Scope::Local,
        key: valid_link_key(provider, root),
    };
    assert!(
        engine
            .dependencies
            .iter()
            .any(|dep| dep.consumer == consumer && dep.provider == provider && dep.addr == addr),
        "missing recorded dependency consumer={consumer:?} provider={provider:?}"
    );
}

#[test]
fn demand_for_head_pulls_stored_parent_chain_into_memory() {
    let (_dir, db) = temp_db();
    let idx = SqliteIndex::open(&db).unwrap();

    let root = link("root", None, None);
    let root_id = store(&idx, root, 1);
    let mid = link("mid", Some(root_id), Some(root_id));
    let mid_id = store(&idx, mid, 2);
    let head = link("head", Some(mid_id), Some(root_id));
    let head_id = store(&idx, head, 3);

    let mut engine = EngineState::<LinkProjector>::new();
    engine.enqueue_admit(head_id);
    let steps = turn::drain(&mut engine, &idx, 100).unwrap();

    assert!(steps > 0);
    assert_eq!(engine.mem.len(), 3);
    assert_valid(&engine, root_id);
    assert_valid(&engine, mid_id);
    assert_valid(&engine, head_id);
    assert_eq!(engine.pending_admit_len(), 0);
    assert_eq!(engine.pending_project_len(), 0);
    assert_eq!(engine.pending_query_len(), 0);
    assert_validated_offer_provenance(&engine);
    assert_projected_link_report(&engine, head_id, root_id, &[root_id, mid_id, head_id]);
    assert_recorded_dependency(&engine, mid_id, root_id, root_id);
    assert_recorded_dependency(&engine, head_id, mid_id, root_id);
}

#[test]
fn later_in_memory_parent_admission_wakes_stored_child() {
    let (_dir, db) = temp_db();
    let idx = SqliteIndex::open(&db).unwrap();

    let root = link("root", None, None);
    let root_id = link_id(&root);
    let child_id = store(&idx, link("child", Some(root_id), Some(root_id)), 1);

    let mut engine = EngineState::<LinkProjector>::new();
    engine.enqueue_admit(child_id);
    turn::drain(&mut engine, &idx, 100).unwrap();

    assert_eq!(engine.mem.len(), 1);
    assert_eq!(engine.validity.get(&child_id), Some(&Validity::Invalid));
    assert!(
        !engine.projector_state.projected.contains_key(&child_id),
        "unready facts must not produce projector-owned read-model state"
    );

    let admitted_root = engine.admit_item(root);
    assert_eq!(admitted_root, root_id);
    turn::drain(&mut engine, &idx, 100).unwrap();

    assert_eq!(engine.mem.len(), 2);
    assert_valid(&engine, root_id);
    assert_valid(&engine, child_id);
    assert_eq!(engine.pending_admit_len(), 0);
    assert_eq!(engine.pending_project_len(), 0);
    assert_eq!(engine.pending_query_len(), 0);
    assert_validated_offer_provenance(&engine);
    assert_projected_link_report(&engine, child_id, root_id, &[root_id, child_id]);
}

#[test]
fn requeueing_valid_fact_does_not_duplicate_validated_offers() {
    let (_dir, db) = temp_db();
    let idx = SqliteIndex::open(&db).unwrap();

    let root_id = store(&idx, link("root", None, None), 1);

    let mut engine = EngineState::<LinkProjector>::new();
    engine.enqueue_admit(root_id);
    turn::drain(&mut engine, &idx, 100).unwrap();
    assert_valid(&engine, root_id);
    assert_eq!(engine.validated.len(), 1);

    engine.enqueue_project(root_id);
    turn::drain(&mut engine, &idx, 100).unwrap();

    assert_valid(&engine, root_id);
    assert_eq!(
        engine.validated.len(),
        1,
        "a validated owner/address should be promoted only once"
    );
    assert_eq!(engine.pending_admit_len(), 0);
    assert_eq!(engine.pending_project_len(), 0);
    assert_eq!(engine.pending_query_len(), 0);
    assert_validated_offer_provenance(&engine);
}

#[derive(Clone)]
struct EmitItem(u8);

#[derive(Default)]
struct EmitState;

struct EmitProjector;

const EMIT: Role = Role("emit");

impl Projector for EmitProjector {
    type Item = EmitItem;
    type State = EmitState;
    type Update = ();

    fn encode(item: &Self::Item) -> Vec<u8> {
        vec![0xE0, item.0]
    }

    fn decode(bytes: &[u8]) -> Result<Self::Item, String> {
        match bytes {
            [0xE0, value] => Ok(EmitItem(*value)),
            _ => Err("not an emit item".to_string()),
        }
    }

    fn extract(item: &Self::Item) -> Vec<Offer<Asserted>> {
        vec![Offer::offer(EMIT, Key(fact_id(&Self::encode(item))))]
    }

    fn project(
        item: &Admitted<Self::Item>,
        _ctx: Context,
        _st: &Self::State,
    ) -> ProjectOutcome<Self::Update> {
        let emitted = if item.item().0 == 0 {
            vec![EmittedFact {
                bytes: Self::encode(&EmitItem(1)),
            }]
        } else {
            vec![]
        };
        ProjectOutcome {
            validity: Validity::Valid,
            emitted,
            updates: vec![],
        }
    }

    fn update_owner(_update: &Self::Update) -> FactId {
        [0; 32]
    }

    fn apply_update(_st: &mut Self::State, _update: Self::Update) {}
}

struct EmptyStorage;

impl Storage for EmptyStorage {
    fn load_fact(&self, _id: &FactId) -> Result<Option<Vec<u8>>, String> {
        Ok(None)
    }

    fn offerers_for(&self, _addr: EdgeAddr) -> Result<Vec<FactId>, String> {
        Ok(vec![])
    }

    fn needers_for(&self, _addr: EdgeAddr) -> Result<Vec<FactId>, String> {
        Ok(vec![])
    }
}

#[test]
fn emitted_facts_reenter_the_in_memory_worklist() {
    let mut engine = EngineState::<EmitProjector>::new();
    let seed_id = engine.admit_item(EmitItem(0));
    let emitted_id = fact_id(&EmitProjector::encode(&EmitItem(1)));

    turn::drain(&mut engine, &EmptyStorage, 100).unwrap();

    assert!(engine.mem.contains(&seed_id));
    assert!(engine.mem.contains(&emitted_id));
    assert_eq!(engine.validity.get(&seed_id), Some(&Validity::Valid));
    assert_eq!(engine.validity.get(&emitted_id), Some(&Validity::Valid));
}

#[derive(Clone)]
enum GateItem {
    NeedsRoleA { key: Key, offer: Key },
    ProviderRoleB { key: Key },
    InvalidEmitter,
    Emitted,
}

#[derive(Default)]
struct GateState {
    calls: usize,
}

struct GateProjector;

struct GateUpdate {
    owner: FactId,
}

const GATE_A: Role = Role("gate-a");
const GATE_B: Role = Role("gate-b");

impl Projector for GateProjector {
    type Item = GateItem;
    type State = GateState;
    type Update = GateUpdate;

    fn encode(item: &Self::Item) -> Vec<u8> {
        let mut bytes = vec![0xA0];
        match item {
            GateItem::NeedsRoleA { key, offer } => {
                bytes.push(0);
                bytes.extend_from_slice(&key.0);
                bytes.extend_from_slice(&offer.0);
            }
            GateItem::ProviderRoleB { key } => {
                bytes.push(1);
                bytes.extend_from_slice(&key.0);
            }
            GateItem::InvalidEmitter => {
                bytes.push(2);
            }
            GateItem::Emitted => {
                bytes.push(3);
            }
        }
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self::Item, String> {
        match bytes {
            [0xA0, 3] => Ok(GateItem::Emitted),
            _ => Err("gate decode is only needed for emitted facts".to_string()),
        }
    }

    fn extract(item: &Self::Item) -> Vec<Offer<Asserted>> {
        match item {
            GateItem::NeedsRoleA { key, offer } => {
                vec![Offer::need(GATE_A, *key), Offer::offer(GATE_A, *offer)]
            }
            GateItem::ProviderRoleB { key } => vec![Offer::offer(GATE_B, *key)],
            GateItem::InvalidEmitter => vec![],
            GateItem::Emitted => vec![],
        }
    }

    fn project(
        item: &Admitted<Self::Item>,
        _ctx: Context,
        _st: &Self::State,
    ) -> ProjectOutcome<Self::Update> {
        let updates = vec![GateUpdate { owner: item.id() }];
        match item.item() {
            GateItem::InvalidEmitter => ProjectOutcome {
                validity: Validity::Invalid,
                emitted: vec![EmittedFact {
                    bytes: Self::encode(&GateItem::Emitted),
                }],
                updates,
            },
            _ => ProjectOutcome {
                validity: Validity::Valid,
                emitted: vec![],
                updates,
            },
        }
    }

    fn update_owner(update: &Self::Update) -> FactId {
        update.owner
    }

    fn apply_update(st: &mut Self::State, _update: Self::Update) {
        st.calls += 1;
    }
}

#[test]
fn unmet_need_is_rejected_before_projector_can_emit_updates_or_promote() {
    let missing = Key([7; 32]);
    let offered = Key([8; 32]);
    let mut engine = EngineState::<GateProjector>::new();
    let id = engine.admit_item(GateItem::NeedsRoleA {
        key: missing,
        offer: offered,
    });

    turn::drain(&mut engine, &EmptyStorage, 100).unwrap();

    assert_eq!(engine.validity.get(&id), Some(&Validity::Invalid));
    assert_eq!(engine.projector_state.calls, 0);
    assert!(engine.validated.is_empty());
}

#[test]
fn same_key_with_wrong_role_does_not_satisfy_verified_readiness() {
    let shared_key = Key([9; 32]);
    let child_offer = Key([10; 32]);
    let mut engine = EngineState::<GateProjector>::new();
    let provider = engine.admit_item(GateItem::ProviderRoleB { key: shared_key });
    turn::drain(&mut engine, &EmptyStorage, 100).unwrap();

    assert_eq!(engine.validity.get(&provider), Some(&Validity::Valid));
    assert_eq!(engine.validated.len(), 1);
    assert_eq!(engine.validated[0].offer.role, GATE_B);
    assert_eq!(engine.projector_state.calls, 1);

    let needer = engine.admit_item(GateItem::NeedsRoleA {
        key: shared_key,
        offer: child_offer,
    });
    turn::drain(&mut engine, &EmptyStorage, 100).unwrap();

    assert_eq!(engine.validity.get(&needer), Some(&Validity::Invalid));
    assert_eq!(engine.projector_state.calls, 1);
    assert_eq!(engine.validated.len(), 1);
}

#[test]
fn child_with_wrong_root_domain_does_not_validate() {
    let (_dir, db) = temp_db();
    let idx = SqliteIndex::open(&db).unwrap();

    let root_a = link("root-a", None, None);
    let root_a_id = store(&idx, root_a, 1);
    let root_b = link("root-b", None, None);
    let root_b_id = store(&idx, root_b, 2);
    let child = link("child", Some(root_a_id), Some(root_b_id));
    let child_id = store(&idx, child, 3);

    let mut engine = EngineState::<LinkProjector>::new();
    engine.enqueue_admit(root_a_id);
    engine.enqueue_admit(root_b_id);
    engine.enqueue_admit(child_id);
    turn::drain(&mut engine, &idx, 100).unwrap();

    assert_valid(&engine, root_a_id);
    assert_valid(&engine, root_b_id);
    assert_eq!(engine.validity.get(&child_id), Some(&Validity::Invalid));
    assert!(
        !engine
            .validated
            .iter()
            .any(|vo| vo.owner == child_id && vo.offer.key == valid_link_key(child_id, root_b_id)),
        "wrong-root child must not promote its claimed valid_link statement"
    );
}

#[test]
fn invalid_projection_output_does_not_emit_facts() {
    let mut engine = EngineState::<GateProjector>::new();
    let seed = engine.admit_item(GateItem::InvalidEmitter);
    let emitted = fact_id(&GateProjector::encode(&GateItem::Emitted));

    turn::drain(&mut engine, &EmptyStorage, 100).unwrap();

    assert_eq!(engine.validity.get(&seed), Some(&Validity::Invalid));
    assert_eq!(engine.projector_state.calls, 1);
    assert!(engine.mem.contains(&seed));
    assert!(!engine.mem.contains(&emitted));
}

#[derive(Clone)]
struct BadUpdateItem;

#[derive(Default)]
struct BadUpdateState {
    applied: usize,
}

struct BadUpdateProjector;

struct BadUpdate {
    owner: FactId,
}

const BAD_UPDATE: Role = Role("bad-update");

impl Projector for BadUpdateProjector {
    type Item = BadUpdateItem;
    type State = BadUpdateState;
    type Update = BadUpdate;

    fn encode(_item: &Self::Item) -> Vec<u8> {
        vec![0xB0]
    }

    fn decode(bytes: &[u8]) -> Result<Self::Item, String> {
        match bytes {
            [0xB0] => Ok(BadUpdateItem),
            _ => Err("not a bad-update item".to_string()),
        }
    }

    fn extract(_item: &Self::Item) -> Vec<Offer<Asserted>> {
        vec![Offer::offer(BAD_UPDATE, Key([12; 32]))]
    }

    fn project(
        _item: &Admitted<Self::Item>,
        _ctx: Context,
        _st: &Self::State,
    ) -> ProjectOutcome<Self::Update> {
        ProjectOutcome {
            validity: Validity::Valid,
            emitted: vec![],
            updates: vec![BadUpdate { owner: [13; 32] }],
        }
    }

    fn update_owner(update: &Self::Update) -> FactId {
        update.owner
    }

    fn apply_update(st: &mut Self::State, _update: Self::Update) {
        st.applied += 1;
    }
}

#[test]
fn engine_rejects_projector_update_for_different_fact() {
    let mut engine = EngineState::<BadUpdateProjector>::new();
    let id = engine.admit_item(BadUpdateItem);

    let err = turn::drain(&mut engine, &EmptyStorage, 100).unwrap_err();

    assert!(err.contains("projector returned state update for a different fact"));
    assert_eq!(engine.projector_state.applied, 0);
    assert!(engine.validated.is_empty());
    assert!(!engine.validity.contains_key(&id));
}

#[test]
fn turn_exposes_load_fact_effect_before_storage_is_interpreted() {
    let root = link("root", None, None);
    let root_id = link_id(&root);
    let mut engine = EngineState::<LinkProjector>::new();
    engine.enqueue_admit(root_id);

    let first = turn::turn(&mut engine).unwrap();
    assert_eq!(first, TurnOutcome::Effect(EffectRequest::LoadFact(root_id)));
    assert_eq!(engine.pending_admit_len(), 0);
    assert!(!engine.mem.contains(&root_id));

    turn::apply_effect(
        &mut engine,
        EffectResult::FactLoaded {
            id: root_id,
            bytes: Some(LinkProjector::encode(&root)),
        },
    )
    .unwrap();

    let second = turn::turn(&mut engine).unwrap();
    assert_eq!(
        second,
        TurnOutcome::Projected {
            id: root_id,
            validity: Some(Validity::Valid),
        }
    );
    assert_eq!(engine.validity.get(&root_id), Some(&Validity::Valid));
}
