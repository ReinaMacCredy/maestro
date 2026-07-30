use maestro::domain::contract::proof::{
    ProofArtifactHashV1, Stage0ProofError, Stage0ProofGateKindV1, Stage0ProofGateV1,
    Stage0ProofManifestV1, Stage0ProofOutcomeV1, Stage0ProofResultV1,
    VERIFIED_NON_PROMOTING_RESULT_CLASS,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

fn artifact(path: &str, seed: u8) -> ProofArtifactHashV1 {
    ProofArtifactHashV1::new(path, [seed; 32]).expect("proof artifact hash")
}

fn gate(kind: Stage0ProofGateKindV1, result: Stage0ProofResultV1) -> Stage0ProofGateV1 {
    let is_external = kind == Stage0ProofGateKindV1::ExternalInputAuthorization;
    let result_class = if is_external {
        VERIFIED_NON_PROMOTING_RESULT_CLASS
    } else {
        "verified"
    };
    let result_sha256 = if is_external {
        Sha256::digest(VERIFIED_NON_PROMOTING_RESULT_CLASS.as_bytes()).into()
    } else {
        [kind.tag() as u8 + 48; 32]
    };
    Stage0ProofGateV1::new(
        kind,
        if is_external {
            vec![]
        } else {
            vec![artifact(
                &format!("contracts/vnext/stage0/{}/source.json", kind.name()),
                kind.tag() as u8,
            )]
        },
        vec![artifact(
            &format!("tools/vnext_contracts/stage0/{}/validate.py", kind.name()),
            kind.tag() as u8 + 16,
        )],
        if is_external {
            vec![]
        } else {
            vec![artifact(
                &format!("contracts/vnext/stage0/{}/input.json", kind.name()),
                kind.tag() as u8 + 32,
            )]
        },
        Stage0ProofOutcomeV1::new(result, result_class, result_sha256).expect("proof outcome"),
        if is_external {
            vec![]
        } else {
            vec![("rows".to_owned(), kind.tag())]
        },
    )
    .expect("proof gate")
}

fn passing_gates() -> Vec<Stage0ProofGateV1> {
    Stage0ProofGateKindV1::ALL
        .into_iter()
        .map(|kind| gate(kind, Stage0ProofResultV1::Passed))
        .collect()
}

fn parse_sha256(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64, "SHA-256 hexadecimal must be exact");
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16).expect("SHA-256 hexadecimal") as u8;
        let low = (pair[1] as char).to_digit(16).expect("SHA-256 hexadecimal") as u8;
        bytes[index] = (high << 4) | low;
    }
    assert_eq!(
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        value,
        "SHA-256 hexadecimal must be canonical lowercase"
    );
    bytes
}

fn emitted_artifacts(gate: &Value, field: &str) -> Vec<ProofArtifactHashV1> {
    gate[field]
        .as_array()
        .expect("proof artifact rows")
        .iter()
        .map(|row| {
            ProofArtifactHashV1::new(
                row["path"].as_str().expect("proof artifact path"),
                parse_sha256(row["sha256"].as_str().expect("proof artifact SHA-256")),
            )
            .expect("typed proof artifact")
        })
        .collect()
}

fn emitted_counts(gate: &Value) -> Vec<(String, u64)> {
    gate["semantic_counts"]
        .as_array()
        .expect("proof semantic counts")
        .iter()
        .map(|row| {
            (
                row["name"]
                    .as_str()
                    .expect("proof semantic count name")
                    .to_owned(),
                row["value"].as_u64().expect("proof semantic count value"),
            )
        })
        .collect()
}

#[test]
fn stage_zero_proof_manifest_requires_every_exact_passing_gate() {
    let manifest = Stage0ProofManifestV1::new(passing_gates()).expect("complete proof manifest");
    assert_eq!(manifest.gate_count(), Stage0ProofGateKindV1::ALL.len());
    assert_eq!(
        manifest
            .gates()
            .iter()
            .map(Stage0ProofGateV1::kind)
            .collect::<Vec<_>>(),
        Stage0ProofGateKindV1::ALL
    );
    assert!(!manifest.canonical_bytes().expect("proof bytes").is_empty());
}

#[test]
fn every_omitted_or_failed_stage_zero_gate_blocks_manifest_identity() {
    for kind in Stage0ProofGateKindV1::ALL {
        let mut omitted = passing_gates();
        omitted.retain(|gate| gate.kind() != kind);
        assert!(matches!(
            Stage0ProofManifestV1::new(omitted),
            Err(Stage0ProofError::IncompleteGateSet)
        ));

        let failed = passing_gates()
            .into_iter()
            .map(|candidate| {
                if candidate.kind() == kind {
                    gate(kind, Stage0ProofResultV1::Failed)
                } else {
                    candidate
                }
            })
            .collect();
        assert!(matches!(
            Stage0ProofManifestV1::new(failed),
            Err(Stage0ProofError::FailedGate(failed_kind)) if failed_kind == kind
        ));
    }
}

