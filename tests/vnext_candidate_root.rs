use maestro::domain::vnext::contract::assembly::{
    candidate_root_schema_closure_v1, facet_schema_id_v1, finalization_facet_kinds_v1,
    finalization_input_schema_id_v1, fixture_facet_value_v1, normative_inputs_schema_id_v1,
};
use maestro::domain::vnext::contract::component::CandidateContractComponentV1;
use maestro::domain::vnext::contract::component_kind::ContractComponentKindV1;
use maestro::domain::vnext::contract::decision_closure::{
    DecisionClosureV1, DecisionConsequenceClassificationV1, DecisionMaterializationSourceV1,
    ExternalDecisionClosureRecordV1, ExternalDesignAuthorityClosureV1,
    ExternalLineageDispositionV1, RawExternalDecisionRecordV1, RequiredDecisionMaterializationV1,
    TerminalDecisionStatusV1,
};
use maestro::domain::vnext::contract::finalization::{
    DesignBasisV1, DesignFinalizationManifestV1, FinalizationInputKindV1, PinnedFinalizationInputV1,
};
use maestro::domain::vnext::contract::handoff::CanonicalBuildHandoffV1;
use maestro::domain::vnext::contract::materialization::{
    DecisionMaterializationResolutionV1, MaterializationBaseV1, MaterializationError,
};
use maestro::domain::vnext::contract::provenance::ComponentProvenanceV1;
use maestro::domain::vnext::contract::root::{CandidateContractRootV1, ContractRootError};
use maestro::domain::vnext::identity::{
    ContractComponentIdV1, DecisionClosureIdV1, DecisionMaterializationIdV1,
    DecisionResolutionIdV1, DesignRevisionIdV1, DesignSourceBindingIdV1, SchemaClosureV1,
    SchemaIdV1, decision_closure_identity, decision_materialization_identity,
    design_revision_identity, design_source_binding_identity,
};
use maestro::foundation::core::deterministic_cbor::CborValue;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn closure_id(seed: u64) -> maestro::domain::vnext::identity::DecisionClosureIdV1 {
    decision_closure_identity(&CborValue::Array(vec![CborValue::Unsigned(seed)]))
        .expect("closure identity")
}

fn materialization_id(seed: u64) -> maestro::domain::vnext::identity::DecisionMaterializationIdV1 {
    decision_materialization_identity(&CborValue::Array(vec![CborValue::Unsigned(seed)]))
        .expect("materialization identity")
}

fn fixture_decision_closure(materialization_count: usize) -> DecisionClosureV1 {
    let records = (0..materialization_count)
        .map(|index| {
            let identifier = format!("decision-{index:04}");
            let raw = RawExternalDecisionRecordV1::new(
                &identifier,
                TerminalDecisionStatusV1::Locked,
                format!("raw:{identifier}").into_bytes(),
                format!("body:{identifier}").into_bytes(),
                vec![],
                vec![],
            )
            .expect("raw fixture Decision");
            ExternalDecisionClosureRecordV1::new(
                raw,
                ExternalLineageDispositionV1::None,
                DecisionConsequenceClassificationV1::Material,
            )
            .expect("fixture Decision")
        })
        .collect::<Vec<_>>();
    let materializations = records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            RequiredDecisionMaterializationV1::new(
                format!("maestro.vnext.candidate-contract.normative-inputs.v1/{index:04}"),
                ContractComponentKindV1::NormativeInputs,
                vec![
                    DecisionMaterializationSourceV1::new(
                        record.raw().id(),
                        *record.raw().raw_body_hash(),
                    )
                    .expect("fixture materialization source"),
                ],
            )
            .expect("fixture materialization")
        })
        .collect::<Vec<_>>();
    let expected_ids = records
        .iter()
        .map(|record| record.raw().id().to_owned())
        .collect::<Vec<_>>();
    let external = ExternalDesignAuthorityClosureV1::new(
        records,
        materializations,
        &expected_ids,
        vec![],
        vec![],
    )
    .expect("fixture external closure");
    DecisionClosureV1::from_external(&external).expect("fixture Decision closure")
}

