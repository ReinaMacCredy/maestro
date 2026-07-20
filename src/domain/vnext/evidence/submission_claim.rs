use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{ClaimSubjectV1, ClaimV1, SubmissionRefV1};
use crate::domain::vnext::identity::{
    ConstraintExprV1, FieldDescriptorV1, SchemaDescriptorV1, SchemaError, TypeExprV1,
};
use crate::foundation::core::deterministic_cbor::CborValue;

pub const SUBMISSION_CLAIM_SET_DOMAIN_V1: &[u8] = b"maestro.submission-claim-set.v1";
const MAX_SUBMISSION_IDENTIFIER_BYTES_V1: usize = 256;
const MAX_CLAIM_IDENTIFIER_BYTES_V1: usize = 256;

pub fn submission_claim_set_schema_v1() -> Result<SchemaDescriptorV1, SchemaError> {
    SchemaDescriptorV1::new(
        "SubmissionClaimSetV1",
        1,
        vec![
            FieldDescriptorV1::new(
                1,
                "submission_id",
                TypeExprV1::AsciiText,
                vec![ConstraintExprV1::NoAdditional],
            )?,
            FieldDescriptorV1::new(
                2,
                "claim_count",
                TypeExprV1::Unsigned,
                vec![ConstraintExprV1::UnsignedRange {
                    minimum: 1,
                    maximum: u64::MAX,
                }],
            )?,
            FieldDescriptorV1::new(
                3,
                "entries",
                TypeExprV1::OrderedList(Box::new(TypeExprV1::Tuple(vec![
                    TypeExprV1::AsciiText,
                    TypeExprV1::ExactBytes(32),
                    TypeExprV1::ExactBytes(32),
                ]))),
                vec![ConstraintExprV1::BoundedLength {
                    minimum: 1,
                    maximum: u64::MAX,
                }],
            )?,
            FieldDescriptorV1::new(
                4,
                "digest",
                TypeExprV1::ExactBytes(32),
                vec![ConstraintExprV1::NoAdditional],
            )?,
        ],
        vec![],
    )
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ClaimEntryV1 {
    claim_id: Vec<u8>,
    normalized_proposition_hash: [u8; 32],
    claim_record_hash: [u8; 32],
}

impl ClaimEntryV1 {
    pub fn from_claim(claim: &ClaimV1) -> Self {
        Self {
            claim_id: claim.claim_id().render().into_bytes(),
            normalized_proposition_hash: *claim.normalized_proposition_hash(),
            claim_record_hash: *claim.record_hash(),
        }
    }

    pub fn from_stage0_carrier(
        claim_id: Vec<u8>,
        normalized_proposition_hash: [u8; 32],
        claim_record_hash: [u8; 32],
    ) -> Result<Self, SubmissionClaimSetError> {
        let entry = Self {
            claim_id,
            normalized_proposition_hash,
            claim_record_hash,
        };
        validate_entry(&entry)?;
        Ok(entry)
    }

    pub fn claim_id(&self) -> &[u8] {
        &self.claim_id
    }

    pub fn normalized_proposition_hash(&self) -> &[u8; 32] {
        &self.normalized_proposition_hash
    }

    pub fn claim_record_hash(&self) -> &[u8; 32] {
        &self.claim_record_hash
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionClaimSetV1 {
    submission_id: Vec<u8>,
    entries: Vec<ClaimEntryV1>,
    digest: [u8; 32],
    submission_ref: Option<SubmissionRefV1>,
    claim_subjects: Option<Vec<ClaimSubjectV1>>,
}

impl SubmissionClaimSetV1 {
    pub fn from_claims(
        submission_ref: SubmissionRefV1,
        claims: &[ClaimV1],
    ) -> Result<Self, SubmissionClaimSetError> {
        if claims.is_empty() {
            return Err(SubmissionClaimSetError::Empty);
        }
        if claims
            .iter()
            .any(|claim| claim.submission() != submission_ref)
        {
            return Err(SubmissionClaimSetError::CrossSubmissionClaim);
        }
        let mut projected: Vec<_> = claims
            .iter()
            .map(|claim| (ClaimEntryV1::from_claim(claim), claim.subject().clone()))
            .collect();
        projected.sort_by(|(left, _), (right, _)| {
            (left.normalized_proposition_hash, left.claim_id.as_slice())
                .cmp(&(right.normalized_proposition_hash, right.claim_id.as_slice()))
        });
        let (entries, claim_subjects) = projected.into_iter().unzip();
        Self::build(
            submission_ref.render().into_bytes(),
            entries,
            Some(submission_ref),
            Some(claim_subjects),
        )
    }

    pub fn from_stage0_carrier(
        submission_id: Vec<u8>,
        entries: Vec<ClaimEntryV1>,
    ) -> Result<Self, SubmissionClaimSetError> {
        Self::build(submission_id, entries, None, None)
    }

    fn build(
        submission_id: Vec<u8>,
        entries: Vec<ClaimEntryV1>,
        submission_ref: Option<SubmissionRefV1>,
        claim_subjects: Option<Vec<ClaimSubjectV1>>,
    ) -> Result<Self, SubmissionClaimSetError> {
        if entries.is_empty() {
            return Err(SubmissionClaimSetError::Empty);
        }

        let claim_count =
            u64::try_from(entries.len()).map_err(|_| SubmissionClaimSetError::LengthOverflow)?;
        validate_identifiers(&submission_id, &entries)?;
        validate_entries(&entries)?;
        let digest = compute_digest(&submission_id, claim_count, &entries)?;
        Ok(Self {
            submission_id,
            entries,
            digest,
            submission_ref,
            claim_subjects,
        })
    }

    pub fn decode_stage0_record(
        submission_id: Vec<u8>,
        claim_count: u64,
        entries: Vec<ClaimEntryV1>,
        expected_digest: [u8; 32],
    ) -> Result<Self, SubmissionClaimSetError> {
        if entries.is_empty() {
            return Err(SubmissionClaimSetError::Empty);
        }
        if u64::try_from(entries.len()).ok() != Some(claim_count) {
            return Err(SubmissionClaimSetError::CountMismatch);
        }
        validate_identifiers(&submission_id, &entries)?;
        validate_entries(&entries)?;
        let digest = compute_digest(&submission_id, claim_count, &entries)?;
        if digest != expected_digest {
            return Err(SubmissionClaimSetError::DigestMismatch);
        }
        Ok(Self {
            submission_id,
            entries,
            digest,
            submission_ref: None,
            claim_subjects: None,
        })
    }

    pub fn submission_id(&self) -> &[u8] {
        &self.submission_id
    }

    pub fn submission_ref(&self) -> Option<SubmissionRefV1> {
        self.submission_ref
    }

    pub fn is_authoritative(&self) -> bool {
        self.submission_ref.is_some() && self.claim_subjects.is_some()
    }

    pub fn claim_subjects(&self) -> Option<&[ClaimSubjectV1]> {
        self.claim_subjects.as_deref()
    }

    pub fn claim_count(&self) -> u64 {
        u64::try_from(self.entries.len()).expect("invariant: constructor checked claim count")
    }

    pub fn entries(&self) -> &[ClaimEntryV1] {
        &self.entries
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn canonical_digest_input(&self) -> Result<Vec<u8>, SubmissionClaimSetError> {
        encode_digest_input(&self.submission_id, self.claim_count(), &self.entries)
    }

    pub fn schema_value(&self) -> Result<CborValue, SubmissionClaimSetError> {
        let submission_id = std::str::from_utf8(&self.submission_id)
            .map_err(|_| SubmissionClaimSetError::NonAsciiIdentifier)?;
        if !submission_id.is_ascii() {
            return Err(SubmissionClaimSetError::NonAsciiIdentifier);
        }
        let entries = self
            .entries
            .iter()
            .map(|entry| {
                let claim_id = std::str::from_utf8(&entry.claim_id)
                    .map_err(|_| SubmissionClaimSetError::NonAsciiIdentifier)?;
                if !claim_id.is_ascii() {
                    return Err(SubmissionClaimSetError::NonAsciiIdentifier);
                }
                Ok(CborValue::Array(vec![
                    CborValue::Text(claim_id.to_owned()),
                    CborValue::Bytes(entry.normalized_proposition_hash.to_vec()),
                    CborValue::Bytes(entry.claim_record_hash.to_vec()),
                ]))
            })
            .collect::<Result<Vec<_>, SubmissionClaimSetError>>()?;
        Ok(CborValue::Array(vec![
            CborValue::Text(submission_id.to_owned()),
            CborValue::Unsigned(self.claim_count()),
            CborValue::Array(entries),
            CborValue::Bytes(self.digest.to_vec()),
        ]))
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SubmissionClaimSetError {
    #[error("SubmissionClaimSetV1 must contain at least one Claim")]
    Empty,
    #[error("SubmissionClaimSetV1 contains a Claim bound to another Submission")]
    CrossSubmissionClaim,
    #[error("SubmissionClaimSetV1 claim_count does not match its entries")]
    CountMismatch,
    #[error("SubmissionClaimSetV1 entries are not in canonical proposition-hash/claim-id order")]
    NonCanonicalOrder,
    #[error("SubmissionClaimSetV1 contains a duplicate claim_id")]
    DuplicateClaimId,
    #[error("SubmissionClaimSetV1 contains a duplicate normalized_proposition_hash")]
    DuplicateNormalizedProposition,
    #[error("SubmissionClaimSetV1 contains a duplicate claim_record_hash")]
    DuplicateClaimRecord,
    #[error("SubmissionClaimSetV1 digest does not match its exact canonical bytes")]
    DigestMismatch,
    #[error("SubmissionClaimSetV1 variable-field length exceeds unsigned-64 encoding")]
    LengthOverflow,
    #[error("SubmissionClaimSetV1 identifiers must use canonical ASCII bytes")]
    NonAsciiIdentifier,
    #[error("SubmissionClaimSetV1 submission_id must be non-empty")]
    EmptySubmissionId,
    #[error(
        "SubmissionClaimSetV1 submission_id exceeds the finite v1 limit of {MAX_SUBMISSION_IDENTIFIER_BYTES_V1} bytes"
    )]
    SubmissionIdTooLong,
    #[error("SubmissionClaimSetV1 Claim id must be non-empty")]
    EmptyClaimId,
    #[error(
        "SubmissionClaimSetV1 Claim id exceeds the finite v1 limit of {MAX_CLAIM_IDENTIFIER_BYTES_V1} bytes"
    )]
    ClaimIdTooLong,
    #[error("SubmissionClaimSetV1 contains an all-zero normalized proposition hash")]
    ZeroNormalizedProposition,
    #[error("SubmissionClaimSetV1 contains an all-zero Claim record hash")]
    ZeroClaimRecord,
}

fn validate_entries(entries: &[ClaimEntryV1]) -> Result<(), SubmissionClaimSetError> {
    let mut claim_ids = BTreeSet::new();
    let mut proposition_hashes = BTreeSet::new();
    let mut record_hashes = BTreeSet::new();

    for entry in entries {
        validate_entry(entry)?;
        if !claim_ids.insert(entry.claim_id.as_slice()) {
            return Err(SubmissionClaimSetError::DuplicateClaimId);
        }
        if !proposition_hashes.insert(entry.normalized_proposition_hash) {
            return Err(SubmissionClaimSetError::DuplicateNormalizedProposition);
        }
        if !record_hashes.insert(entry.claim_record_hash) {
            return Err(SubmissionClaimSetError::DuplicateClaimRecord);
        }
    }

    if entries.windows(2).any(|pair| {
        (
            pair[0].normalized_proposition_hash,
            pair[0].claim_id.as_slice(),
        ) >= (
            pair[1].normalized_proposition_hash,
            pair[1].claim_id.as_slice(),
        )
    }) {
        return Err(SubmissionClaimSetError::NonCanonicalOrder);
    }
    Ok(())
}

fn validate_identifiers(
    submission_id: &[u8],
    entries: &[ClaimEntryV1],
) -> Result<(), SubmissionClaimSetError> {
    if !submission_id.is_ascii() || entries.iter().any(|entry| !entry.claim_id.is_ascii()) {
        return Err(SubmissionClaimSetError::NonAsciiIdentifier);
    }
    if submission_id.is_empty() {
        return Err(SubmissionClaimSetError::EmptySubmissionId);
    }
    if submission_id.len() > MAX_SUBMISSION_IDENTIFIER_BYTES_V1 {
        return Err(SubmissionClaimSetError::SubmissionIdTooLong);
    }
    Ok(())
}

fn validate_entry(entry: &ClaimEntryV1) -> Result<(), SubmissionClaimSetError> {
    if !entry.claim_id.is_ascii() {
        return Err(SubmissionClaimSetError::NonAsciiIdentifier);
    }
    if entry.claim_id.is_empty() {
        return Err(SubmissionClaimSetError::EmptyClaimId);
    }
    if entry.claim_id.len() > MAX_CLAIM_IDENTIFIER_BYTES_V1 {
        return Err(SubmissionClaimSetError::ClaimIdTooLong);
    }
    if entry.normalized_proposition_hash == [0; 32] {
        return Err(SubmissionClaimSetError::ZeroNormalizedProposition);
    }
    if entry.claim_record_hash == [0; 32] {
        return Err(SubmissionClaimSetError::ZeroClaimRecord);
    }
    Ok(())
}

fn compute_digest(
    submission_id: &[u8],
    claim_count: u64,
    entries: &[ClaimEntryV1],
) -> Result<[u8; 32], SubmissionClaimSetError> {
    Ok(Sha256::digest(encode_digest_input(submission_id, claim_count, entries)?).into())
}

fn encode_digest_input(
    submission_id: &[u8],
    claim_count: u64,
    entries: &[ClaimEntryV1],
) -> Result<Vec<u8>, SubmissionClaimSetError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(SUBMISSION_CLAIM_SET_DOMAIN_V1);
    append_variable_field(&mut bytes, submission_id)?;
    bytes.extend_from_slice(&claim_count.to_be_bytes());
    for entry in entries {
        append_variable_field(&mut bytes, &entry.claim_id)?;
        bytes.extend_from_slice(&entry.normalized_proposition_hash);
        bytes.extend_from_slice(&entry.claim_record_hash);
    }
    Ok(bytes)
}

fn append_variable_field(
    output: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), SubmissionClaimSetError> {
    let length = u64::try_from(value.len()).map_err(|_| SubmissionClaimSetError::LengthOverflow)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}
