//! Frozen generated capability catalog consumed by the Stage-6 transport.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::integration::public_literals::{
    ActionSpecRefV1, CeremonySpecRefV1, OperationSpecRefV1,
};

const ACTION_CATALOG_JSON: &str =
    include_str!("../../../../../contracts/vnext/catalogs/generated/catalog-09-action-spec.json");
const CEREMONY_CATALOG_JSON: &str =
    include_str!("../../../../../contracts/vnext/catalogs/generated/catalog-05-ceremony.json");
const OBSERVATION_CATALOG_JSON: &str =
    include_str!("../../../../../contracts/vnext/catalogs/generated/catalog-01-observation.json");
const EFFECT_CATALOG_JSON: &str =
    include_str!("../../../../../contracts/vnext/catalogs/generated/catalog-02-effect.json");
const REPOSITORY_CAPACITY_CATALOG_JSON: &str = include_str!(
    "../../../../../contracts/vnext/catalogs/generated/catalog-03-repository-capacity.json"
);
const INSTALLATION_CAPACITY_CATALOG_JSON: &str = include_str!(
    "../../../../../contracts/vnext/catalogs/generated/catalog-04-installation-capacity.json"
);
const ACTION_LEAF_CATALOG_JSON: &str =
    include_str!("../../../../../contracts/vnext/catalogs/generated/catalog-06-action-leaf.json");
const REPOSITORY_CONTINUITY_CATALOG_JSON: &str = include_str!(
    "../../../../../contracts/vnext/catalogs/generated/catalog-07-repository-continuity.json"
);
const INSTALLATION_CONTINUITY_CATALOG_JSON: &str = include_str!(
    "../../../../../contracts/vnext/catalogs/generated/catalog-08-installation-continuity.json"
);
const GRAMMAR_JSON: &str = include_str!(
    "../../../../../contracts/vnext/catalogs/generated/catalog-profile-grammar-v1.json"
);
const PUBLIC_IDENTITY_JSON: &str = include_str!(
    "../../../../../contracts/vnext/stage0/public-identity/public-identity-closure.v1.json"
);

pub const ACTION_TAG_COUNT_V1: u64 = 145;
pub const CEREMONY_TAG_COUNT_V1: u64 = 11;
const _: () = assert!(CEREMONY_NAMES_V1.len() == CEREMONY_TAG_COUNT_V1 as usize);

const ACTION_CATALOG_SHA256: &str =
    "df15c67ba312585869836aaeda1c849b3bb84e98ba65baf78c51fde85473113c";
const CEREMONY_CATALOG_SHA256: &str =
    "efb01184d39c5384a2ddac2237ada963468f606d2b4a85b339b420635b4ffb3e";
const GRAMMAR_SHA256: &str = "38fd4e88ecaa185fc76f7dd20cf221304daf4a5e8058d8b5187d74828d1d5196";
const PUBLIC_IDENTITY_SHA256: &str =
    "8a6d87232657cce435d9322138c95b33c9855159963b6c2dadb637b7f94dc0c8";
const OWNER_RELATION_CATALOGS: [(&str, &str, usize); 9] = [
    (
        OBSERVATION_CATALOG_JSON,
        "3f9acd1efb25c1ade2555a538f603d9c09d7bc9349a7634263bee88fc5529091",
        43,
    ),
    (
        EFFECT_CATALOG_JSON,
        "39a1c4795b02933c83b54c17faafa23add55643501a5b9dd7154bc1ec0f3c910",
        23,
    ),
    (
        REPOSITORY_CAPACITY_CATALOG_JSON,
        "1fb9a54678cd1c05fa4d17546c395c15c46557bfabcdf6256a4b4719f95461ec",
        6,
    ),
    (
        INSTALLATION_CAPACITY_CATALOG_JSON,
        "2eb7f66d19786feb12368af6467c335466ea4c06c0b4c83c0d4025d00d7f1208",
        6,
    ),
    (
        CEREMONY_CATALOG_JSON,
        CEREMONY_CATALOG_SHA256,
        CEREMONY_TAG_COUNT_V1 as usize,
    ),
    (
        ACTION_LEAF_CATALOG_JSON,
        "034b914f1caaaa302f8f35286142863f082e7a1e7d244013db5041e378d06259",
        ACTION_TAG_COUNT_V1 as usize,
    ),
    (
        REPOSITORY_CONTINUITY_CATALOG_JSON,
        "1f4e78644fd1361e5a203e868809cccd7d86ea6cd8506bb981e5f1af48c5a8f0",
        35,
    ),
    (
        INSTALLATION_CONTINUITY_CATALOG_JSON,
        "800e581591b3c95ffb245fedcc1fa3613ae32027dffb10228e30e796626901c0",
        30,
    ),
    (
        ACTION_CATALOG_JSON,
        ACTION_CATALOG_SHA256,
        ACTION_TAG_COUNT_V1 as usize,
    ),
];
const OWNER_RELATION_TOTAL_ROWS: usize = 444;