fn fixture_finalization(
    schemas: &SchemaClosureV1,
    design_revision_id: DesignRevisionIdV1,
    decision_closure_id: DecisionClosureIdV1,
    root: &CandidateContractRootV1,
) -> DesignFinalizationManifestV1 {
    let stage_proof_binding_fields = root
        .components()
        .iter()
        .find(|component| component.kind() == ContractComponentKindV1::StageProofMatrix)
        .map(|component| {
            cbor_array(component.value(), "StageProofMatrix facet value")[1..].to_vec()
        })
        .expect("StageProofMatrix facet component");
    let inputs = FinalizationInputKindV1::ALL
        .into_iter()
        .map(|kind| {
            let owner_facet_ids = finalization_facet_kinds_v1(kind)
                .iter()
                .map(|facet_kind| {
                    *root
                        .components()
                        .iter()
                        .find(|component| component.kind() == *facet_kind)
                        .expect("owner facet component")
                        .component_id()
                })
                .collect::<Vec<_>>();
            let mut fields = vec![
                CborValue::Unsigned(1),
                CborValue::Unsigned(kind.tag()),
                CborValue::Bytes(design_revision_id.as_bytes().to_vec()),
                CborValue::Bytes(decision_closure_id.as_bytes().to_vec()),
                CborValue::Bytes(root.root_id().as_bytes().to_vec()),
                CborValue::Array(
                    owner_facet_ids
                        .iter()
                        .map(|identifier| CborValue::Bytes(identifier.as_bytes().to_vec()))
                        .collect(),
                ),
            ];
            if kind == FinalizationInputKindV1::StageProofMatrix {
                fields.extend(stage_proof_binding_fields.clone());
            }
            let value = CborValue::Array(fields);
            match kind {
                FinalizationInputKindV1::ClosureRequirement => {
                    PinnedFinalizationInputV1::closure_requirement(schemas, value)
                }
                FinalizationInputKindV1::DeterministicSynthesis => {
                    PinnedFinalizationInputV1::deterministic_synthesis(schemas, value)
                }
                FinalizationInputKindV1::ScopeAndExclusions => {
                    PinnedFinalizationInputV1::scope_and_exclusions(schemas, value)
                }
                FinalizationInputKindV1::CapabilityCensusAndJourneys => {
                    PinnedFinalizationInputV1::capability_census_and_journeys(schemas, value)
                }
                FinalizationInputKindV1::MigrationRollbackRemoval => {
                    PinnedFinalizationInputV1::migration_rollback_removal(schemas, value)
                }
                FinalizationInputKindV1::StageProofMatrix => {
                    PinnedFinalizationInputV1::stage_proof_matrix(schemas, value)
                }
                FinalizationInputKindV1::ReviewEvidence => {
                    PinnedFinalizationInputV1::review_evidence(schemas, value)
                }
                FinalizationInputKindV1::EdgeSweepEvidence => {
                    PinnedFinalizationInputV1::edge_sweep_evidence(schemas, value)
                }
                FinalizationInputKindV1::RiskRecovery => {
                    PinnedFinalizationInputV1::risk_recovery(schemas, value)
                }
                FinalizationInputKindV1::FreshnessReferences => {
                    PinnedFinalizationInputV1::freshness_references(schemas, value)
                }
                FinalizationInputKindV1::CanonicalizationPolicy => {
                    PinnedFinalizationInputV1::canonicalization_policy(schemas, value)
                }
            }
            .expect("pinned finalization input")
        })
        .collect();
    DesignFinalizationManifestV1::new(
        schemas,
        DesignBasisV1::design_revision(design_revision_id),
        decision_closure_id,
        root,
        inputs,
    )
    .expect("design finalization manifest")
}

#[test]
fn initial_external_design_closure_never_fabricates_a_prior_root() {
    let decision_closure_id = closure_id(1);
    let base = MaterializationBaseV1::initial_external_design_closure(decision_closure_id);
    let first = DecisionMaterializationResolutionV1::new(
        decision_closure_id,
        base.clone(),
        materialization_id(2),
    )
    .expect("initial materialization resolution");
    let second =
        DecisionMaterializationResolutionV1::new(decision_closure_id, base, materialization_id(2))
            .expect("same initial materialization resolution");

    assert_eq!(
        first.materialization_base().initial_decision_closure_id(),
        Some(&decision_closure_id)
    );
    assert_eq!(first.materialization_base().prior_root_id(), None);
    assert_eq!(first.resolution_id(), second.resolution_id());
    assert_eq!(
        first.canonical_bytes().expect("canonical bytes"),
        second.canonical_bytes().expect("canonical bytes")
    );
}

