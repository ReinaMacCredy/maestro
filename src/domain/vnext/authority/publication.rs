use thiserror::Error;

use crate::domain::vnext::identity::{
    ContractRootIdV1, SchemaIdV1, StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1,
};
use crate::domain::vnext::persistence::{StoreHeadV1, StoreObjectV1};
use crate::foundation::core::deterministic_cbor::CborValue;

use super::{
    ActionRequestIdV1, ActionResultIdV1, GenesisGrantIdV1, GrantIdV1, IdempotencyKeyIdV1,
    IssueBootstrapMandateRequestV1, OrdinaryBoundedGrantV1, OrdinaryGrantDelegationV1,
    PrincipalBindingIdV1, ResponseOriginV1, SessionIdV1,
};

pub const ISSUE_BOOTSTRAP_MANDATE_IDEMPOTENCY_NAMESPACE_V1: &str =
    "authority.issue-bootstrap-mandate.v1";
pub const ISSUE_ROOT_ATTACHED_BOUNDED_GRANT_IDEMPOTENCY_NAMESPACE_V1: &str =
    "authority.issue-root-attached-bounded-grant.v1";
pub const REISSUE_ROOT_ATTACHED_GRANT_ONE_TO_ONE_IDEMPOTENCY_NAMESPACE_V1: &str =
    "authority.reissue-root-attached-grant-one-to-one.v1";
pub const REVOKE_GRANT_IDEMPOTENCY_NAMESPACE_V1: &str = "authority.revoke-grant.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorityPublicationLineageV1 {
    contract_root_id: ContractRootIdV1,
    previous_generation_id: Option<StoreGenerationIdV1>,
    expected_old: Option<StoreHeadIdV1>,
    prior_authority_root: Option<StoreObjectIdV1>,
}

impl AuthorityPublicationLineageV1 {
    pub const fn initial(contract_root_id: ContractRootIdV1) -> Self {
        Self {
            contract_root_id,
            previous_generation_id: None,
            expected_old: None,
            prior_authority_root: None,
        }
    }

    pub const fn successor(
        contract_root_id: ContractRootIdV1,
        previous_generation_id: StoreGenerationIdV1,
        expected_old: StoreHeadIdV1,
        prior_authority_root: StoreObjectIdV1,
    ) -> Self {
        Self {
            contract_root_id,
            previous_generation_id: Some(previous_generation_id),
            expected_old: Some(expected_old),
            prior_authority_root: Some(prior_authority_root),
        }
    }

    pub const fn contract_root_id(self) -> ContractRootIdV1 {
        self.contract_root_id
    }

    pub const fn previous_generation_id(self) -> Option<StoreGenerationIdV1> {
        self.previous_generation_id
    }

    pub const fn expected_old(self) -> Option<StoreHeadIdV1> {
        self.expected_old
    }

