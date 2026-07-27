use crate::domain::authority::ActionRequestIdV1;

use super::{CustodyAssessmentV1, DistributionDomainRefV1, DistributionScopedObjectRefV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CutoverPlanOwnerFactsV1<const TARGETS: usize> {
    pub(crate) domain: DistributionDomainRefV1,
    pub(crate) request_id: ActionRequestIdV1,
    pub(crate) request_or_ceremony_ref: DistributionScopedObjectRefV1,
    pub(crate) plan_ref: DistributionScopedObjectRefV1,
    pub(crate) idempotency_key_ref: DistributionScopedObjectRefV1,
    pub(crate) prior_commit_ref: Option<DistributionScopedObjectRefV1>,
    pub(crate) prior_receipt_ref: Option<DistributionScopedObjectRefV1>,
    pub(crate) target_identity_refs: [DistributionScopedObjectRefV1; TARGETS],
    pub(crate) target_custodies: [CustodyAssessmentV1; TARGETS],
}

impl<const TARGETS: usize> CutoverPlanOwnerFactsV1<TARGETS> {
    #[expect(
        clippy::too_many_arguments,
        reason = "the owner facts preserve every frozen Distribution plan reference"
    )]
    pub(crate) fn new(
        domain: DistributionDomainRefV1,
        request_id: ActionRequestIdV1,
        request_or_ceremony_ref: DistributionScopedObjectRefV1,
        plan_ref: DistributionScopedObjectRefV1,
        idempotency_key_ref: DistributionScopedObjectRefV1,
        prior_commit_ref: Option<DistributionScopedObjectRefV1>,
        prior_receipt_ref: Option<DistributionScopedObjectRefV1>,
        target_identity_refs: [DistributionScopedObjectRefV1; TARGETS],
        target_custodies: [CustodyAssessmentV1; TARGETS],
    ) -> Self {
        Self {
            domain,
            request_id,
            request_or_ceremony_ref,
            plan_ref,
            idempotency_key_ref,
            prior_commit_ref,
            prior_receipt_ref,
            target_identity_refs,
            target_custodies,
        }
    }
}