#[test]
fn initial_base_must_be_the_live_decision_closure() {
    let result = DecisionMaterializationResolutionV1::new(
        closure_id(1),
        MaterializationBaseV1::initial_external_design_closure(closure_id(2)),
        materialization_id(3),
    );

    assert_eq!(
        result,
        Err(MaterializationError::InitialBaseDoesNotMatchDecisionClosure)
    );
}

#[test]
fn descriptor_backed_stage_zero_assembly_constructs_root_manifest_and_handoff() {
    let schemas = candidate_root_schema_closure_v1().expect("candidate root schema closure");
    let expected_descriptor_count =
        ContractComponentKindV1::ALL.len() + FinalizationInputKindV1::ALL.len();
    assert_eq!(schemas.descriptors().len(), expected_descriptor_count);
    assert_eq!(
        normative_inputs_schema_id_v1(&schemas)
            .expect("normative schema")
            .render(),
        "sha256:a261fc3a548da9ab6dfd3d64541d7844bb8f20cff83ac8800d50e3865236e8e6"
    );
    assert_eq!(
        facet_schema_id_v1(&schemas, ContractComponentKindV1::LiteralSchemaClosure)
            .expect("literal schema closure facet")
            .render(),
        "sha256:51dc8914f2d4274e82cacefe26dd1b8cb63b1b9d1f5a0358bbeb35528a1c9fec"
    );
    assert_eq!(
        facet_schema_id_v1(
            &schemas,
            ContractComponentKindV1::PublicationAuthorityRequirement
        )
        .expect("publication authority facet")
        .render(),
        "sha256:0890cab87770499bfe3872046032b066907cb0b805c64474b6f749bf467dadff"
    );
    assert_eq!(
        finalization_input_schema_id_v1(&schemas, FinalizationInputKindV1::ClosureRequirement)
            .expect("closure requirement schema")
            .render(),
        "sha256:9611d790c6b22231758e977e812d154bd33ef550d9b28cc0e9928a04d6b2c28f"
    );

    let decision_closure = fixture_decision_closure(3);
    let decision_closure_id = *decision_closure.closure_id();
    let design_revision_id = design_revision_identity(&CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Bytes(decision_closure_id.as_bytes().to_vec()),
    ]))
    .expect("design revision");
    let normative_schema_id = normative_inputs_schema_id_v1(&schemas).expect("normative schema");
    let fixture_materialization_count = decision_closure.materializations().len() as u64;
    let mut components = Vec::new();
    for (index, required_materialization) in decision_closure.materializations().iter().enumerate()
    {
        let materialization_id = *required_materialization.materialization_id();
        let slot_tag = u64::try_from(index + 1).expect("fixture slot tag");
        let source_binding_id = design_source_binding_identity(&CborValue::Array(vec![
            CborValue::Unsigned(slot_tag),
            CborValue::Bytes(materialization_id.as_bytes().to_vec()),
        ]))
        .expect("normative source binding");
        let value = CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(materialization_id.as_bytes().to_vec()),
            CborValue::Array(
                required_materialization
                    .sources()
                    .iter()
                    .map(|source| {
                        CborValue::Array(vec![
                            CborValue::Text(source.decision_id().to_owned()),
                            CborValue::Bytes(source.raw_body_hash().to_vec()),
                        ])
                    })
                    .collect(),
            ),
        ]);
        components.push(
            CandidateContractComponentV1::new(
                &schemas,
                ContractComponentKindV1::NormativeInputs,
                normative_schema_id,
                value,
                vec![],
                ComponentProvenanceV1::design_slot(design_revision_id, slot_tag, source_binding_id)
                    .expect("normative provenance"),
            )
            .expect("normative component"),
        );
    }
    let mut normative_ids = components
        .iter()
        .map(|component| *component.component_id())
        .collect::<Vec<_>>();
    normative_ids.sort_by_key(|identifier| *identifier.as_bytes());
    for kind in ContractComponentKindV1::ALL {
        if kind == ContractComponentKindV1::NormativeInputs {
            continue;
        }
        let source_binding_id = design_source_binding_identity(&CborValue::Array(vec![
            CborValue::Unsigned(kind.tag()),
            CborValue::Bytes(decision_closure_id.as_bytes().to_vec()),
        ]))
        .expect("source binding");
        components.push(
            CandidateContractComponentV1::new(
                &schemas,
                kind,
                facet_schema_id_v1(&schemas, kind).expect("facet schema"),
                fixture_facet_value_v1(kind, [kind.tag() as u8; 32], vec![[3; 32]]),
                normative_ids.clone(),
                ComponentProvenanceV1::design_slot(
                    design_revision_id,
                    kind.tag(),
                    source_binding_id,
                )
                .expect("facet provenance"),
            )
            .expect("aggregate facet"),
        );
    }
    for kind in ContractComponentKindV1::ALL {
        if kind == ContractComponentKindV1::NormativeInputs {
            continue;
        }
        let duplicate_source_binding = design_source_binding_identity(&CborValue::Array(vec![
            CborValue::Unsigned(10_000 + kind.tag()),
            CborValue::Bytes(decision_closure_id.as_bytes().to_vec()),
        ]))
        .expect("duplicate source binding");
        let duplicate = CandidateContractComponentV1::new(
            &schemas,
            kind,
            facet_schema_id_v1(&schemas, kind).expect("duplicate facet schema"),
            fixture_facet_value_v1(
                kind,
                [kind.tag().saturating_add(1) as u8; 32],
                vec![[4; 32]],
            ),
            normative_ids.clone(),
            ComponentProvenanceV1::design_slot(
                design_revision_id,
                kind.tag(),
                duplicate_source_binding,
            )
            .expect("duplicate facet provenance"),
        )
        .expect("duplicate aggregate facet");
        let mut duplicated = components.clone();
        duplicated.push(duplicate);
        assert!(matches!(
            CandidateContractRootV1::new(&schemas, duplicated),
            Err(ContractRootError::DuplicateAggregateComponentKind(actual)) if actual == kind
        ));
    }
    let root = CandidateContractRootV1::new(&schemas, components).expect("candidate root");
    let finalization =
        fixture_finalization(&schemas, design_revision_id, decision_closure_id, &root);
    let handoff =
        CanonicalBuildHandoffV1::project(&finalization, &root).expect("canonical handoff");

    let expected_component_count = usize::try_from(fixture_materialization_count)
        .expect("fixture materialization count fits usize")
        + ContractComponentKindV1::ALL.len()
        - 1;
    assert_eq!(root.components().len(), expected_component_count);
    assert_eq!(
        finalization.pinned_inputs().len(),
        FinalizationInputKindV1::ALL.len()
    );
    assert_eq!(handoff.components().len(), expected_component_count);
}

