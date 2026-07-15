use maestro::domain::vnext::contract::component_kind::ContractComponentKindV1;
use maestro::domain::vnext::contract::decision_closure::{
    DecisionClosureError, DecisionClosureV1, DecisionConsequenceClassificationV1,
    DecisionMaterializationSourceV1, DerivedDecisionEffectStatusV1,
    ExternalDecisionClosureRecordV1, ExternalDesignAuthorityClosureV1,
    ExternalLineageDispositionV1, IgnoredUnilateralClaimV1, RawExternalDecisionRecordV1,
    RequiredDecisionMaterializationV1, TerminalDecisionStatusV1,
};
use serde_json::Value;
use std::fs;

fn raw(
    id: &str,
    status: TerminalDecisionStatusV1,
    supersedes: Vec<&str>,
    superseded_by: Vec<&str>,
) -> RawExternalDecisionRecordV1 {
    RawExternalDecisionRecordV1::new(
        id,
        status,
        format!("raw:{id}").into_bytes(),
        format!("body:{id}").into_bytes(),
        supersedes.into_iter().map(str::to_owned).collect(),
        superseded_by.into_iter().map(str::to_owned).collect(),
    )
    .expect("valid raw external record")
}

fn materialization(
    records: &[ExternalDecisionClosureRecordV1],
) -> RequiredDecisionMaterializationV1 {
    RequiredDecisionMaterializationV1::new(
        "maestro.vnext.candidate-contract.normative-inputs.v1/test",
        ContractComponentKindV1::NormativeInputs,
        records
            .iter()
            .map(|record| {
                DecisionMaterializationSourceV1::new(
                    record.raw().id(),
                    *record.raw().raw_body_hash(),
                )
                .expect("source")
            })
            .collect(),
    )
    .expect("materialization")
}

fn digest(value: &str) -> [u8; 32] {
    let value = value.strip_prefix("sha256:").unwrap_or(value);
    assert_eq!(value.len(), 64);
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = u8::from_str_radix(std::str::from_utf8(pair).expect("hex pair"), 16)
            .expect("lowercase hex digest");
    }
    bytes
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("string array")
        .iter()
        .map(|item| item.as_str().expect("string").to_owned())
        .collect()
}

fn rendered(bytes: &[u8; 32]) -> String {
    let mut value = String::from("sha256:");
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut value, "{byte:02x}").expect("write digest");
    }
    value
}

