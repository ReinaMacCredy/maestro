use thiserror::Error;

use super::ActionAuthorityBasisKindV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityActionLeafV1 {
    AllocateGovernedCapacitySlot,
    EstablishConsumptionCellRoot,
    IssueRootAttachedBoundedGrant,
    ReissueRootAttachedGrantOneToOne,
    RevokeGrant,
}

const ACTION_SPEC_PROTOCOL_REVISION_V1: u64 = 1;
const ACTION_SPEC_MANIFEST_ID_V1: &str =
    "7a7a1311690fcdec194c0d9c9afbbe4e28f1332b567ed23451daf06ebca02970";
const ACTION_SPEC_GRAMMAR_ID_V1: &str =
    "b7ef635dcd29af4fc41f20cd670b726e5627c2f7210344d058e7c188ace69647";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryActionLeafV1 {
    CreateDraftWork,
    CancelWork,
    CompleteWork,
    AbsorbWork,
    SubmitWorkCompletion,
    RejectWorkCompletion,
    ReturnWorkForRepair,
    SubmitStep,
    SatisfyStep,
    RejectStepSubmission,
    RecoverStepSubmission,
    PublishInitialContract,
    AmendContract,
    AppendDesignRevision,
    ResolveDecision,
    AcquireStepExecution,
}

impl RepositoryActionLeafV1 {
    pub const ALL: [Self; 16] = [
        Self::CreateDraftWork,
        Self::CancelWork,
        Self::CompleteWork,
        Self::AbsorbWork,
        Self::SubmitWorkCompletion,
        Self::RejectWorkCompletion,
        Self::ReturnWorkForRepair,
        Self::SubmitStep,
        Self::SatisfyStep,
        Self::RejectStepSubmission,
        Self::RecoverStepSubmission,
        Self::PublishInitialContract,
        Self::AmendContract,
        Self::AppendDesignRevision,
        Self::ResolveDecision,
        Self::AcquireStepExecution,
    ];