#[test]
fn emitted_candidate_root_has_canonical_identities_without_minting_provenance() {
    let output =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("contracts/vnext/stage0/candidate-root");
    assert!(
        output.is_dir(),
        "Stage 0 is incomplete: emitted candidate-root artifacts are required"
    );

    let schemas_document = read_document(&output, "candidate-root-schema-descriptors.v1.json");
    let design_document = read_document(&output, "design-revision.v1.json");
    let root_document = read_document(&output, "candidate-contract-root.v1.json");
    let finalization_document = read_document(&output, "design-finalization-manifest.v1.json");
    let handoff_document = read_document(&output, "canonical-build-handoff.v1.json");
    let bindings_document = read_document(&output, "decision-root-bindings.v1.json");
    let schemas = candidate_root_schema_closure_v1().expect("candidate root schema closure");

    let descriptor_rows = json_array(&schemas_document, "descriptors");
    assert_eq!(descriptor_rows.len(), schemas.descriptors().len());
    for row in descriptor_rows {
        let name = json_string(row, "schema_name");
        let expected = schemas
            .schema_id(name, 1)
            .expect("emitted descriptor must belong to the Rust closure");
        assert_eq!(json_string(row, "schema_id"), expected.render());
    }

    let component_ids = validate_emitted_components(&schemas, &root_document);
    let rows = json_array(&root_document, "components");
    let expected_component_count =
        json_array(&bindings_document, "bindings").len() + ContractComponentKindV1::ALL.len() - 1;
    assert_eq!(rows.len(), expected_component_count);
    assert_eq!(
        component_ids
            .iter()
            .map(ContractComponentIdV1::render)
            .collect::<Vec<_>>(),
        rows.iter()
            .map(|row| json_string(row, "component_id").to_owned())
            .collect::<Vec<_>>(),
    );
    let root_canonical = cbor_from_json(json_value(&root_document, "canonical_value"));
    let root_fields = cbor_array(&root_canonical, "candidate root canonical value");
    assert_eq!(
        cbor_unsigned(&root_fields[1], "candidate root component count") as usize,
        expected_component_count
    );
    let canonical_rows = cbor_array(&root_fields[2], "candidate root component rows");
    assert_eq!(canonical_rows.len(), component_ids.len());
    for (canonical_row, component_id) in canonical_rows.iter().zip(&component_ids) {
        assert_eq!(
            cbor_identity(
                &cbor_array(canonical_row, "candidate root component row")[0],
                "candidate root component identity",
            ),
            *component_id.as_bytes()
        );
    }
    assert_document_identity(&root_document, "maestro.vnext.candidate-contract-root.v1");

    DesignRevisionIdV1::parse(json_string(&design_document, "identity"))
        .expect("design revision identity");
    DecisionClosureIdV1::parse(json_string(&finalization_document, "decision_closure_id"))
        .expect("decision closure identity");
    let input_ids = json_array(&finalization_document, "pinned_inputs")
        .iter()
        .map(|row| reconstruct_pinned_input(&schemas, row))
        .collect::<Vec<_>>();
    assert_eq!(input_ids.len(), FinalizationInputKindV1::ALL.len());
    assert_eq!(
        json_string(&finalization_document, "candidate_contract_root_id"),
        json_string(&root_document, "identity")
    );
    assert_document_identity(
        &finalization_document,
        "maestro.vnext.design-finalization-manifest.v1",
    );

    assert_eq!(
        json_string(&handoff_document, "candidate_contract_root_id"),
        json_string(&root_document, "identity")
    );
    assert_eq!(
        json_string(&handoff_document, "finalization_manifest_id"),
        json_string(&finalization_document, "identity")
    );
    assert_eq!(
        json_u64(&handoff_document, "component_count") as usize,
        expected_component_count
    );
    assert_eq!(
        json_u64(&handoff_document, "pinned_input_count") as usize,
        FinalizationInputKindV1::ALL.len()
    );
    assert_document_identity(
        &handoff_document,
        "maestro.vnext.build-handoff-projection.v1",
    );
}