#[test]
fn emitted_decision_closures_and_materializations_reconstruct_exactly_in_rust() {
    let external_path =
        "contracts/vnext/stage0/decision-closure/external-design-authority-closure.v1.json";
    let decision_path = "contracts/vnext/stage0/decision-closure/decision-closure.v1.json";
    let external: Value =
        serde_json::from_slice(&fs::read(external_path).expect("external closure JSON"))
            .expect("external closure document");
    let decision: Value =
        serde_json::from_slice(&fs::read(decision_path).expect("Decision closure JSON"))
            .expect("Decision closure document");

    let records = external["records"]
        .as_array()
        .expect("external records")
        .iter()
        .map(|record| {
            let status = TerminalDecisionStatusV1::try_from(
                record["terminal_status"].as_str().expect("terminal status"),
            )
            .expect("terminal Decision status");
            let raw = RawExternalDecisionRecordV1::from_committed_hashes(
                record["id"].as_str().expect("Decision id"),
                status,
                hex_bytes(
                    record["raw_record_bytes"]["bytes"]
                        .as_str()
                        .expect("raw bytes"),
                ),
                digest(
                    record["raw_record_sha256"]
                        .as_str()
                        .expect("raw record hash"),
                ),
                digest(record["raw_body_sha256"].as_str().expect("raw body hash")),
                strings(&record["raw_supersedes"]),
                strings(&record["raw_superseded_by"]),
            )
            .expect("committed raw Decision");
            let normalized_successor = record["normalized_successor"].as_str().map(str::to_owned);
            let lineage = ExternalLineageDispositionV1::from_literals(
                record["external_authoring_disposition"]
                    .as_str()
                    .expect("lineage disposition"),
                normalized_successor,
            )
            .expect("lineage disposition");
            let rationale = record["rationale_disposition"].as_str().map(str::to_owned);
            let consequence = DecisionConsequenceClassificationV1::from_literals(
                record["consequence_classification"]
                    .as_str()
                    .expect("consequence classification"),
                rationale,
            )
            .expect("consequence classification");
            ExternalDecisionClosureRecordV1::new(raw, lineage, consequence)
                .expect("external Decision record")
        })
        .collect::<Vec<_>>();
    let materializations = external["materializations"]
        .as_array()
        .expect("materializations")
        .iter()
        .map(|row| {
            let sources = row["decision_sources"]
                .as_array()
                .expect("materialization sources")
                .iter()
                .map(|source| {
                    DecisionMaterializationSourceV1::new(
                        source["id"].as_str().expect("source Decision id"),
                        digest(source["body_sha256"].as_str().expect("source body hash")),
                    )
                    .expect("materialization source")
                })
                .collect();
            let materialization = RequiredDecisionMaterializationV1::new(
                row["artifact_id"].as_str().expect("artifact id"),
                ContractComponentKindV1::try_from(
                    row["component_kind_tag"].as_u64().expect("component kind"),
                )
                .expect("known component kind"),
                sources,
            )
            .expect("required materialization");
            assert_eq!(
                materialization.materialization_id().render(),
                format!("sha256:{}", row["id"].as_str().expect("materialization id"))
            );
            materialization
        })
        .collect::<Vec<_>>();
    let ignored = external["lineage"]["ignored_unilateral_claims"]
        .as_array()
        .expect("ignored unilateral claims")
        .iter()
        .map(|row| {
            IgnoredUnilateralClaimV1::new(
                row["source"].as_str().expect("claim source"),
                row["claimed_predecessor"]
                    .as_str()
                    .expect("claimed predecessor"),
            )
            .expect("ignored unilateral claim")
        })
        .collect();
    let recognized = strings(&external["lineage"]["recognized_external_composite_heads"]);
    let expected_ids = records
        .iter()
        .map(|record| record.raw().id().to_owned())
        .collect::<Vec<_>>();
    let external_closure = ExternalDesignAuthorityClosureV1::new(
        records,
        materializations,
        &expected_ids,
        ignored,
        recognized,
    )
    .expect("external authority closure");

    assert_eq!(
        rendered(external_closure.external_closure_id()),
        external["identity"].as_str().expect("external identity")
    );
    assert_eq!(
        external_closure.canonical_bytes().expect("external CBOR"),
        fs::read(
            "contracts/vnext/stage0/decision-closure/external-design-authority-closure.v1.cbor"
        )
        .expect("external staged CBOR")
    );
    let decision_closure =
        DecisionClosureV1::from_external(&external_closure).expect("Decision closure");
    assert_eq!(
        decision_closure.closure_id().render(),
        decision["identity"].as_str().expect("Decision identity")
    );
    assert_eq!(
        decision_closure.canonical_bytes().expect("Decision CBOR"),
        fs::read("contracts/vnext/stage0/decision-closure/decision-closure.v1.cbor")
            .expect("Decision staged CBOR")
    );
}

fn hex_bytes(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("hex pair"), 16).expect("hex byte")
        })
        .collect()
}

