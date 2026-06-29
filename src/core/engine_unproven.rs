//! Queue-oriented in-memory engine model. Durable storage remains behind
//! [`Storage`]; this module owns the proof-facing state split:
//!
//! - `to_admit`: load/decode/index facts into memory.
//! - `to_project`: validate already-admitted facts.
//! - need queries: pull stored offerers for newly indexed needs.
//! - offer queries: wake stored/local needers for newly validated offers.
//!
//! Projection promotion is gated by `linktoy-verus-core`, an executable Verus
//! crate that this engine calls as ordinary Rust.

use std::collections::{HashMap, HashSet, VecDeque};

use linktoy_verus_core::{
    fact_ready_core, project_fact_core, AdmittedFactCore, Bytes32Core, EdgeAddrCore,
    ValidatedOfferCore, ValidityCore,
};

use super::admit::Admitted;
use super::index::Index;
use super::item::{fact_id, FactId};
use super::offer::{Key, Offer, Role, Scope};
use super::projector::Projector;
use super::typestate::{Asserted, Context, Validated, Validity};
use crate::helpers::crypto_unproven::role_id;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct EdgeAddr {
    pub role: Role,
    pub scope: Scope,
    pub key: Key,
}

impl EdgeAddr {
    fn from_offer<V>(offer: &Offer<V>) -> Self {
        Self {
            role: offer.role,
            scope: offer.scope,
            key: offer.key,
        }
    }
}

/// Durable lookup contract used by the in-memory engine. SQLite is one
/// implementation; the proof assumes this contract, not the SQL implementation.
pub trait Storage {
    fn load_fact(&self, id: &FactId) -> Result<Option<Vec<u8>>, String>;
    fn offerers_for(&self, addr: EdgeAddr) -> Result<Vec<FactId>, String>;
    fn needers_for(&self, addr: EdgeAddr) -> Result<Vec<FactId>, String>;
}

impl<T: Index + ?Sized> Storage for T {
    fn load_fact(&self, id: &FactId) -> Result<Option<Vec<u8>>, String> {
        Index::load_fact(self, id)
    }

    fn offerers_for(&self, addr: EdgeAddr) -> Result<Vec<FactId>, String> {
        self.offers_for_key(addr.role, addr.scope, &addr.key)
    }

    fn needers_for(&self, addr: EdgeAddr) -> Result<Vec<FactId>, String> {
        self.needs_for_key(addr.role, addr.scope, &addr.key)
    }
}

pub struct MemIndex<P: Projector> {
    facts: HashMap<FactId, P::Item>,
    edges: HashMap<FactId, Vec<Offer<Asserted>>>,
    offers: HashMap<EdgeAddr, HashSet<FactId>>,
    needs: HashMap<EdgeAddr, HashSet<FactId>>,
}

impl<P: Projector> Default for MemIndex<P> {
    fn default() -> Self {
        Self {
            facts: HashMap::new(),
            edges: HashMap::new(),
            offers: HashMap::new(),
            needs: HashMap::new(),
        }
    }
}

