use thiserror::Error;

use crate::foundation::core::deterministic_cbor::CborError;
use crate::foundation::core::deterministic_cbor::CborValue;

use super::{MigrationDigestV1, MigrationIdentityErrorV1, QuarantineEntryV1};

const ROLLBACK_ASSESSMENT_DOMAIN_V1: &[u8] = b"maestro.vnext.migration.rollback-assessment.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CutoverAcceptanceV1 {
    PreAccept,
    Accepted,
}

impl CutoverAcceptanceV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::PreAccept => 1,
            Self::Accepted => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectCrossingV1 {
    ProvenNotCrossed,
    PossibleOrUnknown,
    ConfirmedCrossed,
}

impl EffectCrossingV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::ProvenNotCrossed => 1,
            Self::PossibleOrUnknown => 2,
            Self::ConfirmedCrossed => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RollbackDispositionV1 {
    ProtectedExactV1RollbackEligible,
    RecoveryRequired,
    VNextFreshGenerationRecoveryOnly,
    RefusedStaleHost,
}

impl RollbackDispositionV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::ProtectedExactV1RollbackEligible => 1,
            Self::RecoveryRequired => 2,
            Self::VNextFreshGenerationRecoveryOnly => 3,
            Self::RefusedStaleHost => 4,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollbackAssessmentV1 {
    cutover_attempt_id: MigrationDigestV1,
    observed_host_attempt_id: Option<MigrationDigestV1>,
    acceptance: CutoverAcceptanceV1,
    effect_crossing: EffectCrossingV1,
    disposition: RollbackDispositionV1,
    id: MigrationDigestV1,
}

#[cfg(test)]
pub trait Stage9Stage10CutoverHostAdapterV1 {
    fn cutover_host_facts(
        &self,
        cutover_attempt_id: MigrationDigestV1,
    ) -> Result<
        (
            Option<MigrationDigestV1>,
            CutoverAcceptanceV1,
            EffectCrossingV1,
        ),
        RollbackAssessmentErrorV1,
    >;
}

impl RollbackAssessmentV1 {
    pub(in crate::domain::migration) fn assess_cutover_observation(
        cutover_attempt_id: MigrationDigestV1,
        observed_host_attempt_id: Option<MigrationDigestV1>,
        acceptance: CutoverAcceptanceV1,
        effect_crossing: EffectCrossingV1,
    ) -> Result<Self, RollbackAssessmentErrorV1> {
        if cutover_attempt_id.as_bytes() == &[0; 32]
            || observed_host_attempt_id.is_some_and(|id| id.as_bytes() == &[0; 32])
        {
            return Err(RollbackAssessmentErrorV1::InvalidHostBinding);
        }
        let disposition = if observed_host_attempt_id != Some(cutover_attempt_id) {
            RollbackDispositionV1::RefusedStaleHost
        } else {
            match (acceptance, effect_crossing) {
                (CutoverAcceptanceV1::PreAccept, EffectCrossingV1::ProvenNotCrossed) => {
                    RollbackDispositionV1::ProtectedExactV1RollbackEligible
                }
                (CutoverAcceptanceV1::PreAccept, _) => RollbackDispositionV1::RecoveryRequired,
                (CutoverAcceptanceV1::Accepted, EffectCrossingV1::ProvenNotCrossed) => {
                    RollbackDispositionV1::RecoveryRequired
                }
                (CutoverAcceptanceV1::Accepted, _) => {
                    RollbackDispositionV1::VNextFreshGenerationRecoveryOnly
                }
            }
        };
        let id = MigrationDigestV1::identify(
            ROLLBACK_ASSESSMENT_DOMAIN_V1,
            &CborValue::Array(vec![
                cutover_attempt_id.canonical_value(),
                CborValue::optional(
                    observed_host_attempt_id.map(MigrationDigestV1::canonical_value),
                ),
                CborValue::Unsigned(acceptance.tag()),
                CborValue::Unsigned(effect_crossing.tag()),
                CborValue::Unsigned(disposition.tag()),
            ]),
        )?;
        Ok(Self {
            cutover_attempt_id,
            observed_host_attempt_id,
            acceptance,
            effect_crossing,
            disposition,
            id,
        })
    }

    #[cfg(test)]
    pub fn from_cutover_host_adapter<A: Stage9Stage10CutoverHostAdapterV1>(
        cutover_attempt_id: MigrationDigestV1,
        adapter: &A,
    ) -> Result<Self, RollbackAssessmentErrorV1> {
        let (observed_host_attempt_id, acceptance, effect_crossing) =
            adapter.cutover_host_facts(cutover_attempt_id)?;
        Self::assess_cutover_observation(
            cutover_attempt_id,
            observed_host_attempt_id,
            acceptance,
            effect_crossing,
        )
    }

    pub const fn cutover_attempt_id(&self) -> MigrationDigestV1 {
        self.cutover_attempt_id
    }

    pub const fn observed_host_attempt_id(&self) -> Option<MigrationDigestV1> {
        self.observed_host_attempt_id
    }

    pub const fn acceptance(&self) -> CutoverAcceptanceV1 {
        self.acceptance
    }

    pub const fn effect_crossing(&self) -> EffectCrossingV1 {
        self.effect_crossing
    }

    pub const fn disposition(&self) -> RollbackDispositionV1 {
        self.disposition
    }

    pub const fn id(&self) -> MigrationDigestV1 {
        self.id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::domain::migration) enum ProtectedV1RollbackOutcomeV1 {
    Restored {
        source_id: MigrationDigestV1,
        source_sha256: MigrationDigestV1,
        bytes: Vec<u8>,
    },
    Refused {
        disposition: RollbackDispositionV1,
    },
}

pub(in crate::domain::migration) fn restore_protected_exact_v1(
    assessment: &RollbackAssessmentV1,
    entry: &QuarantineEntryV1,
    protected_bytes: &[u8],
) -> Result<ProtectedV1RollbackOutcomeV1, RollbackRestoreErrorV1> {
    if assessment.disposition() != RollbackDispositionV1::ProtectedExactV1RollbackEligible {
        return Ok(ProtectedV1RollbackOutcomeV1::Refused {
            disposition: assessment.disposition(),
        });
    }
    if u64::try_from(protected_bytes.len())
        .map_err(|_| RollbackRestoreErrorV1::ProtectedBytesMismatch)?
        != entry.source_byte_length()
        || MigrationDigestV1::digest_bytes(protected_bytes)? != entry.source_sha256()
    {
        return Err(RollbackRestoreErrorV1::ProtectedBytesMismatch);
    }
    Ok(ProtectedV1RollbackOutcomeV1::Restored {
        source_id: entry.source_id(),
        source_sha256: entry.source_sha256(),
        bytes: protected_bytes.to_vec(),
    })
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RollbackAssessmentErrorV1 {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error("cutover host binding contains a zero attempt identity")]
    InvalidHostBinding,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(in crate::domain::migration) enum RollbackRestoreErrorV1 {
    #[error(transparent)]
    Identity(#[from] MigrationIdentityErrorV1),
    #[error("protected exact-v1 bytes differ from the sealed quarantine entry")]
    ProtectedBytesMismatch,
}
