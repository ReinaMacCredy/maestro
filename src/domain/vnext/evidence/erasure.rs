use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::authority::{
    ActionRequestIdV1, AuthorizationReceiptV1, RepositoryActionLeafV1,
};
use crate::domain::vnext::identity::{CollectionPlanIdV1, LogicalTombstoneIdV1, StoreObjectIdV1};
use crate::domain::vnext::persistence::ControlledCopyErasurePlanV1;
use crate::foundation::core::deterministic_cbor::{CborError, CborValue};

use super::assessment::{
    AssessmentError, AssessmentInvalidationReasonV1, AssessmentInvalidationV1, AssessmentV1,
    EvidenceMutationAuthorityV1,
};
use super::identity::{
    AssessmentIdV1, EvidenceIdentityError, SecurityErasureIntentIdV1, SecurityErasureReceiptIdV1,
    domain_hash, require_nonzero,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityErasureIntentV1 {
    id: SecurityErasureIntentIdV1,
    payload_object_id: StoreObjectIdV1,
    action_request_id: ActionRequestIdV1,
    action_request_hash: [u8; 32],
    authorization_receipt_id: crate::domain::vnext::authority::AuthorizationReceiptIdV1,
    authorization_receipt: AuthorizationReceiptV1,
    authorization_receipt_hash: [u8; 32],
    accepted_h_time: u64,
    evidence_cut_hash: [u8; 32],
    dependency_closure_hash: [u8; 32],
    affected_assessments: Vec<AssessmentIdV1>,
    authority_basis_object_id: StoreObjectIdV1,
    authority_epoch: u64,
    authority_epoch_commitment: [u8; 32],
    controlled_copy_plan: ControlledCopyErasurePlanV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityErasureReceiptV1 {
    id: SecurityErasureReceiptIdV1,
    intent_id: SecurityErasureIntentIdV1,
    payload_object_id: StoreObjectIdV1,
    tombstone_id: LogicalTombstoneIdV1,
    collection_plan_id: CollectionPlanIdV1,
    destroyed_payload_hash: [u8; 32],
    physical_absence_receipt_hash: [u8; 32],
    finalized_at: u64,
    dependency_closure_hash: [u8; 32],
    affected_assessments: Vec<AssessmentIdV1>,
    controlled_copy_plan_id: [u8; 32],
    controlled_copy_absence_receipt_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SecurityErasureFinalizationV1 {
    pub(crate) tombstone_id: LogicalTombstoneIdV1,
    pub(crate) collection_plan_id: CollectionPlanIdV1,
    pub(crate) destroyed_payload_hash: [u8; 32],
    pub(crate) physical_absence_receipt_hash: [u8; 32],
    pub(crate) controlled_copy_plan_id: [u8; 32],
    pub(crate) controlled_copy_absence_receipt_hash: [u8; 32],
    pub(crate) finalized_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityErasurePublicationV1 {
    intent: SecurityErasureIntentV1,
    invalidations: Vec<AssessmentInvalidationV1>,
}

impl SecurityErasurePublicationV1 {
    pub(crate) fn begin(
        payload_object_id: StoreObjectIdV1,
        authority: &EvidenceMutationAuthorityV1,
        current_cut_assessments: Vec<&AssessmentV1>,
        controlled_copy_plan: ControlledCopyErasurePlanV1,
    ) -> Result<Self, SecurityErasureError> {
        require_nonzero(*payload_object_id.as_bytes(), "erased Evidence payload")?;
        if authority.action() != RepositoryActionLeafV1::SecurityEraseEvidencePayload {
            return Err(SecurityErasureError::Assessment(
                AssessmentError::InvalidMutationAuthority,
            ));
        }
        if controlled_copy_plan.object_id() != payload_object_id {
            return Err(SecurityErasureError::InvalidControlledCopyPlan);
        }
        let mut current_cut = current_cut_assessments;
        current_cut.sort_unstable_by_key(|assessment| assessment.id());
        if current_cut
            .windows(2)
            .any(|pair| pair[0].id() == pair[1].id())
        {
            return Err(SecurityErasureError::DuplicateAssessment);
        }
        let available = current_cut
            .iter()
            .map(|assessment| assessment.id())
            .collect::<std::collections::BTreeSet<_>>();
        if current_cut.iter().any(|assessment| {
            assessment
                .dependency_assessment_ids()
                .iter()
                .any(|dependency| !available.contains(dependency))
        }) {
            return Err(SecurityErasureError::IncompleteDependencyClosure);
        }
        let mut affected = current_cut
            .iter()
            .filter(|assessment| assessment.references_payload(payload_object_id))
            .map(|assessment| assessment.id())
            .collect::<std::collections::BTreeSet<_>>();
        loop {
            let parents = current_cut
                .iter()
                .filter(|assessment| !affected.contains(&assessment.id()))
                .filter(|assessment| {
                    assessment
                        .dependency_assessment_ids()
                        .iter()
                        .any(|dependency| affected.contains(dependency))
                })
                .map(|assessment| assessment.id())
                .collect::<Vec<_>>();
            if parents.is_empty() {
                break;
            }
            affected.extend(parents);
        }
        let assessments = current_cut
            .into_iter()
            .filter(|assessment| affected.contains(&assessment.id()))
            .collect::<Vec<_>>();
        let affected_assessments = assessments
            .iter()
            .map(|assessment| assessment.id())
            .collect::<Vec<_>>();
        let dependency_closure_hash = domain_hash(
            "maestro.vnext.evidence.security-erasure-dependency-closure.v1",
            &CborValue::Array(vec![
                CborValue::Bytes(payload_object_id.as_bytes().to_vec()),
                CborValue::Array(
                    affected_assessments
                        .iter()
                        .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                        .collect(),
                ),
            ]),
        )?;
        let authorization_receipt_hash = domain_hash(
            "maestro.vnext.evidence.security-erasure-authority.v1",
            &CborValue::Bytes(authority.receipt().canonical_bytes()?),
        )?;
        let value = CborValue::Array(vec![
            CborValue::Bytes(payload_object_id.as_bytes().to_vec()),
            CborValue::Bytes(authority.request_id().as_bytes().to_vec()),
            CborValue::Bytes(authority.action_request_hash().to_vec()),
            CborValue::Bytes(authority.receipt().id().as_bytes().to_vec()),
            CborValue::Bytes(authority.receipt().canonical_bytes()?),
            CborValue::Bytes(authorization_receipt_hash.to_vec()),
            CborValue::Unsigned(authority.accepted_h_time()),
            CborValue::Bytes(authority.evidence_cut_hash().to_vec()),
            CborValue::Bytes(dependency_closure_hash.to_vec()),
            CborValue::Array(
                affected_assessments
                    .iter()
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
            CborValue::Bytes(authority.authority_basis_object_id().as_bytes().to_vec()),
            CborValue::Unsigned(authority.authority_epoch()),
            CborValue::Bytes(authority.authority_epoch_commitment().to_vec()),
            controlled_copy_plan.canonical_value(),
        ]);
        let intent = SecurityErasureIntentV1 {
            id: SecurityErasureIntentIdV1::from_bytes(domain_hash(
                "maestro.vnext.evidence.security-erasure-intent.v1",
                &value,
            )?)?,
            payload_object_id,
            action_request_id: authority.request_id(),
            action_request_hash: *authority.action_request_hash(),
            authorization_receipt_id: authority.receipt().id(),
            authorization_receipt: authority.receipt().clone(),
            authorization_receipt_hash,
            accepted_h_time: authority.accepted_h_time(),
            evidence_cut_hash: *authority.evidence_cut_hash(),
            dependency_closure_hash,
            affected_assessments,
            authority_basis_object_id: authority.authority_basis_object_id(),
            authority_epoch: authority.authority_epoch(),
            authority_epoch_commitment: authority.authority_epoch_commitment(),
            controlled_copy_plan,
        };
        let invalidations = assessments
            .into_iter()
            .map(|assessment| {
                AssessmentInvalidationV1::authorized(
                    assessment,
                    AssessmentInvalidationReasonV1::InputTombstoned,
                    dependency_closure_hash,
                    authority,
                    *authority.evidence_cut_hash(),
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            intent,
            invalidations,
        })
    }

    pub const fn intent(&self) -> &SecurityErasureIntentV1 {
        &self.intent
    }

    pub fn invalidations(&self) -> &[AssessmentInvalidationV1] {
        &self.invalidations
    }
}

impl SecurityErasureIntentV1 {
    pub const fn id(&self) -> SecurityErasureIntentIdV1 {
        self.id
    }

    pub const fn payload_object_id(&self) -> StoreObjectIdV1 {
        self.payload_object_id
    }

    pub const fn action_request_id(&self) -> ActionRequestIdV1 {
        self.action_request_id
    }

    pub const fn action_request_hash(&self) -> &[u8; 32] {
        &self.action_request_hash
    }

    pub const fn authorization_receipt(&self) -> &AuthorizationReceiptV1 {
        &self.authorization_receipt
    }

    pub const fn authorization_receipt_hash(&self) -> &[u8; 32] {
        &self.authorization_receipt_hash
    }

    pub const fn accepted_h_time(&self) -> u64 {
        self.accepted_h_time
    }

    pub const fn evidence_cut_hash(&self) -> &[u8; 32] {
        &self.evidence_cut_hash
    }

    pub const fn authority_basis_object_id(&self) -> StoreObjectIdV1 {
        self.authority_basis_object_id
    }

    pub const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    pub const fn authority_epoch_commitment(&self) -> [u8; 32] {
        self.authority_epoch_commitment
    }

    pub(crate) const fn controlled_copy_plan(&self) -> &ControlledCopyErasurePlanV1 {
        &self.controlled_copy_plan
    }

    pub fn affected_assessments(&self) -> &[AssessmentIdV1] {
        &self.affected_assessments
    }

    pub const fn dependency_closure_hash(&self) -> &[u8; 32] {
        &self.dependency_closure_hash
    }

    pub fn canonical_value(&self) -> Result<CborValue, SecurityErasureError> {
        Ok(CborValue::Array(vec![
            CborValue::Bytes(self.id.as_bytes().to_vec()),
            CborValue::Bytes(self.payload_object_id.as_bytes().to_vec()),
            CborValue::Bytes(self.action_request_id.as_bytes().to_vec()),
            CborValue::Bytes(self.action_request_hash.to_vec()),
            CborValue::Bytes(self.authorization_receipt_id.as_bytes().to_vec()),
            CborValue::Bytes(self.authorization_receipt.canonical_bytes()?),
            CborValue::Bytes(self.authorization_receipt_hash.to_vec()),
            CborValue::Unsigned(self.accepted_h_time),
            CborValue::Bytes(self.evidence_cut_hash.to_vec()),
            CborValue::Bytes(self.dependency_closure_hash.to_vec()),
            CborValue::Array(
                self.affected_assessments
                    .iter()
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
            CborValue::Bytes(self.authority_basis_object_id.as_bytes().to_vec()),
            CborValue::Unsigned(self.authority_epoch),
            CborValue::Bytes(self.authority_epoch_commitment.to_vec()),
            self.controlled_copy_plan.canonical_value(),
        ]))
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, SecurityErasureError> {
        let CborValue::Array(fields) = value else {
            return Err(SecurityErasureError::InvalidStoredErasure);
        };
        let [
            id,
            payload_object_id,
            action_request_id,
            action_request_hash,
            authorization_receipt_id,
            CborValue::Bytes(authorization_receipt),
            authorization_receipt_hash,
            CborValue::Unsigned(accepted_h_time),
            evidence_cut_hash,
            dependency_closure_hash,
            CborValue::Array(affected_assessments),
            authority_basis_object_id,
            CborValue::Unsigned(authority_epoch),
            authority_epoch_commitment,
            controlled_copy_plan,
        ] = fields.as_slice()
        else {
            return Err(SecurityErasureError::InvalidStoredErasure);
        };
        let authorization_receipt =
            AuthorizationReceiptV1::from_canonical_bytes(authorization_receipt)?;
        let affected_assessments = affected_assessments
            .iter()
            .map(|id| -> Result<AssessmentIdV1, SecurityErasureError> {
                Ok(AssessmentIdV1::from_bytes(exact_digest(id)?)?)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let intent = Self {
            id: SecurityErasureIntentIdV1::from_bytes(exact_digest(id)?)?,
            payload_object_id: StoreObjectIdV1::from_digest(exact_digest(payload_object_id)?),
            action_request_id: ActionRequestIdV1::from_digest(exact_digest(action_request_id)?),
            action_request_hash: exact_digest(action_request_hash)?,
            authorization_receipt_id:
                crate::domain::vnext::authority::AuthorizationReceiptIdV1::from_digest(
                    exact_digest(authorization_receipt_id)?,
                ),
            authorization_receipt,
            authorization_receipt_hash: exact_digest(authorization_receipt_hash)?,
            accepted_h_time: *accepted_h_time,
            evidence_cut_hash: exact_digest(evidence_cut_hash)?,
            dependency_closure_hash: exact_digest(dependency_closure_hash)?,
            affected_assessments,
            authority_basis_object_id: StoreObjectIdV1::from_digest(exact_digest(
                authority_basis_object_id,
            )?),
            authority_epoch: *authority_epoch,
            authority_epoch_commitment: exact_digest(authority_epoch_commitment)?,
            controlled_copy_plan: ControlledCopyErasurePlanV1::from_canonical_value(
                controlled_copy_plan,
            )
            .map_err(|_| SecurityErasureError::InvalidStoredErasure)?,
        };
        let identity_value = CborValue::Array(fields[1..].to_vec());
        let expected_authorization_receipt_hash = domain_hash(
            "maestro.vnext.evidence.security-erasure-authority.v1",
            &CborValue::Bytes(intent.authorization_receipt.canonical_bytes()?),
        )?;
        let expected_dependency_closure_hash = domain_hash(
            "maestro.vnext.evidence.security-erasure-dependency-closure.v1",
            &CborValue::Array(vec![
                CborValue::Bytes(intent.payload_object_id.as_bytes().to_vec()),
                CborValue::Array(
                    intent
                        .affected_assessments
                        .iter()
                        .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                        .collect(),
                ),
            ]),
        )?;
        let expected_authority_epoch_commitment: [u8; 32] =
            Sha256::digest(crate::foundation::core::deterministic_cbor::encode(
                &CborValue::Unsigned(intent.authority_epoch),
            )?)
            .into();
        if intent
            .affected_assessments
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || intent.authorization_receipt_id != intent.authorization_receipt.id()
            || intent.authorization_receipt.request_id() != intent.action_request_id
            || intent.authorization_receipt_hash != expected_authorization_receipt_hash
            || intent.dependency_closure_hash != expected_dependency_closure_hash
            || intent.authority_epoch == 0
            || expected_authority_epoch_commitment != intent.authority_epoch_commitment
            || intent.controlled_copy_plan.object_id() != intent.payload_object_id
            || intent.id
                != SecurityErasureIntentIdV1::from_bytes(domain_hash(
                    "maestro.vnext.evidence.security-erasure-intent.v1",
                    &identity_value,
                )?)?
            || intent.canonical_value()? != *value
        {
            return Err(SecurityErasureError::InvalidStoredErasure);
        }
        Ok(intent)
    }

    pub(crate) fn finalize(
        &self,
        finalization: SecurityErasureFinalizationV1,
    ) -> Result<SecurityErasureReceiptV1, SecurityErasureError> {
        let SecurityErasureFinalizationV1 {
            tombstone_id,
            collection_plan_id,
            destroyed_payload_hash,
            physical_absence_receipt_hash,
            controlled_copy_plan_id,
            controlled_copy_absence_receipt_hash,
            finalized_at,
        } = finalization;
        require_nonzero(destroyed_payload_hash, "destroyed Evidence payload hash")?;
        require_nonzero(
            physical_absence_receipt_hash,
            "Evidence physical-absence receipt",
        )?;
        require_nonzero(
            controlled_copy_absence_receipt_hash,
            "Evidence controlled-copy absence receipt",
        )?;
        if controlled_copy_plan_id != self.controlled_copy_plan.plan_id() {
            return Err(SecurityErasureError::InvalidFinalization);
        }
        if finalized_at < self.accepted_h_time {
            return Err(SecurityErasureError::InvalidFinalization);
        }
        let value = CborValue::Array(vec![
            CborValue::Bytes(self.id.as_bytes().to_vec()),
            CborValue::Bytes(self.payload_object_id.as_bytes().to_vec()),
            CborValue::Bytes(tombstone_id.as_bytes().to_vec()),
            CborValue::Bytes(collection_plan_id.as_bytes().to_vec()),
            CborValue::Bytes(destroyed_payload_hash.to_vec()),
            CborValue::Bytes(physical_absence_receipt_hash.to_vec()),
            CborValue::Bytes(controlled_copy_plan_id.to_vec()),
            CborValue::Bytes(controlled_copy_absence_receipt_hash.to_vec()),
            CborValue::Unsigned(finalized_at),
            CborValue::Bytes(self.dependency_closure_hash.to_vec()),
            CborValue::Array(
                self.affected_assessments
                    .iter()
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
        ]);
        Ok(SecurityErasureReceiptV1 {
            id: SecurityErasureReceiptIdV1::from_bytes(domain_hash(
                "maestro.vnext.evidence.security-erasure-receipt.v1",
                &value,
            )?)?,
            intent_id: self.id,
            payload_object_id: self.payload_object_id,
            tombstone_id,
            collection_plan_id,
            destroyed_payload_hash,
            physical_absence_receipt_hash,
            finalized_at,
            dependency_closure_hash: self.dependency_closure_hash,
            affected_assessments: self.affected_assessments.clone(),
            controlled_copy_plan_id,
            controlled_copy_absence_receipt_hash,
        })
    }
}

impl SecurityErasureReceiptV1 {
    pub const fn id(&self) -> SecurityErasureReceiptIdV1 {
        self.id
    }

    pub fn affected_assessments(&self) -> &[AssessmentIdV1] {
        &self.affected_assessments
    }

    pub const fn intent_id(&self) -> SecurityErasureIntentIdV1 {
        self.intent_id
    }

    pub const fn payload_object_id(&self) -> StoreObjectIdV1 {
        self.payload_object_id
    }

    pub const fn dependency_closure_hash(&self) -> &[u8; 32] {
        &self.dependency_closure_hash
    }

    pub(crate) const fn tombstone_id(&self) -> LogicalTombstoneIdV1 {
        self.tombstone_id
    }

    pub(crate) const fn collection_plan_id(&self) -> CollectionPlanIdV1 {
        self.collection_plan_id
    }

    pub(crate) const fn destroyed_payload_hash(&self) -> &[u8; 32] {
        &self.destroyed_payload_hash
    }

    pub(crate) const fn physical_absence_receipt_hash(&self) -> &[u8; 32] {
        &self.physical_absence_receipt_hash
    }

    pub(crate) const fn controlled_copy_plan_id(&self) -> [u8; 32] {
        self.controlled_copy_plan_id
    }

    pub(crate) const fn controlled_copy_absence_receipt_hash(&self) -> [u8; 32] {
        self.controlled_copy_absence_receipt_hash
    }

    pub const fn is_general_garbage_collection(&self) -> bool {
        false
    }

    pub fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.id.as_bytes().to_vec()),
            CborValue::Bytes(self.intent_id.as_bytes().to_vec()),
            CborValue::Bytes(self.payload_object_id.as_bytes().to_vec()),
            CborValue::Bytes(self.tombstone_id.as_bytes().to_vec()),
            CborValue::Bytes(self.collection_plan_id.as_bytes().to_vec()),
            CborValue::Bytes(self.destroyed_payload_hash.to_vec()),
            CborValue::Bytes(self.physical_absence_receipt_hash.to_vec()),
            CborValue::Bytes(self.controlled_copy_plan_id.to_vec()),
            CborValue::Bytes(self.controlled_copy_absence_receipt_hash.to_vec()),
            CborValue::Unsigned(self.finalized_at),
            CborValue::Bytes(self.dependency_closure_hash.to_vec()),
            CborValue::Array(
                self.affected_assessments
                    .iter()
                    .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                    .collect(),
            ),
        ])
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, SecurityErasureError> {
        let CborValue::Array(fields) = value else {
            return Err(SecurityErasureError::InvalidStoredErasure);
        };
        let [
            id,
            intent_id,
            payload_object_id,
            tombstone_id,
            collection_plan_id,
            destroyed_payload_hash,
            physical_absence_receipt_hash,
            controlled_copy_plan_id,
            controlled_copy_absence_receipt_hash,
            CborValue::Unsigned(finalized_at),
            dependency_closure_hash,
            CborValue::Array(affected_assessments),
        ] = fields.as_slice()
        else {
            return Err(SecurityErasureError::InvalidStoredErasure);
        };
        let affected_assessments = affected_assessments
            .iter()
            .map(|id| -> Result<AssessmentIdV1, SecurityErasureError> {
                Ok(AssessmentIdV1::from_bytes(exact_digest(id)?)?)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let receipt = Self {
            id: SecurityErasureReceiptIdV1::from_bytes(exact_digest(id)?)?,
            intent_id: SecurityErasureIntentIdV1::from_bytes(exact_digest(intent_id)?)?,
            payload_object_id: StoreObjectIdV1::from_digest(exact_digest(payload_object_id)?),
            tombstone_id: LogicalTombstoneIdV1::from_digest(exact_digest(tombstone_id)?),
            collection_plan_id: CollectionPlanIdV1::from_digest(exact_digest(collection_plan_id)?),
            destroyed_payload_hash: exact_digest(destroyed_payload_hash)?,
            physical_absence_receipt_hash: exact_digest(physical_absence_receipt_hash)?,
            controlled_copy_plan_id: exact_digest(controlled_copy_plan_id)?,
            controlled_copy_absence_receipt_hash: exact_digest(
                controlled_copy_absence_receipt_hash,
            )?,
            finalized_at: *finalized_at,
            dependency_closure_hash: exact_digest(dependency_closure_hash)?,
            affected_assessments,
        };
        let identity_value = CborValue::Array(fields[1..].to_vec());
        if receipt
            .affected_assessments
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || receipt.id
                != SecurityErasureReceiptIdV1::from_bytes(domain_hash(
                    "maestro.vnext.evidence.security-erasure-receipt.v1",
                    &identity_value,
                )?)?
            || receipt.canonical_value() != *value
        {
            return Err(SecurityErasureError::InvalidStoredErasure);
        }
        Ok(receipt)
    }
}

fn exact_digest(value: &CborValue) -> Result<[u8; 32], SecurityErasureError> {
    let CborValue::Bytes(value) = value else {
        return Err(SecurityErasureError::InvalidStoredErasure);
    };
    value
        .as_slice()
        .try_into()
        .map_err(|_| SecurityErasureError::InvalidStoredErasure)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SecurityErasureError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] EvidenceIdentityError),
    #[error(transparent)]
    Assessment(#[from] AssessmentError),
    #[error(transparent)]
    ActionResult(#[from] crate::domain::vnext::authority::ActionResultError),
    #[error("Security erasure repeats an affected Assessment")]
    DuplicateAssessment,
    #[error("Security erasure has an incomplete Assessment dependency closure")]
    IncompleteDependencyClosure,
    #[error("Security erasure finalization lacks a coherent physical-destruction proof")]
    InvalidFinalization,
    #[error("Security erasure does not bind the exact controlled-copy census")]
    InvalidControlledCopyPlan,
    #[error("stored security-erasure Intent or Receipt is malformed or non-canonical")]
    InvalidStoredErasure,
}
