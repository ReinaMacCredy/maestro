use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::super::closed::AuthorityContextKindV1;
use super::super::identity::{
    AuthorityContextIdV1, AuthorityContinuityManifestIdV1, StateTokenIdV1,
};
use super::allocation::StoreAllocatedContinuityStateTokenV1;
use super::catalog::{ContinuityClassIdV1, ContinuityReferenceV1, ContinuitySemanticOwnerV1};
use super::totality::{AuthorityContinuityManifestV1, ClassDispositionV1};
use super::trusted_time::{
    AcceptedAuthorityTimeFloorV1, HTimeAcceptanceRelationV1, HTimeCarryBasisV1,
};
use super::{InstallationAuthorityContinuityClassV1, RepositoryAuthorityContinuityClassV1};

const MAX_CLOSURE_ITEMS: usize = 4_096;
const MAX_GRAPH_EDGES: usize = 8_192;
const MAX_FACET_ITEMS: usize = 4_096;
const SUPPORTED_PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ContinuityClosureFacetV1 {
    CanonicalRecords = 1,
    Graph = 2,
    Replay = 3,
    HistoricalSpend = 4,
    UnresolvedEffect = 5,
}

impl ContinuityClosureFacetV1 {
    pub const ALL: [Self; 5] = [
        Self::CanonicalRecords,
        Self::Graph,
        Self::Replay,
        Self::HistoricalSpend,
        Self::UnresolvedEffect,
    ];
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinuityCarrierProfileStatusV1 {
    Confirmed {
        profile: ContinuityReferenceV1,
        accepted_prefix: ContinuityReferenceV1,
        handoff_state: ContinuityReferenceV1,
        fence: ContinuityReferenceV1,
        currentness: ContinuityReferenceV1,
    },
    Uncertain,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuitySemanticCutV1 {
    pub cut_sequence: u64,
    pub source_store_generation: u64,
    pub successor_store_generation: u64,
    pub authority_epoch: u64,
    pub stable_lineage: ContinuityReferenceV1,
    pub selected_trusted_time_stack: ContinuityReferenceV1,
    pub carrier_profile: ContinuityCarrierProfileStatusV1,
    pub accepted_time: AcceptedAuthorityTimeFloorV1,
    pub lane_state_closure_root: ContinuityReferenceV1,
    pub source_floor_root: ContinuityReferenceV1,
    pub gap_companions: Vec<ContinuityReferenceV1>,
    pub floor_provenance: Vec<ContinuityReferenceV1>,
    pub external_revision_cells: Vec<ContinuityReferenceV1>,
    pub cma_remaining_root: ContinuityReferenceV1,
    pub cma_spent_root: ContinuityReferenceV1,
    pub canonical_records: Vec<ContinuityReferenceV1>,
    pub graph_nodes: Vec<ContinuityReferenceV1>,
    pub replay_items: Vec<ContinuityReferenceV1>,
    pub historical_spend_items: Vec<ContinuityReferenceV1>,
    pub unresolved_effects: Vec<ContinuityReferenceV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityContinuityPredecessorV1 {
    ContextGenesis {
        origin_commitment: ContinuityReferenceV1,
    },
    PriorClosure {
        closure_id: AuthorityContinuityClosureIdV1,
        state_token: StateTokenIdV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuityExactRootV1 {
    pub root: ContinuityReferenceV1,
    pub declared_count: u16,
    pub items: Vec<ContinuityReferenceV1>,
}

impl ContinuityExactRootV1 {
    pub(crate) fn new(
        class_id: ContinuityClassIdV1,
        facet: ContinuityClosureFacetV1,
        cut_sequence: u64,
        mut items: Vec<ContinuityReferenceV1>,
    ) -> Result<Self, AuthorityContinuityClosureError> {
        items.sort_unstable();
        if cut_sequence == 0
            || items.len() > MAX_FACET_ITEMS
            || items.iter().any(|item| item.as_bytes() == &[0; 32])
            || items.iter().copied().collect::<BTreeSet<_>>().len() != items.len()
        {
            return Err(AuthorityContinuityClosureError::InvalidExactRoot);
        }
        let declared_count = u16::try_from(items.len())
            .map_err(|_| AuthorityContinuityClosureError::InvalidExactRoot)?;
        let value = CborValue::Array(vec![
            CborValue::text("maestro.vnext.authority-continuity-exact-root.v1")?,
            class_id.schema_value(),
            CborValue::Unsigned(facet as u64),
            CborValue::Unsigned(cut_sequence),
            reference_array(&items),
        ]);
        Ok(Self {
            root: ContinuityReferenceV1::from_digest(hash(&value)?),
            declared_count,
            items,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClosureFacetDispositionKindV1 {
    ContributesExactRoot(ContinuityExactRootV1),
    DerivedCheck {
        invariant: ContinuityReferenceV1,
        proof: ContinuityReferenceV1,
    },
    NotApplicable {
        owner_invariant: ContinuityReferenceV1,
        proof: ContinuityReferenceV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityFacetDispositionV1 {
    pub facet: ContinuityClosureFacetV1,
    pub disposition: ClosureFacetDispositionKindV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityClassClosureV1 {
    pub class_id: ContinuityClassIdV1,
    pub owner: ContinuitySemanticOwnerV1,
    pub facets: Vec<AuthorityContinuityFacetDispositionV1>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ContinuityGraphEdgeV1 {
    pub from: ContinuityReferenceV1,
    pub to: ContinuityReferenceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityClosureInputV1 {
    pub manifest_id: AuthorityContinuityManifestIdV1,
    pub context_kind: AuthorityContextKindV1,
    pub context_id: AuthorityContextIdV1,
    pub predecessor: AuthorityContinuityPredecessorV1,
    pub semantic_cut: AuthorityContinuitySemanticCutV1,
    pub class_entries: Vec<AuthorityContinuityClassClosureV1>,
    pub graph_edges: Vec<ContinuityGraphEdgeV1>,
    pub protocol_version: u16,
}

#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AuthorityContinuityClosureIdV1([u8; 32]);

impl AuthorityContinuityClosureIdV1 {
    pub const fn from_digest(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn render(&self) -> String {
        let mut rendered = String::with_capacity(71);
        rendered.push_str("sha256:");
        for byte in self.0 {
            use std::fmt::Write;
            write!(&mut rendered, "{byte:02x}")
                .expect("invariant: writing hexadecimal into String cannot fail");
        }
        rendered
    }
}

impl fmt::Debug for AuthorityContinuityClosureIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AuthorityContinuityClosureIdV1")
            .field(&self.render())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityClosureV1 {
    id: AuthorityContinuityClosureIdV1,
    manifest_id: AuthorityContinuityManifestIdV1,
    context_kind: AuthorityContextKindV1,
    context_id: AuthorityContextIdV1,
    predecessor: AuthorityContinuityPredecessorV1,
    successor_state_token: StateTokenIdV1,
    store_allocation_commitment: ContinuityReferenceV1,
    store_publication_clock: u64,
    semantic_cut: AuthorityContinuitySemanticCutV1,
    class_entries: Vec<AuthorityContinuityClassClosureV1>,
    graph_edges: Vec<ContinuityGraphEdgeV1>,
    protocol_version: u16,
}

impl AuthorityContinuityClosureV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.authority-continuity-closure.v1";

    pub(crate) fn prove(
        manifest: &AuthorityContinuityManifestV1,
        mut input: AuthorityContinuityClosureInputV1,
        allocation: &StoreAllocatedContinuityStateTokenV1,
    ) -> Result<Self, AuthorityContinuityClosureError> {
        validate_closure(manifest, &input, allocation)?;
        normalize_closure(&mut input);
        let mut closure = Self {
            id: AuthorityContinuityClosureIdV1::from_digest([0; 32]),
            manifest_id: input.manifest_id,
            context_kind: input.context_kind,
            context_id: input.context_id,
            predecessor: input.predecessor,
            successor_state_token: allocation.successor_state_token(),
            store_allocation_commitment: allocation.allocation_commitment(),
            store_publication_clock: allocation.store_publication_clock(),
            semantic_cut: input.semantic_cut,
            class_entries: input.class_entries,
            graph_edges: input.graph_edges,
            protocol_version: input.protocol_version,
        };
        closure.id = AuthorityContinuityClosureIdV1::from_digest(hash(&closure.schema_value()?)?);
        Ok(closure)
    }

    pub fn decode(
        bytes: &[u8],
        manifest: &AuthorityContinuityManifestV1,
    ) -> Result<Self, AuthorityContinuityClosureError> {
        let value = deterministic_cbor::decode(bytes)?;
        let fields = exact_array(&value, 10)?;
        if exact_text(&fields[0])? != Self::SCHEMA_DOMAIN
            || exact_unsigned(&fields[1])? != u64::from(SUPPORTED_PROTOCOL_VERSION)
        {
            return Err(AuthorityContinuityClosureError::UnsupportedVersion);
        }
        let manifest_id = AuthorityContinuityManifestIdV1::from_digest(exact_digest(&fields[2])?);
        let context_kind = AuthorityContextKindV1::try_from(exact_u8(&fields[3])?)
            .map_err(|_| AuthorityContinuityClosureError::DecodeMalformed)?;
        let context_id = AuthorityContextIdV1::from_digest(exact_digest(&fields[4])?);
        let predecessor = parse_predecessor(&fields[5])?;
        let allocation = exact_array(&fields[6], 3)?;
        let successor_state_token = StateTokenIdV1::from_digest(exact_digest(&allocation[0])?);
        let store_allocation_commitment =
            ContinuityReferenceV1::from_digest(exact_digest(&allocation[1])?);
        let store_publication_clock = exact_unsigned(&allocation[2])?;
        let semantic_cut = parse_semantic_cut(&fields[7])?;
        let class_entries = exact_array_any(&fields[8])?
            .iter()
            .map(parse_class_entry)
            .collect::<Result<Vec<_>, _>>()?;
        let graph_edges = exact_array_any(&fields[9])?
            .iter()
            .map(parse_graph_edge)
            .collect::<Result<Vec<_>, _>>()?;
        let input = AuthorityContinuityClosureInputV1 {
            manifest_id,
            context_kind,
            context_id,
            predecessor,
            semantic_cut,
            class_entries,
            graph_edges,
            protocol_version: SUPPORTED_PROTOCOL_VERSION,
        };
        validate_semantic_closure(manifest, &input)?;
        let predecessor_token = predecessor_state_token(predecessor);
        if store_publication_clock == 0
            || successor_state_token.as_bytes() == &[0_u8; 32]
            || store_allocation_commitment.as_bytes() == &[0_u8; 32]
            || predecessor_token == Some(successor_state_token)
        {
            return Err(AuthorityContinuityClosureError::StoreAllocationMismatch);
        }
        let closure = Self {
            id: AuthorityContinuityClosureIdV1::from_digest(hash(&value)?),
            manifest_id,
            context_kind,
            context_id,
            predecessor,
            successor_state_token,
            store_allocation_commitment,
            store_publication_clock,
            semantic_cut: input.semantic_cut,
            class_entries: input.class_entries,
            graph_edges: input.graph_edges,
            protocol_version: SUPPORTED_PROTOCOL_VERSION,
        };
        if closure.canonical_bytes()? != bytes {
            return Err(AuthorityContinuityClosureError::NonCanonicalClosure);
        }
        Ok(closure)
    }

    pub const fn id(&self) -> AuthorityContinuityClosureIdV1 {
        self.id
    }

    pub const fn manifest_id(&self) -> AuthorityContinuityManifestIdV1 {
        self.manifest_id
    }

    pub const fn context_kind(&self) -> AuthorityContextKindV1 {
        self.context_kind
    }

    pub const fn context_id(&self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn predecessor(&self) -> AuthorityContinuityPredecessorV1 {
        self.predecessor
    }

    pub fn predecessor_state_token(&self) -> Option<StateTokenIdV1> {
        match self.predecessor {
            AuthorityContinuityPredecessorV1::ContextGenesis { .. } => None,
            AuthorityContinuityPredecessorV1::PriorClosure { state_token, .. } => Some(state_token),
        }
    }

    pub const fn successor_state_token(&self) -> StateTokenIdV1 {
        self.successor_state_token
    }

    pub const fn store_allocation_commitment(&self) -> ContinuityReferenceV1 {
        self.store_allocation_commitment
    }

    pub const fn store_publication_clock(&self) -> u64 {
        self.store_publication_clock
    }

    pub const fn store_generation(&self) -> u64 {
        self.semantic_cut.successor_store_generation
    }

    pub const fn source_store_generation(&self) -> u64 {
        self.semantic_cut.source_store_generation
    }

    pub const fn authority_epoch(&self) -> u64 {
        self.semantic_cut.authority_epoch
    }

    pub const fn cut_sequence(&self) -> u64 {
        self.semantic_cut.cut_sequence
    }

    pub fn carrier_profile(&self) -> &ContinuityCarrierProfileStatusV1 {
        &self.semantic_cut.carrier_profile
    }

    pub fn accepted_time(&self) -> &AcceptedAuthorityTimeFloorV1 {
        &self.semantic_cut.accepted_time
    }

    pub const fn selected_trusted_time_stack(&self) -> ContinuityReferenceV1 {
        self.semantic_cut.selected_trusted_time_stack
    }

    pub const fn lane_state_closure_root(&self) -> ContinuityReferenceV1 {
        self.semantic_cut.lane_state_closure_root
    }

    pub const fn source_floor_root(&self) -> ContinuityReferenceV1 {
        self.semantic_cut.source_floor_root
    }

    pub fn gap_companions(&self) -> &[ContinuityReferenceV1] {
        &self.semantic_cut.gap_companions
    }

    pub fn floor_provenance(&self) -> &[ContinuityReferenceV1] {
        &self.semantic_cut.floor_provenance
    }

    pub fn external_revision_cells(&self) -> &[ContinuityReferenceV1] {
        &self.semantic_cut.external_revision_cells
    }

    pub const fn cma_remaining_root(&self) -> ContinuityReferenceV1 {
        self.semantic_cut.cma_remaining_root
    }

    pub const fn cma_spent_root(&self) -> ContinuityReferenceV1 {
        self.semantic_cut.cma_spent_root
    }

    pub fn unresolved_effects(&self) -> &[ContinuityReferenceV1] {
        &self.semantic_cut.unresolved_effects
    }

    pub fn canonical_records(&self) -> &[ContinuityReferenceV1] {
        &self.semantic_cut.canonical_records
    }

    pub fn graph_nodes(&self) -> &[ContinuityReferenceV1] {
        &self.semantic_cut.graph_nodes
    }

    pub fn replay_items(&self) -> &[ContinuityReferenceV1] {
        &self.semantic_cut.replay_items
    }

    pub fn historical_spend_items(&self) -> &[ContinuityReferenceV1] {
        &self.semantic_cut.historical_spend_items
    }

    pub fn graph_edges(&self) -> &[ContinuityGraphEdgeV1] {
        &self.graph_edges
    }

    pub fn class_entries(&self) -> &[AuthorityContinuityClassClosureV1] {
        &self.class_entries
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Unsigned(u64::from(self.protocol_version)),
            CborValue::Bytes(self.manifest_id.as_bytes().to_vec()),
            CborValue::Unsigned(self.context_kind as u64),
            CborValue::Bytes(self.context_id.as_bytes().to_vec()),
            predecessor_value(self.predecessor),
            CborValue::Array(vec![
                CborValue::Bytes(self.successor_state_token.as_bytes().to_vec()),
                CborValue::Bytes(self.store_allocation_commitment.as_bytes().to_vec()),
                CborValue::Unsigned(self.store_publication_clock),
            ]),
            semantic_cut_value(&self.semantic_cut),
            CborValue::Array(self.class_entries.iter().map(class_entry_value).collect()),
            CborValue::Array(self.graph_edges.iter().map(graph_edge_value).collect()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityContinuityClosureError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error("continuity closure protocol version is unsupported")]
    UnsupportedVersion,
    #[error("continuity closure does not bind the proven manifest and context")]
    ManifestMismatch,
    #[error("continuity closure does not bind the exact sealed Store successor allocation")]
    StoreAllocationMismatch,
    #[error("continuity closure semantic cut is zero, duplicate, or exceeds a finite bound")]
    InvalidSemanticCut,
    #[error("the exact external carrier profile/fence/currentness is unavailable")]
    CarrierProfileUnavailable,
    #[error("the typed class entries differ from the proven closed class sum")]
    ClassEntryMismatch,
    #[error("a class closure names the wrong source-record semantic owner")]
    WrongOwner,
    #[error("a class does not have exactly one disposition for every closed facet")]
    FacetTotalityMismatch,
    #[error("CanonicalRecordClosure requires one exact canonical-record root")]
    CanonicalRecordFacetRequired,
    #[error("DerivedOnly contributes no independently persisted continuity root")]
    DerivedOnlyContributedRoot,
    #[error("facet roots and the coherent semantic-cut inventory are not exactly equal")]
    FacetInventoryMismatch,
    #[error("a facet root count, item set, or global contribution is duplicate or invalid")]
    InvalidExactRoot,
    #[error("a graph edge references an orphan endpoint")]
    OrphanGraphEndpoint,
    #[error("the typed continuity graph contains a cycle")]
    CyclicGraph,
    #[error("continuity closure bytes are malformed")]
    DecodeMalformed,
    #[error("continuity closure bytes are not the exact canonical encoding")]
    NonCanonicalClosure,
}

fn validate_closure(
    manifest: &AuthorityContinuityManifestV1,
    input: &AuthorityContinuityClosureInputV1,
    allocation: &StoreAllocatedContinuityStateTokenV1,
) -> Result<(), AuthorityContinuityClosureError> {
    validate_semantic_closure(manifest, input)?;
    if allocation.context_id() != input.context_id
        || allocation.store_generation() != input.semantic_cut.successor_store_generation
        || allocation.expected_predecessor() != predecessor_state_token(input.predecessor)
    {
        return Err(AuthorityContinuityClosureError::StoreAllocationMismatch);
    }
    Ok(())
}

fn validate_semantic_closure(
    manifest: &AuthorityContinuityManifestV1,
    input: &AuthorityContinuityClosureInputV1,
) -> Result<(), AuthorityContinuityClosureError> {
    if input.protocol_version != SUPPORTED_PROTOCOL_VERSION {
        return Err(AuthorityContinuityClosureError::UnsupportedVersion);
    }
    if input.manifest_id != manifest.id()
        || input.context_kind != manifest.context_kind()
        || input.class_entries.len() != manifest.class_count()
    {
        return Err(AuthorityContinuityClosureError::ManifestMismatch);
    }
    let cut = &input.semantic_cut;
    let generation_lineage_valid = match input.predecessor {
        AuthorityContinuityPredecessorV1::ContextGenesis { .. } => {
            cut.source_store_generation == 0 && cut.successor_store_generation == 1
        }
        AuthorityContinuityPredecessorV1::PriorClosure { .. } => {
            cut.source_store_generation != 0
                && cut.source_store_generation.checked_add(1)
                    == Some(cut.successor_store_generation)
        }
    };
    if cut.cut_sequence == 0
        || !generation_lineage_valid
        || cut.authority_epoch == 0
        || cut.accepted_time.stable_lineage() != cut.stable_lineage
        || cut.accepted_time.policy_stack() != cut.selected_trusted_time_stack
        || inventories(cut)
            .iter()
            .any(|items| items.len() > MAX_CLOSURE_ITEMS)
        || inventories(cut)
            .iter()
            .any(|items| items.iter().copied().collect::<BTreeSet<_>>().len() != items.len())
        || semantic_record_families(cut)
            .iter()
            .any(|items| items.len() > MAX_CLOSURE_ITEMS)
        || semantic_record_families(cut)
            .iter()
            .any(|items| items.iter().copied().collect::<BTreeSet<_>>().len() != items.len())
    {
        return Err(AuthorityContinuityClosureError::InvalidSemanticCut);
    }
    if !matches!(
        cut.carrier_profile,
        ContinuityCarrierProfileStatusV1::Confirmed { .. }
    ) {
        return Err(AuthorityContinuityClosureError::CarrierProfileUnavailable);
    }

    let mut entries = BTreeMap::new();
    for entry in &input.class_entries {
        if entries.insert(entry.class_id, entry).is_some() {
            return Err(AuthorityContinuityClosureError::ClassEntryMismatch);
        }
    }
    if entries.keys().copied().collect::<Vec<_>>() != manifest.class_ids() {
        return Err(AuthorityContinuityClosureError::ClassEntryMismatch);
    }

    let mut contributed = ContinuityClosureFacetV1::ALL
        .into_iter()
        .map(|facet| (facet, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for entry in &input.class_entries {
        let descriptor = manifest
            .descriptor(entry.class_id)
            .ok_or(AuthorityContinuityClosureError::ClassEntryMismatch)?;
        if entry.owner != descriptor.owner {
            return Err(AuthorityContinuityClosureError::WrongOwner);
        }
        let mut facets = BTreeMap::new();
        for facet in &entry.facets {
            if facets.insert(facet.facet, &facet.disposition).is_some() {
                return Err(AuthorityContinuityClosureError::FacetTotalityMismatch);
            }
        }
        if facets.keys().copied().collect::<Vec<_>>() != ContinuityClosureFacetV1::ALL {
            return Err(AuthorityContinuityClosureError::FacetTotalityMismatch);
        }
        if descriptor.disposition == ClassDispositionV1::CanonicalRecordClosure
            && !matches!(
                facets[&ContinuityClosureFacetV1::CanonicalRecords],
                ClosureFacetDispositionKindV1::ContributesExactRoot(_)
            )
        {
            return Err(AuthorityContinuityClosureError::CanonicalRecordFacetRequired);
        }
        for (facet, disposition) in facets {
            if descriptor.disposition == ClassDispositionV1::DerivedOnly
                && matches!(
                    disposition,
                    ClosureFacetDispositionKindV1::ContributesExactRoot(_)
                )
            {
                return Err(AuthorityContinuityClosureError::DerivedOnlyContributedRoot);
            }
            if let ClosureFacetDispositionKindV1::ContributesExactRoot(root) = disposition {
                let expected_root = ContinuityExactRootV1::new(
                    entry.class_id,
                    facet,
                    input.semantic_cut.cut_sequence,
                    root.items.clone(),
                )?;
                if &expected_root != root {
                    return Err(AuthorityContinuityClosureError::InvalidExactRoot);
                }
                let inventory = contributed
                    .get_mut(&facet)
                    .expect("invariant: every closed facet has an inventory");
                for item in &root.items {
                    if !inventory.insert(*item) {
                        return Err(AuthorityContinuityClosureError::InvalidExactRoot);
                    }
                }
            }
        }
    }

    let expected = [
        (
            ContinuityClosureFacetV1::CanonicalRecords,
            &cut.canonical_records,
        ),
        (ContinuityClosureFacetV1::Graph, &cut.graph_nodes),
        (ContinuityClosureFacetV1::Replay, &cut.replay_items),
        (
            ContinuityClosureFacetV1::HistoricalSpend,
            &cut.historical_spend_items,
        ),
        (
            ContinuityClosureFacetV1::UnresolvedEffect,
            &cut.unresolved_effects,
        ),
    ];
    for (facet, items) in expected {
        if contributed[&facet] != items.iter().copied().collect::<BTreeSet<_>>() {
            return Err(AuthorityContinuityClosureError::FacetInventoryMismatch);
        }
    }
    validate_graph(&cut.graph_nodes, &input.graph_edges)
}

fn predecessor_state_token(
    predecessor: AuthorityContinuityPredecessorV1,
) -> Option<StateTokenIdV1> {
    match predecessor {
        AuthorityContinuityPredecessorV1::ContextGenesis { .. } => None,
        AuthorityContinuityPredecessorV1::PriorClosure { state_token, .. } => Some(state_token),
    }
}

fn validate_graph(
    nodes: &[ContinuityReferenceV1],
    edges: &[ContinuityGraphEdgeV1],
) -> Result<(), AuthorityContinuityClosureError> {
    if edges.len() > MAX_GRAPH_EDGES
        || edges.iter().copied().collect::<BTreeSet<_>>().len() != edges.len()
    {
        return Err(AuthorityContinuityClosureError::InvalidExactRoot);
    }
    let nodes = nodes.iter().copied().collect::<BTreeSet<_>>();
    let mut indegree = nodes
        .iter()
        .map(|node| (*node, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<ContinuityReferenceV1, Vec<ContinuityReferenceV1>>::new();
    for edge in edges {
        if !nodes.contains(&edge.from) || !nodes.contains(&edge.to) {
            return Err(AuthorityContinuityClosureError::OrphanGraphEndpoint);
        }
        *indegree
            .get_mut(&edge.to)
            .expect("invariant: endpoint totality was checked") += 1;
        dependents.entry(edge.from).or_default().push(edge.to);
    }
    let mut queue = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(*node))
        .collect::<VecDeque<_>>();
    let mut visited = 0;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        for dependent in dependents.get(&node).into_iter().flatten() {
            let degree = indegree
                .get_mut(dependent)
                .expect("invariant: dependent belongs to graph");
            *degree -= 1;
            if *degree == 0 {
                queue.push_back(*dependent);
            }
        }
    }
    if visited == nodes.len() {
        Ok(())
    } else {
        Err(AuthorityContinuityClosureError::CyclicGraph)
    }
}

fn normalize_closure(input: &mut AuthorityContinuityClosureInputV1) {
    input.class_entries.sort_by_key(|entry| entry.class_id);
    input.graph_edges.sort_unstable();
    for items in inventories_mut(&mut input.semantic_cut) {
        items.sort_unstable();
    }
    for items in semantic_record_families_mut(&mut input.semantic_cut) {
        items.sort_unstable();
    }
    for entry in &mut input.class_entries {
        entry.facets.sort_by_key(|facet| facet.facet);
        for facet in &mut entry.facets {
            if let ClosureFacetDispositionKindV1::ContributesExactRoot(root) =
                &mut facet.disposition
            {
                root.items.sort_unstable();
            }
        }
    }
}

fn semantic_record_families(
    cut: &AuthorityContinuitySemanticCutV1,
) -> [&Vec<ContinuityReferenceV1>; 3] {
    [
        &cut.gap_companions,
        &cut.floor_provenance,
        &cut.external_revision_cells,
    ]
}

fn semantic_record_families_mut(
    cut: &mut AuthorityContinuitySemanticCutV1,
) -> [&mut Vec<ContinuityReferenceV1>; 3] {
    [
        &mut cut.gap_companions,
        &mut cut.floor_provenance,
        &mut cut.external_revision_cells,
    ]
}

fn inventories(cut: &AuthorityContinuitySemanticCutV1) -> [&Vec<ContinuityReferenceV1>; 5] {
    [
        &cut.canonical_records,
        &cut.graph_nodes,
        &cut.replay_items,
        &cut.historical_spend_items,
        &cut.unresolved_effects,
    ]
}

fn inventories_mut(
    cut: &mut AuthorityContinuitySemanticCutV1,
) -> [&mut Vec<ContinuityReferenceV1>; 5] {
    [
        &mut cut.canonical_records,
        &mut cut.graph_nodes,
        &mut cut.replay_items,
        &mut cut.historical_spend_items,
        &mut cut.unresolved_effects,
    ]
}

fn predecessor_value(predecessor: AuthorityContinuityPredecessorV1) -> CborValue {
    match predecessor {
        AuthorityContinuityPredecessorV1::ContextGenesis { origin_commitment } => {
            CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Bytes(origin_commitment.as_bytes().to_vec()),
            ])
        }
        AuthorityContinuityPredecessorV1::PriorClosure {
            closure_id,
            state_token,
        } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Bytes(closure_id.as_bytes().to_vec()),
            CborValue::Bytes(state_token.as_bytes().to_vec()),
        ]),
    }
}

fn semantic_cut_value(cut: &AuthorityContinuitySemanticCutV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(cut.cut_sequence),
        CborValue::Unsigned(cut.source_store_generation),
        CborValue::Unsigned(cut.successor_store_generation),
        CborValue::Unsigned(cut.authority_epoch),
        CborValue::Bytes(cut.stable_lineage.as_bytes().to_vec()),
        CborValue::Bytes(cut.selected_trusted_time_stack.as_bytes().to_vec()),
        carrier_value(&cut.carrier_profile),
        accepted_time_value(&cut.accepted_time),
        CborValue::Bytes(cut.lane_state_closure_root.as_bytes().to_vec()),
        CborValue::Bytes(cut.source_floor_root.as_bytes().to_vec()),
        reference_array(&cut.gap_companions),
        reference_array(&cut.floor_provenance),
        reference_array(&cut.external_revision_cells),
        CborValue::Bytes(cut.cma_remaining_root.as_bytes().to_vec()),
        CborValue::Bytes(cut.cma_spent_root.as_bytes().to_vec()),
        reference_array(&cut.canonical_records),
        reference_array(&cut.graph_nodes),
        reference_array(&cut.replay_items),
        reference_array(&cut.historical_spend_items),
        reference_array(&cut.unresolved_effects),
    ])
}

fn carrier_value(carrier: &ContinuityCarrierProfileStatusV1) -> CborValue {
    match carrier {
        ContinuityCarrierProfileStatusV1::Confirmed {
            profile,
            accepted_prefix,
            handoff_state,
            fence,
            currentness,
        } => CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(profile.as_bytes().to_vec()),
            CborValue::Bytes(accepted_prefix.as_bytes().to_vec()),
            CborValue::Bytes(handoff_state.as_bytes().to_vec()),
            CborValue::Bytes(fence.as_bytes().to_vec()),
            CborValue::Bytes(currentness.as_bytes().to_vec()),
        ]),
        ContinuityCarrierProfileStatusV1::Uncertain => {
            CborValue::Array(vec![CborValue::Unsigned(2)])
        }
        ContinuityCarrierProfileStatusV1::Unavailable => {
            CborValue::Array(vec![CborValue::Unsigned(3)])
        }
    }
}

pub(crate) fn accepted_time_value(value: &AcceptedAuthorityTimeFloorV1) -> CborValue {
    let carry_basis = match value.carry_basis() {
        HTimeCarryBasisV1::ExactNoLineageChange => CborValue::Array(vec![CborValue::Unsigned(1)]),
        HTimeCarryBasisV1::CompleteCarryMapping { mapping } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Bytes(mapping.as_bytes().to_vec()),
        ]),
    };
    CborValue::Array(vec![
        CborValue::Bytes(value.stable_lineage().as_bytes().to_vec()),
        CborValue::Bytes(value.coordinate().as_bytes().to_vec()),
        CborValue::Bytes(value.policy_stack().as_bytes().to_vec()),
        CborValue::Unsigned(value.lower_bound()),
        CborValue::Unsigned(match value.relation() {
            HTimeAcceptanceRelationV1::ContextGenesis => 1,
            HTimeAcceptanceRelationV1::Same => 2,
            HTimeAcceptanceRelationV1::Advance => 3,
        }),
        carry_basis,
    ])
}

fn class_entry_value(entry: &AuthorityContinuityClassClosureV1) -> CborValue {
    CborValue::Array(vec![
        entry.class_id.schema_value(),
        CborValue::Unsigned(entry.owner as u64),
        CborValue::Array(entry.facets.iter().map(facet_value).collect()),
    ])
}

fn facet_value(facet: &AuthorityContinuityFacetDispositionV1) -> CborValue {
    let disposition = match &facet.disposition {
        ClosureFacetDispositionKindV1::ContributesExactRoot(root) => CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(root.root.as_bytes().to_vec()),
            CborValue::Unsigned(u64::from(root.declared_count)),
            reference_array(&root.items),
        ]),
        ClosureFacetDispositionKindV1::DerivedCheck { invariant, proof } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Bytes(invariant.as_bytes().to_vec()),
            CborValue::Bytes(proof.as_bytes().to_vec()),
        ]),
        ClosureFacetDispositionKindV1::NotApplicable {
            owner_invariant,
            proof,
        } => CborValue::Array(vec![
            CborValue::Unsigned(3),
            CborValue::Bytes(owner_invariant.as_bytes().to_vec()),
            CborValue::Bytes(proof.as_bytes().to_vec()),
        ]),
    };
    CborValue::Array(vec![CborValue::Unsigned(facet.facet as u64), disposition])
}

fn graph_edge_value(edge: &ContinuityGraphEdgeV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Bytes(edge.from.as_bytes().to_vec()),
        CborValue::Bytes(edge.to.as_bytes().to_vec()),
    ])
}

fn parse_predecessor(
    value: &CborValue,
) -> Result<AuthorityContinuityPredecessorV1, AuthorityContinuityClosureError> {
    match exact_array_any(value)? {
        [CborValue::Unsigned(1), origin] => Ok(AuthorityContinuityPredecessorV1::ContextGenesis {
            origin_commitment: ContinuityReferenceV1::from_digest(exact_digest(origin)?),
        }),
        [CborValue::Unsigned(2), closure_id, state_token] => {
            Ok(AuthorityContinuityPredecessorV1::PriorClosure {
                closure_id: AuthorityContinuityClosureIdV1::from_digest(exact_digest(closure_id)?),
                state_token: StateTokenIdV1::from_digest(exact_digest(state_token)?),
            })
        }
        _ => Err(AuthorityContinuityClosureError::DecodeMalformed),
    }
}

fn parse_semantic_cut(
    value: &CborValue,
) -> Result<AuthorityContinuitySemanticCutV1, AuthorityContinuityClosureError> {
    let fields = exact_array(value, 20)?;
    Ok(AuthorityContinuitySemanticCutV1 {
        cut_sequence: exact_unsigned(&fields[0])?,
        source_store_generation: exact_unsigned(&fields[1])?,
        successor_store_generation: exact_unsigned(&fields[2])?,
        authority_epoch: exact_unsigned(&fields[3])?,
        stable_lineage: ContinuityReferenceV1::from_digest(exact_digest(&fields[4])?),
        selected_trusted_time_stack: ContinuityReferenceV1::from_digest(exact_digest(&fields[5])?),
        carrier_profile: parse_carrier(&fields[6])?,
        accepted_time: parse_accepted_time(&fields[7])?,
        lane_state_closure_root: ContinuityReferenceV1::from_digest(exact_digest(&fields[8])?),
        source_floor_root: ContinuityReferenceV1::from_digest(exact_digest(&fields[9])?),
        gap_companions: parse_references(&fields[10])?,
        floor_provenance: parse_references(&fields[11])?,
        external_revision_cells: parse_references(&fields[12])?,
        cma_remaining_root: ContinuityReferenceV1::from_digest(exact_digest(&fields[13])?),
        cma_spent_root: ContinuityReferenceV1::from_digest(exact_digest(&fields[14])?),
        canonical_records: parse_references(&fields[15])?,
        graph_nodes: parse_references(&fields[16])?,
        replay_items: parse_references(&fields[17])?,
        historical_spend_items: parse_references(&fields[18])?,
        unresolved_effects: parse_references(&fields[19])?,
    })
}

fn parse_carrier(
    value: &CborValue,
) -> Result<ContinuityCarrierProfileStatusV1, AuthorityContinuityClosureError> {
    match exact_array_any(value)? {
        [
            CborValue::Unsigned(1),
            profile,
            accepted_prefix,
            handoff_state,
            fence,
            currentness,
        ] => Ok(ContinuityCarrierProfileStatusV1::Confirmed {
            profile: ContinuityReferenceV1::from_digest(exact_digest(profile)?),
            accepted_prefix: ContinuityReferenceV1::from_digest(exact_digest(accepted_prefix)?),
            handoff_state: ContinuityReferenceV1::from_digest(exact_digest(handoff_state)?),
            fence: ContinuityReferenceV1::from_digest(exact_digest(fence)?),
            currentness: ContinuityReferenceV1::from_digest(exact_digest(currentness)?),
        }),
        [CborValue::Unsigned(2)] => Ok(ContinuityCarrierProfileStatusV1::Uncertain),
        [CborValue::Unsigned(3)] => Ok(ContinuityCarrierProfileStatusV1::Unavailable),
        _ => Err(AuthorityContinuityClosureError::DecodeMalformed),
    }
}

fn parse_accepted_time(
    value: &CborValue,
) -> Result<AcceptedAuthorityTimeFloorV1, AuthorityContinuityClosureError> {
    let fields = exact_array(value, 6)?;
    let relation = match exact_u8(&fields[4])? {
        1 => HTimeAcceptanceRelationV1::ContextGenesis,
        2 => HTimeAcceptanceRelationV1::Same,
        3 => HTimeAcceptanceRelationV1::Advance,
        _ => return Err(AuthorityContinuityClosureError::DecodeMalformed),
    };
    let carry_basis = match exact_array_any(&fields[5])? {
        [CborValue::Unsigned(1)] => HTimeCarryBasisV1::ExactNoLineageChange,
        [CborValue::Unsigned(2), mapping] => HTimeCarryBasisV1::CompleteCarryMapping {
            mapping: ContinuityReferenceV1::from_digest(exact_digest(mapping)?),
        },
        _ => return Err(AuthorityContinuityClosureError::DecodeMalformed),
    };
    if relation == HTimeAcceptanceRelationV1::ContextGenesis
        && matches!(carry_basis, HTimeCarryBasisV1::ExactNoLineageChange)
    {
        return Err(AuthorityContinuityClosureError::DecodeMalformed);
    }
    Ok(AcceptedAuthorityTimeFloorV1::from_persisted_parts(
        ContinuityReferenceV1::from_digest(exact_digest(&fields[0])?),
        ContinuityReferenceV1::from_digest(exact_digest(&fields[1])?),
        ContinuityReferenceV1::from_digest(exact_digest(&fields[2])?),
        exact_unsigned(&fields[3])?,
        relation,
        carry_basis,
    ))
}

fn parse_class_entry(
    value: &CborValue,
) -> Result<AuthorityContinuityClassClosureV1, AuthorityContinuityClosureError> {
    let fields = exact_array(value, 3)?;
    Ok(AuthorityContinuityClassClosureV1 {
        class_id: parse_class_id(&fields[0])?,
        owner: parse_owner(&fields[1])?,
        facets: exact_array_any(&fields[2])?
            .iter()
            .map(parse_facet)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn parse_class_id(
    value: &CborValue,
) -> Result<ContinuityClassIdV1, AuthorityContinuityClosureError> {
    let fields = exact_array(value, 2)?;
    let context = AuthorityContextKindV1::try_from(exact_u8(&fields[0])?)
        .map_err(|_| AuthorityContinuityClosureError::DecodeMalformed)?;
    let tag = exact_u8(&fields[1])?;
    match context {
        AuthorityContextKindV1::RepositoryAuthorityContext => {
            RepositoryAuthorityContinuityClassV1::try_from(tag)
                .map(ContinuityClassIdV1::Repository)
                .map_err(|_| AuthorityContinuityClosureError::DecodeMalformed)
        }
        AuthorityContextKindV1::InstallationAuthorityContext => {
            InstallationAuthorityContinuityClassV1::try_from(tag)
                .map(ContinuityClassIdV1::Installation)
                .map_err(|_| AuthorityContinuityClosureError::DecodeMalformed)
        }
    }
}

fn parse_owner(
    value: &CborValue,
) -> Result<ContinuitySemanticOwnerV1, AuthorityContinuityClosureError> {
    Ok(match exact_u8(value)? {
        1 => ContinuitySemanticOwnerV1::Authority,
        2 => ContinuitySemanticOwnerV1::Work,
        3 => ContinuitySemanticOwnerV1::Contract,
        4 => ContinuitySemanticOwnerV1::Design,
        5 => ContinuitySemanticOwnerV1::Execution,
        6 => ContinuitySemanticOwnerV1::Evidence,
        7 => ContinuitySemanticOwnerV1::Planning,
        8 => ContinuitySemanticOwnerV1::Coordination,
        9 => ContinuitySemanticOwnerV1::Memory,
        10 => ContinuitySemanticOwnerV1::Intake,
        11 => ContinuitySemanticOwnerV1::Research,
        12 => ContinuitySemanticOwnerV1::Distribution,
        13 => ContinuitySemanticOwnerV1::Installation,
        14 => ContinuitySemanticOwnerV1::Persistence,
        _ => return Err(AuthorityContinuityClosureError::DecodeMalformed),
    })
}

fn parse_facet(
    value: &CborValue,
) -> Result<AuthorityContinuityFacetDispositionV1, AuthorityContinuityClosureError> {
    let fields = exact_array(value, 2)?;
    let facet = match exact_u8(&fields[0])? {
        1 => ContinuityClosureFacetV1::CanonicalRecords,
        2 => ContinuityClosureFacetV1::Graph,
        3 => ContinuityClosureFacetV1::Replay,
        4 => ContinuityClosureFacetV1::HistoricalSpend,
        5 => ContinuityClosureFacetV1::UnresolvedEffect,
        _ => return Err(AuthorityContinuityClosureError::DecodeMalformed),
    };
    let disposition_fields = exact_array_any(&fields[1])?;
    let disposition = match disposition_fields {
        [CborValue::Unsigned(1), root, declared_count, items] => {
            ClosureFacetDispositionKindV1::ContributesExactRoot(ContinuityExactRootV1 {
                root: ContinuityReferenceV1::from_digest(exact_digest(root)?),
                declared_count: exact_unsigned(declared_count)?
                    .try_into()
                    .map_err(|_| AuthorityContinuityClosureError::DecodeMalformed)?,
                items: parse_references(items)?,
            })
        }
        [CborValue::Unsigned(2), invariant, proof] => ClosureFacetDispositionKindV1::DerivedCheck {
            invariant: ContinuityReferenceV1::from_digest(exact_digest(invariant)?),
            proof: ContinuityReferenceV1::from_digest(exact_digest(proof)?),
        },
        [CborValue::Unsigned(3), invariant, proof] => {
            ClosureFacetDispositionKindV1::NotApplicable {
                owner_invariant: ContinuityReferenceV1::from_digest(exact_digest(invariant)?),
                proof: ContinuityReferenceV1::from_digest(exact_digest(proof)?),
            }
        }
        _ => return Err(AuthorityContinuityClosureError::DecodeMalformed),
    };
    Ok(AuthorityContinuityFacetDispositionV1 { facet, disposition })
}

fn parse_graph_edge(
    value: &CborValue,
) -> Result<ContinuityGraphEdgeV1, AuthorityContinuityClosureError> {
    let fields = exact_array(value, 2)?;
    Ok(ContinuityGraphEdgeV1 {
        from: ContinuityReferenceV1::from_digest(exact_digest(&fields[0])?),
        to: ContinuityReferenceV1::from_digest(exact_digest(&fields[1])?),
    })
}

fn parse_references(
    value: &CborValue,
) -> Result<Vec<ContinuityReferenceV1>, AuthorityContinuityClosureError> {
    exact_array_any(value)?
        .iter()
        .map(|value| exact_digest(value).map(ContinuityReferenceV1::from_digest))
        .collect()
}

fn exact_array(
    value: &CborValue,
    length: usize,
) -> Result<&[CborValue], AuthorityContinuityClosureError> {
    let values = exact_array_any(value)?;
    if values.len() == length {
        Ok(values)
    } else {
        Err(AuthorityContinuityClosureError::DecodeMalformed)
    }
}

fn exact_array_any(value: &CborValue) -> Result<&[CborValue], AuthorityContinuityClosureError> {
    match value {
        CborValue::Array(values) => Ok(values),
        _ => Err(AuthorityContinuityClosureError::DecodeMalformed),
    }
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], AuthorityContinuityClosureError> {
    match value {
        CborValue::Bytes(bytes) => bytes
            .as_slice()
            .try_into()
            .map_err(|_| AuthorityContinuityClosureError::DecodeMalformed),
        _ => Err(AuthorityContinuityClosureError::DecodeMalformed),
    }
}

fn exact_unsigned(value: &CborValue) -> Result<u64, AuthorityContinuityClosureError> {
    match value {
        CborValue::Unsigned(value) => Ok(*value),
        _ => Err(AuthorityContinuityClosureError::DecodeMalformed),
    }
}

fn exact_u8(value: &CborValue) -> Result<u8, AuthorityContinuityClosureError> {
    exact_unsigned(value)?
        .try_into()
        .map_err(|_| AuthorityContinuityClosureError::DecodeMalformed)
}

fn exact_text(value: &CborValue) -> Result<&str, AuthorityContinuityClosureError> {
    match value {
        CborValue::Text(value) => Ok(value),
        _ => Err(AuthorityContinuityClosureError::DecodeMalformed),
    }
}

pub(crate) fn reference_array(values: &[ContinuityReferenceV1]) -> CborValue {
    CborValue::Array(
        values
            .iter()
            .map(|value| CborValue::Bytes(value.as_bytes().to_vec()))
            .collect(),
    )
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}
