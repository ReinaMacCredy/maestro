use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::identity::{
    IdentityError, Stage0ProofManifestIdV1, stage0_proof_manifest_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

pub const STAGE0_PROOF_MANIFEST_VERSION_V1: u64 = 1;
pub const VERIFIED_NON_PROMOTING_RESULT_CLASS: &str = "verified_non_promoting";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u64)]
pub enum Stage0ProofGateKindV1 {
    ExternalInputAuthorization = 1,
    DecisionClosure = 2,
    CatalogPredecessor = 3,
    IncorporatedCatalogCheckpoints = 4,
    CatalogSuccessor = 5,
    PublicContracts = 6,
    PublicIdentity = 7,
    SubmissionClaim = 8,
    Dispatch = 9,
    EffectHome = 10,
    ResourceRelease = 11,
    CurrentSurfaceConsumerCensus = 12,
    PersistenceArchiveGoldenFixtures = 13,
    MigrationRollback = 14,
    RootAssemblySourceBinding = 15,
}

impl Stage0ProofGateKindV1 {
    pub const ALL: [Self; 15] = [
        Self::ExternalInputAuthorization,
        Self::DecisionClosure,
        Self::CatalogPredecessor,
        Self::IncorporatedCatalogCheckpoints,
        Self::CatalogSuccessor,
        Self::PublicContracts,
        Self::PublicIdentity,
        Self::SubmissionClaim,
        Self::Dispatch,
        Self::EffectHome,
        Self::ResourceRelease,
        Self::CurrentSurfaceConsumerCensus,
        Self::PersistenceArchiveGoldenFixtures,
        Self::MigrationRollback,
        Self::RootAssemblySourceBinding,
    ];

    pub const fn tag(self) -> u64 {
        self as u64
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::ExternalInputAuthorization => "external_input_authorization",
            Self::DecisionClosure => "decision_closure",
            Self::CatalogPredecessor => "catalog_predecessor",
            Self::IncorporatedCatalogCheckpoints => "incorporated_catalog_checkpoints",
            Self::CatalogSuccessor => "catalog_successor",
            Self::PublicContracts => "public_contracts",
            Self::PublicIdentity => "public_identity",
            Self::SubmissionClaim => "submission_claim",
            Self::Dispatch => "dispatch",
            Self::EffectHome => "effect_home",
            Self::ResourceRelease => "resource_release",
            Self::CurrentSurfaceConsumerCensus => "current_surface_consumer_census",
            Self::PersistenceArchiveGoldenFixtures => "persistence_archive_golden_fixtures",
            Self::MigrationRollback => "migration_rollback",
            Self::RootAssemblySourceBinding => "root_assembly_source_binding",
        }
    }
}

