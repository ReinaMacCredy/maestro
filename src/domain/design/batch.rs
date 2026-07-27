use std::collections::BTreeSet;

use crate::domain::identity::DecisionResolutionIdV1;
use crate::foundation::core::deterministic_cbor::{self, CborValue};

use super::{
    AuthorityRequirementRefV1, AuthorizationReceiptRefV1, DecisionBatchReceiptIdV1,
    DecisionV1Error, ResolutionV1, canonical_hash_v1,
};

const DECISION_BATCH_RECEIPT_VERSION_V1: u64 = 1;
const MAX_BATCH_RESOLUTIONS_V1: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionBatchReceiptV1 {
    resolution_ids: Vec<DecisionResolutionIdV1>,
    authority_requirement: AuthorityRequirementRefV1,
    authorization_receipt: AuthorizationReceiptRefV1,
    receipt_id: DecisionBatchReceiptIdV1,
}

impl DecisionBatchReceiptV1 {
    pub fn new(
        resolutions: Vec<ResolutionV1>,
        authority_requirement: AuthorityRequirementRefV1,
        authorization_receipt: AuthorizationReceiptRefV1,
    ) -> Result<Self, DecisionV1Error> {
        if !(2..=MAX_BATCH_RESOLUTIONS_V1).contains(&resolutions.len()) {
            return Err(DecisionV1Error::BatchCardinality);
        }
        let decision_ids: BTreeSet<_> = resolutions.iter().map(ResolutionV1::decision_id).collect();
        let mut resolution_ids: Vec<_> = resolutions
            .iter()
            .map(|resolution| *resolution.resolution_id())
            .collect();
        resolution_ids.sort_unstable();
        if decision_ids.len() != resolutions.len()
            || resolution_ids.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(DecisionV1Error::DuplicateBatchResolution);
        }
        let value = batch_value(
            &resolution_ids,
            authority_requirement,
            authorization_receipt,
        );
        let receipt_id = DecisionBatchReceiptIdV1::from_digest(canonical_hash_v1(
            "maestro.vnext.decision-batch-receipt.v1",
            value,
        )?);
        Ok(Self {
            resolution_ids,
            authority_requirement,
            authorization_receipt,
            receipt_id,
        })
    }

    pub fn resolution_ids(&self) -> &[DecisionResolutionIdV1] {
        &self.resolution_ids
    }

    pub const fn receipt_id(&self) -> &DecisionBatchReceiptIdV1 {
        &self.receipt_id
    }

    pub const fn authority_requirement(&self) -> &AuthorityRequirementRefV1 {
        &self.authority_requirement
    }

    pub const fn authorization_receipt(&self) -> &AuthorizationReceiptRefV1 {
        &self.authorization_receipt
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DecisionV1Error> {
        Ok(deterministic_cbor::encode(&batch_value(
            &self.resolution_ids,
            self.authority_requirement,
            self.authorization_receipt,
        ))?)
    }
}

fn batch_value(
    resolution_ids: &[DecisionResolutionIdV1],
    authority_requirement: AuthorityRequirementRefV1,
    authorization_receipt: AuthorizationReceiptRefV1,
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(DECISION_BATCH_RECEIPT_VERSION_V1),
        CborValue::Array(
            resolution_ids
                .iter()
                .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
                .collect(),
        ),
        authority_requirement.canonical_value(),
        authorization_receipt.canonical_value(),
    ])
}