#[test]
fn composite_external_authoring_is_preserved_but_never_promoted() {
    let first = ExternalDecisionClosureRecordV1::new(
        raw(
            "dec-a",
            TerminalDecisionStatusV1::Superseded,
            vec![],
            vec!["dec-c"],
        ),
        ExternalLineageDispositionV1::CompositeExternalAuthoring,
        DecisionConsequenceClassificationV1::Material,
    )
    .expect("first record");
    let second = ExternalDecisionClosureRecordV1::new(
        raw(
            "dec-b",
            TerminalDecisionStatusV1::Superseded,
            vec![],
            vec!["dec-c"],
        ),
        ExternalLineageDispositionV1::CompositeExternalAuthoring,
        DecisionConsequenceClassificationV1::Material,
    )
    .expect("second record");
    let head = ExternalDecisionClosureRecordV1::new(
        raw(
            "dec-c",
            TerminalDecisionStatusV1::Locked,
            vec!["dec-a", "dec-b"],
            vec![],
        ),
        ExternalLineageDispositionV1::CompositeExternalAuthoring,
        DecisionConsequenceClassificationV1::Material,
    )
    .expect("composite head");
    let records = vec![first, second, head];
    let closure = ExternalDesignAuthorityClosureV1::new(
        records.clone(),
        vec![materialization(&records)],
        &records
            .iter()
            .map(|record| record.raw().id().to_owned())
            .collect::<Vec<_>>(),
        vec![],
        vec![],
    )
    .expect("external closure");

    let canonical = DecisionClosureV1::from_external(&closure).expect("canonical closure");

    assert!(canonical.normalized_successor_edges().is_empty());
    assert_eq!(
        canonical.derived_effect_status("dec-a"),
        Some(DerivedDecisionEffectStatusV1::SupersededButEffectLive)
    );
    assert_eq!(
        canonical.derived_effect_status("dec-c"),
        Some(DerivedDecisionEffectStatusV1::Unapplied)
    );
    assert_eq!(
        canonical
            .root_binding_requirements()
            .materialization_ids()
            .len(),
        1
    );
}

#[test]
fn missing_or_stale_materialization_fails_closed() {
    let record = ExternalDecisionClosureRecordV1::new(
        raw("dec-a", TerminalDecisionStatusV1::Locked, vec![], vec![]),
        ExternalLineageDispositionV1::None,
        DecisionConsequenceClassificationV1::Material,
    )
    .expect("record");
    let expected = vec!["dec-a".to_owned()];
    assert!(matches!(
        ExternalDesignAuthorityClosureV1::new(
            vec![record.clone()],
            vec![],
            &expected,
            vec![],
            vec![],
        ),
        Err(DecisionClosureError::MissingMaterialization)
    ));

    let stale = RequiredDecisionMaterializationV1::new(
        "maestro.vnext.candidate-contract.normative-inputs.v1/test",
        ContractComponentKindV1::NormativeInputs,
        vec![DecisionMaterializationSourceV1::new("dec-a", [0; 32]).expect("stale source")],
    )
    .expect("stale materialization shape");
    assert!(matches!(
        ExternalDesignAuthorityClosureV1::new(vec![record], vec![stale], &expected, vec![], vec![],),
        Err(DecisionClosureError::StaleMaterialization)
    ));
}

#[test]
fn open_status_and_reordered_input_are_rejected() {
    assert!(matches!(
        TerminalDecisionStatusV1::try_from("open"),
        Err(DecisionClosureError::OpenDecision)
    ));

    let first = ExternalDecisionClosureRecordV1::new(
        raw("dec-a", TerminalDecisionStatusV1::Locked, vec![], vec![]),
        ExternalLineageDispositionV1::None,
        DecisionConsequenceClassificationV1::RationaleOnly {
            disposition: "methodology-only".to_owned(),
        },
    )
    .expect("first record");
    let second = ExternalDecisionClosureRecordV1::new(
        raw("dec-b", TerminalDecisionStatusV1::Locked, vec![], vec![]),
        ExternalLineageDispositionV1::None,
        DecisionConsequenceClassificationV1::RationaleOnly {
            disposition: "grouping-only".to_owned(),
        },
    )
    .expect("second record");
    assert!(matches!(
        ExternalDesignAuthorityClosureV1::new(
            vec![second, first],
            vec![],
            &["dec-a".to_owned(), "dec-b".to_owned()],
            vec![],
            vec![],
        ),
        Err(DecisionClosureError::RecordsNotStrictlySorted)
    ));
}
