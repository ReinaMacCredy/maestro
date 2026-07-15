use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::closed::{AuthorityContextKindV1, AuthorityTagError};
use super::identity::{AuthorityContextIdV1, CapacityRootIdV1, CmaWithdrawalCapacityIdV1};

const MAX_INITIAL_CAPACITY: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RepositoryGovernedCapacitySlotKindV1 {
    RepositoryOrdinaryMutation = 1,
    RepositoryAuthorityAdministration = 2,
    RepositoryEvidenceAcquisition = 3,
    RepositoryPlanningPublication = 4,
    RepositoryExternalEffect = 5,
    RepositoryPersistenceMaintenance = 6,
}

impl RepositoryGovernedCapacitySlotKindV1 {
    pub const ALL: [Self; 6] = [
        Self::RepositoryOrdinaryMutation,
        Self::RepositoryAuthorityAdministration,
        Self::RepositoryEvidenceAcquisition,
        Self::RepositoryPlanningPublication,
        Self::RepositoryExternalEffect,
        Self::RepositoryPersistenceMaintenance,
    ];
}

impl TryFrom<u8> for RepositoryGovernedCapacitySlotKindV1 {
    type Error = AuthorityTagError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::RepositoryOrdinaryMutation),
            2 => Ok(Self::RepositoryAuthorityAdministration),
            3 => Ok(Self::RepositoryEvidenceAcquisition),
            4 => Ok(Self::RepositoryPlanningPublication),
            5 => Ok(Self::RepositoryExternalEffect),
            6 => Ok(Self::RepositoryPersistenceMaintenance),
            value => Err(AuthorityTagError::UnknownRepositoryGovernedCapacityKind(
                value,
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum InstallationGovernedCapacitySlotKindV1 {
    InstallationAuthorityAdministration = 1,
    InstallationDistributionMutation = 2,
    InstallationGovernedReviewPublication = 3,
    InstallationExternalEffect = 4,
    InstallationWriterAdministration = 5,
    InstallationPersistenceMaintenance = 6,
}

impl InstallationGovernedCapacitySlotKindV1 {
    pub const ALL: [Self; 6] = [
        Self::InstallationAuthorityAdministration,
        Self::InstallationDistributionMutation,
        Self::InstallationGovernedReviewPublication,
        Self::InstallationExternalEffect,
        Self::InstallationWriterAdministration,
        Self::InstallationPersistenceMaintenance,
    ];
}

impl TryFrom<u8> for InstallationGovernedCapacitySlotKindV1 {
    type Error = AuthorityTagError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::InstallationAuthorityAdministration),
            2 => Ok(Self::InstallationDistributionMutation),
            3 => Ok(Self::InstallationGovernedReviewPublication),
            4 => Ok(Self::InstallationExternalEffect),
            5 => Ok(Self::InstallationWriterAdministration),
            6 => Ok(Self::InstallationPersistenceMaintenance),
            value => Err(AuthorityTagError::UnknownInstallationGovernedCapacityKind(
                value,
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GovernedCapacityKindV1 {
    Repository(RepositoryGovernedCapacitySlotKindV1),
    Installation(InstallationGovernedCapacitySlotKindV1),
}

impl GovernedCapacityKindV1 {
    pub const fn context_kind(self) -> AuthorityContextKindV1 {
        match self {
            Self::Repository(_) => AuthorityContextKindV1::RepositoryAuthorityContext,
            Self::Installation(_) => AuthorityContextKindV1::InstallationAuthorityContext,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapacityUseDispositionV1 {
    FreshCommit,
    Replay,
    NoOp,
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GovernedCapacityRootV1 {
    id: CapacityRootIdV1,
    context_kind: AuthorityContextKindV1,
    context_id: AuthorityContextIdV1,
    kind: GovernedCapacityKindV1,
    initial_max: u32,
    spent: u32,
}

impl GovernedCapacityRootV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.governed-capacity-root.v1";

    pub fn new(
        id: CapacityRootIdV1,
        context_kind: AuthorityContextKindV1,
        context_id: AuthorityContextIdV1,
        kind: GovernedCapacityKindV1,
        initial_max: u32,
    ) -> Result<Self, CapacityError> {
        if initial_max == 0 || initial_max > MAX_INITIAL_CAPACITY {
            return Err(CapacityError::InvalidInitialMaximum);
        }
        if kind.context_kind() != context_kind {
            return Err(CapacityError::ContextKindMismatch);
        }
        Ok(Self {
            id,
            context_kind,
            context_id,
            kind,
            initial_max,
            spent: 0,
        })
    }

    pub fn transition(
        self,
        expected_context_id: AuthorityContextIdV1,
        expected_kind: GovernedCapacityKindV1,
        expected_spent: u32,
        disposition: CapacityUseDispositionV1,
    ) -> Result<GovernedCapacityTransitionV1, CapacityError> {
        if expected_context_id != self.context_id {
            return Err(CapacityError::ContextMismatch);
        }
        if expected_kind != self.kind {
            return Err(CapacityError::CapacityKindMismatch);
        }
        if expected_spent != self.spent {
            return Err(CapacityError::ExpectedSpentMismatch);
        }
        if disposition != CapacityUseDispositionV1::FreshCommit {
            return Ok(GovernedCapacityTransitionV1 {
                root: self,
                debit: None,
            });
        }
        if self.spent == self.initial_max {
            return Err(CapacityError::Exhausted);
        }
        let resulting_spent = self.spent.checked_add(1).ok_or(CapacityError::Exhausted)?;
        let debit = GovernedCapacityDebitV1 {
            root_id: self.id,
            context_kind: self.context_kind,
            context_id: self.context_id,
            kind: self.kind,
            ordinal: self.spent,
            prior_spent: self.spent,
            resulting_spent,
        };
        Ok(GovernedCapacityTransitionV1 {
            root: Self {
                spent: resulting_spent,
                ..self
            },
            debit: Some(debit),
        })
    }

    pub const fn id(self) -> CapacityRootIdV1 {
        self.id
    }

    pub const fn context_kind(self) -> AuthorityContextKindV1 {
        self.context_kind
    }

    pub const fn context_id(self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn kind(self) -> GovernedCapacityKindV1 {
        self.kind
    }

    pub const fn initial_max(self) -> u32 {
        self.initial_max
    }

    pub const fn spent(self) -> u32 {
        self.spent
    }

    pub const fn remaining(self) -> u32 {
        self.initial_max - self.spent
    }

    pub fn schema_value(self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Bytes(self.id.as_bytes().to_vec()),
            CborValue::Unsigned(self.context_kind as u64),
            CborValue::Bytes(self.context_id.as_bytes().to_vec()),
            CborValue::Unsigned(capacity_kind_tag(self.kind)),
            CborValue::Unsigned(u64::from(self.initial_max)),
            CborValue::Unsigned(u64::from(self.spent)),
        ]))
    }

    pub fn canonical_bytes(self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GovernedCapacityDebitV1 {
    root_id: CapacityRootIdV1,
    context_kind: AuthorityContextKindV1,
    context_id: AuthorityContextIdV1,
    kind: GovernedCapacityKindV1,
    ordinal: u32,
    prior_spent: u32,
    resulting_spent: u32,
}

impl GovernedCapacityDebitV1 {
    pub const SCHEMA_DOMAIN: &'static str = "maestro.vnext.governed-capacity-debit.v1";

    pub const fn root_id(self) -> CapacityRootIdV1 {
        self.root_id
    }

    pub const fn context_kind(self) -> AuthorityContextKindV1 {
        self.context_kind
    }

    pub const fn context_id(self) -> AuthorityContextIdV1 {
        self.context_id
    }

    pub const fn kind(self) -> GovernedCapacityKindV1 {
        self.kind
    }

    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    pub const fn prior_spent(self) -> u32 {
        self.prior_spent
    }

    pub const fn resulting_spent(self) -> u32 {
        self.resulting_spent
    }

    pub const fn quantity(self) -> u32 {
        1
    }

    pub fn schema_value(self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Bytes(self.root_id.as_bytes().to_vec()),
            CborValue::Unsigned(self.context_kind as u64),
            CborValue::Bytes(self.context_id.as_bytes().to_vec()),
            CborValue::Unsigned(capacity_kind_tag(self.kind)),
            CborValue::Unsigned(u64::from(self.ordinal)),
            CborValue::Unsigned(u64::from(self.prior_spent)),
            CborValue::Unsigned(u64::from(self.resulting_spent)),
        ]))
    }

    pub fn canonical_bytes(self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

const fn capacity_kind_tag(kind: GovernedCapacityKindV1) -> u64 {
    match kind {
        GovernedCapacityKindV1::Repository(kind) => kind as u64,
        GovernedCapacityKindV1::Installation(kind) => kind as u64,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GovernedCapacityTransitionV1 {
    root: GovernedCapacityRootV1,
    debit: Option<GovernedCapacityDebitV1>,
}

impl GovernedCapacityTransitionV1 {
    pub const fn root(&self) -> &GovernedCapacityRootV1 {
        &self.root
    }

    pub const fn debit(&self) -> Option<&GovernedCapacityDebitV1> {
        self.debit.as_ref()
    }

    pub const fn into_root(self) -> GovernedCapacityRootV1 {
        self.root
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CmaObservationPublicationPurposeV1 {
    TrustedTimeAcquisition = 1,
    RecoveryExternalRegistration = 2,
    RecoveryExternalStatus = 3,
    MaintenanceExecutorCurrentness = 4,
    ProspectiveContinuityCarrier = 5,
}

impl CmaObservationPublicationPurposeV1 {
    pub const ALL: [Self; 5] = [
        Self::TrustedTimeAcquisition,
        Self::RecoveryExternalRegistration,
        Self::RecoveryExternalStatus,
        Self::MaintenanceExecutorCurrentness,
        Self::ProspectiveContinuityCarrier,
    ];
}

impl TryFrom<u8> for CmaObservationPublicationPurposeV1 {
    type Error = AuthorityTagError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::TrustedTimeAcquisition),
            2 => Ok(Self::RecoveryExternalRegistration),
            3 => Ok(Self::RecoveryExternalStatus),
            4 => Ok(Self::MaintenanceExecutorCurrentness),
            5 => Ok(Self::ProspectiveContinuityCarrier),
            value => Err(AuthorityTagError::UnknownCmaObservationPublicationPurpose(
                value,
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CmaEffectWithdrawalSlotFamilyV1 {
    MaintenanceExecutorCurrentness = 1,
    ProspectiveContinuityCarrier = 2,
    PlannedTurnoverHighWater = 3,
    RepositoryRecoveryAdmission = 4,
    InstallationRecoveryAdmission = 5,
}

impl CmaEffectWithdrawalSlotFamilyV1 {
    pub const ALL: [Self; 5] = [
        Self::MaintenanceExecutorCurrentness,
        Self::ProspectiveContinuityCarrier,
        Self::PlannedTurnoverHighWater,
        Self::RepositoryRecoveryAdmission,
        Self::InstallationRecoveryAdmission,
    ];
}

impl TryFrom<u8> for CmaEffectWithdrawalSlotFamilyV1 {
    type Error = AuthorityTagError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::MaintenanceExecutorCurrentness),
            2 => Ok(Self::ProspectiveContinuityCarrier),
            3 => Ok(Self::PlannedTurnoverHighWater),
            4 => Ok(Self::RepositoryRecoveryAdmission),
            5 => Ok(Self::InstallationRecoveryAdmission),
            value => Err(AuthorityTagError::UnknownCmaEffectWithdrawalSlotFamily(
                value,
            )),
        }
    }
}

// TODO(authority): Remove this compatibility alias on or after 2026-10-15 once
// all Stage 2 callers use the explicit Observation-publication purpose name.
pub type CmaWithdrawalPurposeV1 = CmaObservationPublicationPurposeV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CmaWithdrawalCapacityV1 {
    id: CmaWithdrawalCapacityIdV1,
    purpose: CmaWithdrawalPurposeV1,
    initial_max: u32,
    spent: u32,
}

impl CmaWithdrawalCapacityV1 {
    pub fn new(
        id: CmaWithdrawalCapacityIdV1,
        purpose: CmaWithdrawalPurposeV1,
        initial_max: u32,
    ) -> Result<Self, CapacityError> {
        if initial_max == 0 || initial_max > MAX_INITIAL_CAPACITY {
            return Err(CapacityError::InvalidInitialMaximum);
        }
        Ok(Self {
            id,
            purpose,
            initial_max,
            spent: 0,
        })
    }

    pub fn spend(self, expected_spent: u32) -> Result<Self, CapacityError> {
        self.advance_spent(expected_spent, expected_spent.saturating_add(1))
    }

    pub fn advance_spent(
        self,
        expected_spent: u32,
        next_spent: u32,
    ) -> Result<Self, CapacityError> {
        if expected_spent != self.spent {
            return Err(CapacityError::ExpectedSpentMismatch);
        }
        if next_spent != self.spent.saturating_add(1) {
            return Err(CapacityError::NonMonotonicSpend);
        }
        if next_spent > self.initial_max {
            return Err(CapacityError::Exhausted);
        }
        Ok(Self {
            spent: next_spent,
            ..self
        })
    }

    pub const fn remaining(self) -> u32 {
        self.initial_max - self.spent
    }

    pub const fn id(self) -> CmaWithdrawalCapacityIdV1 {
        self.id
    }

    pub const fn purpose(self) -> CmaWithdrawalPurposeV1 {
        self.purpose
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CapacityError {
    #[error("capacity initial maximum must be within the finite nonzero bound")]
    InvalidInitialMaximum,
    #[error("capacity kind does not belong to the Authority context kind")]
    ContextKindMismatch,
    #[error("capacity cannot be donated across Authority contexts")]
    ContextMismatch,
    #[error("capacity cannot be donated across governed-capacity kinds")]
    CapacityKindMismatch,
    #[error("capacity expected-spent value is stale")]
    ExpectedSpentMismatch,
    #[error("capacity spend must advance by exactly one and can never refill or refund")]
    NonMonotonicSpend,
    #[error("capacity is exhausted")]
    Exhausted,
}