pub const ACTION_SPEC_MANIFEST_REF_V1: &str =
    "sha256:7a7a1311690fcdec194c0d9c9afbbe4e28f1332b567ed23451daf06ebca02970";
pub const CEREMONY_SPEC_MANIFEST_REF_V1: &str =
    "sha256:fb9ba972eb2fe8f6861e71cd6c2c6af23a9fdb75986ffbed8c0a2ce319288485";
pub const PUBLIC_CATALOG_REF_V1: &str =
    "sha256:ea84817fc6ff3314992900a31ce337eb151183ad5d996e7939cb44f4f5af21b1";
pub const ACTION_REQUEST_SCHEMA_REF_V1: &str =
    "sha256:73f173d3654625a19278aa6c413714b04349f5d2500924ff0f780168a713192d";
pub const CEREMONY_REQUEST_SCHEMA_REF_V1: &str =
    "sha256:101aeb99c80e89812c859976834af2bfd5bcecf1b0344ad9a8c0e60a83b0b18a";

const ACTION_OUTCOMES_V1: [&str; 7] = [
    "committed",
    "no_op",
    "rejected",
    "stale",
    "conflict",
    "unavailable",
    "in_doubt",
];
const CEREMONY_NAMES_V1: [&str; 11] = [
    "InstallationContextGenesis",
    "RepositoryV1Cutover",
    "InstallationV1Cutover",
    "RecoverRepositoryStoreGeneration",
    "RecoverInstallationStoreGeneration",
    "ActivateVerifiedRepositoryGeneration",
    "ActivateVerifiedInstallationGeneration",
    "RecoverPreStoreBinarySlot",
    "RecoverPreStoreWriterCohort",
    "EstablishRepositoryRecoveryAdmission",
    "EstablishInstallationRecoveryAdmission",
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CatalogOwnerV1 {
    Work,
    Step,
    Contract,
    Design,
    Decision,
    Execution,
    Evidence,
    GatePolicy,
    Authority,
    Coordination,
    Orchestration,
    Planning,
    Projection,
    Persistence,
    SearchMaintenance,
    Memory,
    Intake,
    Research,
    Integration,
    Distribution,
    Installation,
}

impl CatalogOwnerV1 {
    pub const ALL: [Self; 21] = [
        Self::Work,
        Self::Step,
        Self::Contract,
        Self::Design,
        Self::Decision,
        Self::Execution,
        Self::Evidence,
        Self::GatePolicy,
        Self::Authority,
        Self::Coordination,
        Self::Orchestration,
        Self::Planning,
        Self::Projection,
        Self::Persistence,
        Self::SearchMaintenance,
        Self::Memory,
        Self::Intake,
        Self::Research,
        Self::Integration,
        Self::Distribution,
        Self::Installation,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Work => "Work",
            Self::Step => "Step",
            Self::Contract => "Contract",
            Self::Design => "Design",
            Self::Decision => "Decision",
            Self::Execution => "Execution",
            Self::Evidence => "Evidence",
            Self::GatePolicy => "GatePolicy",
            Self::Authority => "Authority",
            Self::Coordination => "Coordination",
            Self::Orchestration => "Orchestration",
            Self::Planning => "Planning",
            Self::Projection => "Projection",
            Self::Persistence => "Persistence",
            Self::SearchMaintenance => "SearchMaintenance",
            Self::Memory => "Memory",
            Self::Intake => "Intake",
            Self::Research => "Research",
            Self::Integration => "Integration",
            Self::Distribution => "Distribution",
            Self::Installation => "Installation",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|owner| owner.name() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationCatalogKindV1 {
    Action,
    Ceremony,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CeremonyContextKindV1 {
    NoStore,
    PreStore,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OperationCatalogEntryV1 {
    ordinal: u16,
    name: String,
    descriptor_ref: String,
    owner: CatalogOwnerV1,
    owner_profile_ref: String,
    kind: OperationCatalogKindV1,
    descriptor: Value,
}

impl OperationCatalogEntryV1 {
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn descriptor_ref(&self) -> &str {
        &self.descriptor_ref
    }

    pub const fn owner(&self) -> CatalogOwnerV1 {
        self.owner
    }

    pub fn owner_profile_ref(&self) -> &str {
        &self.owner_profile_ref
    }

    pub const fn kind(&self) -> OperationCatalogKindV1 {
        self.kind
    }

    pub fn descriptor(&self) -> &Value {
        &self.descriptor
    }

    pub fn material_dependency_stamp(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"maestro.vnext.material-dependency-stamp.v1\0");
        for value in [
            self.descriptor_ref.as_str(),
            self.owner_profile_ref.as_str(),
            match self.kind {
                OperationCatalogKindV1::Action => ACTION_SPEC_MANIFEST_REF_V1,
                OperationCatalogKindV1::Ceremony => CEREMONY_SPEC_MANIFEST_REF_V1,
            },
            PUBLIC_CATALOG_REF_V1,
        ] {
            hasher.update((value.len() as u64).to_be_bytes());
            hasher.update(value.as_bytes());
        }
        hasher.finalize().into()
    }

    pub fn operation_spec_ref(&self) -> OperationSpecRefV1 {
        match self.kind {
            OperationCatalogKindV1::Action => OperationSpecRefV1::Action(ActionSpecRefV1 {
                exact_action_spec_ref: self.descriptor_ref.clone(),
                exact_schema_id: ACTION_REQUEST_SCHEMA_REF_V1.to_owned(),
                exact_core_catalog_ref: ACTION_SPEC_MANIFEST_REF_V1.to_owned(),
                exact_public_catalog_ref: PUBLIC_CATALOG_REF_V1.to_owned(),
            }),
            OperationCatalogKindV1::Ceremony => OperationSpecRefV1::Ceremony(CeremonySpecRefV1 {
                exact_ceremony_spec_ref: self.descriptor_ref.clone(),
                exact_schema_id: CEREMONY_REQUEST_SCHEMA_REF_V1.to_owned(),
                exact_core_catalog_ref: CEREMONY_SPEC_MANIFEST_REF_V1.to_owned(),
                exact_public_catalog_ref: PUBLIC_CATALOG_REF_V1.to_owned(),
            }),
        }
    }

    pub fn ceremony_context(&self) -> Option<CeremonyContextKindV1> {
        match (self.kind, self.name.as_str()) {
            (OperationCatalogKindV1::Ceremony, "InstallationContextGenesis") => {
                Some(CeremonyContextKindV1::NoStore)
            }
            (OperationCatalogKindV1::Ceremony, _) => Some(CeremonyContextKindV1::PreStore),
            (OperationCatalogKindV1::Action, _) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedCapabilityCatalogV1 {
    grammar_ref: String,
    owner_relation_row_count: usize,
    actions: Vec<OperationCatalogEntryV1>,
    ceremonies: Vec<OperationCatalogEntryV1>,
}

impl GeneratedCapabilityCatalogV1 {
    pub fn load_frozen() -> Result<Self, GeneratedCatalogErrorV1> {
        let owner_relation_row_count = validate_owner_relation_closure(
            &parse_owner_relation_documents(&OWNER_RELATION_CATALOGS)?,
        )?;
        verify_hash(GRAMMAR_JSON.as_bytes(), GRAMMAR_SHA256)?;
        verify_hash(PUBLIC_IDENTITY_JSON.as_bytes(), PUBLIC_IDENTITY_SHA256)?;
        Self::from_documents(
            ACTION_CATALOG_JSON,
            CEREMONY_CATALOG_JSON,
            GRAMMAR_JSON,
            PUBLIC_IDENTITY_JSON,
            owner_relation_row_count,
        )
    }

    pub fn grammar_ref(&self) -> &str {
        &self.grammar_ref
    }

    pub const fn owner_relation_row_count(&self) -> usize {
        self.owner_relation_row_count
    }

    pub fn actions(&self) -> &[OperationCatalogEntryV1] {
        &self.actions
    }

    pub fn ceremonies(&self) -> &[OperationCatalogEntryV1] {
        &self.ceremonies
    }

    pub fn operation_count(&self) -> usize {
        self.actions.len() + self.ceremonies.len()
    }

    pub fn action(&self, descriptor_ref: &str) -> Option<&OperationCatalogEntryV1> {
        self.actions
            .iter()
            .find(|entry| entry.descriptor_ref == descriptor_ref)
    }

    pub fn ceremony(&self, descriptor_ref: &str) -> Option<&OperationCatalogEntryV1> {
        self.ceremonies
            .iter()
            .find(|entry| entry.descriptor_ref == descriptor_ref)
    }

    pub fn operation(&self, descriptor_ref: &str) -> Option<&OperationCatalogEntryV1> {
        self.action(descriptor_ref)
            .or_else(|| self.ceremony(descriptor_ref))
    }

    pub fn action_named(&self, name: &str) -> Option<&OperationCatalogEntryV1> {
        self.actions.iter().find(|entry| entry.name == name)
    }

    pub fn ceremony_named(&self, name: &str) -> Option<&OperationCatalogEntryV1> {
        self.ceremonies.iter().find(|entry| entry.name == name)
    }

    fn from_documents(
        action_json: &str,
        ceremony_json: &str,
        grammar_json: &str,
        public_identity_json: &str,
        owner_relation_row_count: usize,
    ) -> Result<Self, GeneratedCatalogErrorV1> {
        let grammar: Value = serde_json::from_str(grammar_json)?;
        let action: Value = serde_json::from_str(action_json)?;
        let ceremony: Value = serde_json::from_str(ceremony_json)?;
        let public_identity: Value = serde_json::from_str(public_identity_json)?;

        let grammar_id = text_at(&grammar, &["catalog_profile_grammar", "grammar_id"])
            .or_else(|| text_at(&action, &["grammar_id"]))
            .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
        if public_identity.get("closure_id").and_then(Value::as_str) != Some(PUBLIC_CATALOG_REF_V1)
            || text_at(&action, &["manifest_id"])
                != ACTION_SPEC_MANIFEST_REF_V1.strip_prefix("sha256:")
            || text_at(&ceremony, &["manifest_id"])
                != CEREMONY_SPEC_MANIFEST_REF_V1.strip_prefix("sha256:")
        {
            return Err(GeneratedCatalogErrorV1::FrozenIdentityMismatch);
        }

        let owner_profiles = grammar
            .get("owner_profiles")
            .and_then(Value::as_array)
            .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
        if owner_profiles.len() != CatalogOwnerV1::ALL.len() {
            return Err(GeneratedCatalogErrorV1::OwnerClosureMismatch);
        }
        let mut owners = BTreeMap::new();
        for profile in owner_profiles {
            let name = text_at(profile, &["name"])
                .and_then(CatalogOwnerV1::parse)
                .ok_or(GeneratedCatalogErrorV1::OwnerClosureMismatch)?;
            let descriptor_id = text_at(profile, &["descriptor_id"])
                .filter(|value| is_digest(value))
                .ok_or(GeneratedCatalogErrorV1::OwnerClosureMismatch)?;
            if owners.insert(descriptor_id.to_owned(), name).is_some() {
                return Err(GeneratedCatalogErrorV1::OwnerClosureMismatch);
            }
        }

        let actions = parse_entries(&action, OperationCatalogKindV1::Action, &owners)?;
        let ceremonies = parse_entries(&ceremony, OperationCatalogKindV1::Ceremony, &owners)?;
        validate_action_closure(&actions)?;
        validate_ceremony_closure(&ceremonies)?;

        Ok(Self {
            grammar_ref: format!("sha256:{grammar_id}"),
            owner_relation_row_count,
            actions,
            ceremonies,
        })
    }
}

fn parse_owner_relation_documents(
    catalogs: &[(&str, &str, usize)],
) -> Result<Vec<(Value, usize)>, GeneratedCatalogErrorV1> {
    catalogs
        .iter()
        .map(|(document, expected_hash, expected_rows)| {
            verify_hash(document.as_bytes(), expected_hash)?;
            Ok((serde_json::from_str(document)?, *expected_rows))
        })
        .collect()
}

fn validate_owner_relation_closure(
    documents: &[(Value, usize)],
) -> Result<usize, GeneratedCatalogErrorV1> {
    if documents.len() != OWNER_RELATION_CATALOGS.len() {
        return Err(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch);
    }
    let mut total_rows = 0usize;
    let mut relation_domains = BTreeSet::new();
    let mut relation_ids = BTreeSet::new();
    for (domain_index, (document, expected_rows)) in documents.iter().enumerate() {
        let descriptors = document
            .get("descriptors")
            .and_then(Value::as_array)
            .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
        let relation = document
            .get("primary_owner_relation")
            .and_then(Value::as_object)
            .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
        let rows = relation
            .get("rows")
            .and_then(Value::as_array)
            .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
        if descriptors.len() != *expected_rows || rows.len() != *expected_rows {
            return Err(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch);
        }
        let relation_envelope = relation
            .get("identity_envelope")
            .and_then(Value::as_array)
            .filter(|value| value.len() == 2)
            .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
        let expected_domain = format!(
            "maestro.vnext.catalog.primary-owner-relation.{}.v1",
            domain_index + 1
        );
        let relation_domain = relation_envelope[0]
            .as_str()
            .filter(|value| {
                *value == expected_domain && relation_domains.insert((*value).to_owned())
            })
            .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
        let relation_id = relation
            .get("relation_id")
            .and_then(Value::as_str)
            .filter(|value| is_digest(value) && relation_ids.insert((*value).to_owned()))
            .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
        if relation_envelope.get(1) != relation.get("rows")
            || relation_domain.is_empty()
            || relation_id.is_empty()
        {
            return Err(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch);
        }

        let mut descriptor_ids = BTreeSet::new();
        let mut relation_rows = BTreeSet::new();
        for (descriptor, relation_row) in descriptors.iter().zip(rows) {
            let descriptor_id = descriptor
                .get("descriptor_id")
                .and_then(Value::as_str)
                .filter(|value| is_digest(value))
                .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
            let descriptor_envelope = descriptor
                .get("identity_envelope")
                .and_then(Value::as_array)
                .filter(|value| value.len() == 3)
                .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
            let descriptor_value = descriptor
                .get("value")
                .and_then(Value::as_array)
                .filter(|value| value.len() >= 3)
                .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
            let owner = descriptor_value[2]
                .as_array()
                .filter(|value| value.len() == 2)
                .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
            let row = relation_row
                .as_array()
                .filter(|value| value.len() == 3)
                .ok_or(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch)?;
            if descriptor_envelope[1]
                .get("bytes")
                .and_then(Value::as_str)
                .is_none_or(|value| !is_digest(value))
                || descriptor_envelope.get(2) != descriptor.get("value")
                || row[0] != descriptor_value[0]
                || row[1] != owner[0]
                || row[2] != owner[1]
                || !descriptor_ids.insert(descriptor_id)
                || !relation_rows.insert(serde_json::to_string(relation_row)?)
            {
                return Err(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch);
            }
        }
        total_rows += rows.len();
    }
    if total_rows != OWNER_RELATION_TOTAL_ROWS {
        return Err(GeneratedCatalogErrorV1::OwnerRelationClosureMismatch);
    }
    Ok(total_rows)
}

fn parse_entries(
    document: &Value,
    kind: OperationCatalogKindV1,
    owners: &BTreeMap<String, CatalogOwnerV1>,
) -> Result<Vec<OperationCatalogEntryV1>, GeneratedCatalogErrorV1> {
    let descriptors = document
        .get("descriptors")
        .and_then(Value::as_array)
        .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
    let manifest_rows = document
        .get("manifest_rows")
        .and_then(Value::as_array)
        .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
    let owner_rows = document
        .pointer("/primary_owner_relation/rows")
        .and_then(Value::as_array)
        .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
    if descriptors.len() != manifest_rows.len() || descriptors.len() != owner_rows.len() {
        return Err(GeneratedCatalogErrorV1::CatalogCardinalityMismatch);
    }

    let mut seen_ids = BTreeSet::new();
    let mut seen_names = BTreeSet::new();
    descriptors
        .iter()
        .enumerate()
        .map(|(index, descriptor)| {
            let descriptor_id = text_at(descriptor, &["descriptor_id"])
                .filter(|value| is_digest(value))
                .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
            let value = descriptor
                .get("value")
                .and_then(Value::as_array)
                .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
            let name = value
                .get(1)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
            let owner_profile_id = value
                .get(2)
                .and_then(Value::as_array)
                .and_then(|value| value.get(1))
                .and_then(|value| value.get("bytes"))
                .and_then(Value::as_str)
                .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
            let owner = owners
                .get(owner_profile_id)
                .copied()
                .ok_or(GeneratedCatalogErrorV1::OwnerClosureMismatch)?;
            if !seen_ids.insert(descriptor_id) || !seen_names.insert(name) {
                return Err(GeneratedCatalogErrorV1::DuplicateCatalogEntry);
            }
            Ok(OperationCatalogEntryV1 {
                ordinal: u16::try_from(index + 1)
                    .map_err(|_| GeneratedCatalogErrorV1::CatalogCardinalityMismatch)?,
                name: name.to_owned(),
                descriptor_ref: format!("sha256:{descriptor_id}"),
                owner,
                owner_profile_ref: format!("sha256:{owner_profile_id}"),
                kind,
                descriptor: descriptor.clone(),
            })
        })
        .collect()
}

fn validate_action_closure(
    actions: &[OperationCatalogEntryV1],
) -> Result<(), GeneratedCatalogErrorV1> {
    let expected = BTreeMap::from([
        (CatalogOwnerV1::Work, 7usize),
        (CatalogOwnerV1::Step, 4),
        (CatalogOwnerV1::Contract, 2),
        (CatalogOwnerV1::Design, 4),
        (CatalogOwnerV1::Decision, 5),
        (CatalogOwnerV1::Execution, 16),
        (CatalogOwnerV1::Evidence, 7),
        (CatalogOwnerV1::Authority, 48),
        (CatalogOwnerV1::Coordination, 9),
        (CatalogOwnerV1::Planning, 4),
        (CatalogOwnerV1::Persistence, 10),
        (CatalogOwnerV1::Distribution, 13),
        (CatalogOwnerV1::SearchMaintenance, 2),
        (CatalogOwnerV1::Memory, 7),
        (CatalogOwnerV1::Intake, 3),
        (CatalogOwnerV1::Research, 4),
    ]);
    let mut actual = BTreeMap::new();
    for action in actions {
        *actual.entry(action.owner).or_insert(0usize) += 1;
        let outcomes = action
            .descriptor
            .get("value")
            .and_then(Value::as_array)
            .and_then(|value| value.get(value.len().saturating_sub(2)))
            .and_then(Value::as_array)
            .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
        let names = outcomes
            .iter()
            .filter_map(|row| row.as_array()?.get(1)?.as_str())
            .collect::<Vec<_>>();
        if names != ACTION_OUTCOMES_V1 {
            return Err(GeneratedCatalogErrorV1::ActionOutcomeClosureMismatch);
        }
    }
    if actions.len() != ACTION_TAG_COUNT_V1 as usize || actual != expected {
        return Err(GeneratedCatalogErrorV1::CatalogCardinalityMismatch);
    }
    Ok(())
}

fn validate_ceremony_closure(
    ceremonies: &[OperationCatalogEntryV1],
) -> Result<(), GeneratedCatalogErrorV1> {
    let names = ceremonies
        .iter()
        .map(OperationCatalogEntryV1::name)
        .collect::<Vec<_>>();
    if names != CEREMONY_NAMES_V1 {
        return Err(GeneratedCatalogErrorV1::CatalogCardinalityMismatch);
    }
    for ceremony in ceremonies {
        let modes = ceremony
            .descriptor
            .get("value")
            .and_then(Value::as_array)
            .and_then(|value| value.get(3))
            .and_then(Value::as_array)
            .ok_or(GeneratedCatalogErrorV1::MalformedFrozenCatalog)?;
        if modes.iter().filter_map(Value::as_u64).collect::<Vec<_>>() != [1, 2, 3, 4] {
            return Err(GeneratedCatalogErrorV1::CeremonyModeClosureMismatch);
        }
    }
    Ok(())
}

fn verify_hash(bytes: &[u8], expected: &str) -> Result<(), GeneratedCatalogErrorV1> {
    let actual = lower_hex(&Sha256::digest(bytes));
    if actual == expected {
        Ok(())
    } else {
        Err(GeneratedCatalogErrorV1::FrozenArtifactHashMismatch)
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}")
            .expect("invariant: writing hexadecimal into a String cannot fail");
    }
    encoded
}

fn text_at<'value>(value: &'value Value, path: &[&str]) -> Option<&'value str> {
    path.iter()
        .try_fold(value, |current, segment| current.get(*segment))?
        .as_str()
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

#[derive(Debug, Error)]
pub enum GeneratedCatalogErrorV1 {
    #[error("a frozen Stage-0 catalog artifact hash does not match its certified bytes")]
    FrozenArtifactHashMismatch,
    #[error("a frozen Stage-0 catalog identity does not match the certified closure")]
    FrozenIdentityMismatch,
    #[error("the frozen generated catalog is malformed")]
    MalformedFrozenCatalog,
    #[error("the frozen generated catalog has an invalid row cardinality")]
    CatalogCardinalityMismatch,
    #[error("the frozen generated catalog repeats a descriptor identity or name")]
    DuplicateCatalogEntry,
    #[error("the frozen owner profile closure is not exact")]
    OwnerClosureMismatch,
    #[error("the nine frozen primary-owner relations do not form the exact 444-row closure")]
    OwnerRelationClosureMismatch,
    #[error("the Action catalog does not expose the exact seven semantic outcomes")]
    ActionOutcomeClosureMismatch,
    #[error("the Ceremony catalog does not expose the exact four request modes")]
    CeremonyModeClosureMismatch,
    #[error("the frozen generated catalog is not valid JSON")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relation_documents() -> Vec<(Value, usize)> {
        parse_owner_relation_documents(&OWNER_RELATION_CATALOGS).expect("frozen relations")
    }

    #[test]
    fn nine_owner_relation_domains_close_exactly_444_rows() {
        let documents = relation_documents();
        assert_eq!(
            validate_owner_relation_closure(&documents).expect("relation closure"),
            OWNER_RELATION_TOTAL_ROWS
        );
        assert_eq!(
            documents
                .iter()
                .map(|(_, expected)| *expected)
                .collect::<Vec<_>>(),
            vec![43, 23, 6, 6, 11, 145, 35, 30, 145]
        );
    }

    #[test]
    fn relation_substitution_duplication_and_missing_row_mutants_fail() {
        let documents = relation_documents();

        let mut substituted = documents.clone();
        substituted[8].0["primary_owner_relation"]["rows"][0] =
            substituted[8].0["primary_owner_relation"]["rows"][93].clone();
        let substituted_rows = substituted[8].0["primary_owner_relation"]["rows"].clone();
        substituted[8].0["primary_owner_relation"]["identity_envelope"][1] = substituted_rows;
        assert!(validate_owner_relation_closure(&substituted).is_err());

        let mut duplicated = documents.clone();
        duplicated[0].0["primary_owner_relation"]["rows"][1] =
            duplicated[0].0["primary_owner_relation"]["rows"][0].clone();
        let duplicated_rows = duplicated[0].0["primary_owner_relation"]["rows"].clone();
        duplicated[0].0["primary_owner_relation"]["identity_envelope"][1] = duplicated_rows;
        assert!(validate_owner_relation_closure(&duplicated).is_err());

        let mut missing = documents;
        missing[5]
            .0
            .get_mut("primary_owner_relation")
            .and_then(|value| value.get_mut("rows"))
            .and_then(Value::as_array_mut)
            .expect("relation rows")
            .pop();
        let missing_rows = missing[5].0["primary_owner_relation"]["rows"].clone();
        missing[5].0["primary_owner_relation"]["identity_envelope"][1] = missing_rows;
        assert!(validate_owner_relation_closure(&missing).is_err());
    }
}
