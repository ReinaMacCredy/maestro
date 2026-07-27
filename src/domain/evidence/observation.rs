use thiserror::Error;

use crate::domain::authority::{
    CmaBranchIdV1, CmaObservationPublicationPurposeV1, ContinuityMaintenanceAuthorityBasisV1,
    ExecutionProducerV1, ExecutorAssertionIdV1, PrincipalIdV1, SessionIdV1, SlotIdV1,
    StateTokenIdV1,
};
use crate::domain::contract::runtime::ContractGenerationIdV1;
use crate::domain::execution::{
    DispatchAttemptIdV1, ExecutionAttemptOwnerV1, ReconciliationAttemptIdV1, RunIdV1, RunV1,
    StepAttemptIdV1,
};
use crate::domain::identity::{
    IdentityError, SchemaIdV1, StoreDomainIdV1, StoreObjectIdV1, derive_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::identity::{EvidenceIdentityError, ObservationRecordIdV1, domain_hash, require_nonzero};

pub const OBSERVATION_RECORD_VERSION_V1: u64 = 1;
pub const OBSERVATION_RECORD_DOMAIN_V1: &str = "maestro.vnext.evidence.observation-record.v1";
const MAX_OBSERVATION_SUBJECTS_V1: usize = 1_024;
const MAX_OBSERVATION_LINEAGE_V1: usize = 4_096;
const MAX_MEDIA_TYPE_BYTES_V1: usize = 128;

macro_rules! observation_kinds {
    ($(($variant:ident, $tag:literal, $name:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum ObservationKindV1 {
            $($variant),+
        }

        impl ObservationKindV1 {
            pub const ALL: [Self; 43] = [$(Self::$variant),+];

            pub const fn tag(self) -> u64 {
                match self {
                    $(Self::$variant => $tag),+
                }
            }

            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }

            pub fn from_tag(tag: u64) -> Result<Self, ObservationError> {
                match tag {
                    $($tag => Ok(Self::$variant)),+,
                    value => Err(ObservationError::UnknownKind(value)),
                }
            }

            pub const fn producer_action_tag(self) -> u64 {
                match self {
                    Self::BootstrapMandatePresentation => 43,
                    Self::BootstrapMandateResponse => 44,
                    Self::TrustedTimeAtomicUnit
                    | Self::TrustedTimeAcquisition
                    | Self::MaintenanceExecutorCurrentness
                    | Self::ProspectiveContinuityCarrier
                    | Self::RecoveryExternalAnchorRevision
                    | Self::RecoveryExternalRegistration
                    | Self::RecoveryExternalStatus
                    | Self::ExternalHighWaterReadback
                    | Self::CarrierIntegrityFault => 45,
                    _ => 39,
                }
            }

            pub const fn source_route_tags(self) -> &'static [u64] {
                match self {
                    Self::TrustedTimeAtomicUnit => &[1],
                    Self::TrustedTimeAcquisition => &[2],
                    Self::MaintenanceExecutorCurrentness => &[7],
                    Self::ProspectiveContinuityCarrier => &[8],
                    Self::RecoveryExternalAnchorRevision => &[4, 6],
                    Self::RecoveryExternalRegistration => &[3],
                    Self::RecoveryExternalStatus => &[5],
                    Self::ExternalHighWaterReadback => &[9],
                    Self::CarrierIntegrityFault => &[10],
                    _ => &[],
                }
            }

            pub const fn cma_compatibility(self) -> &'static [(u64, u64)] {
                match self {
                    Self::TrustedTimeAtomicUnit => &[(1, 1)],
                    Self::TrustedTimeAcquisition => &[(1, 2)],
                    Self::MaintenanceExecutorCurrentness => &[(4, 7)],
                    Self::ProspectiveContinuityCarrier => &[(5, 8)],
                    Self::RecoveryExternalAnchorRevision => &[(2, 4), (3, 6)],
                    Self::RecoveryExternalRegistration => &[(2, 3)],
                    Self::RecoveryExternalStatus => &[(3, 5)],
                    Self::ExternalHighWaterReadback => &[(5, 9)],
                    Self::CarrierIntegrityFault => &[(5, 10)],
                    _ => &[],
                }
            }

            pub fn contract(self) -> Result<ObservationKindContractV1, ObservationError> {
                ObservationKindContractV1::for_kind(self)
            }
        }
    };
}

observation_kinds!(
    (DeterministicProcedure, 1, "DeterministicProcedure"),
    (NondeterministicProcedure, 2, "NondeterministicProcedure"),
    (ManualProcedure, 3, "ManualProcedure"),
    (VisualCapture, 4, "VisualCapture"),
    (ReviewFinding, 5, "ReviewFinding"),
    (SecurityAnalysis, 6, "SecurityAnalysis"),
    (InstalledBinary, 7, "InstalledBinary"),
    (RemoteReadback, 8, "RemoteReadback"),
    (RemoteDelivery, 9, "RemoteDelivery"),
    (RemoteRelease, 10, "RemoteRelease"),
    (ProcessLiveness, 11, "ProcessLiveness"),
    (SkillActivation, 12, "SkillActivation"),
    (HookDelivery, 13, "HookDelivery"),
    (EventStreamIntegrity, 14, "EventStreamIntegrity"),
    (ResearchSource, 15, "ResearchSource"),
    (CapabilityProbe, 16, "CapabilityProbe"),
    (
        BootstrapMandatePresentation,
        17,
        "BootstrapMandatePresentation"
    ),
    (BootstrapMandateResponse, 18, "BootstrapMandateResponse"),
    (GovernedReviewPresentation, 19, "GovernedReviewPresentation"),
    (GovernedReviewResponse, 20, "GovernedReviewResponse"),
    (
        HumanBindingSubjectSelection,
        21,
        "HumanBindingSubjectSelection"
    ),
    (HumanPresence, 22, "HumanPresence"),
    (CredentialControl, 23, "CredentialControl"),
    (HumanAssurance, 24, "HumanAssurance"),
    (SourceAssurance, 25, "SourceAssurance"),
    (SourceCommonCause, 26, "SourceCommonCause"),
    (SourceContradiction, 27, "SourceContradiction"),
    (SourceRevocation, 28, "SourceRevocation"),
    (TrustedTimeAtomicUnit, 29, "TrustedTimeAtomicUnit"),
    (TrustedTimeAcquisition, 30, "TrustedTimeAcquisition"),
    (
        MaintenanceExecutorCurrentness,
        31,
        "MaintenanceExecutorCurrentness"
    ),
    (
        ProspectiveContinuityCarrier,
        32,
        "ProspectiveContinuityCarrier"
    ),
    (
        RecoveryExternalAnchorRevision,
        33,
        "RecoveryExternalAnchorRevision"
    ),
    (
        RecoveryExternalRegistration,
        34,
        "RecoveryExternalRegistration"
    ),
    (RecoveryExternalStatus, 35, "RecoveryExternalStatus"),
    (ExternalHighWaterReadback, 36, "ExternalHighWaterReadback"),
    (CarrierIntegrityFault, 37, "CarrierIntegrityFault"),
    (
        DistributionFilesystemState,
        38,
        "DistributionFilesystemState"
    ),
    (
        DistributionInstalledResource,
        39,
        "DistributionInstalledResource"
    ),
    (DistributionManagerState, 40, "DistributionManagerState"),
    (PublicationIntegrity, 41, "PublicationIntegrity"),
    (StoreIntegrity, 42, "StoreIntegrity"),
    (
        ProtectedContinuityDiagnostic,
        43,
        "ProtectedContinuityDiagnostic"
    ),
);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ObservationSubjectKindV1 {
    Work,
    Step,
    Submission,
    Run,
    Repository,
    Installation,
    Authority,
    Resource,
    External,
}

impl ObservationSubjectKindV1 {
    pub const fn tag(self) -> u64 {
        match self {
            Self::Work => 1,
            Self::Step => 2,
            Self::Submission => 3,
            Self::Run => 4,
            Self::Repository => 5,
            Self::Installation => 6,
            Self::Authority => 7,
            Self::Resource => 8,
            Self::External => 9,
        }
    }