    pub const fn prior_authority_root(self) -> Option<StoreObjectIdV1> {
        self.prior_authority_root
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueBootstrapMandatePublicationV1 {
    pub(super) request: IssueBootstrapMandateRequestV1,
    pub(super) contract_root_id: ContractRootIdV1,
    pub(super) previous_generation_id: StoreGenerationIdV1,
    pub(super) expected_old: StoreHeadIdV1,
    pub(super) prior_authority_root: StoreObjectIdV1,
    pub(super) next_or_inspect_ref: Option<[u8; 32]>,
}

impl IssueBootstrapMandatePublicationV1 {
    pub fn new(
        request: IssueBootstrapMandateRequestV1,
        lineage: AuthorityPublicationLineageV1,
        next_or_inspect_ref: Option<[u8; 32]>,
    ) -> Result<Self, AuthorityPublicationPlanError> {
        let (Some(previous_generation_id), Some(expected_old), Some(prior_authority_root)) = (
            lineage.previous_generation_id,
            lineage.expected_old,
            lineage.prior_authority_root,
        ) else {
            return Err(AuthorityPublicationPlanError::InvalidGenerationLineage);
        };
        if next_or_inspect_ref == Some([0; 32]) {
            return Err(AuthorityPublicationPlanError::ZeroCommitment);
        }
        Ok(Self {
            request,
            contract_root_id: lineage.contract_root_id,
            previous_generation_id,
            expected_old,
            prior_authority_root,
            next_or_inspect_ref,
        })
    }

    pub fn request(&self) -> &IssueBootstrapMandateRequestV1 {
        &self.request
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrantActionIdentityV1 {
    request_id: ActionRequestIdV1,
    idempotency_key: IdempotencyKeyIdV1,
}

impl GrantActionIdentityV1 {
    pub const fn new(request_id: ActionRequestIdV1, idempotency_key: IdempotencyKeyIdV1) -> Self {
        Self {
            request_id,
            idempotency_key,
        }
    }

    pub const fn request_id(self) -> ActionRequestIdV1 {
        self.request_id
    }

    pub const fn idempotency_key(self) -> IdempotencyKeyIdV1 {
        self.idempotency_key
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GrantAdministrationAuthorityV1 {
    actor_binding_id: PrincipalBindingIdV1,
    actor_session_id: SessionIdV1,
    terminal_grant_id: GrantIdV1,
}

impl GrantAdministrationAuthorityV1 {
    pub const fn new(
        actor_binding_id: PrincipalBindingIdV1,
        actor_session_id: SessionIdV1,
        terminal_grant_id: GrantIdV1,
    ) -> Self {
        Self {
            actor_binding_id,
            actor_session_id,
            terminal_grant_id,
        }
    }

    pub const fn actor_binding_id(self) -> PrincipalBindingIdV1 {
        self.actor_binding_id
    }

    pub const fn actor_session_id(self) -> SessionIdV1 {
        self.actor_session_id
    }

    pub const fn terminal_grant_id(self) -> GrantIdV1 {
        self.terminal_grant_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssueRootAttachedBoundedGrantPublicationV1 {
    pub(super) identity: GrantActionIdentityV1,
    pub(super) lineage: AuthorityPublicationLineageV1,
    pub(super) parent_genesis_grant_id: GenesisGrantIdV1,
    pub(super) grant: OrdinaryBoundedGrantV1,
    pub(super) delegation: OrdinaryGrantDelegationV1,
}

impl IssueRootAttachedBoundedGrantPublicationV1 {
    pub fn new(
        identity: GrantActionIdentityV1,
        lineage: AuthorityPublicationLineageV1,
        parent_genesis_grant_id: GenesisGrantIdV1,
        grant: OrdinaryBoundedGrantV1,
        delegation: OrdinaryGrantDelegationV1,
    ) -> Result<Self, AuthorityPublicationPlanError> {
        require_successor_lineage(lineage)?;
        Ok(Self {
            identity,
            lineage,
            parent_genesis_grant_id,
            grant,
            delegation,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReissueRootAttachedGrantOneToOnePublicationV1 {
    pub(super) identity: GrantActionIdentityV1,
    pub(super) lineage: AuthorityPublicationLineageV1,
    pub(super) authority: GrantAdministrationAuthorityV1,
    pub(super) retired_grant_id: GrantIdV1,
    pub(super) grant: OrdinaryBoundedGrantV1,
    pub(super) delegation: OrdinaryGrantDelegationV1,
}

impl ReissueRootAttachedGrantOneToOnePublicationV1 {
    pub fn new(
        identity: GrantActionIdentityV1,
        lineage: AuthorityPublicationLineageV1,
        authority: GrantAdministrationAuthorityV1,
        retired_grant_id: GrantIdV1,
        grant: OrdinaryBoundedGrantV1,
        delegation: OrdinaryGrantDelegationV1,
    ) -> Result<Self, AuthorityPublicationPlanError> {
        require_successor_lineage(lineage)?;
        if retired_grant_id == grant.grant().id()
            || authority.terminal_grant_id() == retired_grant_id
            || authority.terminal_grant_id() == grant.grant().id()
        {
            return Err(AuthorityPublicationPlanError::SelfAuthorizingGrantMutation);
        }
        Ok(Self {
            identity,
            lineage,
            authority,
            retired_grant_id,
            grant,
            delegation,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevokeGrantPublicationV1 {
    pub(super) identity: GrantActionIdentityV1,
    pub(super) lineage: AuthorityPublicationLineageV1,
    pub(super) authority: GrantAdministrationAuthorityV1,
    pub(super) target_grant_id: GrantIdV1,
}

impl RevokeGrantPublicationV1 {
    pub fn new(
        identity: GrantActionIdentityV1,
        lineage: AuthorityPublicationLineageV1,
        authority: GrantAdministrationAuthorityV1,
        target_grant_id: GrantIdV1,
    ) -> Result<Self, AuthorityPublicationPlanError> {
        require_successor_lineage(lineage)?;
        if authority.terminal_grant_id() == target_grant_id {
            return Err(AuthorityPublicationPlanError::SelfAuthorizingGrantMutation);
        }
        Ok(Self {
            identity,
            lineage,
            authority,
            target_grant_id,
        })
    }
}

fn require_successor_lineage(
    lineage: AuthorityPublicationLineageV1,
) -> Result<(), AuthorityPublicationPlanError> {
    if lineage.previous_generation_id.is_none()
        || lineage.expected_old.is_none()
        || lineage.prior_authority_root.is_none()
    {
        return Err(AuthorityPublicationPlanError::InvalidGenerationLineage);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityPublicationKindV1 {
    Committed,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityPublicationOutcomeV1 {
    pub(super) kind: AuthorityPublicationKindV1,
    pub(super) head: StoreHeadV1,
    pub(super) result: StoreObjectV1,
    pub(super) logical_result_id: ActionResultIdV1,
}

impl AuthorityPublicationOutcomeV1 {
    pub const fn kind(&self) -> AuthorityPublicationKindV1 {
        self.kind
    }

    pub fn head(&self) -> &StoreHeadV1 {
        &self.head
    }

    pub fn result(&self) -> &StoreObjectV1 {
        &self.result
    }

    pub const fn logical_result_id(&self) -> ActionResultIdV1 {
        self.logical_result_id
    }

    pub const fn response_origin(&self) -> ResponseOriginV1 {
        match self.kind {
            AuthorityPublicationKindV1::Committed => ResponseOriginV1::Fresh,
            AuthorityPublicationKindV1::Replayed => ResponseOriginV1::Replay {
                original_result_id: self.logical_result_id,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuthoritySchemaV1 {
    AuthorityMandate,
    BootstrapMandateIssuanceBinding,
    AuthorizationReceipt,
    ActionResult,
    IssueBootstrapMandateRequest,
    ConsentSlotBindingParameter,
    ActionAuthorityBasis,
    AuthorityContext,
    GovernedCapacityDebit,
    AuthorityContinuityManifest,
    PrincipalBinding,
    Session,
    BootstrapGenesisGrant,
    BootstrapMandateInteractionObservationJoin,
    RevocationSet,
    BootstrapAuthoritySnapshot,
    GovernedCapacityRoot,
    AuthorityContinuityClosure,
    SuccessVisibleAuthorityContinuityState,
    AdmittedTransitionGuard,
    LinearizationCoverageWitness,
    AuthorityContinuityPostCutConsequenceSet,
    OrdinaryBoundedGrant,
    OrdinaryGrantDelegation,
}

impl AuthoritySchemaV1 {
    pub(super) const ALL: [Self; 24] = [
        Self::AuthorityMandate,
        Self::BootstrapMandateIssuanceBinding,
        Self::AuthorizationReceipt,
        Self::ActionResult,
        Self::IssueBootstrapMandateRequest,
        Self::ConsentSlotBindingParameter,
        Self::ActionAuthorityBasis,
        Self::AuthorityContext,
        Self::GovernedCapacityDebit,
        Self::AuthorityContinuityManifest,
        Self::PrincipalBinding,
        Self::Session,
        Self::BootstrapGenesisGrant,
        Self::BootstrapMandateInteractionObservationJoin,
        Self::RevocationSet,
        Self::BootstrapAuthoritySnapshot,
        Self::GovernedCapacityRoot,
        Self::SuccessVisibleAuthorityContinuityState,
        Self::AdmittedTransitionGuard,
        Self::LinearizationCoverageWitness,
        Self::AuthorityContinuityPostCutConsequenceSet,
        Self::AuthorityContinuityClosure,
        Self::OrdinaryBoundedGrant,
        Self::OrdinaryGrantDelegation,
    ];

    pub(super) fn id(self) -> Result<SchemaIdV1, crate::domain::vnext::identity::IdentityError> {
        SchemaIdV1::parse(match self {
            Self::AuthorityMandate => {
                "sha256:3833a1928367215f687b3fd803ebf22928622116fd312714d3ba374e0f2457a5"
            }
            Self::BootstrapMandateIssuanceBinding => {
                "sha256:e595bce8592675c612be9f804576f2793ad327acbdab0f8bb258938f15474d56"
            }
            Self::AuthorizationReceipt => {
                "sha256:095fdb17e6979aa5aea74e2e269cdd672766da87ad4a17417749e9efda075778"
            }
            Self::ActionResult => {
                "sha256:8fe476764320b5f0ac8056209d0a5b505e7a07b938386949fdf0f2e1dbfea9c0"
            }
            Self::IssueBootstrapMandateRequest => {
                "sha256:e6b8a4732963968f90267644cf3abf3407df9365912ce043cd9668a1b78125d4"
            }
            Self::ConsentSlotBindingParameter => {
                "sha256:30fdb0f64204cbeb3d6178df3801725fffb81c262dd40f81ccd6fe77417f1be1"
            }
            Self::ActionAuthorityBasis => {
                "sha256:e5dd9020cff772e1bae39ba902a5e9be1be3eb17beeec1520bdcb4c760ce3968"
            }
            Self::AuthorityContext => {
                "sha256:0540b8b9d99249952ba7badd68268173bc43f9f4c314e3f980c269a0361cf006"
            }
            Self::GovernedCapacityDebit => {
                "sha256:00307bd5d7de8e473f3bd06c4153c39b9e3789a9cad5b0bfca004a9ecd368c8a"
            }
            Self::AuthorityContinuityManifest => {
                "sha256:beaf4519431cd6e7d8487ed061de0c9cb48c76477224f58de837c4ba2da790a4"
            }
            Self::PrincipalBinding => {
                "sha256:70390ac7d4d513974efbe4b7162c2fd75b2b881d1e4054ab4aa333cffd8b98cf"
            }
            Self::Session => {
                "sha256:3b97103e3d5d22a7780e9948edac58daea885889986a30a1ab30370656fce078"
            }
            Self::BootstrapGenesisGrant => {
                "sha256:ae0e5c562d39b6a792ca330e12dec3539faad4575850af5285a9033c3383d276"
            }
            Self::BootstrapMandateInteractionObservationJoin => {
                "sha256:8c82067618da6844f359158a6776463e76bfba229c72ab26b5dda47cce120c63"
            }
            Self::RevocationSet => {
                "sha256:fcb6c1a8319326d68aced3abcecd1c7c8e62a8cd11bc6679ddeaa2924605e2c7"
            }
            Self::BootstrapAuthoritySnapshot => {
                "sha256:fc2c3f2a9bd4e4d19a7c98ac27d2b302709a685aa1fb7fea2515801b71f6b7f0"
            }
            Self::GovernedCapacityRoot => {
                "sha256:aad1f227fc516a5429332870548038f691809424e75f1ac26a52ffcc5f762ea2"
            }
            Self::AuthorityContinuityClosure => {
                "sha256:e2f8f1c49f47594d772c5b9a5439dfe15aece2194f9fd0c22046490c0540508b"
            }
            Self::SuccessVisibleAuthorityContinuityState => {
                "sha256:655f9852c928ad54d5f0f57b7558db1f09fc9383035253ef787f3fd69571590f"
            }
            Self::AdmittedTransitionGuard => {
                "sha256:4f3381296a965f3b3e0e031f08db329178c1e02cf474fac1d0f09dbcf6ab5f30"
            }
            Self::LinearizationCoverageWitness => {
                "sha256:59f519f555002ac0bdea8c5723b476b14bb415a31621b635f6512485297a1c86"
            }
            Self::AuthorityContinuityPostCutConsequenceSet => {
                "sha256:61884215711d4df2702c7f0e32af88878d037b3267dffa6c26d85550d090b0aa"
            }
            Self::OrdinaryBoundedGrant => OrdinaryBoundedGrantV1::STORE_SCHEMA_ID,
            Self::OrdinaryGrantDelegation => OrdinaryGrantDelegationV1::STORE_SCHEMA_ID,
        })
    }

    pub(super) fn accepts_value(self, value: &CborValue) -> bool {
        let CborValue::Array(fields) = value else {
            return false;
        };
        let valid_length = match self {
            Self::AuthorityMandate => fields.len() == 14,
            Self::BootstrapMandateIssuanceBinding => fields.len() == 5,
            Self::AuthorizationReceipt => fields.len() == 7,
            Self::ActionResult => fields.len() == 11,
            Self::IssueBootstrapMandateRequest => fields.len() == 14,
            Self::ConsentSlotBindingParameter => fields.len() == 4,
            Self::ActionAuthorityBasis => fields.len() == 3,
            Self::AuthorityContext => matches!(fields.len(), 7 | 9),
            Self::GovernedCapacityDebit => fields.len() == 8,
            Self::AuthorityContinuityManifest => fields.len() == 9,
            Self::PrincipalBinding => fields.len() == 8,
            Self::Session => fields.len() == 8,
            Self::BootstrapGenesisGrant => fields.len() == 11,
            Self::BootstrapMandateInteractionObservationJoin => fields.len() == 9,
            Self::RevocationSet => fields.len() == 3,
            Self::BootstrapAuthoritySnapshot => fields.len() == 15,
            Self::GovernedCapacityRoot => fields.len() == 7,
            Self::AuthorityContinuityClosure => fields.len() == 10,
            Self::SuccessVisibleAuthorityContinuityState => fields.len() == 26,
            Self::AdmittedTransitionGuard => fields.len() == 26,
            Self::LinearizationCoverageWitness => fields.len() == 8,
            Self::AuthorityContinuityPostCutConsequenceSet => fields.len() == 13,
            Self::OrdinaryBoundedGrant => fields.len() == 11,
            Self::OrdinaryGrantDelegation => fields.len() == 6,
        };
        if !valid_length {
            return false;
        }
        match self.value_domain() {
            Some(expected) => matches!(&fields[0], CborValue::Text(actual) if actual == expected),
            None => true,
        }
    }

    const fn value_domain(self) -> Option<&'static str> {
        match self {
            Self::AuthorityMandate => Some("maestro.vnext.authority-mandate-value.v1"),
            Self::BootstrapMandateIssuanceBinding => {
                Some("maestro.vnext.bootstrap-mandate-issuance-binding-value.v1")
            }
            Self::AuthorizationReceipt | Self::ActionResult | Self::ActionAuthorityBasis => None,
            Self::IssueBootstrapMandateRequest => {
                Some("maestro.vnext.issue-bootstrap-mandate-request.v1")
            }
            Self::ConsentSlotBindingParameter => {
                Some("maestro.vnext.consent-slot-binding-parameter.v1")
            }
            Self::AuthorityContext => Some("maestro.vnext.authority-context-value.v1"),
            Self::GovernedCapacityDebit => Some("maestro.vnext.governed-capacity-debit.v1"),
            Self::AuthorityContinuityManifest => {
                Some("maestro.vnext.authority-continuity-totality-manifest.v1")
            }
            Self::PrincipalBinding => Some("maestro.vnext.principal-binding-value.v1"),
            Self::Session => Some("maestro.vnext.session-value.v1"),
            Self::BootstrapGenesisGrant => Some("maestro.vnext.bootstrap-genesis-grant-value.v1"),
            Self::BootstrapMandateInteractionObservationJoin => {
                Some("maestro.vnext.bootstrap-mandate-interaction-observation-join-value.v1")
            }
            Self::RevocationSet => Some("maestro.vnext.revocation-set-value.v1"),
            Self::BootstrapAuthoritySnapshot => {
                Some("maestro.vnext.bootstrap-authority-snapshot.v1")
            }
            Self::GovernedCapacityRoot => Some("maestro.vnext.governed-capacity-root.v1"),
            Self::AuthorityContinuityClosure => {
                Some("maestro.vnext.authority-continuity-closure.v1")
            }
            Self::SuccessVisibleAuthorityContinuityState => {
                Some("maestro.vnext.success-visible-authority-continuity-state.v1")
            }
            Self::AdmittedTransitionGuard => {
                Some("maestro.vnext.authority-transition-guard-evaluation.v1")
            }
            Self::LinearizationCoverageWitness => {
                Some("maestro.vnext.linearization-coverage-witness.v1")
            }
            Self::AuthorityContinuityPostCutConsequenceSet => {
                Some("maestro.vnext.authority-continuity-post-cut-consequence-set.v1")
            }
            Self::OrdinaryBoundedGrant => Some("maestro.vnext.ordinary-bounded-grant.v1"),
            Self::OrdinaryGrantDelegation => Some("maestro.vnext.ordinary-grant-delegation.v1"),
        }
    }

    #[cfg(test)]
    const fn schema_name(self) -> &'static str {
        match self {
            Self::AuthorityMandate => "AuthorityMandateV1",
            Self::BootstrapMandateIssuanceBinding => "BootstrapMandateIssuanceBindingV1",
            Self::AuthorizationReceipt => "AuthorizationReceiptV1",
            Self::ActionResult => "ActionResultV1",
            Self::IssueBootstrapMandateRequest => "IssueBootstrapMandateRequestV1",
            Self::ConsentSlotBindingParameter => "ConsentSlotBindingParameterV1",
            Self::ActionAuthorityBasis => "ActionAuthorityBasisV1",
            Self::AuthorityContext => "AuthorityContextV1",
            Self::GovernedCapacityDebit => "GovernedCapacityDebitV1",
            Self::AuthorityContinuityManifest => "AuthorityContinuityManifestV1",
            Self::PrincipalBinding => "PrincipalBindingV1",
            Self::Session => "SessionV1",
            Self::BootstrapGenesisGrant => "BootstrapGenesisGrantV1",
            Self::BootstrapMandateInteractionObservationJoin => {
                "BootstrapMandateInteractionObservationJoinV1"
            }
            Self::RevocationSet => "RevocationSetV1",
            Self::BootstrapAuthoritySnapshot => "BootstrapAuthoritySnapshotV1",
            Self::GovernedCapacityRoot => "GovernedCapacityRootV1",
            Self::AuthorityContinuityClosure => "AuthorityContinuityClosureV1",
            Self::SuccessVisibleAuthorityContinuityState => {
                "SuccessVisibleAuthorityContinuityStateV1"
            }
            Self::AdmittedTransitionGuard => "AdmittedTransitionGuardV1",
            Self::LinearizationCoverageWitness => "LinearizationCoverageWitnessV1",
            Self::AuthorityContinuityPostCutConsequenceSet => {
                "AuthorityContinuityPostCutConsequenceSetV1"
            }
            Self::OrdinaryBoundedGrant => "OrdinaryBoundedGrantV1",
            Self::OrdinaryGrantDelegation => "OrdinaryGrantDelegationV1",
        }
    }

    #[cfg(test)]
    const fn descriptor_field_count(self) -> usize {
        match self {
            Self::AuthorityMandate => 13,
            Self::BootstrapMandateIssuanceBinding => 4,
            Self::AuthorizationReceipt => 7,
            Self::ActionResult => 11,
            Self::IssueBootstrapMandateRequest => 13,
            Self::ConsentSlotBindingParameter => 3,
            Self::ActionAuthorityBasis => 3,
            Self::AuthorityContext => 8,
            Self::GovernedCapacityDebit => 7,
            Self::AuthorityContinuityManifest => 8,
            Self::PrincipalBinding => 7,
            Self::Session => 7,
            Self::BootstrapGenesisGrant => 10,
            Self::BootstrapMandateInteractionObservationJoin => 8,
            Self::RevocationSet => 2,
            Self::BootstrapAuthoritySnapshot => 14,
            Self::GovernedCapacityRoot => 6,
            Self::AuthorityContinuityClosure => 9,
            Self::SuccessVisibleAuthorityContinuityState => 25,
            Self::AdmittedTransitionGuard => 25,
            Self::LinearizationCoverageWitness => 7,
            Self::AuthorityContinuityPostCutConsequenceSet => 12,
            Self::OrdinaryBoundedGrant => 10,
            Self::OrdinaryGrantDelegation => 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::Value;

    use super::AuthoritySchemaV1;

    #[test]
    fn runtime_schema_registry_preserves_the_exact_frozen_stage_two_prefix() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("contracts/vnext/stage2/authority/schema-descriptors.v1.json");
        let document: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let descriptors = document["descriptors"].as_array().unwrap();
        assert_eq!(descriptors.len(), 22);
        for (runtime, descriptor) in AuthoritySchemaV1::ALL[..22].iter().zip(descriptors) {
            assert_eq!(descriptor["schema_name"], runtime.schema_name());
            assert_eq!(
                descriptor["fields"].as_array().unwrap().len(),
                runtime.descriptor_field_count()
            );
            let runtime_id = runtime.id().unwrap().render();
            assert_eq!(
                descriptor["descriptor_id"].as_str().unwrap(),
                runtime_id.strip_prefix("sha256:").unwrap()
            );
        }
    }

    #[test]
    fn runtime_schema_registry_appends_stage_three_tags_twenty_three_and_twenty_four() {
        let additions = [
            (23, AuthoritySchemaV1::OrdinaryBoundedGrant),
            (24, AuthoritySchemaV1::OrdinaryGrantDelegation),
        ];
        assert_eq!(AuthoritySchemaV1::ALL.len(), 24);
        for (tag, expected) in additions {
            assert_eq!(AuthoritySchemaV1::ALL[tag - 1], expected);
        }
        assert_eq!(
            AuthoritySchemaV1::ALL[22].schema_name(),
            "OrdinaryBoundedGrantV1"
        );
        assert_eq!(
            AuthoritySchemaV1::ALL[23].schema_name(),
            "OrdinaryGrantDelegationV1"
        );
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityPublicationPlanError {
    #[error("Authority publication Generation lineage is not exact")]
    InvalidGenerationLineage,
    #[error("Authority publication commitments must be nonzero")]
    ZeroCommitment,
    #[error("the target or candidate Grant cannot authorize its own mutation")]
    SelfAuthorizingGrantMutation,
}