fn validate_emitted_components(
    schemas: &maestro::domain::vnext::identity::SchemaClosureV1,
    document: &Value,
) -> Vec<ContractComponentIdV1> {
    json_array(document, "components")
        .iter()
        .map(|row| {
            let kind_tag = json_u64(row, "kind_tag");
            let kind = ContractComponentKindV1::try_from(kind_tag).expect("component kind");
            let schema_id = SchemaIdV1::parse(json_string(row, "schema_id")).expect("schema id");
            let canonical = cbor_from_json(json_value(row, "canonical_value"));
            let fields = cbor_array(&canonical, "component canonical value");
            assert_eq!(cbor_unsigned(&fields[0], "component version"), 1);
            assert_eq!(cbor_unsigned(&fields[1], "component kind"), kind_tag);
            assert_eq!(
                cbor_identity(&fields[2], "component schema id"),
                *schema_id.as_bytes()
            );
            let dependencies = cbor_array(&fields[4], "component dependencies")
                .iter()
                .map(|value| {
                    ContractComponentIdV1::parse(&rendered_identity(cbor_identity(
                        value,
                        "component dependency",
                    )))
                    .expect("component dependency identity")
                })
                .collect::<Vec<_>>();
            schemas
                .validate_value(&schema_id, &fields[3])
                .expect("component value must match its schema");
            let expected_id = ContractComponentIdV1::parse(json_string(row, "component_id"))
                .expect("component identity");
            match json_string(json_value(row, "provenance"), "kind") {
                "design_slot" => {
                    let component = CandidateContractComponentV1::new(
                        schemas,
                        kind,
                        schema_id,
                        fields[3].clone(),
                        dependencies,
                        reconstruct_design_provenance(json_value(row, "provenance")),
                    )
                    .expect("reconstruct externally constructible component");
                    assert_eq!(component.component_id(), &expected_id);
                    assert_eq!(
                        component.canonical_bytes().expect("component bytes"),
                        deterministic_cbor_bytes(&canonical),
                    );
                }
                "decision_materialization" => {
                    validate_decision_materialization_provenance(
                        json_value(row, "provenance"),
                        &fields[5],
                    );
                    assert_canonical_identity(
                        "maestro.vnext.contract-component.v1",
                        &canonical,
                        &expected_id.render(),
                    );
                }
                other => panic!("unsupported emitted provenance kind: {other}"),
            }
            expected_id
        })
        .collect()
}