    pub const fn literal(self) -> &'static str {
        match self {
            Self::CreateDraftWork => "CreateDraftWork",
            Self::CancelWork => "CancelWork",
            Self::CompleteWork => "CompleteWork",
            Self::AbsorbWork => "AbsorbWork",
            Self::SubmitWorkCompletion => "SubmitWorkCompletion",
            Self::RejectWorkCompletion => "RejectWorkCompletion",
            Self::ReturnWorkForRepair => "ReturnWorkForRepair",
            Self::SubmitStep => "SubmitStep",
            Self::SatisfyStep => "SatisfyStep",
            Self::RejectStepSubmission => "RejectStepSubmission",
            Self::RecoverStepSubmission => "RecoverStepSubmission",
            Self::PublishInitialContract => "PublishInitialContract",
            Self::AmendContract => "AmendContract",
            Self::AppendDesignRevision => "AppendDesignRevision",
            Self::ResolveDecision => "ResolveDecision",
            Self::AcquireStepExecution => "AcquireStepExecution",
        }
    }

    pub const fn global_tag(self) -> u64 {
        match self {
            Self::CreateDraftWork => 1,
            Self::CancelWork => 2,
            Self::CompleteWork => 3,
            Self::AbsorbWork => 4,
            Self::SubmitWorkCompletion => 5,
            Self::RejectWorkCompletion => 6,
            Self::ReturnWorkForRepair => 7,
            Self::SubmitStep => 8,
            Self::SatisfyStep => 9,
            Self::RejectStepSubmission => 10,
            Self::RecoverStepSubmission => 11,
            Self::PublishInitialContract => 12,
            Self::AmendContract => 13,
            Self::AppendDesignRevision => 15,
            Self::ResolveDecision => 20,
            Self::AcquireStepExecution => 23,
        }
    }

    pub const fn owner_tag(self) -> u64 {
        match self {
            Self::CreateDraftWork
            | Self::CancelWork
            | Self::CompleteWork
            | Self::AbsorbWork
            | Self::SubmitWorkCompletion
            | Self::RejectWorkCompletion
            | Self::ReturnWorkForRepair => 1,
            Self::SubmitStep
            | Self::SatisfyStep
            | Self::RejectStepSubmission
            | Self::RecoverStepSubmission => 2,
            Self::PublishInitialContract | Self::AmendContract => 3,
            Self::AppendDesignRevision => 4,
            Self::ResolveDecision => 5,
            Self::AcquireStepExecution => 6,
        }
    }

    pub const fn local_tag(self) -> u64 {
        match self {
            Self::CreateDraftWork | Self::SubmitStep | Self::PublishInitialContract => 1,
            Self::CancelWork
            | Self::SatisfyStep
            | Self::AmendContract
            | Self::AppendDesignRevision => 2,
            Self::CompleteWork | Self::RejectStepSubmission | Self::ResolveDecision => 3,
            Self::AbsorbWork | Self::RecoverStepSubmission => 4,
            Self::SubmitWorkCompletion => 5,
            Self::RejectWorkCompletion => 6,
            Self::ReturnWorkForRepair => 7,
            Self::AcquireStepExecution => 1,
        }
    }

    pub const fn owner_descriptor_id(self) -> &'static str {
        match self {
            Self::CreateDraftWork
            | Self::CancelWork
            | Self::CompleteWork
            | Self::AbsorbWork
            | Self::SubmitWorkCompletion
            | Self::RejectWorkCompletion
            | Self::ReturnWorkForRepair => {
                "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee"
            }
            Self::SubmitStep
            | Self::SatisfyStep
            | Self::RejectStepSubmission
            | Self::RecoverStepSubmission => {
                "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e"
            }
            Self::PublishInitialContract | Self::AmendContract => {
                "e33f42c43c3fadf498db847773ed47e26a459453cc65f14dde9bb5d05cf356ab"
            }
            Self::AppendDesignRevision => {
                "85aad446bae62f47851f719f74296bd2576f30894b95ccb4b3b0c59790a80dc5"
            }
            Self::ResolveDecision => {
                "a3d6c9c0dcd9b5e3447cf4dc45edf5d1b338c99dfc27a61df23966b7514ae9dc"
            }
            Self::AcquireStepExecution => {
                "82d922e944dc4fe27d3101bc725e0caea82093e8dabe79ed5732ee5c8da91292"
            }
        }
    }

    pub const fn descriptor_id(self) -> &'static str {
        match self {
            Self::CreateDraftWork => {
                "56ded201d62fbb94486581d13cc6a086b3e114ad889aa1a954841f7f646afc40"
            }
            Self::CancelWork => "b58d2fecb0f1b27146884f85847cb1b22575b32d8d6e92efe6608cf582420615",
            Self::CompleteWork => {
                "163de9814514910c9ca1d5b1f76ac982e0788bd8a81025eff5c551a0a923b5d2"
            }
            Self::AbsorbWork => "4fb2d35f4bd7c2169bec6bc51af840f325c419c28de0041d60b69bc4691125ea",
            Self::SubmitWorkCompletion => {
                "7d8083d10f75348f805e89e8fcd5f27d81face12d2d8ae1047a01c53fbfb2803"
            }
            Self::RejectWorkCompletion => {
                "d03d0a753eb0821b43f002de3eab1afb32a7ede75ff0a3588b0718292c0d7b3f"
            }
            Self::ReturnWorkForRepair => {
                "2fbc3d51f0b750cb9a1292d404ada520c17fac6c9fd960350188d97f3b6acc0b"
            }
            Self::SubmitStep => "c5a7079af2dafa9acc956477b3004b5fb21dd688b4022677416164c4485f96d6",
            Self::SatisfyStep => "a5887f9b87af5f6a5f466df222b3a76b2eca5762b33a5694b4ebcb61b3db127e",
            Self::RejectStepSubmission => {
                "b75460c72c4907e2893bb48559b2c19d99ecb4d27ab43a10adda4ee95dfbc62a"
            }
            Self::RecoverStepSubmission => {
                "130cb3ecfd8146ba869de9a1198b3c3b6b67b2c06101cecf62a955db5b587e13"
            }
            Self::PublishInitialContract => {
                "5c3bf7e45cc2e8348bb5a6ce403cf6d14f718c7ef4514ca01e60370174387234"
            }
            Self::AmendContract => {
                "65020299f9f323f4a098c2ff240cbbc984bfc5f6f761712c995e38486d2046f4"
            }
            Self::AppendDesignRevision => {
                "4235a07743d0fa3557f612b0d4dd499afcd02adadcc92facfcbc806645a99e83"
            }
            Self::ResolveDecision => {
                "4e05d1c7d9314a843d43538c399ece7df7da52793062c7ca43805e8f763f75ac"
            }
            Self::AcquireStepExecution => {
                "8fe0e1c9141feb86e36badb1a861d49a94ea2224a8c1d0b7a859cd53b7f7a9a2"
            }
        }
    }

    pub const fn protocol_revision(self) -> u64 {
        ACTION_SPEC_PROTOCOL_REVISION_V1
    }

    pub const fn manifest_id(self) -> &'static str {
        ACTION_SPEC_MANIFEST_ID_V1
    }

    pub const fn grammar_id(self) -> &'static str {
        ACTION_SPEC_GRAMMAR_ID_V1
    }
}

