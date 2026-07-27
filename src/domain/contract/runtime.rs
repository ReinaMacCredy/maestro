use std::fmt;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::authority::{
    ActionOutcomeV1, ActionRequestIdV1, ActionResultV1, AuthorityIdentityError,
    AuthorizationReceiptIdV1, IdempotencyKeyIdV1,
};
use crate::domain::identity::{ContractRootIdV1, DesignFinalizationManifestIdV1};
use crate::domain::step::{StepGraphSnapshotV1, StepStateV1};
use crate::domain::work::WorkIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::finalization::DesignFinalizationManifestV1;

const CONTRACT_REVISION_DOMAIN_V1: &str = "maestro.vnext.contract-revision.v1";
const CONTRACT_GENERATION_DOMAIN_V1: &str = "maestro.vnext.contract-generation.v1";
const CONTRACT_PUBLICATION_REQUEST_DOMAIN_V1: &str =
    "maestro.vnext.contract-publication-request.v1";

macro_rules! contract_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn parse(rendered: &str) -> Result<Self, ContractRuntimeError> {
                Ok(Self(parse_identity(rendered)?))
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            pub const fn into_bytes(self) -> [u8; 32] {
                self.0
            }

            pub fn render(&self) -> String {
                render_identity(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.render())
                    .finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.render())
            }
        }
    };
}