fn reconstruct_design_provenance(value: &Value) -> ComponentProvenanceV1 {
    ComponentProvenanceV1::design_slot(
        DesignRevisionIdV1::parse(json_string(value, "design_revision_id"))
            .expect("design revision identity"),
        json_u64(value, "slot_tag"),
        DesignSourceBindingIdV1::parse(json_string(value, "source_binding_id"))
            .expect("design source binding identity"),
    )
    .expect("design slot provenance")
}

fn validate_decision_materialization_provenance(value: &Value, canonical: &CborValue) {
    let resolution_id = DecisionResolutionIdV1::parse(json_string(value, "resolution_id"))
        .expect("Decision resolution identity");
    let materialization_id =
        DecisionMaterializationIdV1::parse(json_string(value, "materialization_id"))
            .expect("Decision materialization identity");
    assert_eq!(
        canonical,
        &CborValue::Array(vec![
            CborValue::Unsigned(3),
            CborValue::Bytes(resolution_id.as_bytes().to_vec()),
            CborValue::Bytes(materialization_id.as_bytes().to_vec()),
        ])
    );
}

fn reconstruct_pinned_input(
    schemas: &maestro::domain::vnext::identity::SchemaClosureV1,
    row: &Value,
) -> PinnedFinalizationInputV1 {
    let kind = FinalizationInputKindV1::try_from(json_u64(row, "kind_tag"))
        .expect("finalization input kind");
    let value = cbor_from_json(json_value(row, "canonical_value"));
    let input = match kind {
        FinalizationInputKindV1::ClosureRequirement => {
            PinnedFinalizationInputV1::closure_requirement(schemas, value)
        }
        FinalizationInputKindV1::DeterministicSynthesis => {
            PinnedFinalizationInputV1::deterministic_synthesis(schemas, value)
        }
        FinalizationInputKindV1::ScopeAndExclusions => {
            PinnedFinalizationInputV1::scope_and_exclusions(schemas, value)
        }
        FinalizationInputKindV1::CapabilityCensusAndJourneys => {
            PinnedFinalizationInputV1::capability_census_and_journeys(schemas, value)
        }
        FinalizationInputKindV1::MigrationRollbackRemoval => {
            PinnedFinalizationInputV1::migration_rollback_removal(schemas, value)
        }
        FinalizationInputKindV1::StageProofMatrix => {
            PinnedFinalizationInputV1::stage_proof_matrix(schemas, value)
        }
        FinalizationInputKindV1::ReviewEvidence => {
            PinnedFinalizationInputV1::review_evidence(schemas, value)
        }
        FinalizationInputKindV1::EdgeSweepEvidence => {
            PinnedFinalizationInputV1::edge_sweep_evidence(schemas, value)
        }
        FinalizationInputKindV1::RiskRecovery => {
            PinnedFinalizationInputV1::risk_recovery(schemas, value)
        }
        FinalizationInputKindV1::FreshnessReferences => {
            PinnedFinalizationInputV1::freshness_references(schemas, value)
        }
        FinalizationInputKindV1::CanonicalizationPolicy => {
            PinnedFinalizationInputV1::canonicalization_policy(schemas, value)
        }
    }
    .expect("reconstruct finalization input");
    assert_eq!(input.schema_id().render(), json_string(row, "schema_id"));
    assert_eq!(input.input_id().render(), json_string(row, "input_id"));
    input
}

fn read_document(output: &Path, name: &str) -> Value {
    let path = output.join(name);
    serde_json::from_slice(&fs::read(&path).expect("emitted candidate artifact"))
        .expect("emitted candidate artifact JSON")
}