    fn from_tag(tag: u64) -> Result<Self, ObservationError> {
        match tag {
            1 => Ok(Self::Work),
            2 => Ok(Self::Step),
            3 => Ok(Self::Submission),
            4 => Ok(Self::Run),
            5 => Ok(Self::Repository),
            6 => Ok(Self::Installation),
            7 => Ok(Self::Authority),
            8 => Ok(Self::Resource),
            9 => Ok(Self::External),
            _ => Err(ObservationError::InvalidStoredObservation),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObservationSubjectV1 {
    kind: ObservationSubjectKindV1,
    subject_id: [u8; 32],
    contract_generation_id: Option<ContractGenerationIdV1>,
    revision_id: [u8; 32],
}

impl ObservationSubjectV1 {
    pub fn new(
        kind: ObservationSubjectKindV1,
        subject_id: [u8; 32],
        revision_id: [u8; 32],
    ) -> Result<Self, ObservationError> {
        if kind == ObservationSubjectKindV1::Work {
            return Err(ObservationError::InvalidSubjects);
        }
        require_nonzero(subject_id, "Observation subject")?;
        require_nonzero(revision_id, "Observation subject revision")?;
        Ok(Self {
            kind,
            subject_id,
            contract_generation_id: None,
            revision_id,
        })
    }

    pub fn for_work(
        work_id: [u8; 32],
        contract_generation_id: ContractGenerationIdV1,
        contract_root_id: [u8; 32],
    ) -> Result<Self, ObservationError> {
        require_nonzero(work_id, "Observation Work")?;
        require_nonzero(
            *contract_generation_id.as_bytes(),
            "Observation Contract Generation",
        )?;
        require_nonzero(contract_root_id, "Observation Contract Root")?;
        Ok(Self {
            kind: ObservationSubjectKindV1::Work,
            subject_id: work_id,
            contract_generation_id: Some(contract_generation_id),
            revision_id: contract_root_id,
        })
    }

    pub const fn kind(self) -> ObservationSubjectKindV1 {
        self.kind
    }

    pub const fn revision_id(&self) -> &[u8; 32] {
        &self.revision_id
    }

    pub const fn contract_generation_id(&self) -> Option<ContractGenerationIdV1> {
        self.contract_generation_id
    }

    pub const fn subject_id(&self) -> &[u8; 32] {
        &self.subject_id
    }

    pub(crate) fn canonical_value(self) -> CborValue {
        let mut fields = vec![
            CborValue::Unsigned(self.kind.tag()),
            bytes(&self.subject_id),
        ];
        if let Some(contract_generation_id) = self.contract_generation_id {
            fields.push(bytes(contract_generation_id.as_bytes()));
        }
        fields.push(bytes(&self.revision_id));
        CborValue::Array(fields)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationPayloadCommonV1 {
    subject_set_hash: [u8; 32],
    procedure_hash: [u8; 32],
    environment_hash: [u8; 32],
    toolchain_hash: [u8; 32],
    observed_at: u64,
    recorded_at: u64,
    clock_basis_hash: [u8; 32],
}

impl ObservationPayloadCommonV1 {
    pub fn new(
        subjects: &[ObservationSubjectV1],
        procedure_hash: [u8; 32],
        environment_hash: [u8; 32],
        toolchain_hash: [u8; 32],
        observed_at: u64,
        recorded_at: u64,
        clock_basis_hash: [u8; 32],
    ) -> Result<Self, ObservationError> {
        let subject_set_hash = domain_hash(
            "maestro.vnext.evidence.observation-payload-subject-set.v1",
            &CborValue::Array(
                subjects
                    .iter()
                    .copied()
                    .map(ObservationSubjectV1::canonical_value)
                    .collect(),
            ),
        )?;
        for (label, value) in [
            ("payload subject set", subject_set_hash),
            ("payload procedure", procedure_hash),
            ("payload environment", environment_hash),
            ("payload toolchain", toolchain_hash),
            ("payload clock basis", clock_basis_hash),
        ] {
            require_nonzero(value, label)?;
        }
        if observed_at == 0 || recorded_at < observed_at {
            return Err(ObservationError::InvalidPayloadSemantics);
        }
        Ok(Self {
            subject_set_hash,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at,
            recorded_at,
            clock_basis_hash,
        })
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            bytes(&self.subject_set_hash),
            bytes(&self.procedure_hash),
            bytes(&self.environment_hash),
            bytes(&self.toolchain_hash),
            CborValue::Unsigned(self.observed_at),
            CborValue::Unsigned(self.recorded_at),
            bytes(&self.clock_basis_hash),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationPayloadFieldV1 {
    Digest([u8; 32]),
    Count(u64),
    Timestamp(u64),
    Tag(u64),
    Boolean(bool),
}

impl ObservationPayloadFieldV1 {
    const fn field_type(self) -> ObservationPayloadFieldTypeV1 {
        match self {
            Self::Digest(_) => ObservationPayloadFieldTypeV1::Digest,
            Self::Count(_) => ObservationPayloadFieldTypeV1::Count,
            Self::Timestamp(_) => ObservationPayloadFieldTypeV1::Timestamp,
            Self::Tag(_) => ObservationPayloadFieldTypeV1::Tag,
            Self::Boolean(_) => ObservationPayloadFieldTypeV1::Boolean,
        }
    }

    fn canonical_value(self) -> CborValue {
        match self {
            Self::Digest(value) => CborValue::Array(vec![CborValue::Unsigned(1), bytes(&value)]),
            Self::Count(value) => {
                CborValue::Array(vec![CborValue::Unsigned(2), CborValue::Unsigned(value)])
            }
            Self::Timestamp(value) => {
                CborValue::Array(vec![CborValue::Unsigned(3), CborValue::Unsigned(value)])
            }
            Self::Tag(value) => {
                CborValue::Array(vec![CborValue::Unsigned(4), CborValue::Unsigned(value)])
            }
            Self::Boolean(value) => {
                CborValue::Array(vec![CborValue::Unsigned(5), CborValue::Bool(value)])
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationPayloadFieldTypeV1 {
    Digest,
    Count,
    Timestamp,
    Tag,
    Boolean,
}

impl ObservationPayloadFieldTypeV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::Digest => 1,
            Self::Count => 2,
            Self::Timestamp => 3,
            Self::Tag => 4,
            Self::Boolean => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationPayloadFieldSpecV1 {
    name: &'static str,
    field_type: ObservationPayloadFieldTypeV1,
}

impl ObservationPayloadFieldSpecV1 {
    pub const fn name(self) -> &'static str {
        self.name
    }

    pub const fn field_type(self) -> ObservationPayloadFieldTypeV1 {
        self.field_type
    }
}

const fn payload_field(
    name: &'static str,
    field_type: ObservationPayloadFieldTypeV1,
) -> ObservationPayloadFieldSpecV1 {
    ObservationPayloadFieldSpecV1 { name, field_type }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalObservationPayloadV1 {
    kind: ObservationKindV1,
    schema_id: SchemaIdV1,
    fields: Vec<ObservationPayloadFieldV1>,
}

impl NominalObservationPayloadV1 {
    pub fn new(
        kind: ObservationKindV1,
        fields: Vec<ObservationPayloadFieldV1>,
    ) -> Result<Self, ObservationError> {
        if expected_payload_detail_tag(kind) != 9 {
            return Err(ObservationError::InvalidPayloadSemantics);
        }
        let specs = payload_field_specs(kind);
        if fields.len() != specs.len()
            || fields
                .iter()
                .zip(specs)
                .any(|(value, spec)| value.field_type() != spec.field_type)
            || fields.iter().any(|value| {
                matches!(value, ObservationPayloadFieldV1::Digest(bytes) if bytes == &[0; 32])
                    || matches!(
                        value,
                        ObservationPayloadFieldV1::Timestamp(0) | ObservationPayloadFieldV1::Tag(0)
                    )
            })
        {
            return Err(ObservationError::InvalidPayloadSemantics);
        }
        Ok(Self {
            kind,
            schema_id: payload_schema_id(kind)?,
            fields,
        })
    }

    pub const fn kind(&self) -> ObservationKindV1 {
        self.kind
    }

    pub const fn schema_id(&self) -> SchemaIdV1 {
        self.schema_id
    }

    pub fn fields(&self) -> &[ObservationPayloadFieldV1] {
        &self.fields
    }

    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.kind.tag()),
            bytes(self.schema_id.as_bytes()),
            CborValue::Array(
                self.fields
                    .iter()
                    .copied()
                    .map(ObservationPayloadFieldV1::canonical_value)
                    .collect(),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationPayloadDetailV1 {
    Deterministic {
        executable_bytes_hash: [u8; 32],
        executable_version_hash: [u8; 32],
        arguments_hash: [u8; 32],
        working_directory_hash: [u8; 32],
        relevant_environment_hash: [u8; 32],
        subject_revision_hash: [u8; 32],
        dirty_state_hash: [u8; 32],
        exit_status_hash: [u8; 32],
        stdout_hash: [u8; 32],
        stderr_hash: [u8; 32],
    },
    Nondeterministic {
        seed_or_absence_hash: [u8; 32],
        sample_plan_hash: [u8; 32],
        repetitions: u64,
        thresholds_hash: [u8; 32],
        confidence_or_error_model_hash: [u8; 32],
        result_set_hash: [u8; 32],
    },
    Manual {
        actor_binding_hash: [u8; 32],
        procedure_material_hash: [u8; 32],
        target_build_hash: [u8; 32],
        device_hash: [u8; 32],
        capture_artifact_hash: [u8; 32],
    },
    Visual {
        actor_binding_hash: [u8; 32],
        target_build_hash: [u8; 32],
        device_hash: [u8; 32],
        viewport_hash: [u8; 32],
        display_scale_hash: [u8; 32],
        theme_hash: [u8; 32],
        capture_artifact_hash: [u8; 32],
    },
    Review {
        reviewer_binding_hash: [u8; 32],
        model_context_hash: [u8; 32],
        tool_context_hash: [u8; 32],
        review_packet_hash: [u8; 32],
        tree_hash: [u8; 32],
        findings_hash: [u8; 32],
        confidence_hash: [u8; 32],
    },
    Security {
        threat_scope_hash: [u8; 32],
        scanner_hash: [u8; 32],
        ruleset_hash: [u8; 32],
        dependency_snapshot_hash: [u8; 32],
        exclusions_hash: [u8; 32],
        target_tree_hash: [u8; 32],
        findings_hash: [u8; 32],
    },
    InstalledBinary {
        installed_path_hash: [u8; 32],
        binary_digest: [u8; 32],
        build_provenance_hash: [u8; 32],
        commit_provenance_hash: [u8; 32],
    },
    Delivery {
        target_hash: [u8; 32],
        immutable_artifact_hash: [u8; 32],
        tag_hash: [u8; 32],
        commit_hash: [u8; 32],
        publication_receipt_hash: [u8; 32],
        endpoint_snapshot_hash: [u8; 32],
        availability_time: u64,
    },
    Nominal(NominalObservationPayloadV1),
}

impl ObservationPayloadDetailV1 {
    pub const fn tag(&self) -> u64 {
        match self {
            Self::Deterministic { .. } => 1,
            Self::Nondeterministic { .. } => 2,
            Self::Manual { .. } => 3,
            Self::Visual { .. } => 4,
            Self::Review { .. } => 5,
            Self::Security { .. } => 6,
            Self::InstalledBinary { .. } => 7,
            Self::Delivery { .. } => 8,
            Self::Nominal(_) => 9,
        }
    }

    fn canonical_value(&self) -> CborValue {
        let (tag, mut fields) = match self {
            Self::Deterministic {
                executable_bytes_hash,
                executable_version_hash,
                arguments_hash,
                working_directory_hash,
                relevant_environment_hash,
                subject_revision_hash,
                dirty_state_hash,
                exit_status_hash,
                stdout_hash,
                stderr_hash,
            } => (
                1,
                vec![
                    executable_bytes_hash,
                    executable_version_hash,
                    arguments_hash,
                    working_directory_hash,
                    relevant_environment_hash,
                    subject_revision_hash,
                    dirty_state_hash,
                    exit_status_hash,
                    stdout_hash,
                    stderr_hash,
                ]
                .into_iter()
                .map(bytes)
                .collect(),
            ),
            Self::Nondeterministic {
                seed_or_absence_hash,
                sample_plan_hash,
                repetitions,
                thresholds_hash,
                confidence_or_error_model_hash,
                result_set_hash,
            } => {
                return CborValue::Array(vec![
                    CborValue::Unsigned(2),
                    bytes(seed_or_absence_hash),
                    bytes(sample_plan_hash),
                    CborValue::Unsigned(*repetitions),
                    bytes(thresholds_hash),
                    bytes(confidence_or_error_model_hash),
                    bytes(result_set_hash),
                ]);
            }
            Self::Manual {
                actor_binding_hash,
                procedure_material_hash,
                target_build_hash,
                device_hash,
                capture_artifact_hash,
            } => (
                3,
                vec![
                    actor_binding_hash,
                    procedure_material_hash,
                    target_build_hash,
                    device_hash,
                    capture_artifact_hash,
                ]
                .into_iter()
                .map(bytes)
                .collect(),
            ),
            Self::Visual {
                actor_binding_hash,
                target_build_hash,
                device_hash,
                viewport_hash,
                display_scale_hash,
                theme_hash,
                capture_artifact_hash,
            } => (
                4,
                vec![
                    actor_binding_hash,
                    target_build_hash,
                    device_hash,
                    viewport_hash,
                    display_scale_hash,
                    theme_hash,
                    capture_artifact_hash,
                ]
                .into_iter()
                .map(bytes)
                .collect(),
            ),
            Self::Review {
                reviewer_binding_hash,
                model_context_hash,
                tool_context_hash,
                review_packet_hash,
                tree_hash,
                findings_hash,
                confidence_hash,
            } => (
                5,
                vec![
                    reviewer_binding_hash,
                    model_context_hash,
                    tool_context_hash,
                    review_packet_hash,
                    tree_hash,
                    findings_hash,
                    confidence_hash,
                ]
                .into_iter()
                .map(bytes)
                .collect(),
            ),
            Self::Security {
                threat_scope_hash,
                scanner_hash,
                ruleset_hash,
                dependency_snapshot_hash,
                exclusions_hash,
                target_tree_hash,
                findings_hash,
            } => (
                6,
                vec![
                    threat_scope_hash,
                    scanner_hash,
                    ruleset_hash,
                    dependency_snapshot_hash,
                    exclusions_hash,
                    target_tree_hash,
                    findings_hash,
                ]
                .into_iter()
                .map(bytes)
                .collect(),
            ),
            Self::InstalledBinary {
                installed_path_hash,
                binary_digest,
                build_provenance_hash,
                commit_provenance_hash,
            } => (
                7,
                vec![
                    installed_path_hash,
                    binary_digest,
                    build_provenance_hash,
                    commit_provenance_hash,
                ]
                .into_iter()
                .map(bytes)
                .collect(),
            ),
            Self::Delivery {
                target_hash,
                immutable_artifact_hash,
                tag_hash,
                commit_hash,
                publication_receipt_hash,
                endpoint_snapshot_hash,
                availability_time,
            } => {
                return CborValue::Array(vec![
                    CborValue::Unsigned(8),
                    bytes(target_hash),
                    bytes(immutable_artifact_hash),
                    bytes(tag_hash),
                    bytes(commit_hash),
                    bytes(publication_receipt_hash),
                    bytes(endpoint_snapshot_hash),
                    CborValue::Unsigned(*availability_time),
                ]);
            }
            Self::Nominal(payload) => (9, vec![payload.canonical_value()]),
        };
        fields.insert(0, CborValue::Unsigned(tag));
        CborValue::Array(fields)
    }

    fn validate_for(&self, kind: ObservationKindV1) -> Result<(), ObservationError> {
        let expected = expected_payload_detail_tag(kind);
        let expected_schema_id = payload_schema_id(kind)?;
        let value = self.canonical_value();
        let CborValue::Array(fields) = &value else {
            unreachable!("typed payload detail is always an array")
        };
        if fields.first() != Some(&CborValue::Unsigned(expected))
            || fields.iter().skip(1).any(
                |field| matches!(field, CborValue::Bytes(value) if value.as_slice() == [0; 32]),
            )
            || matches!(self, Self::Nondeterministic { repetitions: 0, .. })
            || matches!(self, Self::Nominal(payload) if payload.kind() != kind || payload.schema_id() != expected_schema_id)
            || matches!(
                self,
                Self::Delivery {
                    availability_time: 0,
                    ..
                }
            )
        {
            return Err(ObservationError::InvalidPayloadSemantics);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationPayloadV1 {
    kind: ObservationKindV1,
    common: ObservationPayloadCommonV1,
    detail: ObservationPayloadDetailV1,
    semantic_hash: [u8; 32],
}

impl ObservationPayloadV1 {
    pub fn new(
        kind: ObservationKindV1,
        common: ObservationPayloadCommonV1,
        detail: ObservationPayloadDetailV1,
    ) -> Result<Self, ObservationError> {
        detail.validate_for(kind)?;
        let value = observation_payload_value(kind, common, &detail);
        Ok(Self {
            kind,
            common,
            detail,
            semantic_hash: domain_hash("maestro.vnext.evidence.observation-payload.v1", &value)?,
        })
    }

    pub const fn kind(&self) -> ObservationKindV1 {
        self.kind
    }

    pub const fn semantic_hash(&self) -> &[u8; 32] {
        &self.semantic_hash
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ObservationError> {
        Ok(deterministic_cbor::encode(&observation_payload_value(
            self.kind,
            self.common,
            &self.detail,
        ))?)
    }

    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, ObservationError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = &value else {
            return Err(ObservationError::InvalidPayloadSemantics);
        };
        let [
            CborValue::Unsigned(1),
            CborValue::Unsigned(kind),
            common,
            detail,
        ] = fields.as_slice()
        else {
            return Err(ObservationError::InvalidPayloadSemantics);
        };
        let kind = ObservationKindV1::from_tag(*kind)?;
        let common = parse_payload_common(common)?;
        let detail = parse_payload_detail(detail)?;
        let payload = Self::new(kind, common, detail)?;
        if payload.canonical_bytes()? != bytes {
            return Err(ObservationError::InvalidPayloadSemantics);
        }
        Ok(payload)
    }

    pub(crate) fn matches_observation(
        &self,
        observation: &ObservationV1,
    ) -> Result<bool, ObservationError> {
        Ok(self.kind == observation.kind
            && self.common
                == ObservationPayloadCommonV1::new(
                    &observation.subjects,
                    observation.procedure_hash,
                    observation.environment_hash,
                    observation.toolchain_hash,
                    observation.observed_at,
                    observation.recorded_at,
                    observation.clock_basis_hash,
                )?)
    }
}

fn observation_payload_value(
    kind: ObservationKindV1,
    common: ObservationPayloadCommonV1,
    detail: &ObservationPayloadDetailV1,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Unsigned(kind.tag()),
        common.canonical_value(),
        detail.canonical_value(),
    ])
}

fn parse_payload_common(value: &CborValue) -> Result<ObservationPayloadCommonV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidPayloadSemantics);
    };
    let [
        subject_set_hash,
        procedure_hash,
        environment_hash,
        toolchain_hash,
        CborValue::Unsigned(observed_at),
        CborValue::Unsigned(recorded_at),
        clock_basis_hash,
    ] = fields.as_slice()
    else {
        return Err(ObservationError::InvalidPayloadSemantics);
    };
    let common = ObservationPayloadCommonV1 {
        subject_set_hash: exact_observation_digest(subject_set_hash)?,
        procedure_hash: exact_observation_digest(procedure_hash)?,
        environment_hash: exact_observation_digest(environment_hash)?,
        toolchain_hash: exact_observation_digest(toolchain_hash)?,
        observed_at: *observed_at,
        recorded_at: *recorded_at,
        clock_basis_hash: exact_observation_digest(clock_basis_hash)?,
    };
    for value in [
        common.subject_set_hash,
        common.procedure_hash,
        common.environment_hash,
        common.toolchain_hash,
        common.clock_basis_hash,
    ] {
        require_nonzero(value, "payload common field")?;
    }
    if common.observed_at == 0 || common.recorded_at < common.observed_at {
        return Err(ObservationError::InvalidPayloadSemantics);
    }
    Ok(common)
}

fn parse_payload_detail(value: &CborValue) -> Result<ObservationPayloadDetailV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidPayloadSemantics);
    };
    let digest = |value: &CborValue| exact_observation_digest(value);
    match fields.as_slice() {
        [CborValue::Unsigned(1), a, b, c, d, e, f, g, h, i, j] => {
            Ok(ObservationPayloadDetailV1::Deterministic {
                executable_bytes_hash: digest(a)?,
                executable_version_hash: digest(b)?,
                arguments_hash: digest(c)?,
                working_directory_hash: digest(d)?,
                relevant_environment_hash: digest(e)?,
                subject_revision_hash: digest(f)?,
                dirty_state_hash: digest(g)?,
                exit_status_hash: digest(h)?,
                stdout_hash: digest(i)?,
                stderr_hash: digest(j)?,
            })
        }
        [
            CborValue::Unsigned(2),
            a,
            b,
            CborValue::Unsigned(repetitions),
            c,
            d,
            e,
        ] => Ok(ObservationPayloadDetailV1::Nondeterministic {
            seed_or_absence_hash: digest(a)?,
            sample_plan_hash: digest(b)?,
            repetitions: *repetitions,
            thresholds_hash: digest(c)?,
            confidence_or_error_model_hash: digest(d)?,
            result_set_hash: digest(e)?,
        }),
        [CborValue::Unsigned(3), a, b, c, d, e] => Ok(ObservationPayloadDetailV1::Manual {
            actor_binding_hash: digest(a)?,
            procedure_material_hash: digest(b)?,
            target_build_hash: digest(c)?,
            device_hash: digest(d)?,
            capture_artifact_hash: digest(e)?,
        }),
        [CborValue::Unsigned(4), a, b, c, d, e, f, g] => Ok(ObservationPayloadDetailV1::Visual {
            actor_binding_hash: digest(a)?,
            target_build_hash: digest(b)?,
            device_hash: digest(c)?,
            viewport_hash: digest(d)?,
            display_scale_hash: digest(e)?,
            theme_hash: digest(f)?,
            capture_artifact_hash: digest(g)?,
        }),
        [CborValue::Unsigned(5), a, b, c, d, e, f, g] => Ok(ObservationPayloadDetailV1::Review {
            reviewer_binding_hash: digest(a)?,
            model_context_hash: digest(b)?,
            tool_context_hash: digest(c)?,
            review_packet_hash: digest(d)?,
            tree_hash: digest(e)?,
            findings_hash: digest(f)?,
            confidence_hash: digest(g)?,
        }),
        [CborValue::Unsigned(6), a, b, c, d, e, f, g] => Ok(ObservationPayloadDetailV1::Security {
            threat_scope_hash: digest(a)?,
            scanner_hash: digest(b)?,
            ruleset_hash: digest(c)?,
            dependency_snapshot_hash: digest(d)?,
            exclusions_hash: digest(e)?,
            target_tree_hash: digest(f)?,
            findings_hash: digest(g)?,
        }),
        [CborValue::Unsigned(7), a, b, c, d] => Ok(ObservationPayloadDetailV1::InstalledBinary {
            installed_path_hash: digest(a)?,
            binary_digest: digest(b)?,
            build_provenance_hash: digest(c)?,
            commit_provenance_hash: digest(d)?,
        }),
        [
            CborValue::Unsigned(8),
            a,
            b,
            c,
            d,
            e,
            f,
            CborValue::Unsigned(availability_time),
        ] => Ok(ObservationPayloadDetailV1::Delivery {
            target_hash: digest(a)?,
            immutable_artifact_hash: digest(b)?,
            tag_hash: digest(c)?,
            commit_hash: digest(d)?,
            publication_receipt_hash: digest(e)?,
            endpoint_snapshot_hash: digest(f)?,
            availability_time: *availability_time,
        }),
        [CborValue::Unsigned(9), payload] => Ok(ObservationPayloadDetailV1::Nominal(
            parse_nominal_payload(payload)?,
        )),
        _ => Err(ObservationError::InvalidPayloadSemantics),
    }
}

fn parse_nominal_payload(
    value: &CborValue,
) -> Result<NominalObservationPayloadV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidPayloadSemantics);
    };
    let [
        CborValue::Unsigned(kind),
        schema_id,
        CborValue::Array(values),
    ] = fields.as_slice()
    else {
        return Err(ObservationError::InvalidPayloadSemantics);
    };
    let kind = ObservationKindV1::from_tag(*kind)?;
    let payload = NominalObservationPayloadV1::new(
        kind,
        values
            .iter()
            .map(parse_payload_field)
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    if payload.schema_id().as_bytes() != &exact_observation_digest(schema_id)? {
        return Err(ObservationError::InvalidPayloadSemantics);
    }
    Ok(payload)
}

fn parse_payload_field(value: &CborValue) -> Result<ObservationPayloadFieldV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidPayloadSemantics);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(1), value] => Ok(ObservationPayloadFieldV1::Digest(
            exact_observation_digest(value)?,
        )),
        [CborValue::Unsigned(2), CborValue::Unsigned(value)] => {
            Ok(ObservationPayloadFieldV1::Count(*value))
        }
        [CborValue::Unsigned(3), CborValue::Unsigned(value)] => {
            Ok(ObservationPayloadFieldV1::Timestamp(*value))
        }
        [CborValue::Unsigned(4), CborValue::Unsigned(value)] => {
            Ok(ObservationPayloadFieldV1::Tag(*value))
        }
        [CborValue::Unsigned(5), CborValue::Bool(value)] => {
            Ok(ObservationPayloadFieldV1::Boolean(*value))
        }
        _ => Err(ObservationError::InvalidPayloadSemantics),
    }
}

fn payload_schema_id(kind: ObservationKindV1) -> Result<SchemaIdV1, ObservationError> {
    Ok(derive_identity(&payload_schema_value(kind)?)?)
}

fn payload_schema_value(kind: ObservationKindV1) -> Result<CborValue, ObservationError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.evidence-observation-payload-schema.v1")?,
        CborValue::Unsigned(kind.tag()),
        CborValue::text(kind.name())?,
        CborValue::Unsigned(expected_payload_detail_tag(kind)),
        CborValue::Array(
            payload_field_specs(kind)
                .iter()
                .map(|field| {
                    Ok(CborValue::Array(vec![
                        CborValue::text(field.name)?,
                        CborValue::Unsigned(field.field_type.tag()),
                    ]))
                })
                .collect::<Result<Vec<_>, ObservationError>>()?,
        ),
    ]))
}