contract_identity!(ContractRevisionIdV1);
contract_identity!(ContractGenerationIdV1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialContractStepPublicationV1 {
    graph: StepGraphSnapshotV1,
    step_states: Vec<StepStateV1>,
}

impl InitialContractStepPublicationV1 {
    pub fn new(
        work_id: WorkIdV1,
        contract_generation_id: ContractGenerationIdV1,
        contract_root_id: ContractRootIdV1,
        graph: StepGraphSnapshotV1,
    ) -> Result<Self, ContractRuntimeError> {
        if graph.scope().work_id() != work_id
            || graph.contract_generation_id() != contract_generation_id
            || graph.contract_root_id() != contract_root_id
        {
            return Err(ContractRuntimeError::CandidateStepGraphMismatch);
        }
        let step_states = graph
            .nodes()
            .iter()
            .map(|node| StepStateV1::new_open(node.binding()))
            .collect();
        Ok(Self { graph, step_states })
    }

    pub fn graph(&self) -> &StepGraphSnapshotV1 {
        &self.graph
    }

    pub fn step_states(&self) -> &[StepStateV1] {
        &self.step_states
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ContractPublicationKindV1 {
    Initial = 1,
    Amendment = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizationBindingV1 {
    manifest_id: DesignFinalizationManifestIdV1,
    candidate_root_id: ContractRootIdV1,
}

impl FinalizationBindingV1 {
    pub fn from_manifest(manifest: &DesignFinalizationManifestV1) -> Self {
        Self {
            manifest_id: *manifest.manifest_id(),
            candidate_root_id: *manifest.candidate_contract_root_id(),
        }
    }

    pub const fn manifest_id(&self) -> DesignFinalizationManifestIdV1 {
        self.manifest_id
    }

    pub const fn candidate_root_id(&self) -> ContractRootIdV1 {
        self.candidate_root_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractRevisionV1 {
    id: ContractRevisionIdV1,
    work_id: WorkIdV1,
    ordinal: u64,
    previous_revision_id: Option<ContractRevisionIdV1>,
    finalization: FinalizationBindingV1,
}

impl ContractRevisionV1 {
    pub fn new(
        work_id: WorkIdV1,
        ordinal: u64,
        previous_revision_id: Option<ContractRevisionIdV1>,
        manifest: &DesignFinalizationManifestV1,
    ) -> Result<Self, ContractRuntimeError> {
        Self::from_binding(
            work_id,
            ordinal,
            previous_revision_id,
            FinalizationBindingV1::from_manifest(manifest),
        )
    }

    fn from_binding(
        work_id: WorkIdV1,
        ordinal: u64,
        previous_revision_id: Option<ContractRevisionIdV1>,
        finalization: FinalizationBindingV1,
    ) -> Result<Self, ContractRuntimeError> {
        if ordinal == 0
            || (ordinal == 1) != previous_revision_id.is_none()
            || (ordinal > 1 && previous_revision_id.is_none())
        {
            return Err(ContractRuntimeError::InvalidRevisionLineage);
        }
        let value = revision_value(work_id, ordinal, previous_revision_id, finalization)?;
        Ok(Self {
            id: ContractRevisionIdV1(hash_value(&value)?),
            work_id,
            ordinal,
            previous_revision_id,
            finalization,
        })
    }

    pub const fn id(&self) -> ContractRevisionIdV1 {
        self.id
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    pub const fn previous_revision_id(&self) -> Option<ContractRevisionIdV1> {
        self.previous_revision_id
    }

    pub const fn finalization(&self) -> FinalizationBindingV1 {
        self.finalization
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ContractRuntimeError> {
        Ok(deterministic_cbor::encode(&revision_value(
            self.work_id,
            self.ordinal,
            self.previous_revision_id,
            self.finalization,
        )?)?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractPublicationRequestV1 {
    request_id: ActionRequestIdV1,
    semantic_hash: [u8; 32],
    kind: ContractPublicationKindV1,
    work_id: WorkIdV1,
    expected_current_generation_id: Option<ContractGenerationIdV1>,
    expected_current_root_id: Option<ContractRootIdV1>,
    candidate_revision_id: ContractRevisionIdV1,
    candidate_root_id: ContractRootIdV1,
    finalization_manifest_id: DesignFinalizationManifestIdV1,
    idempotency_key_id: IdempotencyKeyIdV1,
}

impl ContractPublicationRequestV1 {
    pub fn new(
        kind: ContractPublicationKindV1,
        work_id: WorkIdV1,
        expected_current_generation_id: Option<ContractGenerationIdV1>,
        expected_current_root_id: Option<ContractRootIdV1>,
        revision: &ContractRevisionV1,
        idempotency_key_id: IdempotencyKeyIdV1,
    ) -> Result<Self, ContractRuntimeError> {
        if revision.work_id != work_id {
            return Err(ContractRuntimeError::WorkMismatch);
        }
        match kind {
            ContractPublicationKindV1::Initial
                if expected_current_generation_id.is_some()
                    || expected_current_root_id.is_some()
                    || revision.ordinal != 1 =>
            {
                return Err(ContractRuntimeError::InvalidInitialPublicationBasis);
            }
            ContractPublicationKindV1::Amendment
                if expected_current_generation_id.is_none()
                    || expected_current_root_id.is_none()
                    || revision.ordinal <= 1 =>
            {
                return Err(ContractRuntimeError::InvalidAmendmentBasis);
            }
            _ => {}
        }
        let semantic_value = publication_meaning_value(
            kind,
            work_id,
            expected_current_generation_id,
            expected_current_root_id,
            revision.id,
            revision.finalization,
        )?;
        let semantic_hash = hash_value(&semantic_value)?;
        let request_value = publication_request_value(&semantic_value, idempotency_key_id)?;
        let request_id = ActionRequestIdV1::derive(&hexadecimal(&hash_value(&request_value)?))?;
        Ok(Self {
            request_id,
            semantic_hash,
            kind,
            work_id,
            expected_current_generation_id,
            expected_current_root_id,
            candidate_revision_id: revision.id,
            candidate_root_id: revision.finalization.candidate_root_id,
            finalization_manifest_id: revision.finalization.manifest_id,
            idempotency_key_id,
        })
    }

    pub const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub const fn semantic_hash(&self) -> &[u8; 32] {
        &self.semantic_hash
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub const fn expected_current_generation_id(&self) -> Option<ContractGenerationIdV1> {
        self.expected_current_generation_id
    }

    pub const fn expected_current_root_id(&self) -> Option<ContractRootIdV1> {
        self.expected_current_root_id
    }

    pub const fn candidate_revision_id(&self) -> ContractRevisionIdV1 {
        self.candidate_revision_id
    }

    pub const fn finalization_manifest_id(&self) -> DesignFinalizationManifestIdV1 {
        self.finalization_manifest_id
    }

    pub const fn idempotency_key_id(&self) -> IdempotencyKeyIdV1 {
        self.idempotency_key_id
    }

    pub const fn kind(&self) -> ContractPublicationKindV1 {
        self.kind
    }

    pub const fn candidate_root_id(&self) -> ContractRootIdV1 {
        self.candidate_root_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ContractRuntimeError> {
        Ok(deterministic_cbor::encode(&publication_request_value(
            &publication_meaning_value(
                self.kind,
                self.work_id,
                self.expected_current_generation_id,
                self.expected_current_root_id,
                self.candidate_revision_id,
                FinalizationBindingV1 {
                    manifest_id: self.finalization_manifest_id,
                    candidate_root_id: self.candidate_root_id,
                },
            )?,
            self.idempotency_key_id,
        )?)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContractPublicationAuthorityV1 {
    request_id: ActionRequestIdV1,
    receipt_id: AuthorizationReceiptIdV1,
    transition_guard_digest: [u8; 32],
}

impl ContractPublicationAuthorityV1 {
    pub(crate) fn from_store_commit(
        request: &ContractPublicationRequestV1,
        result: &ActionResultV1,
        transition_guard_digest: [u8; 32],
    ) -> Result<Self, ContractRuntimeError> {
        if result.outcome() != ActionOutcomeV1::Committed {
            return Err(ContractRuntimeError::AuthorizationResultNotCommitted);
        }
        let receipt = result
            .authorization_receipt()
            .ok_or(ContractRuntimeError::MissingAuthorizationReceipt)?;
        if result.request_id() != request.request_id {
            return Err(ContractRuntimeError::AuthorizationRequestMismatch);
        }
        if receipt.request_id() != request.request_id {
            return Err(ContractRuntimeError::AuthorizationRequestMismatch);
        }
        if transition_guard_digest == [0; 32] {
            return Err(ContractRuntimeError::EmptyTransitionGuard);
        }
        Ok(Self {
            request_id: request.request_id,
            receipt_id: receipt.id(),
            transition_guard_digest,
        })
    }

    pub const fn receipt_id(&self) -> AuthorizationReceiptIdV1 {
        self.receipt_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedContractPublicationV1 {
    request: ContractPublicationRequestV1,
    revision: ContractRevisionV1,
    ordinal: u64,
    previous_generation_id: Option<ContractGenerationIdV1>,
}

impl PreparedContractPublicationV1 {
    pub fn initial(
        request: ContractPublicationRequestV1,
        revision: ContractRevisionV1,
    ) -> Result<Self, ContractRuntimeError> {
        validate_request_revision(&request, &revision)?;
        if request.kind != ContractPublicationKindV1::Initial {
            return Err(ContractRuntimeError::InvalidInitialPublicationBasis);
        }
        Ok(Self {
            request,
            revision,
            ordinal: 1,
            previous_generation_id: None,
        })
    }

    pub fn amendment(
        current: &ContractGenerationV1,
        request: ContractPublicationRequestV1,
        revision: ContractRevisionV1,
    ) -> Result<ContractAmendmentPreparationV1, ContractRuntimeError> {
        validate_request_revision(&request, &revision)?;
        let next_ordinal = current
            .ordinal
            .checked_add(1)
            .ok_or(ContractRuntimeError::OrdinalOverflow)?;
        if request.kind != ContractPublicationKindV1::Amendment
            || request.work_id != current.work_id
            || request.expected_current_generation_id != Some(current.id)
            || request.expected_current_root_id != Some(current.root_id)
            || revision.ordinal != next_ordinal
            || revision.previous_revision_id != Some(current.revision_id)
        {
            return Err(ContractRuntimeError::InvalidAmendmentBasis);
        }
        if revision.finalization.candidate_root_id == current.root_id {
            return Ok(ContractAmendmentPreparationV1::NoOp(
                ContractAmendmentNoOpV1 {
                    work_id: current.work_id,
                    generation_id: current.id,
                    root_id: current.root_id,
                    request_id: request.request_id,
                },
            ));
        }
        Ok(ContractAmendmentPreparationV1::Required(Box::new(Self {
            request,
            revision,
            ordinal: next_ordinal,
            previous_generation_id: Some(current.id),
        })))
    }

    pub(crate) fn authorize(
        self,
        authority: ContractPublicationAuthorityV1,
    ) -> Result<ContractGenerationV1, ContractRuntimeError> {
        if authority.request_id != self.request.request_id {
            return Err(ContractRuntimeError::AuthorizationRequestMismatch);
        }
        ContractGenerationV1::new(
            self.revision,
            self.ordinal,
            self.previous_generation_id,
            self.request.request_id,
            authority,
        )
    }

    pub fn predicted_generation_id(&self) -> Result<ContractGenerationIdV1, ContractRuntimeError> {
        Ok(ContractGenerationIdV1(hash_value(
            &generation_identity_value(
                self.revision.work_id,
                self.ordinal,
                self.previous_generation_id,
                self.revision.id,
                self.revision.finalization,
            )?,
        )?))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractAmendmentPreparationV1 {
    NoOp(ContractAmendmentNoOpV1),
    Required(Box<PreparedContractPublicationV1>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContractAmendmentNoOpV1 {
    work_id: WorkIdV1,
    generation_id: ContractGenerationIdV1,
    root_id: ContractRootIdV1,
    request_id: ActionRequestIdV1,
}

impl ContractAmendmentNoOpV1 {
    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub const fn generation_id(&self) -> ContractGenerationIdV1 {
        self.generation_id
    }

    pub const fn root_id(&self) -> ContractRootIdV1 {
        self.root_id
    }

    pub const fn request_id(&self) -> ActionRequestIdV1 {
        self.request_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractGenerationV1 {
    id: ContractGenerationIdV1,
    work_id: WorkIdV1,
    ordinal: u64,
    previous_generation_id: Option<ContractGenerationIdV1>,
    revision_id: ContractRevisionIdV1,
    root_id: ContractRootIdV1,
    finalization_manifest_id: DesignFinalizationManifestIdV1,
    publication_request_id: ActionRequestIdV1,
    authorization_receipt_id: AuthorizationReceiptIdV1,
    transition_guard_digest: [u8; 32],
}

impl ContractGenerationV1 {
    fn new(
        revision: ContractRevisionV1,
        ordinal: u64,
        previous_generation_id: Option<ContractGenerationIdV1>,
        publication_request_id: ActionRequestIdV1,
        authority: ContractPublicationAuthorityV1,
    ) -> Result<Self, ContractRuntimeError> {
        if ordinal == 0 || (ordinal == 1) != previous_generation_id.is_none() {
            return Err(ContractRuntimeError::InvalidGenerationLineage);
        }
        let identity_value = generation_identity_value(
            revision.work_id,
            ordinal,
            previous_generation_id,
            revision.id,
            revision.finalization,
        )?;
        Ok(Self {
            id: ContractGenerationIdV1(hash_value(&identity_value)?),
            work_id: revision.work_id,
            ordinal,
            previous_generation_id,
            revision_id: revision.id,
            root_id: revision.finalization.candidate_root_id,
            finalization_manifest_id: revision.finalization.manifest_id,
            publication_request_id,
            authorization_receipt_id: authority.receipt_id,
            transition_guard_digest: authority.transition_guard_digest,
        })
    }

    pub const fn id(&self) -> ContractGenerationIdV1 {
        self.id
    }

    pub const fn work_id(&self) -> WorkIdV1 {
        self.work_id
    }

    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    pub const fn revision_id(&self) -> ContractRevisionIdV1 {
        self.revision_id
    }

    pub const fn root_id(&self) -> ContractRootIdV1 {
        self.root_id
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ContractRuntimeError> {
        Ok(deterministic_cbor::encode(&generation_value(
            self.work_id,
            self.ordinal,
            self.previous_generation_id,
            self.revision_id,
            FinalizationBindingV1 {
                manifest_id: self.finalization_manifest_id,
                candidate_root_id: self.root_id,
            },
            self.publication_request_id,
            ContractPublicationAuthorityV1 {
                request_id: self.publication_request_id,
                receipt_id: self.authorization_receipt_id,
                transition_guard_digest: self.transition_guard_digest,
            },
        )?)?)
    }

    #[cfg(test)]
    pub(crate) fn test_fixture(work_id: WorkIdV1, root_id: ContractRootIdV1, seed: u8) -> Self {
        let rendered = |value: u8| format!("sha256:{}", format!("{value:02x}").repeat(32));
        Self {
            id: ContractGenerationIdV1::parse(&rendered(seed)).expect("test fixture"),
            work_id,
            ordinal: 1,
            previous_generation_id: None,
            revision_id: ContractRevisionIdV1::parse(&rendered(seed.saturating_add(1)))
                .expect("test fixture"),
            root_id,
            finalization_manifest_id: DesignFinalizationManifestIdV1::parse(&rendered(
                seed.saturating_add(2),
            ))
            .expect("test fixture"),
            publication_request_id: ActionRequestIdV1::derive("contract-generation-test-fixture")
                .expect("test fixture"),
            authorization_receipt_id: AuthorizationReceiptIdV1::parse(&rendered(
                seed.saturating_add(3),
            ))
            .expect("test fixture"),
            transition_guard_digest: [seed.saturating_add(4); 32],
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ContractRuntimeError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    AuthorityIdentity(#[from] AuthorityIdentityError),
    #[error("Contract identity must be canonical lowercase sha256:<64hex>")]
    InvalidRenderedIdentity,
    #[error("Contract Revision ordinal and predecessor do not form one monotonic lineage")]
    InvalidRevisionLineage,
    #[error("initial Contract publication must have no current Generation or root")]
    InvalidInitialPublicationBasis,
    #[error(
        "Contract amendment must bind one exact current Generation/root and successor Revision"
    )]
    InvalidAmendmentBasis,
    #[error("Contract publication Work does not match its candidate Revision")]
    WorkMismatch,
    #[error(
        "initial Contract publication Step Graph does not match the exact Work, Generation, and root"
    )]
    CandidateStepGraphMismatch,
    #[error("Contract publication request and candidate Revision commitments differ")]
    RequestRevisionMismatch,
    #[error("Authorization Receipt does not bind the exact Contract publication request")]
    AuthorizationRequestMismatch,
    #[error("committed Contract publication Action Result lacks its Authorization Receipt")]
    MissingAuthorizationReceipt,
    #[error("Contract publication authority requires a committed Action Result")]
    AuthorizationResultNotCommitted,
    #[error("Contract publication transition guard cannot use the zero commitment")]
    EmptyTransitionGuard,
    #[error("Contract Generation ordinal and predecessor do not form one monotonic lineage")]
    InvalidGenerationLineage,
    #[error("Contract lineage ordinal exhausted unsigned-64 capacity")]
    OrdinalOverflow,
}

fn validate_request_revision(
    request: &ContractPublicationRequestV1,
    revision: &ContractRevisionV1,
) -> Result<(), ContractRuntimeError> {
    if request.work_id != revision.work_id
        || request.candidate_revision_id != revision.id
        || request.candidate_root_id != revision.finalization.candidate_root_id
        || request.finalization_manifest_id != revision.finalization.manifest_id
    {
        return Err(ContractRuntimeError::RequestRevisionMismatch);
    }
    Ok(())
}

fn revision_value(
    work_id: WorkIdV1,
    ordinal: u64,
    previous_revision_id: Option<ContractRevisionIdV1>,
    finalization: FinalizationBindingV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(CONTRACT_REVISION_DOMAIN_V1)?,
        CborValue::Bytes(work_id.as_bytes().to_vec()),
        CborValue::Unsigned(ordinal),
        optional_identity(previous_revision_id.map(|identity| identity.into_bytes())),
        CborValue::Bytes(finalization.manifest_id.as_bytes().to_vec()),
        CborValue::Bytes(finalization.candidate_root_id.as_bytes().to_vec()),
    ]))
}

fn publication_meaning_value(
    kind: ContractPublicationKindV1,
    work_id: WorkIdV1,
    expected_current_generation_id: Option<ContractGenerationIdV1>,
    expected_current_root_id: Option<ContractRootIdV1>,
    candidate_revision_id: ContractRevisionIdV1,
    finalization: FinalizationBindingV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(kind as u64),
        CborValue::Bytes(work_id.as_bytes().to_vec()),
        optional_identity(expected_current_generation_id.map(|identity| identity.into_bytes())),
        optional_identity(expected_current_root_id.map(|identity| *identity.as_bytes())),
        CborValue::Bytes(candidate_revision_id.as_bytes().to_vec()),
        CborValue::Bytes(finalization.manifest_id.as_bytes().to_vec()),
        CborValue::Bytes(finalization.candidate_root_id.as_bytes().to_vec()),
    ]))
}

fn publication_request_value(
    semantic_value: &CborValue,
    idempotency_key_id: IdempotencyKeyIdV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(CONTRACT_PUBLICATION_REQUEST_DOMAIN_V1)?,
        semantic_value.clone(),
        CborValue::Bytes(idempotency_key_id.as_bytes().to_vec()),
    ]))
}

fn generation_value(
    work_id: WorkIdV1,
    ordinal: u64,
    previous_generation_id: Option<ContractGenerationIdV1>,
    revision_id: ContractRevisionIdV1,
    finalization: FinalizationBindingV1,
    publication_request_id: ActionRequestIdV1,
    authority: ContractPublicationAuthorityV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text(CONTRACT_GENERATION_DOMAIN_V1)?,
        CborValue::Bytes(work_id.as_bytes().to_vec()),
        CborValue::Unsigned(ordinal),
        optional_identity(previous_generation_id.map(|identity| identity.into_bytes())),
        CborValue::Bytes(revision_id.as_bytes().to_vec()),
        CborValue::Bytes(finalization.manifest_id.as_bytes().to_vec()),
        CborValue::Bytes(finalization.candidate_root_id.as_bytes().to_vec()),
        CborValue::Bytes(publication_request_id.as_bytes().to_vec()),
        CborValue::Bytes(authority.receipt_id.as_bytes().to_vec()),
        CborValue::Bytes(authority.transition_guard_digest.to_vec()),
    ]))
}

fn generation_identity_value(
    work_id: WorkIdV1,
    ordinal: u64,
    previous_generation_id: Option<ContractGenerationIdV1>,
    revision_id: ContractRevisionIdV1,
    finalization: FinalizationBindingV1,
) -> Result<CborValue, CborError> {
    Ok(CborValue::Array(vec![
        CborValue::text("maestro.vnext.contract-generation-identity.v1")?,
        CborValue::Bytes(work_id.as_bytes().to_vec()),
        CborValue::Unsigned(ordinal),
        optional_identity(previous_generation_id.map(|identity| identity.into_bytes())),
        CborValue::Bytes(revision_id.as_bytes().to_vec()),
        CborValue::Bytes(finalization.manifest_id.as_bytes().to_vec()),
        CborValue::Bytes(finalization.candidate_root_id.as_bytes().to_vec()),
    ]))
}

fn optional_identity(identity: Option<[u8; 32]>) -> CborValue {
    CborValue::optional(identity.map(|identity| CborValue::Bytes(identity.to_vec())))
}

fn hash_value(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}

fn render_identity(bytes: &[u8; 32]) -> String {
    format!("sha256:{}", hexadecimal(bytes))
}

fn hexadecimal(bytes: &[u8; 32]) -> String {
    let mut rendered = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}")
            .expect("invariant: writing hexadecimal into String cannot fail");
    }
    rendered
}

fn parse_identity(rendered: &str) -> Result<[u8; 32], ContractRuntimeError> {
    let hexadecimal = rendered
        .strip_prefix("sha256:")
        .ok_or(ContractRuntimeError::InvalidRenderedIdentity)?;
    if hexadecimal.len() != 64
        || !hexadecimal
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ContractRuntimeError::InvalidRenderedIdentity);
    }
    let mut bytes = [0; 32];
    for (index, pair) in hexadecimal.as_bytes().chunks_exact(2).enumerate() {
        let high = nibble(pair[0]).ok_or(ContractRuntimeError::InvalidRenderedIdentity)?;
        let low = nibble(pair[1]).ok_or(ContractRuntimeError::InvalidRenderedIdentity)?;
        bytes[index] = (high << 4) | low;
    }
    if bytes == [0; 32] {
        return Err(ContractRuntimeError::InvalidRenderedIdentity);
    }
    Ok(bytes)
}

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity<K>(byte: u8) -> crate::domain::identity::ManifestIdentityV1<K>
    where
        K: crate::domain::identity::IdentityKindV1,
    {
        crate::domain::identity::ManifestIdentityV1::parse(&format!(
            "sha256:{}",
            format!("{byte:02x}").repeat(32)
        ))
        .expect("identity")
    }

    fn binding(byte: u8) -> FinalizationBindingV1 {
        FinalizationBindingV1 {
            manifest_id: identity(byte),
            candidate_root_id: identity(byte + 1),
        }
    }

    fn revision(
        work_id: WorkIdV1,
        ordinal: u64,
        previous: Option<ContractRevisionIdV1>,
        byte: u8,
    ) -> ContractRevisionV1 {
        ContractRevisionV1::from_binding(work_id, ordinal, previous, binding(byte))
            .expect("revision")
    }

    #[test]
    fn revisions_and_requests_bind_exact_lineage_and_are_deterministic() {
        let work_id = WorkIdV1::derive("contract-runtime").expect("Work identity");
        let first = revision(work_id, 1, None, 10);
        let duplicate = revision(work_id, 1, None, 10);
        assert_eq!(first, duplicate);
        assert_eq!(
            first.canonical_bytes().unwrap(),
            duplicate.canonical_bytes().unwrap()
        );
        assert!(matches!(
            ContractRevisionV1::from_binding(work_id, 2, None, binding(12)),
            Err(ContractRuntimeError::InvalidRevisionLineage)
        ));

        let request = ContractPublicationRequestV1::new(
            ContractPublicationKindV1::Initial,
            work_id,
            None,
            None,
            &first,
            IdempotencyKeyIdV1::derive("publish-initial").unwrap(),
        )
        .expect("initial request");
        assert_eq!(
            request.candidate_root_id(),
            first.finalization().candidate_root_id()
        );
        assert_eq!(
            request.semantic_hash(),
            &hash_value(
                &publication_meaning_value(
                    request.kind,
                    request.work_id,
                    request.expected_current_generation_id,
                    request.expected_current_root_id,
                    request.candidate_revision_id,
                    first.finalization,
                )
                .unwrap()
            )
            .unwrap()
        );
        let different_key = ContractPublicationRequestV1::new(
            ContractPublicationKindV1::Initial,
            work_id,
            None,
            None,
            &first,
            IdempotencyKeyIdV1::derive("publish-initial-other-key").unwrap(),
        )
        .expect("same meaning with another key");
        assert_eq!(request.semantic_hash(), different_key.semantic_hash());
        assert_ne!(request.request_id(), different_key.request_id());
    }

    #[test]
    fn no_op_amendment_mints_no_generation_and_requires_no_authority() {
        let work_id = WorkIdV1::derive("contract-no-op").unwrap();
        let first = revision(work_id, 1, None, 20);
        let request = ContractPublicationRequestV1::new(
            ContractPublicationKindV1::Initial,
            work_id,
            None,
            None,
            &first,
            IdempotencyKeyIdV1::derive("initial").unwrap(),
        )
        .unwrap();
        let prepared = PreparedContractPublicationV1::initial(request.clone(), first.clone())
            .expect("initial preparation");
        let authority = ContractPublicationAuthorityV1 {
            request_id: request.request_id,
            receipt_id: AuthorizationReceiptIdV1::derive("initial-receipt").unwrap(),
            transition_guard_digest: [7; 32],
        };
        let current = prepared.authorize(authority).expect("initial generation");

        let second =
            ContractRevisionV1::from_binding(work_id, 2, Some(first.id()), first.finalization())
                .unwrap();
        let amendment = ContractPublicationRequestV1::new(
            ContractPublicationKindV1::Amendment,
            work_id,
            Some(current.id()),
            Some(current.root_id()),
            &second,
            IdempotencyKeyIdV1::derive("no-op-amendment").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            PreparedContractPublicationV1::amendment(&current, amendment, second).unwrap(),
            ContractAmendmentPreparationV1::NoOp(_)
        ));
    }

    #[test]
    fn generation_identity_is_predictable_and_excludes_runtime_authority_evidence() {
        let work_id = WorkIdV1::derive("contract-predictable-generation").unwrap();
        let revision = revision(work_id, 1, None, 24);
        let request = ContractPublicationRequestV1::new(
            ContractPublicationKindV1::Initial,
            work_id,
            None,
            None,
            &revision,
            IdempotencyKeyIdV1::derive("predictable-generation").unwrap(),
        )
        .unwrap();
        let prepared = PreparedContractPublicationV1::initial(request.clone(), revision).unwrap();
        let predicted = prepared.predicted_generation_id().unwrap();
        let first = prepared
            .clone()
            .authorize(ContractPublicationAuthorityV1 {
                request_id: request.request_id,
                receipt_id: AuthorizationReceiptIdV1::derive("generation-receipt-a").unwrap(),
                transition_guard_digest: [41; 32],
            })
            .unwrap();
        let second = prepared
            .authorize(ContractPublicationAuthorityV1 {
                request_id: request.request_id,
                receipt_id: AuthorizationReceiptIdV1::derive("generation-receipt-b").unwrap(),
                transition_guard_digest: [42; 32],
            })
            .unwrap();

        assert_eq!(first.id(), predicted);
        assert_eq!(second.id(), predicted);
        assert_ne!(
            first.canonical_bytes().unwrap(),
            second.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn stale_amendment_basis_fails_before_authority_or_generation() {
        let work_id = WorkIdV1::derive("contract-stale").unwrap();
        let first = revision(work_id, 1, None, 30);
        let request = ContractPublicationRequestV1::new(
            ContractPublicationKindV1::Initial,
            work_id,
            None,
            None,
            &first,
            IdempotencyKeyIdV1::derive("initial-stale").unwrap(),
        )
        .unwrap();
        let current = PreparedContractPublicationV1::initial(request.clone(), first.clone())
            .unwrap()
            .authorize(ContractPublicationAuthorityV1 {
                request_id: request.request_id,
                receipt_id: AuthorizationReceiptIdV1::derive("receipt-stale").unwrap(),
                transition_guard_digest: [8; 32],
            })
            .unwrap();
        let second = revision(work_id, 2, Some(first.id()), 32);
        let stale_generation =
            ContractGenerationIdV1::parse(&format!("sha256:{}", "ff".repeat(32))).unwrap();
        let stale = ContractPublicationRequestV1::new(
            ContractPublicationKindV1::Amendment,
            work_id,
            Some(stale_generation),
            Some(current.root_id()),
            &second,
            IdempotencyKeyIdV1::derive("stale-amendment").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            PreparedContractPublicationV1::amendment(&current, stale, second),
            Err(ContractRuntimeError::InvalidAmendmentBasis)
        ));
    }
}