impl<P: Projector> MemIndex<P> {
    pub fn contains(&self, id: &FactId) -> bool {
        self.facts.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    fn item(&self, id: &FactId) -> Option<&P::Item> {
        self.facts.get(id)
    }

    fn edges(&self, id: &FactId) -> Option<&[Offer<Asserted>]> {
        self.edges.get(id).map(Vec::as_slice)
    }

    fn insert(&mut self, id: FactId, item: P::Item, edges: Vec<Offer<Asserted>>) -> bool {
        if self.facts.contains_key(&id) {
            return false;
        }
        for edge in &edges {
            let addr = EdgeAddr::from_offer(edge);
            if edge.is_offer() {
                self.offers.entry(addr).or_default().insert(id);
            } else if edge.is_need() {
                self.needs.entry(addr).or_default().insert(id);
            }
        }
        self.facts.insert(id, item);
        self.edges.insert(id, edges);
        true
    }

    fn offerers(&self, addr: EdgeAddr) -> Vec<FactId> {
        self.offers
            .get(&addr)
            .map(|owners| owners.iter().copied().collect())
            .unwrap_or_default()
    }

    fn needers(&self, addr: EdgeAddr) -> Vec<FactId> {
        self.needs
            .get(&addr)
            .map(|owners| owners.iter().copied().collect())
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
pub struct ValidatedOffer {
    pub owner: FactId,
    pub offer: Offer<Validated>,
}

pub struct EngineState<P: Projector> {
    pub mem: MemIndex<P>,
    pub projector_state: P::State,
    pub validity: HashMap<FactId, Validity>,
    pub validated: Vec<ValidatedOffer>,
    validated_by_addr: HashMap<EdgeAddr, Vec<ValidatedOffer>>,
    promoted_offers: HashSet<(FactId, EdgeAddr)>,
    to_admit: VecDeque<FactId>,
    to_project: VecDeque<FactId>,
    need_queries: VecDeque<EdgeAddr>,
    offer_queries: VecDeque<EdgeAddr>,
    queued_admit: HashSet<FactId>,
    queued_project: HashSet<FactId>,
    queued_need_queries: HashSet<EdgeAddr>,
    queued_offer_queries: HashSet<EdgeAddr>,
}

impl<P: Projector> Default for EngineState<P> {
    fn default() -> Self {
        Self {
            mem: MemIndex::default(),
            projector_state: P::State::default(),
            validity: HashMap::new(),
            validated: Vec::new(),
            validated_by_addr: HashMap::new(),
            promoted_offers: HashSet::new(),
            to_admit: VecDeque::new(),
            to_project: VecDeque::new(),
            need_queries: VecDeque::new(),
            offer_queries: VecDeque::new(),
            queued_admit: HashSet::new(),
            queued_project: HashSet::new(),
            queued_need_queries: HashSet::new(),
            queued_offer_queries: HashSet::new(),
        }
    }
}

impl<P: Projector> EngineState<P>
where
    P::Item: Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue_admit(&mut self, id: FactId) {
        if self.queued_admit.insert(id) {
            self.to_admit.push_back(id);
        }
    }

    pub fn enqueue_project(&mut self, id: FactId) {
        if self.queued_project.insert(id) {
            self.to_project.push_back(id);
        }
    }

    pub fn pending_admit_len(&self) -> usize {
        self.to_admit.len()
    }

    pub fn pending_project_len(&self) -> usize {
        self.to_project.len()
    }

    pub fn pending_query_len(&self) -> usize {
        self.need_queries.len() + self.offer_queries.len()
    }

    /// Admit an already-decoded local item into memory. This is the non-storage
    /// path for facts that should not be written to durable storage by this pass.
    pub fn admit_item(&mut self, item: P::Item) -> FactId {
        let id = fact_id(&P::encode(&item));
        self.index_item(id, item);
        id
    }

    /// Load a content-addressed fact from storage, decode it, and index its
    /// asserted needs/offers in memory. This deliberately does not call
    /// `admit`: storage already owns the persisted bytes and asserted edge rows.
    pub fn admit_from_storage<S: Storage + ?Sized>(
        &mut self,
        id: FactId,
        storage: &S,
    ) -> Result<bool, String> {
        let bytes = storage.load_fact(&id)?;
        self.admit_loaded_fact(id, bytes)
    }

    pub fn admit_loaded_fact(
        &mut self,
        id: FactId,
        bytes: Option<Vec<u8>>,
    ) -> Result<bool, String> {
        if self.mem.contains(&id) {
            return Ok(false);
        }
        let Some(bytes) = bytes else {
            return Ok(false);
        };
        if fact_id(&bytes) != id {
            return Err("storage returned bytes whose hash does not match id".to_string());
        }
        let item = P::decode(&bytes)?;
        if P::encode(&item) != bytes {
            return Err("storage returned non-canonical bytes".to_string());
        }
        self.index_item(id, item);
        Ok(true)
    }

    fn index_item(&mut self, id: FactId, item: P::Item) {
        let edges = P::extract(&item);
        if !self.mem.insert(id, item, edges.clone()) {
            self.enqueue_project_if_not_valid(id);
            return;
        }
        self.enqueue_project(id);
        for need in edges.iter().filter(|edge| edge.is_need()) {
            self.enqueue_need_query(EdgeAddr::from_offer(need));
        }
    }

    pub fn project_one(&mut self, id: FactId) -> Result<Option<Validity>, String> {
        if self.validity.get(&id) == Some(&Validity::Valid) {
            return Ok(Some(Validity::Valid));
        }
        let Some(item) = self.mem.item(&id).cloned() else {
            self.enqueue_admit(id);
            return Ok(None);
        };
        let Some(edges) = self.mem.edges(&id).map(|edges| edges.to_vec()) else {
            self.enqueue_admit(id);
            return Ok(None);
        };

        for need in edges.iter().filter(|edge| edge.is_need()) {
            let addr = EdgeAddr::from_offer(need);
            for provider in self.mem.offerers(addr) {
                if !self.validity.contains_key(&provider) {
                    self.enqueue_project(provider);
                }
            }
            if !self.has_validated_offer(addr) {
                self.enqueue_need_query(addr);
            }
        }

        let core_fact = core_fact(id, &edges);
        let core_ctx = self.core_validated_offers();
        if !fact_ready_core(&core_fact, &core_ctx) {
            self.validity.insert(id, Validity::Invalid);
            return Ok(Some(Validity::Invalid));
        }

        let admitted = Admitted::from_parts(item, id);
        let out = P::project(&admitted, self.collect(&edges), &mut self.projector_state);
        let plan = project_fact_core(&core_fact, &core_ctx, core_validity(out.validity));
        let effective_validity = if plan.valid {
            Validity::Valid
        } else {
            Validity::Invalid
        };
        self.validity.insert(id, effective_validity);

        if effective_validity == Validity::Valid {
            for offer in edges.iter().copied().filter(|edge| edge.is_offer()) {
                let addr = EdgeAddr::from_offer(&offer);
                if !self.promoted_offers.insert((id, addr)) {
                    continue;
                }
                let validated = ValidatedOffer {
                    owner: id,
                    offer: offer.validate(),
                };
                self.validated.push(validated);
                self.validated_by_addr
                    .entry(addr)
                    .or_default()
                    .push(validated);
                for needer in self.mem.needers(addr) {
                    self.enqueue_project_if_not_valid(needer);
                }
                self.enqueue_offer_query(addr);
            }
        }

        if effective_validity == Validity::Valid {
            for emitted in out.emitted {
                let id = fact_id(&emitted.bytes);
                let item = P::decode(&emitted.bytes)?;
                if P::encode(&item) != emitted.bytes {
                    return Err("projector emitted non-canonical bytes".to_string());
                }
                self.index_item(id, item);
            }
        }

        Ok(Some(effective_validity))
    }

    pub fn has_pending_work(&self) -> bool {
        !self.to_admit.is_empty()
            || !self.to_project.is_empty()
            || !self.need_queries.is_empty()
            || !self.offer_queries.is_empty()
    }

    fn enqueue_need_query(&mut self, addr: EdgeAddr) {
        if self.queued_need_queries.insert(addr) {
            self.need_queries.push_back(addr);
        }
    }

    fn enqueue_offer_query(&mut self, addr: EdgeAddr) {
        if self.queued_offer_queries.insert(addr) {
            self.offer_queries.push_back(addr);
        }
    }

    fn enqueue_project_if_unseen(&mut self, id: FactId) {
        if !self.validity.contains_key(&id) {
            self.enqueue_project(id);
        }
    }

    fn enqueue_project_if_not_valid(&mut self, id: FactId) {
        if self.validity.get(&id) != Some(&Validity::Valid) {
            self.enqueue_project(id);
        }
    }

    pub(crate) fn pop_admit_request(&mut self) -> Option<FactId> {
        let id = self.to_admit.pop_front()?;
        self.queued_admit.remove(&id);
        Some(id)
    }

    pub(crate) fn pop_need_query_request(&mut self) -> Option<EdgeAddr> {
        let addr = self.need_queries.pop_front()?;
        self.queued_need_queries.remove(&addr);
        Some(addr)
    }

    pub(crate) fn pop_project_request(&mut self) -> Option<FactId> {
        let id = self.to_project.pop_front()?;
        self.queued_project.remove(&id);
        Some(id)
    }

    pub(crate) fn pop_offer_query_request(&mut self) -> Option<EdgeAddr> {
        let addr = self.offer_queries.pop_front()?;
        self.queued_offer_queries.remove(&addr);
        Some(addr)
    }

    pub(crate) fn enqueue_loaded_offerers(&mut self, ids: Vec<FactId>) {
        for id in ids {
            self.enqueue_admit(id);
            self.enqueue_project_if_unseen(id);
        }
    }

    pub(crate) fn enqueue_loaded_needers(&mut self, ids: Vec<FactId>) {
        for id in ids {
            self.enqueue_admit(id);
            self.enqueue_project_if_not_valid(id);
        }
    }

    fn has_validated_offer(&self, addr: EdgeAddr) -> bool {
        self.validated_by_addr
            .get(&addr)
            .is_some_and(|offers| !offers.is_empty())
    }

    fn collect(&self, edges: &[Offer<Asserted>]) -> Context {
        let mut offers = vec![];
        for need in edges.iter().filter(|edge| edge.is_need()) {
            let addr = EdgeAddr::from_offer(need);
            for vo in self.validated_by_addr.get(&addr).into_iter().flatten() {
                offers.push(vo.offer);
            }
        }
        Context::from(offers)
    }

    fn core_validated_offers(&self) -> Vec<ValidatedOfferCore> {
        self.validated
            .iter()
            .map(|validated| ValidatedOfferCore {
                owner: core_bytes32(validated.owner),
                addr: core_addr(EdgeAddr::from_offer(&validated.offer)),
            })
            .collect()
    }
}

fn core_fact(id: FactId, edges: &[Offer<Asserted>]) -> AdmittedFactCore {
    AdmittedFactCore {
        id: core_bytes32(id),
        needs: edges
            .iter()
            .filter(|edge| edge.is_need())
            .map(|edge| core_addr(EdgeAddr::from_offer(edge)))
            .collect(),
        offers: edges
            .iter()
            .filter(|edge| edge.is_offer())
            .map(|edge| core_addr(EdgeAddr::from_offer(edge)))
            .collect(),
        fields: vec![],
    }
}

fn core_addr(addr: EdgeAddr) -> EdgeAddrCore {
    EdgeAddrCore {
        role: core_role(addr.role),
        scope: core_scope(addr.scope),
        key: core_bytes32(addr.key.0),
    }
}

fn core_validity(validity: Validity) -> ValidityCore {
    match validity {
        Validity::Valid => ValidityCore::Valid,
        Validity::Invalid => ValidityCore::Invalid,
    }
}

fn core_scope(scope: Scope) -> u64 {
    match scope {
        Scope::Local => 0,
    }
}

fn core_role(role: Role) -> Bytes32Core {
    // The verified core uses fixed-width addresses; role names enter it under
    // the same blake3 collision assumption as content-addressed fact ids.
    core_bytes32(role_id(role.0))
}

fn core_bytes32(bytes: [u8; 32]) -> Bytes32Core {
    Bytes32Core {
        w0: word64(&bytes, 0),
        w1: word64(&bytes, 8),
        w2: word64(&bytes, 16),
        w3: word64(&bytes, 24),
    }
}

fn word64(bytes: &[u8; 32], offset: usize) -> u64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(word)
}