#[rustfmt::skip]
fn payload_field_specs(kind: ObservationKindV1) -> Vec<ObservationPayloadFieldSpecV1> {
    use ObservationKindV1 as K;
    use ObservationPayloadFieldTypeV1 as F;
    let fields: &[ObservationPayloadFieldSpecV1] = match kind {
        K::DeterministicProcedure => &[
            payload_field("executable_bytes_hash", F::Digest), payload_field("executable_version_hash", F::Digest),
            payload_field("arguments_hash", F::Digest), payload_field("working_directory_hash", F::Digest),
            payload_field("relevant_environment_hash", F::Digest), payload_field("subject_revision_hash", F::Digest),
            payload_field("dirty_state_hash", F::Digest), payload_field("exit_status_hash", F::Digest),
            payload_field("stdout_hash", F::Digest), payload_field("stderr_hash", F::Digest),
        ],
        K::NondeterministicProcedure => &[
            payload_field("seed_or_absence_hash", F::Digest), payload_field("sample_plan_hash", F::Digest),
            payload_field("repetitions", F::Count), payload_field("thresholds_hash", F::Digest),
            payload_field("confidence_or_error_model_hash", F::Digest), payload_field("result_set_hash", F::Digest),
        ],
        K::ManualProcedure => &[
            payload_field("actor_binding_hash", F::Digest), payload_field("procedure_material_hash", F::Digest),
            payload_field("target_build_hash", F::Digest), payload_field("device_hash", F::Digest),
            payload_field("capture_artifact_hash", F::Digest),
        ],
        K::VisualCapture => &[
            payload_field("actor_binding_hash", F::Digest), payload_field("target_build_hash", F::Digest),
            payload_field("device_hash", F::Digest), payload_field("viewport_hash", F::Digest),
            payload_field("display_scale_hash", F::Digest), payload_field("theme_hash", F::Digest),
            payload_field("capture_artifact_hash", F::Digest),
        ],
        K::ReviewFinding => &[
            payload_field("reviewer_binding_hash", F::Digest), payload_field("model_context_hash", F::Digest),
            payload_field("tool_context_hash", F::Digest), payload_field("review_packet_hash", F::Digest),
            payload_field("tree_hash", F::Digest), payload_field("findings_hash", F::Digest),
            payload_field("confidence_hash", F::Digest),
        ],
        K::SecurityAnalysis => &[
            payload_field("threat_scope_hash", F::Digest), payload_field("scanner_hash", F::Digest),
            payload_field("ruleset_hash", F::Digest), payload_field("dependency_snapshot_hash", F::Digest),
            payload_field("exclusions_hash", F::Digest), payload_field("target_tree_hash", F::Digest),
            payload_field("findings_hash", F::Digest),
        ],
        K::InstalledBinary => &[
            payload_field("installed_path_hash", F::Digest), payload_field("binary_digest", F::Digest),
            payload_field("build_provenance_hash", F::Digest), payload_field("commit_provenance_hash", F::Digest),
        ],
        K::RemoteDelivery | K::RemoteRelease => &[
            payload_field("target_hash", F::Digest), payload_field("immutable_artifact_hash", F::Digest),
            payload_field("tag_hash", F::Digest), payload_field("commit_hash", F::Digest),
            payload_field("publication_receipt_hash", F::Digest), payload_field("endpoint_snapshot_hash", F::Digest),
            payload_field("availability_time", F::Timestamp),
        ],
        K::RemoteReadback => &[
            payload_field("target_hash", F::Digest), payload_field("request_hash", F::Digest),
            payload_field("response_hash", F::Digest), payload_field("signed_receipt_hash", F::Digest),
            payload_field("snapshot_hash", F::Digest), payload_field("observed_state_hash", F::Digest),
        ],
        K::ProcessLiveness => &[
            payload_field("process_identity_hash", F::Digest), payload_field("executable_hash", F::Digest),
            payload_field("probe_hash", F::Digest), payload_field("state_hash", F::Digest),
            payload_field("started_at", F::Timestamp), payload_field("checked_at", F::Timestamp),
        ],
        K::SkillActivation => &[
            payload_field("acquisition_identity_hash", F::Digest), payload_field("active_context_hash", F::Digest),
            payload_field("release_hash", F::Digest), payload_field("root_skill_resource_hash", F::Digest),
            payload_field("selected_route_hash", F::Digest), payload_field("capability_resolution_hash", F::Digest),
            payload_field("recipe_resolution_hash", F::Digest), payload_field("context_budget_profile_hash", F::Digest),
            payload_field("loaded_resource_closure_hash", F::Digest),
        ],
        K::HookDelivery => &[
            payload_field("hook_hash", F::Digest), payload_field("event_hash", F::Digest),
            payload_field("delivery_receipt_hash", F::Digest), payload_field("target_hash", F::Digest),
            payload_field("delivered_at", F::Timestamp),
        ],
        K::EventStreamIntegrity => &[
            payload_field("stream_hash", F::Digest), payload_field("first_sequence", F::Count),
            payload_field("last_sequence", F::Count), payload_field("chain_hash", F::Digest),
            payload_field("gap_count", F::Count),
        ],
        K::ResearchSource => &[
            payload_field("source_artifact_hash", F::Digest), payload_field("provenance_hash", F::Digest),
            payload_field("license_hash", F::Digest), payload_field("content_hash", F::Digest),
            payload_field("retrieved_at", F::Timestamp),
        ],
        K::CapabilityProbe => &[
            payload_field("capability_hash", F::Digest), payload_field("provider_procedure_hash", F::Digest),
            payload_field("operation_class", F::Tag), payload_field("response_hash", F::Digest),
            payload_field("checked_at", F::Timestamp),
        ],
        K::BootstrapMandatePresentation => &[
            payload_field("mandate_hash", F::Digest), payload_field("request_hash", F::Digest),
            payload_field("package_hash", F::Digest), payload_field("provider_receipt_hash", F::Digest),
            payload_field("delivered_at", F::Timestamp),
        ],
        K::BootstrapMandateResponse => &[
            payload_field("presentation_observation_hash", F::Digest), payload_field("response_hash", F::Digest),
            payload_field("provider_receipt_hash", F::Digest), payload_field("acquired_at", F::Timestamp),
        ],
        K::GovernedReviewPresentation => &[
            payload_field("review_packet_hash", F::Digest), payload_field("tree_hash", F::Digest),
            payload_field("provider_operation_hash", F::Digest), payload_field("delivery_receipt_hash", F::Digest),
            payload_field("delivered_at", F::Timestamp),
        ],
        K::GovernedReviewResponse => &[
            payload_field("presentation_observation_hash", F::Digest), payload_field("response_hash", F::Digest),
            payload_field("findings_hash", F::Digest), payload_field("confidence_hash", F::Digest),
            payload_field("provider_receipt_hash", F::Digest), payload_field("acquired_at", F::Timestamp),
        ],
        K::HumanBindingSubjectSelection => &[
            payload_field("candidate_set_hash", F::Digest), payload_field("selected_subject_hash", F::Digest),
            payload_field("actor_binding_hash", F::Digest), payload_field("ceremony_hash", F::Digest),
            payload_field("selected_at", F::Timestamp),
        ],
        K::HumanPresence => &[
            payload_field("actor_binding_hash", F::Digest), payload_field("presence_method_hash", F::Digest),
            payload_field("challenge_hash", F::Digest), payload_field("response_hash", F::Digest),
            payload_field("observed_at", F::Timestamp),
        ],
        K::CredentialControl => &[
            payload_field("credential_subject_hash", F::Digest), payload_field("controller_hash", F::Digest),
            payload_field("authentication_method_hash", F::Digest), payload_field("receipt_hash", F::Digest),
            payload_field("verified_at", F::Timestamp),
        ],
        K::HumanAssurance | K::SourceAssurance => &[
            payload_field("assurance_subject_hash", F::Digest), payload_field("assessor_hash", F::Digest),
            payload_field("procedure_hash", F::Digest), payload_field("conclusion_hash", F::Digest),
            payload_field("confidence_hash", F::Digest), payload_field("assessed_at", F::Timestamp),
        ],
        K::SourceCommonCause => &[
            payload_field("source_set_hash", F::Digest), payload_field("shared_dependency_hash", F::Digest),
            payload_field("analysis_hash", F::Digest), payload_field("assessed_at", F::Timestamp),
        ],
        K::SourceContradiction => &[
            payload_field("source_set_hash", F::Digest), payload_field("contradiction_hash", F::Digest),
            payload_field("analysis_hash", F::Digest), payload_field("assessed_at", F::Timestamp),
        ],
        K::SourceRevocation => &[
            payload_field("source_hash", F::Digest), payload_field("revocation_hash", F::Digest),
            payload_field("issuer_hash", F::Digest), payload_field("revoked_at", F::Timestamp),
        ],
        K::TrustedTimeAtomicUnit => &[
            payload_field("clock_source_hash", F::Digest), payload_field("lower_bound", F::Timestamp),
            payload_field("upper_bound", F::Timestamp), payload_field("uncertainty_units", F::Count),
            payload_field("attestation_hash", F::Digest),
        ],
        K::TrustedTimeAcquisition => &[
            payload_field("atomic_unit_set_hash", F::Digest), payload_field("reduction_hash", F::Digest),
            payload_field("accepted_lower_bound", F::Timestamp), payload_field("accepted_upper_bound", F::Timestamp),
            payload_field("authority_state_hash", F::Digest),
        ],
        K::MaintenanceExecutorCurrentness => &[
            payload_field("executor_assertion_hash", F::Digest), payload_field("slot_hash", F::Digest),
            payload_field("procedure_hash", F::Digest), payload_field("conclusion_hash", F::Digest),
            payload_field("checked_at", F::Timestamp),
        ],
        K::ProspectiveContinuityCarrier => &[
            payload_field("carrier_hash", F::Digest), payload_field("revision_hash", F::Digest),
            payload_field("integrity_hash", F::Digest), payload_field("applicability_hash", F::Digest),
            payload_field("checked_at", F::Timestamp),
        ],
        K::RecoveryExternalAnchorRevision => &[
            payload_field("anchor_hash", F::Digest), payload_field("prior_revision_hash", F::Digest),
            payload_field("current_revision_hash", F::Digest), payload_field("receipt_hash", F::Digest),
            payload_field("observed_at", F::Timestamp),
        ],
        K::RecoveryExternalRegistration => &[
            payload_field("subject_hash", F::Digest), payload_field("carrier_hash", F::Digest),
            payload_field("registration_receipt_hash", F::Digest), payload_field("registered_at", F::Timestamp),
        ],
        K::RecoveryExternalStatus => &[
            payload_field("subject_hash", F::Digest), payload_field("accepted_revision_hash", F::Digest),
            payload_field("status", F::Tag), payload_field("receipt_hash", F::Digest),
            payload_field("observed_at", F::Timestamp),
        ],
        K::ExternalHighWaterReadback => &[
            payload_field("carrier_hash", F::Digest), payload_field("revision_hash", F::Digest),
            payload_field("snapshot_hash", F::Digest), payload_field("receipt_hash", F::Digest),
            payload_field("observed_at", F::Timestamp),
        ],
        K::CarrierIntegrityFault => &[
            payload_field("carrier_hash", F::Digest), payload_field("fault", F::Tag),
            payload_field("fault_evidence_hash", F::Digest), payload_field("detected_at", F::Timestamp),
        ],
        K::DistributionFilesystemState => &[
            payload_field("target_root_hash", F::Digest), payload_field("file_census_hash", F::Digest),
            payload_field("metadata_hash", F::Digest), payload_field("custody_hash", F::Digest),
            payload_field("observed_at", F::Timestamp),
        ],
        K::DistributionInstalledResource => &[
            payload_field("resource_hash", F::Digest), payload_field("bundle_hash", F::Digest),
            payload_field("release_hash", F::Digest), payload_field("installed_bytes_hash", F::Digest),
            payload_field("installation_receipt_hash", F::Digest), payload_field("observed_at", F::Timestamp),
        ],
        K::DistributionManagerState => &[
            payload_field("manager_hash", F::Digest), payload_field("manager_version_hash", F::Digest),
            payload_field("state_hash", F::Digest), payload_field("receipt_hash", F::Digest),
            payload_field("observed_at", F::Timestamp),
        ],
        K::PublicationIntegrity => &[
            payload_field("resource_hash", F::Digest), payload_field("publication_root_hash", F::Digest),
            payload_field("manifest_hash", F::Digest), payload_field("signature_hash", F::Digest),
            payload_field("verified_at", F::Timestamp),
        ],
        K::StoreIntegrity => &[
            payload_field("store_head_hash", F::Digest), payload_field("generation_hash", F::Digest),
            payload_field("object_census_hash", F::Digest), payload_field("index_hash", F::Digest),
            payload_field("audit_hash", F::Digest), payload_field("verified_at", F::Timestamp),
        ],
        K::ProtectedContinuityDiagnostic => &[
            payload_field("continuity_state_hash", F::Digest), payload_field("diagnostic_kind", F::Tag),
            payload_field("details_hash", F::Digest), payload_field("observed_at", F::Timestamp),
        ],
    };
    fields.to_vec()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationKindContractV1 {
    kind: ObservationKindV1,
    payload_schema_id: SchemaIdV1,
    required_subject_mask: u16,
    allowed_subject_mask: u16,
    allowed_acquisition_mask: u8,
    contract_hash: [u8; 32],
}

impl ObservationKindContractV1 {
    fn for_kind(kind: ObservationKindV1) -> Result<Self, ObservationError> {
        let required_subject_mask = required_subject_mask(kind);
        let allowed_subject_mask = allowed_subject_mask(kind);
        let allowed_acquisition_mask = allowed_acquisition_mask(kind);
        let payload_schema_id = payload_schema_id(kind)?;
        let contract_hash = domain_hash(
            "maestro.vnext.evidence.observation-kind-contract.v1",
            &CborValue::Array(vec![
                CborValue::Unsigned(kind.tag()),
                CborValue::text(kind.name())?,
                bytes(payload_schema_id.as_bytes()),
                CborValue::Unsigned(u64::from(required_subject_mask)),
                CborValue::Unsigned(u64::from(allowed_subject_mask)),
                CborValue::Unsigned(u64::from(allowed_acquisition_mask)),
                CborValue::Unsigned(kind.producer_action_tag()),
                CborValue::Array(
                    kind.source_route_tags()
                        .iter()
                        .copied()
                        .map(CborValue::Unsigned)
                        .collect(),
                ),
                CborValue::Array(
                    kind.cma_compatibility()
                        .iter()
                        .map(|(profile, route)| {
                            CborValue::Array(vec![
                                CborValue::Unsigned(*profile),
                                CborValue::Unsigned(*route),
                            ])
                        })
                        .collect(),
                ),
            ]),
        )?;
        Ok(Self {
            kind,
            payload_schema_id,
            required_subject_mask,
            allowed_subject_mask,
            allowed_acquisition_mask,
            contract_hash,
        })
    }

    pub const fn kind(self) -> ObservationKindV1 {
        self.kind
    }

    pub const fn payload_schema_id(self) -> SchemaIdV1 {
        self.payload_schema_id
    }

    pub fn payload_fields(self) -> Vec<ObservationPayloadFieldSpecV1> {
        payload_field_specs(self.kind)
    }

    pub const fn required_subject_mask(self) -> u16 {
        self.required_subject_mask
    }

    pub const fn allowed_subject_mask(self) -> u16 {
        self.allowed_subject_mask
    }

    pub const fn allowed_acquisition_mask(self) -> u8 {
        self.allowed_acquisition_mask
    }

    pub const fn payload_detail_tag(self) -> u64 {
        expected_payload_detail_tag(self.kind)
    }

    pub const fn contract_hash(&self) -> &[u8; 32] {
        &self.contract_hash
    }

    fn admits_subjects(self, subjects: &[ObservationSubjectV1]) -> bool {
        let actual = subjects
            .iter()
            .fold(0_u16, |mask, subject| mask | subject_mask(subject.kind()));
        actual & self.required_subject_mask == self.required_subject_mask
            && actual & !self.allowed_subject_mask == 0
    }

    fn admits_acquisition(self, acquisition: &ObservationAcquisitionV1) -> bool {
        self.allowed_acquisition_mask & acquisition.mask() != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceRedactionPolicyV1 {
    scanner_ruleset_hash: [u8; 32],
    maximum_payload_bytes: u64,
}

fn secret_scanner_ruleset_hash_v1() -> Result<[u8; 32], ObservationError> {
    Ok(domain_hash(
        "maestro.vnext.evidence.secret-scanner-ruleset.v1",
        &CborValue::Unsigned(1),
    )?)
}

fn secret_scanner_tool_hash_v1() -> Result<[u8; 32], ObservationError> {
    Ok(domain_hash(
        "maestro.vnext.evidence.secret-scanner-implementation.v1",
        &CborValue::Unsigned(1),
    )?)
}

fn secret_finding_count_v1(payload: &ObservationPayloadV1) -> u64 {
    fn findings(value: &CborValue) -> u64 {
        match value {
            CborValue::Bytes(bytes) if bytes.len() == 32 => {
                let unique = bytes
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>();
                u64::from(
                    unique.len() < 8
                        || bytes
                            .iter()
                            .all(|byte| byte.is_ascii_graphic() || *byte == b' '),
                )
            }
            CborValue::Array(values) => values.iter().map(findings).sum(),
            CborValue::Unsigned(_)
            | CborValue::Bool(_)
            | CborValue::Bytes(_)
            | CborValue::Text(_) => 0,
        }
    }
    findings(&observation_payload_value(
        payload.kind,
        payload.common,
        &payload.detail,
    ))
}

impl EvidenceRedactionPolicyV1 {
    pub fn prohibit_secrets_v1(maximum_payload_bytes: u64) -> Result<Self, ObservationError> {
        Self::prohibit_secrets(secret_scanner_ruleset_hash_v1()?, maximum_payload_bytes)
    }

    pub fn prohibit_secrets(
        scanner_ruleset_hash: [u8; 32],
        maximum_payload_bytes: u64,
    ) -> Result<Self, ObservationError> {
        require_nonzero(scanner_ruleset_hash, "Evidence secret scanner ruleset")?;
        if scanner_ruleset_hash != secret_scanner_ruleset_hash_v1()? || maximum_payload_bytes == 0 {
            return Err(ObservationError::InvalidPayloadManifest);
        }
        Ok(Self {
            scanner_ruleset_hash,
            maximum_payload_bytes,
        })
    }

    pub const fn scanner_ruleset_hash(self) -> [u8; 32] {
        self.scanner_ruleset_hash
    }

    pub const fn maximum_payload_bytes(self) -> u64 {
        self.maximum_payload_bytes
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            bytes(&self.scanner_ruleset_hash),
            CborValue::Unsigned(self.maximum_payload_bytes),
            CborValue::Unsigned(1),
        ])
    }

    fn identity(self) -> Result<[u8; 32], ObservationError> {
        Ok(domain_hash(
            "maestro.vnext.evidence.redaction-policy.v1",
            &self.canonical_value(),
        )?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceRetentionClassV1 {
    GateAndRecoveryClosure,
    ExplicitSecurityErasureEligible,
}

impl EvidenceRetentionClassV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::GateAndRecoveryClosure => 1,
            Self::ExplicitSecurityErasureEligible => 2,
        }
    }

    fn from_tag(tag: u64) -> Result<Self, ObservationError> {
        match tag {
            1 => Ok(Self::GateAndRecoveryClosure),
            2 => Ok(Self::ExplicitSecurityErasureEligible),
            _ => Err(ObservationError::InvalidPayloadManifest),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceRetentionPolicyV1 {
    class: EvidenceRetentionClassV1,
    minimum_retain_until: u64,
}

impl EvidenceRetentionPolicyV1 {
    pub fn new(
        class: EvidenceRetentionClassV1,
        minimum_retain_until: u64,
    ) -> Result<Self, ObservationError> {
        if minimum_retain_until == 0 {
            return Err(ObservationError::InvalidPayloadManifest);
        }
        Ok(Self {
            class,
            minimum_retain_until,
        })
    }

    pub const fn class(self) -> EvidenceRetentionClassV1 {
        self.class
    }

    pub const fn minimum_retain_until(self) -> u64 {
        self.minimum_retain_until
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Unsigned(self.class.tag()),
            CborValue::Unsigned(self.minimum_retain_until),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EvidenceSecretScanReceiptV1 {
    payload_object_id: StoreObjectIdV1,
    payload_semantic_hash: [u8; 32],
    payload_byte_length: u64,
    redaction_policy_id: [u8; 32],
    scanner: ExecutionProducerV1,
    scanner_tool_hash: [u8; 32],
    scanned_at: u64,
    secret_finding_count: u64,
}

impl EvidenceSecretScanReceiptV1 {
    pub fn scan(
        payload_object_id: StoreObjectIdV1,
        payload: &ObservationPayloadV1,
        redaction_policy: EvidenceRedactionPolicyV1,
        scanner: ExecutionProducerV1,
        scanned_at: u64,
    ) -> Result<Self, ObservationError> {
        let payload_bytes = payload.canonical_bytes()?;
        Self::from_parts(
            payload_object_id,
            *payload.semantic_hash(),
            u64::try_from(payload_bytes.len())
                .map_err(|_| ObservationError::InvalidSecretScanReceipt)?,
            redaction_policy,
            scanner,
            secret_scanner_tool_hash_v1()?,
            scanned_at,
            secret_finding_count_v1(payload),
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "stored authenticated scan receipts decode every exact payload and scanner binding"
    )]
    fn from_parts(
        payload_object_id: StoreObjectIdV1,
        payload_semantic_hash: [u8; 32],
        payload_byte_length: u64,
        redaction_policy: EvidenceRedactionPolicyV1,
        scanner: ExecutionProducerV1,
        scanner_tool_hash: [u8; 32],
        scanned_at: u64,
        secret_finding_count: u64,
    ) -> Result<Self, ObservationError> {
        require_nonzero(
            *payload_object_id.as_bytes(),
            "scanned Evidence payload Object",
        )?;
        require_nonzero(payload_semantic_hash, "scanned Evidence payload semantics")?;
        require_nonzero(
            *scanner.principal_id().as_bytes(),
            "Evidence scanner Principal",
        )?;
        require_nonzero(scanner_tool_hash, "Evidence scanner tool")?;
        if redaction_policy.scanner_ruleset_hash() != secret_scanner_ruleset_hash_v1()?
            || scanner_tool_hash != secret_scanner_tool_hash_v1()?
            || payload_byte_length == 0
            || scanned_at == 0
        {
            return Err(ObservationError::InvalidSecretScanReceipt);
        }
        Ok(Self {
            payload_object_id,
            payload_semantic_hash,
            payload_byte_length,
            redaction_policy_id: redaction_policy.identity()?,
            scanner,
            scanner_tool_hash,
            scanned_at,
            secret_finding_count,
        })
    }

    pub const fn scanner(self) -> ExecutionProducerV1 {
        self.scanner
    }

    pub const fn scanned_at(self) -> u64 {
        self.scanned_at
    }

    pub const fn secret_finding_count(self) -> u64 {
        self.secret_finding_count
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(1),
            bytes(self.payload_object_id.as_bytes()),
            bytes(&self.payload_semantic_hash),
            CborValue::Unsigned(self.payload_byte_length),
            bytes(&self.redaction_policy_id),
            self.scanner.canonical_value(),
            bytes(&self.scanner_tool_hash),
            CborValue::Unsigned(self.scanned_at),
            CborValue::Unsigned(self.secret_finding_count),
        ])
    }

    fn validates(
        self,
        object_id: StoreObjectIdV1,
        semantic_hash: [u8; 32],
        byte_length: u64,
        redaction_policy: EvidenceRedactionPolicyV1,
        expected_secret_finding_count: u64,
    ) -> Result<bool, ObservationError> {
        Ok(self.payload_object_id == object_id
            && self.payload_semantic_hash == semantic_hash
            && self.payload_byte_length == byte_length
            && self.redaction_policy_id == redaction_policy.identity()?
            && self.scanner_tool_hash == secret_scanner_tool_hash_v1()?
            && self.secret_finding_count == expected_secret_finding_count
            && self.secret_finding_count == 0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidencePayloadManifestV1 {
    object_id: StoreObjectIdV1,
    schema_id: SchemaIdV1,
    byte_length: u64,
    semantic_hash: [u8; 32],
    media_type: String,
    redaction_policy: EvidenceRedactionPolicyV1,
    secret_scan_receipt: EvidenceSecretScanReceiptV1,
    retention_policy: EvidenceRetentionPolicyV1,
}

impl EvidencePayloadManifestV1 {
    pub fn new(
        kind: ObservationKindV1,
        object_id: StoreObjectIdV1,
        payload: &ObservationPayloadV1,
        media_type: &str,
        redaction_policy: EvidenceRedactionPolicyV1,
        secret_scan_receipt: EvidenceSecretScanReceiptV1,
        retention_policy: EvidenceRetentionPolicyV1,
    ) -> Result<Self, ObservationError> {
        if payload.kind() != kind {
            return Err(ObservationError::InvalidPayloadSemantics);
        }
        let secret_finding_count = secret_finding_count_v1(payload);
        let byte_length = u64::try_from(payload.canonical_bytes()?.len())
            .map_err(|_| ObservationError::InvalidPayloadManifest)?;
        let manifest = Self::from_parts(
            kind,
            object_id,
            byte_length,
            *payload.semantic_hash(),
            media_type,
            redaction_policy,
            secret_scan_receipt,
            retention_policy,
        )?;
        if !manifest.secret_scan_receipt.validates(
            object_id,
            *payload.semantic_hash(),
            byte_length,
            redaction_policy,
            secret_finding_count,
        )? {
            return Err(ObservationError::InvalidSecretScanReceipt);
        }
        Ok(manifest)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "stored payload manifests decode every exact integrity and policy binding"
    )]
    fn from_parts(
        kind: ObservationKindV1,
        object_id: StoreObjectIdV1,
        byte_length: u64,
        semantic_hash: [u8; 32],
        media_type: &str,
        redaction_policy: EvidenceRedactionPolicyV1,
        secret_scan_receipt: EvidenceSecretScanReceiptV1,
        retention_policy: EvidenceRetentionPolicyV1,
    ) -> Result<Self, ObservationError> {
        require_nonzero(*object_id.as_bytes(), "Evidence payload Object")?;
        if byte_length == 0
            || media_type.is_empty()
            || media_type.len() > MAX_MEDIA_TYPE_BYTES_V1
            || !media_type.is_ascii()
        {
            return Err(ObservationError::InvalidPayloadManifest);
        }
        require_nonzero(semantic_hash, "Evidence payload semantics")?;
        if byte_length > redaction_policy.maximum_payload_bytes()
            || !secret_scan_receipt.validates(
                object_id,
                semantic_hash,
                byte_length,
                redaction_policy,
                secret_scan_receipt.secret_finding_count(),
            )?
        {
            return Err(ObservationError::InvalidSecretScanReceipt);
        }
        Ok(Self {
            object_id,
            schema_id: kind.contract()?.payload_schema_id(),
            byte_length,
            semantic_hash,
            media_type: media_type.to_owned(),
            redaction_policy,
            secret_scan_receipt,
            retention_policy,
        })
    }

    pub const fn object_id(&self) -> StoreObjectIdV1 {
        self.object_id
    }

    pub const fn schema_id(&self) -> SchemaIdV1 {
        self.schema_id
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn semantic_hash(&self) -> &[u8; 32] {
        &self.semantic_hash
    }

    pub const fn redaction_policy(&self) -> EvidenceRedactionPolicyV1 {
        self.redaction_policy
    }

    pub const fn secret_scan_receipt(&self) -> EvidenceSecretScanReceiptV1 {
        self.secret_scan_receipt
    }

    pub const fn retention_policy(&self) -> EvidenceRetentionPolicyV1 {
        self.retention_policy
    }

    pub(crate) fn validates_exact_secret_scan(
        &self,
        payload: &ObservationPayloadV1,
    ) -> Result<bool, ObservationError> {
        self.secret_scan_receipt.validates(
            self.object_id,
            *payload.semantic_hash(),
            u64::try_from(payload.canonical_bytes()?.len())
                .map_err(|_| ObservationError::InvalidSecretScanReceipt)?,
            self.redaction_policy,
            secret_finding_count_v1(payload),
        )
    }

    fn canonical_value(&self) -> Result<CborValue, ObservationError> {
        Ok(CborValue::Array(vec![
            bytes(self.object_id.as_bytes()),
            bytes(self.schema_id.as_bytes()),
            CborValue::Unsigned(self.byte_length),
            bytes(&self.semantic_hash),
            CborValue::text(&self.media_type)?,
            self.redaction_policy.canonical_value(),
            self.secret_scan_receipt.canonical_value(),
            self.retention_policy.canonical_value(),
        ]))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationAcquisitionV1 {
    EffectFree {
        acquisition_id: [u8; 32],
        transport_contract_hash: [u8; 32],
    },
    RunMediated {
        acquisition_id: [u8; 32],
        run_id: RunIdV1,
        owner: ExecutionAttemptOwnerV1,
    },
    DeclaredDerivation {
        sources: Vec<ObservationRecordIdV1>,
    },
}

impl ObservationAcquisitionV1 {
    pub fn effect_free(
        acquisition_id: [u8; 32],
        transport_contract_hash: [u8; 32],
    ) -> Result<Self, ObservationError> {
        require_nonzero(acquisition_id, "effect-free acquisition")?;
        require_nonzero(transport_contract_hash, "effect-free transport contract")?;
        Ok(Self::EffectFree {
            acquisition_id,
            transport_contract_hash,
        })
    }

    pub fn run_mediated(acquisition_id: [u8; 32], run: &RunV1) -> Result<Self, ObservationError> {
        require_nonzero(acquisition_id, "run-mediated acquisition")?;
        Ok(Self::RunMediated {
            acquisition_id,
            run_id: run.id(),
            owner: run.owner(),
        })
    }

    pub fn declared_derivation(
        mut sources: Vec<ObservationRecordIdV1>,
    ) -> Result<Self, ObservationError> {
        sources.sort_unstable();
        if sources.is_empty()
            || sources.len() > MAX_OBSERVATION_LINEAGE_V1
            || sources.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(ObservationError::InvalidDerivationSources);
        }
        Ok(Self::DeclaredDerivation { sources })
    }

    pub const fn has_run(&self) -> bool {
        matches!(self, Self::RunMediated { .. })
    }

    pub const fn acquisition_id(&self) -> Option<&[u8; 32]> {
        match self {
            Self::EffectFree { acquisition_id, .. } | Self::RunMediated { acquisition_id, .. } => {
                Some(acquisition_id)
            }
            Self::DeclaredDerivation { .. } => None,
        }
    }

    pub const fn run_binding(&self) -> Option<(RunIdV1, ExecutionAttemptOwnerV1)> {
        match self {
            Self::RunMediated { run_id, owner, .. } => Some((*run_id, *owner)),
            Self::EffectFree { .. } | Self::DeclaredDerivation { .. } => None,
        }
    }

    pub fn derivation_sources(&self) -> &[ObservationRecordIdV1] {
        match self {
            Self::DeclaredDerivation { sources } => sources,
            Self::EffectFree { .. } | Self::RunMediated { .. } => &[],
        }
    }

    const fn mask(&self) -> u8 {
        match self {
            Self::EffectFree { .. } => 1,
            Self::RunMediated { .. } => 2,
            Self::DeclaredDerivation { .. } => 4,
        }
    }

    fn canonical_value(&self) -> CborValue {
        match self {
            Self::EffectFree {
                acquisition_id,
                transport_contract_hash,
            } => CborValue::Array(vec![
                CborValue::Unsigned(1),
                bytes(acquisition_id),
                bytes(transport_contract_hash),
            ]),
            Self::RunMediated {
                acquisition_id,
                run_id,
                owner,
            } => CborValue::Array(vec![
                CborValue::Unsigned(2),
                bytes(acquisition_id),
                bytes(run_id.as_bytes()),
                attempt_owner_value(*owner),
            ]),
            Self::DeclaredDerivation { sources } => CborValue::Array(vec![
                CborValue::Unsigned(3),
                CborValue::Array(sources.iter().map(|id| bytes(id.as_bytes())).collect()),
            ]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationPublicationRouteV1 {
    producer_action_tag: u64,
    source_route_tag: Option<u64>,
    cma_profile_tag: Option<u64>,
}

impl ObservationPublicationRouteV1 {
    pub fn new(
        kind: ObservationKindV1,
        producer_action_tag: u64,
        source_route_tag: Option<u64>,
        cma_profile_tag: Option<u64>,
    ) -> Result<Self, ObservationError> {
        if producer_action_tag != kind.producer_action_tag() {
            return Err(ObservationError::InvalidPublicationRoute);
        }
        let routes = kind.source_route_tags();
        match (source_route_tag, cma_profile_tag) {
            (None, None) if routes.is_empty() => {}
            (Some(route), Some(profile))
                if routes.contains(&route)
                    && kind.cma_compatibility().contains(&(profile, route)) => {}
            _ => return Err(ObservationError::InvalidPublicationRoute),
        }
        Ok(Self {
            producer_action_tag,
            source_route_tag,
            cma_profile_tag,
        })
    }

    fn canonical_value(self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Unsigned(self.producer_action_tag),
            CborValue::optional(self.source_route_tag.map(CborValue::Unsigned)),
            CborValue::optional(self.cma_profile_tag.map(CborValue::Unsigned)),
        ])
    }

    pub const fn producer_action_tag(self) -> u64 {
        self.producer_action_tag
    }

    pub const fn source_route_tag(self) -> Option<u64> {
        self.source_route_tag
    }

    pub const fn cma_profile_tag(self) -> Option<u64> {
        self.cma_profile_tag
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationDraftV1 {
    pub kind: ObservationKindV1,
    pub store_domain_id: StoreDomainIdV1,
    pub subjects: Vec<ObservationSubjectV1>,
    pub producer: ExecutionProducerV1,
    pub procedure_hash: [u8; 32],
    pub environment_hash: [u8; 32],
    pub toolchain_hash: [u8; 32],
    pub observed_at: u64,
    pub recorded_at: u64,
    pub clock_basis_hash: [u8; 32],
    pub lineage: Vec<ObservationRecordIdV1>,
    pub payload: EvidencePayloadManifestV1,
    pub acquisition: ObservationAcquisitionV1,
    pub publication_route: ObservationPublicationRouteV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationV1 {
    id: ObservationRecordIdV1,
    kind: ObservationKindV1,
    store_domain_id: StoreDomainIdV1,
    subjects: Vec<ObservationSubjectV1>,
    producer: ExecutionProducerV1,
    procedure_hash: [u8; 32],
    environment_hash: [u8; 32],
    toolchain_hash: [u8; 32],
    observed_at: u64,
    recorded_at: u64,
    clock_basis_hash: [u8; 32],
    lineage: Vec<ObservationRecordIdV1>,
    payload: EvidencePayloadManifestV1,
    acquisition: ObservationAcquisitionV1,
    publication_route: ObservationPublicationRouteV1,
    record_hash: [u8; 32],
}

impl ObservationV1 {
    pub fn new(draft: ObservationDraftV1) -> Result<Self, ObservationError> {
        let ObservationDraftV1 {
            kind,
            store_domain_id,
            mut subjects,
            producer,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at,
            recorded_at,
            clock_basis_hash,
            mut lineage,
            payload,
            acquisition,
            publication_route,
        } = draft;
        require_nonzero(*store_domain_id.as_bytes(), "Observation Store Domain")?;
        require_nonzero(
            *producer.principal_id().as_bytes(),
            "Observation producer Principal",
        )?;
        if let Some(session_id) = producer.session_id() {
            require_nonzero(*session_id.as_bytes(), "Observation producer Session")?;
        }
        for (label, value) in [
            ("Observation procedure", procedure_hash),
            ("Observation environment", environment_hash),
            ("Observation toolchain", toolchain_hash),
            ("Observation clock basis", clock_basis_hash),
        ] {
            require_nonzero(value, label)?;
        }
        if observed_at == 0 || recorded_at < observed_at {
            return Err(ObservationError::InvalidTimeBasis);
        }
        if publication_route.producer_action_tag != kind.producer_action_tag() {
            return Err(ObservationError::InvalidPublicationRoute);
        }
        subjects.sort_unstable();
        if subjects.is_empty()
            || subjects.len() > MAX_OBSERVATION_SUBJECTS_V1
            || subjects.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(ObservationError::InvalidSubjects);
        }
        lineage.sort_unstable();
        if lineage.len() > MAX_OBSERVATION_LINEAGE_V1
            || lineage.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(ObservationError::InvalidLineage);
        }
        if let ObservationAcquisitionV1::DeclaredDerivation { sources } = &acquisition
            && sources != &lineage
        {
            return Err(ObservationError::DerivationLineageMismatch);
        }
        let kind_contract = kind.contract()?;
        if payload.schema_id() != kind_contract.payload_schema_id()
            || !kind_contract.admits_subjects(&subjects)
            || !kind_contract.admits_acquisition(&acquisition)
            || payload.secret_scan_receipt().scanner() != producer
            || payload.secret_scan_receipt().scanned_at() < observed_at
            || payload.secret_scan_receipt().scanned_at() > recorded_at
            || payload.retention_policy().minimum_retain_until() < recorded_at
        {
            return Err(ObservationError::KindSemanticMismatch);
        }
        let identity_value = observation_identity_value(&ObservationIdentityMaterial {
            kind,
            kind_contract_hash: *kind_contract.contract_hash(),
            store_domain_id,
            subjects: &subjects,
            producer,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at,
            recorded_at,
            clock_basis_hash,
            lineage: &lineage,
            payload: &payload,
            acquisition: &acquisition,
            publication_route,
        })?;
        let id = ObservationRecordIdV1::from_bytes(domain_hash(
            "maestro.vnext.evidence.observation-id.v1",
            &identity_value,
        )?)?;
        let record_value = CborValue::Array(vec![
            CborValue::Unsigned(OBSERVATION_RECORD_VERSION_V1),
            bytes(id.as_bytes()),
            identity_value,
        ]);
        let record_hash = domain_hash(OBSERVATION_RECORD_DOMAIN_V1, &record_value)?;
        Ok(Self {
            id,
            kind,
            store_domain_id,
            subjects,
            producer,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            observed_at,
            recorded_at,
            clock_basis_hash,
            lineage,
            payload,
            acquisition,
            publication_route,
            record_hash,
        })
    }

    pub const fn id(&self) -> ObservationRecordIdV1 {
        self.id
    }

    pub const fn kind(&self) -> ObservationKindV1 {
        self.kind
    }

    pub const fn store_domain_id(&self) -> StoreDomainIdV1 {
        self.store_domain_id
    }

    pub const fn producer(&self) -> ExecutionProducerV1 {
        self.producer
    }

    pub const fn producer_principal_id(&self) -> PrincipalIdV1 {
        self.producer.principal_id()
    }

    pub const fn producer_session_id(&self) -> Option<SessionIdV1> {
        self.producer.session_id()
    }

    pub fn subjects(&self) -> &[ObservationSubjectV1] {
        &self.subjects
    }

    pub const fn observed_at(&self) -> u64 {
        self.observed_at
    }

    pub const fn recorded_at(&self) -> u64 {
        self.recorded_at
    }

    pub fn lineage(&self) -> &[ObservationRecordIdV1] {
        &self.lineage
    }

    pub const fn payload(&self) -> &EvidencePayloadManifestV1 {
        &self.payload
    }

    pub fn acquisition(&self) -> &ObservationAcquisitionV1 {
        &self.acquisition
    }

    pub const fn acquisition_id(&self) -> Option<&[u8; 32]> {
        self.acquisition.acquisition_id()
    }

    pub const fn record_hash(&self) -> &[u8; 32] {
        &self.record_hash
    }

    pub const fn publication_route(&self) -> ObservationPublicationRouteV1 {
        self.publication_route
    }

    pub fn canonical_value(&self) -> Result<CborValue, ObservationError> {
        Ok(CborValue::Array(vec![
            CborValue::Unsigned(OBSERVATION_RECORD_VERSION_V1),
            bytes(self.id.as_bytes()),
            observation_identity_value(&ObservationIdentityMaterial {
                kind: self.kind,
                kind_contract_hash: *self.kind.contract()?.contract_hash(),
                store_domain_id: self.store_domain_id,
                subjects: &self.subjects,
                producer: self.producer,
                procedure_hash: self.procedure_hash,
                environment_hash: self.environment_hash,
                toolchain_hash: self.toolchain_hash,
                observed_at: self.observed_at,
                recorded_at: self.recorded_at,
                clock_basis_hash: self.clock_basis_hash,
                lineage: &self.lineage,
                payload: &self.payload,
                acquisition: &self.acquisition,
                publication_route: self.publication_route,
            })?,
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ObservationError> {
        Ok(deterministic_cbor::encode(&self.canonical_value()?)?)
    }

    pub(crate) fn from_canonical_bytes(value: &[u8]) -> Result<Self, ObservationError> {
        let decoded = deterministic_cbor::decode(value)?;
        let observation = Self::from_canonical_value(&decoded)?;
        if observation.canonical_bytes()? != value {
            return Err(ObservationError::InvalidStoredObservation);
        }
        Ok(observation)
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, ObservationError> {
        let CborValue::Array(record) = value else {
            return Err(ObservationError::InvalidStoredObservation);
        };
        let [
            CborValue::Unsigned(OBSERVATION_RECORD_VERSION_V1),
            id,
            CborValue::Array(material),
        ] = record.as_slice()
        else {
            return Err(ObservationError::InvalidStoredObservation);
        };
        let [
            CborValue::Unsigned(kind_tag),
            kind_contract_hash,
            store_domain_id,
            CborValue::Array(subjects),
            producer,
            procedure_hash,
            environment_hash,
            toolchain_hash,
            CborValue::Unsigned(observed_at),
            CborValue::Unsigned(recorded_at),
            clock_basis_hash,
            CborValue::Array(lineage),
            payload,
            acquisition,
            publication_route,
        ] = material.as_slice()
        else {
            return Err(ObservationError::InvalidStoredObservation);
        };
        let kind = ObservationKindV1::from_tag(*kind_tag)?;
        if exact_observation_digest(kind_contract_hash)? != *kind.contract()?.contract_hash() {
            return Err(ObservationError::InvalidStoredObservation);
        }
        let payload = parse_payload_manifest(kind, payload)?;
        let observation = Self::new(ObservationDraftV1 {
            kind,
            store_domain_id: StoreDomainIdV1::from_digest(exact_observation_digest(
                store_domain_id,
            )?),
            subjects: subjects
                .iter()
                .map(parse_observation_subject)
                .collect::<Result<Vec<_>, _>>()?,
            producer: parse_observation_producer(producer)?,
            procedure_hash: exact_observation_digest(procedure_hash)?,
            environment_hash: exact_observation_digest(environment_hash)?,
            toolchain_hash: exact_observation_digest(toolchain_hash)?,
            observed_at: *observed_at,
            recorded_at: *recorded_at,
            clock_basis_hash: exact_observation_digest(clock_basis_hash)?,
            lineage: lineage
                .iter()
                .map(|source| {
                    ObservationRecordIdV1::from_bytes(exact_observation_digest(source)?)
                        .map_err(ObservationError::from)
                })
                .collect::<Result<Vec<_>, _>>()?,
            payload,
            acquisition: parse_observation_acquisition(acquisition)?,
            publication_route: parse_publication_route(kind, publication_route)?,
        })?;
        if observation.id != ObservationRecordIdV1::from_bytes(exact_observation_digest(id)?)?
            || observation.canonical_value()? != *value
        {
            return Err(ObservationError::InvalidStoredObservation);
        }
        Ok(observation)
    }
}

fn parse_observation_subject(value: &CborValue) -> Result<ObservationSubjectV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    match fields.as_slice() {
        [
            CborValue::Unsigned(1),
            work_id,
            contract_generation_id,
            contract_root_id,
        ] => ObservationSubjectV1::for_work(
            exact_observation_digest(work_id)?,
            ContractGenerationIdV1::parse(&format!(
                "sha256:{}",
                exact_observation_digest(contract_generation_id)?
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ))
            .map_err(|_| ObservationError::InvalidStoredObservation)?,
            exact_observation_digest(contract_root_id)?,
        ),
        [CborValue::Unsigned(kind), subject_id, revision_id] => {
            let kind = ObservationSubjectKindV1::from_tag(*kind)?;
            if kind == ObservationSubjectKindV1::Work {
                return Err(ObservationError::InvalidStoredObservation);
            }
            ObservationSubjectV1::new(
                kind,
                exact_observation_digest(subject_id)?,
                exact_observation_digest(revision_id)?,
            )
        }
        _ => Err(ObservationError::InvalidStoredObservation),
    }
}

fn parse_observation_producer(value: &CborValue) -> Result<ExecutionProducerV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(1), principal, session] => Ok(ExecutionProducerV1::SessionBound {
            principal_id: PrincipalIdV1::from_digest(exact_observation_digest(principal)?),
            session_id: SessionIdV1::from_digest(exact_observation_digest(session)?),
        }),
        [
            CborValue::Unsigned(2),
            principal,
            branch,
            slot,
            assertion,
            CborValue::Unsigned(purpose),
            state_token,
            CborValue::Unsigned(authority_epoch),
        ] => Ok(ExecutionProducerV1::ContinuityMaintenance {
            principal_id: PrincipalIdV1::from_digest(exact_observation_digest(principal)?),
            basis: ContinuityMaintenanceAuthorityBasisV1::new(
                CmaBranchIdV1::from_digest(exact_observation_digest(branch)?),
                SlotIdV1::from_digest(exact_observation_digest(slot)?),
                ExecutorAssertionIdV1::from_digest(exact_observation_digest(assertion)?),
            ),
            purpose: CmaObservationPublicationPurposeV1::try_from(
                u8::try_from(*purpose).map_err(|_| ObservationError::InvalidStoredObservation)?,
            )
            .map_err(|_| ObservationError::InvalidStoredObservation)?,
            continuity_state_token: StateTokenIdV1::from_digest(exact_observation_digest(
                state_token,
            )?),
            authority_epoch: *authority_epoch,
        }),
        _ => Err(ObservationError::InvalidStoredObservation),
    }
}

fn parse_payload_manifest(
    kind: ObservationKindV1,
    value: &CborValue,
) -> Result<EvidencePayloadManifestV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    let [
        object_id,
        schema_id,
        CborValue::Unsigned(byte_length),
        semantic_hash,
        CborValue::Text(media_type),
        redaction_policy,
        secret_scan_receipt,
        retention_policy,
    ] = fields.as_slice()
    else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    let redaction_policy = parse_redaction_policy(redaction_policy)?;
    let secret_scan_receipt = parse_secret_scan_receipt(secret_scan_receipt, redaction_policy)?;
    let retention_policy = parse_retention_policy(retention_policy)?;
    let manifest = EvidencePayloadManifestV1::from_parts(
        kind,
        StoreObjectIdV1::from_digest(exact_observation_digest(object_id)?),
        *byte_length,
        exact_observation_digest(semantic_hash)?,
        media_type,
        redaction_policy,
        secret_scan_receipt,
        retention_policy,
    )?;
    if manifest.schema_id() != SchemaIdV1::from_digest(exact_observation_digest(schema_id)?) {
        return Err(ObservationError::InvalidStoredObservation);
    }
    Ok(manifest)
}

fn parse_redaction_policy(
    value: &CborValue,
) -> Result<EvidenceRedactionPolicyV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    let [
        CborValue::Unsigned(1),
        ruleset,
        CborValue::Unsigned(maximum_payload_bytes),
        CborValue::Unsigned(1),
    ] = fields.as_slice()
    else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    EvidenceRedactionPolicyV1::prohibit_secrets(
        exact_observation_digest(ruleset)?,
        *maximum_payload_bytes,
    )
}

fn parse_secret_scan_receipt(
    value: &CborValue,
    redaction_policy: EvidenceRedactionPolicyV1,
) -> Result<EvidenceSecretScanReceiptV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    let [
        CborValue::Unsigned(1),
        object_id,
        semantic_hash,
        CborValue::Unsigned(byte_length),
        redaction_policy_id,
        scanner,
        scanner_tool_hash,
        CborValue::Unsigned(scanned_at),
        CborValue::Unsigned(secret_finding_count),
    ] = fields.as_slice()
    else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    if exact_observation_digest(redaction_policy_id)? != redaction_policy.identity()? {
        return Err(ObservationError::InvalidSecretScanReceipt);
    }
    EvidenceSecretScanReceiptV1::from_parts(
        StoreObjectIdV1::from_digest(exact_observation_digest(object_id)?),
        exact_observation_digest(semantic_hash)?,
        *byte_length,
        redaction_policy,
        parse_observation_producer(scanner)?,
        exact_observation_digest(scanner_tool_hash)?,
        *scanned_at,
        *secret_finding_count,
    )
}

fn parse_retention_policy(
    value: &CborValue,
) -> Result<EvidenceRetentionPolicyV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    let [
        CborValue::Unsigned(1),
        CborValue::Unsigned(class),
        CborValue::Unsigned(minimum_retain_until),
    ] = fields.as_slice()
    else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    EvidenceRetentionPolicyV1::new(
        EvidenceRetentionClassV1::from_tag(*class)?,
        *minimum_retain_until,
    )
}

fn parse_observation_acquisition(
    value: &CborValue,
) -> Result<ObservationAcquisitionV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    match fields.as_slice() {
        [
            CborValue::Unsigned(1),
            acquisition_id,
            transport_contract_hash,
        ] => ObservationAcquisitionV1::effect_free(
            exact_observation_digest(acquisition_id)?,
            exact_observation_digest(transport_contract_hash)?,
        ),
        [CborValue::Unsigned(2), acquisition_id, run_id, owner] => {
            let acquisition_id = exact_observation_digest(acquisition_id)?;
            require_nonzero(acquisition_id, "run-mediated acquisition")?;
            Ok(ObservationAcquisitionV1::RunMediated {
                acquisition_id,
                run_id: RunIdV1::from_bytes(exact_observation_digest(run_id)?)
                    .map_err(|_| ObservationError::InvalidStoredObservation)?,
                owner: parse_attempt_owner(owner)?,
            })
        }
        [CborValue::Unsigned(3), CborValue::Array(sources)] => {
            ObservationAcquisitionV1::declared_derivation(
                sources
                    .iter()
                    .map(|source| {
                        ObservationRecordIdV1::from_bytes(exact_observation_digest(source)?)
                            .map_err(ObservationError::from)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        }
        _ => Err(ObservationError::InvalidStoredObservation),
    }
}

fn parse_attempt_owner(value: &CborValue) -> Result<ExecutionAttemptOwnerV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    let [CborValue::Unsigned(tag), id] = fields.as_slice() else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    let id = exact_observation_digest(id)?;
    match tag {
        1 => Ok(ExecutionAttemptOwnerV1::Step(
            StepAttemptIdV1::from_bytes(id)
                .map_err(|_| ObservationError::InvalidStoredObservation)?,
        )),
        2 => Ok(ExecutionAttemptOwnerV1::Dispatch(
            DispatchAttemptIdV1::from_bytes(id)
                .map_err(|_| ObservationError::InvalidStoredObservation)?,
        )),
        3 => Ok(ExecutionAttemptOwnerV1::Reconciliation(
            ReconciliationAttemptIdV1::from_bytes(id)
                .map_err(|_| ObservationError::InvalidStoredObservation)?,
        )),
        _ => Err(ObservationError::InvalidStoredObservation),
    }
}

fn parse_publication_route(
    kind: ObservationKindV1,
    value: &CborValue,
) -> Result<ObservationPublicationRouteV1, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    let [CborValue::Unsigned(action), route, cma_profile] = fields.as_slice() else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    ObservationPublicationRouteV1::new(
        kind,
        *action,
        parse_optional_observation_u64(route)?,
        parse_optional_observation_u64(cma_profile)?,
    )
}

fn parse_optional_observation_u64(value: &CborValue) -> Result<Option<u64>, ObservationError> {
    let CborValue::Array(fields) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(0)] => Ok(None),
        [CborValue::Unsigned(1), CborValue::Unsigned(value)] => Ok(Some(*value)),
        _ => Err(ObservationError::InvalidStoredObservation),
    }
}

fn exact_observation_digest(value: &CborValue) -> Result<[u8; 32], ObservationError> {
    let CborValue::Bytes(value) = value else {
        return Err(ObservationError::InvalidStoredObservation);
    };
    value
        .as_slice()
        .try_into()
        .map_err(|_| ObservationError::InvalidStoredObservation)
}

struct ObservationIdentityMaterial<'a> {
    kind: ObservationKindV1,
    kind_contract_hash: [u8; 32],
    store_domain_id: StoreDomainIdV1,
    subjects: &'a [ObservationSubjectV1],
    producer: ExecutionProducerV1,
    procedure_hash: [u8; 32],
    environment_hash: [u8; 32],
    toolchain_hash: [u8; 32],
    observed_at: u64,
    recorded_at: u64,
    clock_basis_hash: [u8; 32],
    lineage: &'a [ObservationRecordIdV1],
    payload: &'a EvidencePayloadManifestV1,
    acquisition: &'a ObservationAcquisitionV1,
    publication_route: ObservationPublicationRouteV1,
}

fn observation_identity_value(
    material: &ObservationIdentityMaterial<'_>,
) -> Result<CborValue, ObservationError> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(material.kind.tag()),
        bytes(&material.kind_contract_hash),
        bytes(material.store_domain_id.as_bytes()),
        CborValue::Array(
            material
                .subjects
                .iter()
                .copied()
                .map(ObservationSubjectV1::canonical_value)
                .collect(),
        ),
        material.producer.canonical_value(),
        bytes(&material.procedure_hash),
        bytes(&material.environment_hash),
        bytes(&material.toolchain_hash),
        CborValue::Unsigned(material.observed_at),
        CborValue::Unsigned(material.recorded_at),
        bytes(&material.clock_basis_hash),
        CborValue::Array(
            material
                .lineage
                .iter()
                .map(|id| bytes(id.as_bytes()))
                .collect(),
        ),
        material.payload.canonical_value()?,
        material.acquisition.canonical_value(),
        material.publication_route.canonical_value(),
    ]))
}

fn attempt_owner_value(owner: ExecutionAttemptOwnerV1) -> CborValue {
    match owner {
        ExecutionAttemptOwnerV1::Step(id) => {
            CborValue::Array(vec![CborValue::Unsigned(1), bytes(id.as_bytes())])
        }
        ExecutionAttemptOwnerV1::Dispatch(id) => {
            CborValue::Array(vec![CborValue::Unsigned(2), bytes(id.as_bytes())])
        }
        ExecutionAttemptOwnerV1::Reconciliation(id) => {
            CborValue::Array(vec![CborValue::Unsigned(3), bytes(id.as_bytes())])
        }
    }
}

fn bytes(value: &[u8; 32]) -> CborValue {
    CborValue::Bytes(value.to_vec())
}

const fn subject_mask(kind: ObservationSubjectKindV1) -> u16 {
    1 << (kind.tag() - 1)
}

const fn required_subject_mask(kind: ObservationKindV1) -> u16 {
    use ObservationKindV1 as K;
    use ObservationSubjectKindV1 as S;
    match kind {
        K::DeterministicProcedure
        | K::NondeterministicProcedure
        | K::ManualProcedure
        | K::VisualCapture
        | K::ReviewFinding
        | K::SecurityAnalysis => 0,
        K::HookDelivery | K::EventStreamIntegrity | K::CapabilityProbe | K::StoreIntegrity => {
            subject_mask(S::Repository)
        }
        K::InstalledBinary
        | K::SkillActivation
        | K::DistributionFilesystemState
        | K::DistributionInstalledResource
        | K::DistributionManagerState => subject_mask(S::Installation),
        K::RemoteReadback | K::RemoteDelivery | K::RemoteRelease => subject_mask(S::External),
        K::ProcessLiveness => subject_mask(S::Run),
        K::ResearchSource => subject_mask(S::Work),
        K::BootstrapMandatePresentation
        | K::BootstrapMandateResponse
        | K::GovernedReviewPresentation
        | K::GovernedReviewResponse
        | K::HumanBindingSubjectSelection
        | K::HumanPresence
        | K::CredentialControl
        | K::HumanAssurance
        | K::SourceAssurance
        | K::SourceCommonCause
        | K::SourceContradiction
        | K::SourceRevocation
        | K::TrustedTimeAtomicUnit
        | K::TrustedTimeAcquisition
        | K::MaintenanceExecutorCurrentness
        | K::ProspectiveContinuityCarrier
        | K::RecoveryExternalAnchorRevision
        | K::RecoveryExternalRegistration
        | K::RecoveryExternalStatus
        | K::ExternalHighWaterReadback
        | K::CarrierIntegrityFault
        | K::ProtectedContinuityDiagnostic => subject_mask(S::Authority),
        K::PublicationIntegrity => subject_mask(S::Resource),
    }
}

const fn allowed_subject_mask(kind: ObservationKindV1) -> u16 {
    use ObservationKindV1 as K;
    use ObservationSubjectKindV1 as S;
    match kind {
        K::InstalledBinary
        | K::SkillActivation
        | K::DistributionFilesystemState
        | K::DistributionInstalledResource
        | K::DistributionManagerState => {
            subject_mask(S::Installation) | subject_mask(S::Repository) | subject_mask(S::Resource)
        }
        K::RemoteReadback | K::RemoteDelivery | K::RemoteRelease => {
            subject_mask(S::External)
                | subject_mask(S::Work)
                | subject_mask(S::Step)
                | subject_mask(S::Run)
                | subject_mask(S::Repository)
                | subject_mask(S::Installation)
                | subject_mask(S::Resource)
        }
        K::ProcessLiveness => {
            subject_mask(S::Run) | subject_mask(S::Repository) | subject_mask(S::Installation)
        }
        K::ResearchSource => {
            subject_mask(S::Work)
                | subject_mask(S::Step)
                | subject_mask(S::Resource)
                | subject_mask(S::External)
        }
        K::BootstrapMandatePresentation
        | K::BootstrapMandateResponse
        | K::GovernedReviewPresentation
        | K::GovernedReviewResponse
        | K::HumanBindingSubjectSelection
        | K::HumanPresence
        | K::CredentialControl
        | K::HumanAssurance
        | K::SourceAssurance
        | K::SourceCommonCause
        | K::SourceContradiction
        | K::SourceRevocation
        | K::TrustedTimeAtomicUnit
        | K::TrustedTimeAcquisition
        | K::MaintenanceExecutorCurrentness
        | K::ProspectiveContinuityCarrier
        | K::RecoveryExternalAnchorRevision
        | K::RecoveryExternalRegistration
        | K::RecoveryExternalStatus
        | K::ExternalHighWaterReadback
        | K::CarrierIntegrityFault
        | K::ProtectedContinuityDiagnostic => {
            subject_mask(S::Authority)
                | subject_mask(S::Work)
                | subject_mask(S::Repository)
                | subject_mask(S::Installation)
                | subject_mask(S::External)
                | subject_mask(S::Resource)
        }
        K::PublicationIntegrity => {
            subject_mask(S::Resource) | subject_mask(S::Repository) | subject_mask(S::Installation)
        }
        _ => {
            subject_mask(S::Repository)
                | subject_mask(S::Work)
                | subject_mask(S::Step)
                | subject_mask(S::Submission)
                | subject_mask(S::Run)
                | subject_mask(S::Resource)
                | subject_mask(S::External)
        }
    }
}

const fn allowed_acquisition_mask(kind: ObservationKindV1) -> u8 {
    use ObservationKindV1 as K;
    match kind {
        K::ManualProcedure
        | K::InstalledBinary
        | K::SkillActivation
        | K::BootstrapMandatePresentation
        | K::BootstrapMandateResponse
        | K::GovernedReviewPresentation
        | K::GovernedReviewResponse
        | K::TrustedTimeAtomicUnit
        | K::TrustedTimeAcquisition
        | K::MaintenanceExecutorCurrentness
        | K::ProspectiveContinuityCarrier
        | K::RecoveryExternalAnchorRevision
        | K::RecoveryExternalRegistration
        | K::RecoveryExternalStatus
        | K::ExternalHighWaterReadback
        | K::CarrierIntegrityFault => 1,
        K::ProcessLiveness => 2,
        K::EventStreamIntegrity
        | K::SourceAssurance
        | K::SourceCommonCause
        | K::SourceContradiction
        | K::SourceRevocation
        | K::PublicationIntegrity
        | K::StoreIntegrity
        | K::ProtectedContinuityDiagnostic => 4,
        K::DeterministicProcedure
        | K::ReviewFinding
        | K::SecurityAnalysis
        | K::HumanBindingSubjectSelection
        | K::HumanPresence
        | K::CredentialControl
        | K::HumanAssurance => 1 | 4,
        _ => 1 | 2,
    }
}

const fn expected_payload_detail_tag(kind: ObservationKindV1) -> u64 {
    match kind {
        ObservationKindV1::DeterministicProcedure => 1,
        ObservationKindV1::NondeterministicProcedure => 2,
        ObservationKindV1::ManualProcedure => 3,
        ObservationKindV1::VisualCapture => 4,
        ObservationKindV1::ReviewFinding => 5,
        ObservationKindV1::SecurityAnalysis => 6,
        ObservationKindV1::InstalledBinary => 7,
        ObservationKindV1::RemoteDelivery | ObservationKindV1::RemoteRelease => 8,
        _ => 9,
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ObservationError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] EvidenceIdentityError),
    #[error(transparent)]
    CoreIdentity(#[from] IdentityError),
    #[error("unknown ObservationKindV1 tag {0}")]
    UnknownKind(u64),
    #[error("Evidence payload manifest is incomplete or invalid")]
    InvalidPayloadManifest,
    #[error("Evidence secret scan receipt is missing, stale, unauthenticated, or reports secrets")]
    InvalidSecretScanReceipt,
    #[error("Evidence payload is not the exact closed typed schema for its Observation kind")]
    InvalidPayloadSemantics,
    #[error("Observation derivation sources must be nonempty, unique, and bounded")]
    InvalidDerivationSources,
    #[error("Observation time basis is invalid")]
    InvalidTimeBasis,
    #[error("Observation subjects must be nonempty, unique, and bounded")]
    InvalidSubjects,
    #[error("Observation lineage must be unique and bounded")]
    InvalidLineage,
    #[error("Observation publication route does not match the frozen 43-kind catalog")]
    InvalidPublicationRoute,
    #[error("declared derivation sources must equal the Observation lineage")]
    DerivationLineageMismatch,
    #[error("Observation subjects, payload schema, or acquisition violate its kind contract")]
    KindSemanticMismatch,
    #[error("stored Observation bytes are malformed, non-canonical, or identity-inconsistent")]
    InvalidStoredObservation,
}