fn json_object<'a>(value: &'a Value, context: &str) -> &'a serde_json::Map<String, Value> {
    value
        .as_object()
        .unwrap_or_else(|| panic!("{context} must be an object"))
}

fn json_value<'a>(value: &'a Value, key: &str) -> &'a Value {
    json_object(value, "JSON value")
        .get(key)
        .unwrap_or_else(|| panic!("missing JSON field: {key}"))
}

fn json_string<'a>(value: &'a Value, key: &str) -> &'a str {
    json_value(value, key)
        .as_str()
        .unwrap_or_else(|| panic!("JSON field {key} must be a string"))
}

fn json_u64(value: &Value, key: &str) -> u64 {
    json_value(value, key)
        .as_u64()
        .unwrap_or_else(|| panic!("JSON field {key} must be a non-negative integer"))
}

fn json_array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    json_value(value, key)
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("JSON field {key} must be an array"))
}

fn cbor_from_json(value: &Value) -> CborValue {
    match value {
        Value::Array(items) => CborValue::Array(items.iter().map(cbor_from_json).collect()),
        Value::Number(number) => CborValue::Unsigned(
            number
                .as_u64()
                .expect("candidate canonical numbers must be unsigned"),
        ),
        Value::Object(object) if object.len() == 1 && object.contains_key("bytes") => {
            CborValue::Bytes(hexadecimal_bytes(
                object["bytes"]
                    .as_str()
                    .expect("candidate canonical bytes must be strings"),
            ))
        }
        Value::String(text) => CborValue::Text(text.clone()),
        _ => panic!("unsupported candidate canonical JSON value"),
    }
}

fn cbor_array<'a>(value: &'a CborValue, context: &str) -> &'a [CborValue] {
    match value {
        CborValue::Array(items) => items,
        _ => panic!("{context} must be a CBOR array"),
    }
}

fn cbor_unsigned(value: &CborValue, context: &str) -> u64 {
    match value {
        CborValue::Unsigned(number) => *number,
        _ => panic!("{context} must be an unsigned CBOR value"),
    }
}

fn cbor_identity(value: &CborValue, context: &str) -> [u8; 32] {
    match value {
        CborValue::Bytes(bytes) => bytes
            .as_slice()
            .try_into()
            .unwrap_or_else(|_| panic!("{context} must be exactly 32 bytes")),
        _ => panic!("{context} must be CBOR bytes"),
    }
}

fn hexadecimal_bytes(value: &str) -> Vec<u8> {
    assert!(
        value.len().is_multiple_of(2),
        "hexadecimal values must have even length"
    );
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hexadecimal_nibble(pair[0]).expect("hexadecimal high nibble");
            let low = hexadecimal_nibble(pair[1]).expect("hexadecimal low nibble");
            (high << 4) | low
        })
        .collect()
}

fn hexadecimal_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn rendered_identity(value: [u8; 32]) -> String {
    let mut rendered = String::from("sha256:");
    for byte in value {
        use std::fmt::Write;
        write!(&mut rendered, "{byte:02x}").expect("write identity hexadecimal");
    }
    rendered
}

fn assert_document_identity(document: &Value, domain: &str) {
    let canonical = cbor_from_json(json_value(document, "canonical_value"));
    let expected = hexadecimal_bytes(json_string(document, "canonical_cbor_hex"));
    assert_eq!(deterministic_cbor_bytes(&canonical), expected);
    assert_eq!(
        rendered_identity(Sha256::digest(&expected).into()),
        format!("sha256:{}", json_string(document, "canonical_cbor_sha256"))
    );
    assert_canonical_identity(domain, &canonical, json_string(document, "identity"));
}

fn assert_canonical_identity(domain: &str, canonical: &CborValue, expected: &str) {
    let identity_preimage =
        CborValue::Array(vec![CborValue::Text(domain.to_owned()), canonical.clone()]);
    assert_eq!(
        rendered_identity(Sha256::digest(deterministic_cbor_bytes(&identity_preimage)).into()),
        expected
    );
}

fn deterministic_cbor_bytes(value: &CborValue) -> Vec<u8> {
    maestro::foundation::core::deterministic_cbor::encode(value).expect("deterministic CBOR")
}