impl AuthorityActionLeafV1 {
    pub const ALL: [Self; 5] = [
        Self::AllocateGovernedCapacitySlot,
        Self::EstablishConsumptionCellRoot,
        Self::IssueRootAttachedBoundedGrant,
        Self::ReissueRootAttachedGrantOneToOne,
        Self::RevokeGrant,
    ];

    pub const fn literal(self) -> &'static str {
        match self {
            Self::AllocateGovernedCapacitySlot => "AllocateGovernedCapacitySlot",
            Self::EstablishConsumptionCellRoot => "EstablishConsumptionCellRoot",
            Self::IssueRootAttachedBoundedGrant => "IssueRootAttachedBoundedGrant",
            Self::ReissueRootAttachedGrantOneToOne => "ReissueRootAttachedGrantOneToOne",
            Self::RevokeGrant => "RevokeGrant",
        }
    }

    pub const fn authority_basis(self) -> ActionAuthorityBasisKindV1 {
        match self {
            Self::AllocateGovernedCapacitySlot
            | Self::EstablishConsumptionCellRoot
            | Self::IssueRootAttachedBoundedGrant => ActionAuthorityBasisKindV1::BootstrapControlG0,
            Self::ReissueRootAttachedGrantOneToOne | Self::RevokeGrant => {
                ActionAuthorityBasisKindV1::OrdinaryLiveRuntime
            }
        }
    }

    pub fn parse_exact(literal: &str) -> Result<Self, AuthorityActionBasisErrorV1> {
        Self::ALL
            .into_iter()
            .find(|leaf| leaf.literal() == literal)
            .ok_or_else(|| AuthorityActionBasisErrorV1::UnknownActionLeaf(literal.to_owned()))
    }
}

pub fn exact_authority_basis_for_action(
    literal: &str,
) -> Result<ActionAuthorityBasisKindV1, AuthorityActionBasisErrorV1> {
    Ok(AuthorityActionLeafV1::parse_exact(literal)?.authority_basis())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityActionBasisErrorV1 {
    #[error("Authority Action leaf is unknown or unclassified: {0}")]
    UnknownActionLeaf(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_registry_has_no_cma_or_name_inference() {
        for leaf in AuthorityActionLeafV1::ALL {
            assert_ne!(
                leaf.authority_basis(),
                ActionAuthorityBasisKindV1::ContinuityMaintenance
            );
        }
        assert_eq!(
            exact_authority_basis_for_action("IssueRootAttachedBoundedGrant").unwrap(),
            ActionAuthorityBasisKindV1::BootstrapControlG0
        );
        assert_eq!(
            exact_authority_basis_for_action("AllocateGovernedCapacitySlot").unwrap(),
            ActionAuthorityBasisKindV1::BootstrapControlG0
        );
        assert_eq!(
            exact_authority_basis_for_action("EstablishConsumptionCellRoot").unwrap(),
            ActionAuthorityBasisKindV1::BootstrapControlG0
        );
        assert_eq!(
            exact_authority_basis_for_action("ReissueRootAttachedGrantOneToOne").unwrap(),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime
        );
        assert_eq!(
            exact_authority_basis_for_action("RevokeGrant").unwrap(),
            ActionAuthorityBasisKindV1::OrdinaryLiveRuntime
        );
        assert!(exact_authority_basis_for_action("AdminRevokeGrant").is_err());
        assert!(exact_authority_basis_for_action("IssueRootAttachedBoundedGrantV2").is_err());
    }

    #[test]
    fn repository_stage_three_leaves_are_exact_frozen_catalog_members() {
        let rows = RepositoryActionLeafV1::ALL.map(|leaf| {
            (
                leaf.global_tag(),
                leaf.literal(),
                leaf.owner_tag(),
                leaf.local_tag(),
                leaf.owner_descriptor_id(),
                leaf.descriptor_id(),
                leaf.protocol_revision(),
                leaf.manifest_id(),
                leaf.grammar_id(),
            )
        });
        assert_eq!(rows.len(), 16);
        assert_eq!(rows[1].0, 2);
        assert_eq!(rows[1].1, "CancelWork");
        assert_eq!(
            rows[12].5,
            "65020299f9f323f4a098c2ff240cbbc984bfc5f6f761712c995e38486d2046f4"
        );
        assert_eq!(rows[14].0, 20);
        assert!(rows.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
}