#[test]
fn proof_gate_artifacts_and_semantic_counts_are_canonically_sorted() {
    let gate = Stage0ProofGateV1::new(
        Stage0ProofGateKindV1::DecisionClosure,
        vec![artifact("z/source.json", 1), artifact("a/source.json", 2)],
        vec![artifact("z/validate.py", 3), artifact("a/validate.py", 4)],
        vec![artifact("z/input.json", 5), artifact("a/input.json", 6)],
        Stage0ProofOutcomeV1::new(Stage0ProofResultV1::Passed, "verified", [7; 32])
            .expect("proof outcome"),
        vec![("z_rows".to_owned(), 1), ("a_rows".to_owned(), 2)],
    )
    .expect("sorted proof gate");

    assert_eq!(gate.source_artifacts()[0].path(), "a/source.json");
    assert_eq!(gate.validator_artifacts()[0].path(), "a/validate.py");
    assert_eq!(gate.input_artifacts()[0].path(), "a/input.json");
    assert_eq!(gate.semantic_counts()[0], ("a_rows".to_owned(), 2));
}

#[test]
fn proof_gate_rejects_ambiguous_paths_and_missing_validator_sources() {
    assert!(matches!(
        ProofArtifactHashV1::new("../escape.json", [0; 32]),
        Err(Stage0ProofError::InvalidArtifactPath)
    ));
    assert!(matches!(
        Stage0ProofGateV1::new(
            Stage0ProofGateKindV1::DecisionClosure,
            vec![],
            vec![],
            vec![],
            Stage0ProofOutcomeV1::new(Stage0ProofResultV1::Passed, "verified", [0; 32])
                .expect("proof outcome"),
            vec![],
        ),
        Err(Stage0ProofError::MissingValidator(
            Stage0ProofGateKindV1::DecisionClosure
        ))
    ));
    assert!(matches!(
        Stage0ProofGateV1::new(
            Stage0ProofGateKindV1::ExternalInputAuthorization,
            vec![],
            vec![artifact("validator.py", 1)],
            vec![],
            Stage0ProofOutcomeV1::new(Stage0ProofResultV1::Passed, "verified", [0; 32])
                .expect("proof outcome"),
            vec![],
        ),
        Err(Stage0ProofError::ExternalInputPromotionClass)
    ));
}

#[test]
fn emitted_stage_zero_proof_reconstructs_with_rust_contract_types() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("contracts/vnext/stage0/proof-matrix");
    let json_path = root.join("stage0-proof-manifest.v1.json");
    let cbor_path = root.join("stage0-proof-manifest.v1.cbor");
    assert!(
        json_path.is_file() && cbor_path.is_file(),
        "Stage 0 is incomplete: emitted proof-manifest JSON and CBOR are required"
    );
    let document: Value = serde_json::from_slice(
        &fs::read(&json_path).expect("read emitted Stage0ProofManifest JSON"),
    )
    .expect("parse emitted Stage0ProofManifest JSON");
    let gates = document["gates"]
        .as_array()
        .expect("emitted Stage0ProofManifest gates")
        .iter()
        .map(|gate| {
            let kind =
                Stage0ProofGateKindV1::try_from(gate["tag"].as_u64().expect("proof gate tag"))
                    .expect("typed proof gate kind");
            assert_eq!(
                gate["name"].as_str().expect("proof gate name"),
                kind.name(),
                "proof gate tag/name pairing"
            );
            Stage0ProofGateV1::new(
                kind,
                emitted_artifacts(gate, "source_artifacts"),
                emitted_artifacts(gate, "validator_artifacts"),
                emitted_artifacts(gate, "input_artifacts"),
                Stage0ProofOutcomeV1::new(
                    Stage0ProofResultV1::Passed,
                    gate["result_class"].as_str().expect("proof result class"),
                    parse_sha256(
                        gate["result_sha256"]
                            .as_str()
                            .expect("proof result SHA-256"),
                    ),
                )
                .expect("typed proof outcome"),
                emitted_counts(gate),
            )
            .expect("typed proof gate")
        })
        .collect();
    let manifest = Stage0ProofManifestV1::new(gates).expect("typed emitted proof manifest");
    let encoded = fs::read(cbor_path).expect("read emitted Stage0ProofManifest CBOR");
    assert_eq!(
        manifest.canonical_bytes().expect("typed proof bytes"),
        encoded,
        "Rust typed reconstruction must reproduce Python/Ruby canonical CBOR"
    );
    assert_eq!(
        manifest.manifest_id().render(),
        document["identity"]
            .as_str()
            .expect("emitted proof identity"),
        "Rust typed reconstruction must reproduce the proof manifest identity"
    );
    let encoded_sha256: [u8; 32] = Sha256::digest(&encoded).into();
    assert_eq!(
        encoded_sha256,
        parse_sha256(
            document["canonical_cbor_sha256"]
                .as_str()
                .expect("emitted proof CBOR SHA-256")
        ),
        "Rust must reproduce the emitted proof CBOR receipt"
    );
}