impl TryFrom<u64> for Stage0ProofGateKindV1 {
    type Error = Stage0ProofError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.tag() == value)
            .ok_or(Stage0ProofError::UnknownGate(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum Stage0ProofResultV1 {
    Passed = 1,
    Failed = 2,
}

impl Stage0ProofResultV1 {
    pub const fn tag(self) -> u64 {
        self as u64
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stage0ProofOutcomeV1 {
    result: Stage0ProofResultV1,
    result_class: String,
    result_sha256: [u8; 32],
}

impl Stage0ProofOutcomeV1 {
    pub fn new(
        result: Stage0ProofResultV1,
        result_class: impl Into<String>,
        result_sha256: [u8; 32],
    ) -> Result<Self, Stage0ProofError> {
        let result_class = result_class.into();
        if result_class.is_empty()
            || !result_class.is_ascii()
            || result_class
                .bytes()
                .any(|byte| !(byte.is_ascii_lowercase() || byte == b'_'))
        {
            return Err(Stage0ProofError::InvalidResultClass);
        }
        Ok(Self {
            result,
            result_class,
            result_sha256,
        })
    }

    pub fn result(&self) -> Stage0ProofResultV1 {
        self.result
    }

    pub fn result_class(&self) -> &str {
        &self.result_class
    }

    pub fn result_sha256(&self) -> &[u8; 32] {
        &self.result_sha256
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProofArtifactHashV1 {
    path: String,
    sha256: [u8; 32],
}

impl ProofArtifactHashV1 {
    pub fn new(path: impl Into<String>, sha256: [u8; 32]) -> Result<Self, Stage0ProofError> {
        let path = path.into();
        if path.is_empty()
            || !path.is_ascii()
            || path.starts_with('/')
            || path.contains('\\')
            || path
                .split('/')
                .any(|component| component.is_empty() || component == "." || component == "..")
        {
            return Err(Stage0ProofError::InvalidArtifactPath);
        }
        Ok(Self { path, sha256 })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(&self.path)?,
            CborValue::Bytes(self.sha256.to_vec()),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stage0ProofGateV1 {
    kind: Stage0ProofGateKindV1,
    source_artifacts: Vec<ProofArtifactHashV1>,
    validator_artifacts: Vec<ProofArtifactHashV1>,
    input_artifacts: Vec<ProofArtifactHashV1>,
    outcome: Stage0ProofOutcomeV1,
    semantic_counts: Vec<(String, u64)>,
}

impl Stage0ProofGateV1 {
    pub fn new(
        kind: Stage0ProofGateKindV1,
        source_artifacts: Vec<ProofArtifactHashV1>,
        validator_artifacts: Vec<ProofArtifactHashV1>,
        input_artifacts: Vec<ProofArtifactHashV1>,
        outcome: Stage0ProofOutcomeV1,
        semantic_counts: Vec<(String, u64)>,
    ) -> Result<Self, Stage0ProofError> {
        if validator_artifacts.is_empty() {
            return Err(Stage0ProofError::MissingValidator(kind));
        }
        if kind == Stage0ProofGateKindV1::ExternalInputAuthorization {
            let expected_result_sha256: [u8; 32] =
                Sha256::digest(VERIFIED_NON_PROMOTING_RESULT_CLASS.as_bytes()).into();
            if outcome.result_class() != VERIFIED_NON_PROMOTING_RESULT_CLASS
                || outcome.result_sha256() != &expected_result_sha256
                || !source_artifacts.is_empty()
                || !input_artifacts.is_empty()
                || !semantic_counts.is_empty()
            {
                return Err(Stage0ProofError::ExternalInputPromotionClass);
            }
        }
        let source_artifacts = sorted_artifacts(source_artifacts)?;
        let validator_artifacts = sorted_artifacts(validator_artifacts)?;
        let input_artifacts = sorted_artifacts(input_artifacts)?;
        let semantic_counts = sorted_counts(semantic_counts)?;
        Ok(Self {
            kind,
            source_artifacts,
            validator_artifacts,
            input_artifacts,
            outcome,
            semantic_counts,
        })
    }

    pub fn kind(&self) -> Stage0ProofGateKindV1 {
        self.kind
    }

    pub fn result(&self) -> Stage0ProofResultV1 {
        self.outcome.result()
    }

    pub fn result_class(&self) -> &str {
        self.outcome.result_class()
    }

    pub fn source_artifacts(&self) -> &[ProofArtifactHashV1] {
        &self.source_artifacts
    }

    pub fn validator_artifacts(&self) -> &[ProofArtifactHashV1] {
        &self.validator_artifacts
    }

    pub fn input_artifacts(&self) -> &[ProofArtifactHashV1] {
        &self.input_artifacts
    }

    pub fn result_sha256(&self) -> &[u8; 32] {
        self.outcome.result_sha256()
    }

    pub fn semantic_counts(&self) -> &[(String, u64)] {
        &self.semantic_counts
    }

    fn canonical_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(self.kind.tag()),
            CborValue::text(self.kind.name())?,
            artifact_values(&self.source_artifacts)?,
            artifact_values(&self.validator_artifacts)?,
            artifact_values(&self.input_artifacts)?,
            CborValue::Unsigned(self.outcome.result().tag()),
            CborValue::text(self.outcome.result_class())?,
            CborValue::Bytes(self.outcome.result_sha256().to_vec()),
            CborValue::Array(
                self.semantic_counts
                    .iter()
                    .map(|(name, count)| {
                        Ok(CborValue::Array(vec![
                            CborValue::text(name)?,
                            CborValue::Unsigned(*count),
                        ]))
                    })
                    .collect::<Result<Vec<_>, CborError>>()?,
            ),
        ]))
    }
}

#[derive(Clone, Debug)]
pub struct Stage0ProofManifestV1 {
    gates: Vec<Stage0ProofGateV1>,
    manifest_id: Stage0ProofManifestIdV1,
}

impl Stage0ProofManifestV1 {
    pub fn new(mut gates: Vec<Stage0ProofGateV1>) -> Result<Self, Stage0ProofError> {
        gates.sort_by_key(|gate| gate.kind());
        let actual = gates
            .iter()
            .map(Stage0ProofGateV1::kind)
            .collect::<Vec<_>>();
        if actual != Stage0ProofGateKindV1::ALL {
            return Err(Stage0ProofError::IncompleteGateSet);
        }
        if let Some(gate) = gates
            .iter()
            .find(|gate| gate.result() != Stage0ProofResultV1::Passed)
        {
            return Err(Stage0ProofError::FailedGate(gate.kind()));
        }
        let canonical_value = manifest_value(&gates)?;
        let manifest_id = stage0_proof_manifest_identity(&canonical_value)?;
        Ok(Self { gates, manifest_id })
    }

    pub fn gates(&self) -> &[Stage0ProofGateV1] {
        &self.gates
    }

    pub fn gate_count(&self) -> usize {
        self.gates.len()
    }

    pub fn manifest_id(&self) -> &Stage0ProofManifestIdV1 {
        &self.manifest_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, Stage0ProofError> {
        Ok(deterministic_cbor::encode(&manifest_value(&self.gates)?)?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Stage0ProofError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("proof artifact paths must be canonical relative ASCII paths")]
    InvalidArtifactPath,
    #[error("proof artifact paths must be unique")]
    DuplicateArtifactPath,
    #[error("proof semantic count names must be canonical, sorted, and unique")]
    InvalidSemanticCounts,
    #[error("unknown Stage-0 proof gate {0}")]
    UnknownGate(u64),
    #[error("Stage-0 proof gate {0:?} has no validator source")]
    MissingValidator(Stage0ProofGateKindV1),
    #[error("proof result class must be non-empty lower snake case ASCII")]
    InvalidResultClass,
    #[error(
        "external input authorization proof must bind only its validator and constant non-promoting result"
    )]
    ExternalInputPromotionClass,
    #[error("Stage-0 proof manifest does not contain the exact required gate set")]
    IncompleteGateSet,
    #[error("Stage-0 proof gate {0:?} failed")]
    FailedGate(Stage0ProofGateKindV1),
}

fn sorted_artifacts(
    mut artifacts: Vec<ProofArtifactHashV1>,
) -> Result<Vec<ProofArtifactHashV1>, Stage0ProofError> {
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    if artifacts
        .windows(2)
        .any(|pair| pair[0].path == pair[1].path)
    {
        return Err(Stage0ProofError::DuplicateArtifactPath);
    }
    Ok(artifacts)
}

fn sorted_counts(mut counts: Vec<(String, u64)>) -> Result<Vec<(String, u64)>, Stage0ProofError> {
    if counts
        .iter()
        .any(|(name, _)| name.is_empty() || !name.is_ascii())
    {
        return Err(Stage0ProofError::InvalidSemanticCounts);
    }
    counts.sort_by(|left, right| left.0.cmp(&right.0));
    let names = counts.iter().map(|(name, _)| name).collect::<BTreeSet<_>>();
    if names.len() != counts.len() {
        return Err(Stage0ProofError::InvalidSemanticCounts);
    }
    Ok(counts)
}

fn artifact_values(artifacts: &[ProofArtifactHashV1]) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(
        artifacts
            .iter()
            .map(ProofArtifactHashV1::canonical_value)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn manifest_value(gates: &[Stage0ProofGateV1]) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(STAGE0_PROOF_MANIFEST_VERSION_V1),
        CborValue::Array(
            gates
                .iter()
                .map(Stage0ProofGateV1::canonical_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    ]))
}
