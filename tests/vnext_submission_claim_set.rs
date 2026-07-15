use maestro::domain::vnext::contract::submission_claim::{
    ClaimEntryV1, SUBMISSION_CLAIM_SET_DOMAIN_V1, SubmissionClaimSetError, SubmissionClaimSetV1,
    submission_claim_set_schema_v1,
};
use maestro::domain::vnext::identity::{SchemaClosureV1, SchemaError};
use maestro::foundation::core::deterministic_cbor::CborValue;
use sha2::{Digest, Sha256};

fn hash(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn entry(id: &str, proposition: u8, record: u8) -> ClaimEntryV1 {
    ClaimEntryV1::from_stage0_carrier(id.as_bytes().to_vec(), hash(proposition), hash(record))
        .unwrap()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn freezes_one_and_many_claim_vectors() {
    let one = SubmissionClaimSetV1::from_stage0_carrier(
        b"submission-1".to_vec(),
        vec![entry("claim-a", 1, 11)],
    )
    .expect("one Claim is valid");
    assert_eq!(one.claim_count(), 1);
    assert_eq!(
        hex_encode(one.digest()),
        "eab9b89f7a770711a12e4c64ecc6ec2d700ce7adda2b4bfd6ca80d9a6acd0580"
    );

    let many = SubmissionClaimSetV1::from_stage0_carrier(
        b"submission-2".to_vec(),
        vec![
            entry("claim-z", 1, 12),
            entry("claim-a", 2, 13),
            entry("claim-b", 3, 14),
        ],
    )
    .expect("many Claims are valid");
    assert_eq!(many.claim_count(), 3);
    assert_eq!(
        hex_encode(many.digest()),
        "5b253d94e7e1cebb15c238d3d03b6d3a1bef0ed52b2058a7c3619bad92768a3d"
    );
    assert!(
        many.canonical_digest_input()
            .unwrap()
            .starts_with(SUBMISSION_CLAIM_SET_DOMAIN_V1)
    );
}

#[test]
fn rejects_every_malformed_set_product() {
    assert_eq!(
        SubmissionClaimSetV1::from_stage0_carrier(b"submission".to_vec(), vec![]).unwrap_err(),
        SubmissionClaimSetError::Empty
    );
    assert_eq!(
        SubmissionClaimSetV1::from_stage0_carrier(
            b"submission".to_vec(),
            vec![entry("claim-b", 2, 11), entry("claim-a", 1, 12)],
        )
        .unwrap_err(),
        SubmissionClaimSetError::NonCanonicalOrder
    );
    assert_eq!(
        SubmissionClaimSetV1::from_stage0_carrier(
            b"submission".to_vec(),
            vec![entry("claim-a", 1, 11), entry("claim-a", 2, 12)],
        )
        .unwrap_err(),
        SubmissionClaimSetError::DuplicateClaimId
    );
    assert_eq!(
        SubmissionClaimSetV1::from_stage0_carrier(
            b"submission".to_vec(),
            vec![entry("claim-a", 1, 11), entry("claim-b", 1, 12)],
        )
        .unwrap_err(),
        SubmissionClaimSetError::DuplicateNormalizedProposition
    );
    assert_eq!(
        SubmissionClaimSetV1::from_stage0_carrier(
            b"submission".to_vec(),
            vec![entry("claim-a", 1, 11), entry("claim-b", 2, 11)],
        )
        .unwrap_err(),
        SubmissionClaimSetError::DuplicateClaimRecord
    );
    assert_eq!(
        SubmissionClaimSetV1::from_stage0_carrier(vec![0xff], vec![entry("claim-a", 1, 11)])
            .unwrap_err(),
        SubmissionClaimSetError::NonAsciiIdentifier
    );
    assert_eq!(
        ClaimEntryV1::from_stage0_carrier(vec![0xff], hash(1), hash(11)).unwrap_err(),
        SubmissionClaimSetError::NonAsciiIdentifier
    );
    assert_eq!(
        SubmissionClaimSetV1::from_stage0_carrier(vec![], vec![entry("claim-a", 1, 11)],)
            .unwrap_err(),
        SubmissionClaimSetError::EmptySubmissionId
    );
    assert_eq!(
        SubmissionClaimSetV1::from_stage0_carrier(vec![b's'; 257], vec![entry("claim-a", 1, 11)],)
            .unwrap_err(),
        SubmissionClaimSetError::SubmissionIdTooLong
    );
    assert_eq!(
        ClaimEntryV1::from_stage0_carrier(vec![], hash(1), hash(11)).unwrap_err(),
        SubmissionClaimSetError::EmptyClaimId
    );
    assert_eq!(
        ClaimEntryV1::from_stage0_carrier(vec![b'c'; 257], hash(1), hash(11)).unwrap_err(),
        SubmissionClaimSetError::ClaimIdTooLong
    );
    assert_eq!(
        ClaimEntryV1::from_stage0_carrier(b"claim".to_vec(), [0; 32], hash(11)).unwrap_err(),
        SubmissionClaimSetError::ZeroNormalizedProposition
    );
    assert_eq!(
        ClaimEntryV1::from_stage0_carrier(b"claim".to_vec(), hash(1), [0; 32]).unwrap_err(),
        SubmissionClaimSetError::ZeroClaimRecord
    );

    let valid = SubmissionClaimSetV1::from_stage0_carrier(
        b"submission".to_vec(),
        vec![entry("claim-a", 1, 11), entry("claim-b", 2, 12)],
    )
    .unwrap();
    assert_eq!(
        SubmissionClaimSetV1::decode_stage0_record(
            valid.submission_id().to_vec(),
            3,
            valid.entries().to_vec(),
            *valid.digest(),
        )
        .unwrap_err(),
        SubmissionClaimSetError::CountMismatch
    );
    let mut wrong_digest = *valid.digest();
    wrong_digest[0] ^= 1;
    assert_eq!(
        SubmissionClaimSetV1::decode_stage0_record(
            valid.submission_id().to_vec(),
            valid.claim_count(),
            valid.entries().to_vec(),
            wrong_digest,
        )
        .unwrap_err(),
        SubmissionClaimSetError::DigestMismatch
    );
}

#[test]
fn reference_encoder_matches_the_rust_encoder() {
    let entries = vec![entry("claim-a", 1, 11), entry("claim-b", 2, 12)];
    let claim_set =
        SubmissionClaimSetV1::from_stage0_carrier(b"submission".to_vec(), entries.clone()).unwrap();
    assert!(!claim_set.is_authoritative());

    let mut reference = Vec::from(SUBMISSION_CLAIM_SET_DOMAIN_V1);
    reference.extend_from_slice(&(b"submission".len() as u64).to_be_bytes());
    reference.extend_from_slice(b"submission");
    reference.extend_from_slice(&(entries.len() as u64).to_be_bytes());
    for entry in entries {
        reference.extend_from_slice(&(entry.claim_id().len() as u64).to_be_bytes());
        reference.extend_from_slice(entry.claim_id());
        reference.extend_from_slice(entry.normalized_proposition_hash());
        reference.extend_from_slice(entry.claim_record_hash());
    }
    assert_eq!(claim_set.canonical_digest_input().unwrap(), reference);
    assert_eq!(
        claim_set.digest().as_slice(),
        Sha256::digest(reference).as_slice()
    );
}

#[test]
fn freezes_the_schema_identity_and_rejects_shape_mutants() {
    let descriptor = submission_claim_set_schema_v1().expect("schema descriptor");
    let closure = SchemaClosureV1::new(vec![descriptor]).expect("schema closure");
    let schema_id = closure
        .schema_id("SubmissionClaimSetV1", 1)
        .expect("schema identity");
    assert_eq!(
        schema_id.to_string(),
        "sha256:f70420ba6a0b35be60cc720536671ac92b98097ed4bc3d37ddcca50ec28cf9a4"
    );

    let claim_set = SubmissionClaimSetV1::from_stage0_carrier(
        b"submission".to_vec(),
        vec![entry("claim-a", 1, 11)],
    )
    .unwrap();
    let value = claim_set.schema_value().unwrap();
    closure
        .validate_value(schema_id, &value)
        .expect("valid schema value");

    let CborValue::Array(mut unknown_field) = value.clone() else {
        unreachable!()
    };
    unknown_field.push(CborValue::Unsigned(1));
    assert!(matches!(
        closure.validate_value(schema_id, &CborValue::Array(unknown_field)),
        Err(SchemaError::SchemaValueShapeMismatch)
    ));

    let CborValue::Array(mut zero_entries) = value else {
        unreachable!()
    };
    zero_entries[1] = CborValue::Unsigned(0);
    zero_entries[2] = CborValue::Array(vec![]);
    assert!(matches!(
        closure.validate_value(schema_id, &CborValue::Array(zero_entries)),
        Err(SchemaError::UnsignedRangeViolation) | Err(SchemaError::BoundedLengthViolation)
    ));
}
