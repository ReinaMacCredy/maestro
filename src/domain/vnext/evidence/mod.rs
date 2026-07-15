//! Immutable Evidence-owned Claim records and their Submission associations.

mod claim;
mod identity;

pub use claim::{
    CLAIM_RECORD_DOMAIN_V1, CLAIM_RECORD_VERSION_V1, ClaimError, ClaimSubjectV1, ClaimV1,
    SubmissionRefKindV1, SubmissionRefV1,
};
pub use identity::{CLAIM_ID_DOMAIN_V1, ClaimIdV1, EvidenceIdentityError, ObservationRecordIdV1};
