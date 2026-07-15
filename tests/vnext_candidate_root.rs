use maestro::domain::vnext::contract::assembly::{
    candidate_root_schema_closure_v1, facet_schema_id_v1, finalization_facet_kinds_v1,
    finalization_input_schema_id_v1, fixture_facet_value_v1, normative_inputs_schema_id_v1,
};
use maestro::domain::vnext::contract::component::CandidateContractComponentV1;
use maestro::domain::vnext::contract::component_kind::ContractComponentKindV1;
use maestro::domain::vnext::contract::decision_closure::{
    DecisionClosureError, DecisionClosureV1, DecisionConsequenceClassificationV1,
    DecisionMaterializationSourceV1, ExactDecisionRootBindingV1, ExternalDecisionClosureRecordV1,
    ExternalDesignAuthorityClosureV1, ExternalLineageDispositionV1, RawExternalDecisionRecordV1,
    RequiredDecisionMaterializationV1, TerminalDecisionStatusV1,
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
    ContractComponentIdV1, ContractRootIdV1, DecisionClosureIdV1, DecisionMaterializationIdV1,
    DecisionResolutionIdV1, DesignFinalizationManifestIdV1, DesignRevisionIdV1,
    DesignSourceBindingIdV1, SchemaClosureV1, SchemaIdV1, decision_closure_identity,
    decision_materialization_identity, design_revision_identity, design_source_binding_identity,
};
use maestro::foundation::core::deterministic_cbor::CborValue;
use serde_json::Value;
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

fn rebuild_fixture_root(
    schemas: &SchemaClosureV1,
    original: &CandidateContractRootV1,
    mut normative_components: Vec<CandidateContractComponentV1>,
) -> CandidateContractRootV1 {
    let mut normative_ids = normative_components
        .iter()
        .map(|component| *component.component_id())
        .collect::<Vec<_>>();
    normative_ids.sort_by_key(|identifier| *identifier.as_bytes());
    for component in original
        .components()
        .iter()
        .filter(|component| component.kind() != ContractComponentKindV1::NormativeInputs)
    {
        normative_components.push(
            CandidateContractComponentV1::new(
                schemas,
                component.kind(),
                *component.schema_id(),
                component.value().clone(),
                normative_ids.clone(),
                component.provenance().clone(),
            )
            .expect("rebuilt aggregate component"),
        );
    }
    CandidateContractRootV1::new(schemas, normative_components).expect("rebuilt candidate root")
}

fn fixture_bindings(
    root: &CandidateContractRootV1,
    finalization: &DesignFinalizationManifestV1,
    decision_closure_id: DecisionClosureIdV1,
) -> Vec<ExactDecisionRootBindingV1> {
    let mut bindings = root
        .components()
        .iter()
        .filter(|component| component.kind() == ContractComponentKindV1::NormativeInputs)
        .map(|component| {
            let ComponentProvenanceV1::DecisionMaterialization(provenance) = component.provenance()
            else {
                panic!("NormativeInputs fixture must use Decision materialization provenance");
            };
            ExactDecisionRootBindingV1::new(
                *provenance.materialization_id(),
                *component.component_id(),
                MaterializationBaseV1::initial_external_design_closure(decision_closure_id),
                *root.root_id(),
                *finalization.manifest_id(),
            )
        })
        .collect::<Vec<_>>();
    bindings.sort_by_key(|binding| *binding.materialization_id().as_bytes());
    bindings
}

fn normative_component<'a>(
    root: &'a CandidateContractRootV1,
    materialization_id: &DecisionMaterializationIdV1,
) -> &'a CandidateContractComponentV1 {
    root.components()
        .iter()
        .find(|component| {
            matches!(
                component.provenance(),
                ComponentProvenanceV1::DecisionMaterialization(provenance)
                    if provenance.materialization_id() == materialization_id
            )
        })
        .expect("fixture NormativeInputs component")
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
    for required_materialization in decision_closure.materializations() {
        let materialization_id = *required_materialization.materialization_id();
        let resolution = DecisionMaterializationResolutionV1::new(
            decision_closure_id,
            MaterializationBaseV1::initial_external_design_closure(decision_closure_id),
            materialization_id,
        )
        .expect("decision resolution");
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
                ComponentProvenanceV1::decision_materialization(
                    *resolution.resolution_id(),
                    materialization_id,
                ),
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
    let bindings = fixture_bindings(&root, &finalization, decision_closure_id);
    decision_closure
        .root_binding_requirements()
        .resolve(bindings.clone(), &root, &finalization)
        .expect("exact Decision-root bindings");

    let mut reordered_bindings = bindings.clone();
    reordered_bindings.reverse();
    assert!(matches!(
        decision_closure.root_binding_requirements().resolve(
            reordered_bindings,
            &root,
            &finalization,
        ),
        Err(DecisionClosureError::BindingsNotStrictlySorted)
    ));

    let first_materialization = &decision_closure.materializations()[0];
    let second_materialization = &decision_closure.materializations()[1];
    let first_component = normative_component(&root, first_materialization.materialization_id());
    let original_normatives = root
        .components()
        .iter()
        .filter(|component| component.kind() == ContractComponentKindV1::NormativeInputs)
        .cloned()
        .collect::<Vec<_>>();

    let fabricated_resolution =
        DecisionResolutionIdV1::parse(&format!("sha256:{}", "22".repeat(32)))
            .expect("fabricated resolution identity");
    let wrong_resolution_component = CandidateContractComponentV1::new(
        &schemas,
        ContractComponentKindV1::NormativeInputs,
        *first_component.schema_id(),
        first_component.value().clone(),
        first_component.dependencies().to_vec(),
        ComponentProvenanceV1::decision_materialization(
            fabricated_resolution,
            *first_materialization.materialization_id(),
        ),
    )
    .expect("wrong-resolution component");
    let wrong_resolution_normatives = original_normatives
        .iter()
        .map(|component| {
            if component.component_id() == first_component.component_id() {
                wrong_resolution_component.clone()
            } else {
                component.clone()
            }
        })
        .collect();
    let wrong_resolution_root = rebuild_fixture_root(&schemas, &root, wrong_resolution_normatives);
    let wrong_resolution_finalization = fixture_finalization(
        &schemas,
        design_revision_id,
        decision_closure_id,
        &wrong_resolution_root,
    );
    assert!(matches!(
        decision_closure.root_binding_requirements().resolve(
            fixture_bindings(
                &wrong_resolution_root,
                &wrong_resolution_finalization,
                decision_closure_id,
            ),
            &wrong_resolution_root,
            &wrong_resolution_finalization,
        ),
        Err(DecisionClosureError::NormativeResolutionMismatch)
    ));

    let first_source = &first_materialization.sources()[0];
    let stale_value = CborValue::Array(vec![
        CborValue::Unsigned(1),
        CborValue::Bytes(
            first_materialization
                .materialization_id()
                .as_bytes()
                .to_vec(),
        ),
        CborValue::Array(vec![CborValue::Array(vec![
            CborValue::Text(first_source.decision_id().to_owned()),
            CborValue::Bytes([0xff; 32].to_vec()),
        ])]),
    ]);
    let stale_value_component = CandidateContractComponentV1::new(
        &schemas,
        ContractComponentKindV1::NormativeInputs,
        *first_component.schema_id(),
        stale_value,
        first_component.dependencies().to_vec(),
        first_component.provenance().clone(),
    )
    .expect("stale-value component");
    let stale_value_normatives = original_normatives
        .iter()
        .map(|component| {
            if component.component_id() == first_component.component_id() {
                stale_value_component.clone()
            } else {
                component.clone()
            }
        })
        .collect();
    let stale_value_root = rebuild_fixture_root(&schemas, &root, stale_value_normatives);
    let stale_value_finalization = fixture_finalization(
        &schemas,
        design_revision_id,
        decision_closure_id,
        &stale_value_root,
    );
    assert!(matches!(
        decision_closure.root_binding_requirements().resolve(
            fixture_bindings(
                &stale_value_root,
                &stale_value_finalization,
                decision_closure_id,
            ),
            &stale_value_root,
            &stale_value_finalization,
        ),
        Err(DecisionClosureError::NormativeComponentValueMismatch)
    ));

    let second_component = normative_component(&root, second_materialization.materialization_id());
    let duplicate_provenance_component = CandidateContractComponentV1::new(
        &schemas,
        ContractComponentKindV1::NormativeInputs,
        *first_component.schema_id(),
        first_component.value().clone(),
        vec![*first_component.component_id()],
        first_component.provenance().clone(),
    )
    .expect("duplicate-provenance component");
    let duplicate_provenance_normatives = original_normatives
        .iter()
        .map(|component| {
            if component.component_id() == second_component.component_id() {
                duplicate_provenance_component.clone()
            } else {
                component.clone()
            }
        })
        .collect();
    let duplicate_provenance_root =
        rebuild_fixture_root(&schemas, &root, duplicate_provenance_normatives);
    let duplicate_provenance_finalization = fixture_finalization(
        &schemas,
        design_revision_id,
        decision_closure_id,
        &duplicate_provenance_root,
    );
    let mut duplicate_provenance_bindings = decision_closure
        .materializations()
        .iter()
        .map(|materialization| {
            let component = if materialization.materialization_id()
                == second_materialization.materialization_id()
            {
                duplicate_provenance_root
                    .components()
                    .iter()
                    .find(|component| {
                        component.kind() == ContractComponentKindV1::NormativeInputs
                            && !component.dependencies().is_empty()
                    })
                    .expect("duplicate-provenance replacement")
            } else {
                normative_component(
                    &duplicate_provenance_root,
                    materialization.materialization_id(),
                )
            };
            ExactDecisionRootBindingV1::new(
                *materialization.materialization_id(),
                *component.component_id(),
                MaterializationBaseV1::initial_external_design_closure(decision_closure_id),
                *duplicate_provenance_root.root_id(),
                *duplicate_provenance_finalization.manifest_id(),
            )
        })
        .collect::<Vec<_>>();
    duplicate_provenance_bindings.sort_by_key(|binding| *binding.materialization_id().as_bytes());
    assert!(matches!(
        decision_closure.root_binding_requirements().resolve(
            duplicate_provenance_bindings,
            &duplicate_provenance_root,
            &duplicate_provenance_finalization,
        ),
        Err(DecisionClosureError::DuplicateNormativeMaterialization)
    ));

    let aggregate_component = root
        .components()
        .iter()
        .find(|component| component.kind() != ContractComponentKindV1::NormativeInputs)
        .expect("aggregate component");
    let mut wrong_component = bindings.clone();
    let first = &bindings[0];
    wrong_component[0] = ExactDecisionRootBindingV1::new(
        *first.materialization_id(),
        *aggregate_component.component_id(),
        first.materialization_base().clone(),
        *root.root_id(),
        *finalization.manifest_id(),
    );
    assert!(matches!(
        decision_closure
            .root_binding_requirements()
            .resolve(wrong_component, &root, &finalization,),
        Err(DecisionClosureError::NormativeComponentSetMismatch)
    ));

    let mut wrong_base = bindings.clone();
    wrong_base[0] = ExactDecisionRootBindingV1::new(
        *first.materialization_id(),
        *first.component_id(),
        MaterializationBaseV1::prior_contract_root(*root.root_id()),
        *root.root_id(),
        *finalization.manifest_id(),
    );
    assert!(matches!(
        decision_closure
            .root_binding_requirements()
            .resolve(wrong_base, &root, &finalization,),
        Err(DecisionClosureError::BindingMaterializationBaseMismatch)
    ));

    let wrong_root = ContractRootIdV1::parse(&format!("sha256:{}", "00".repeat(32)))
        .expect("different root identity");
    let mut wrong_root_binding = bindings.clone();
    wrong_root_binding[0] = ExactDecisionRootBindingV1::new(
        *first.materialization_id(),
        *first.component_id(),
        first.materialization_base().clone(),
        wrong_root,
        *finalization.manifest_id(),
    );
    assert!(matches!(
        decision_closure.root_binding_requirements().resolve(
            wrong_root_binding,
            &root,
            &finalization,
        ),
        Err(DecisionClosureError::BindingFinalizationMismatch)
    ));

    let wrong_finalization =
        DesignFinalizationManifestIdV1::parse(&format!("sha256:{}", "11".repeat(32)))
            .expect("different finalization identity");
    let mut wrong_finalization_binding = bindings.clone();
    wrong_finalization_binding[0] = ExactDecisionRootBindingV1::new(
        *first.materialization_id(),
        *first.component_id(),
        first.materialization_base().clone(),
        *root.root_id(),
        wrong_finalization,
    );
    assert!(matches!(
        decision_closure.root_binding_requirements().resolve(
            wrong_finalization_binding,
            &root,
            &finalization,
        ),
        Err(DecisionClosureError::BindingFinalizationMismatch)
    ));
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
fn emitted_candidate_root_reconstructs_with_rust_contract_types() {
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

    let components = reconstruct_components(&schemas, &root_document);
    let root = CandidateContractRootV1::new(&schemas, components).expect("reconstruct root");
    assert_eq!(
        root.root_id().render(),
        json_string(&root_document, "identity")
    );
    assert_document_bytes(&root, &root_document);

    let rows = json_array(&root_document, "components");
    let expected_component_count =
        json_array(&bindings_document, "bindings").len() + ContractComponentKindV1::ALL.len() - 1;
    assert_eq!(rows.len(), expected_component_count);
    assert_eq!(
        root.components()
            .iter()
            .map(|component| component.component_id().render())
            .collect::<Vec<_>>(),
        rows.iter()
            .map(|row| json_string(row, "component_id").to_owned())
            .collect::<Vec<_>>(),
    );

    let design_revision_id = DesignRevisionIdV1::parse(json_string(&design_document, "identity"))
        .expect("design revision identity");
    let decision_closure_id =
        DecisionClosureIdV1::parse(json_string(&finalization_document, "decision_closure_id"))
            .expect("decision closure identity");
    let inputs = json_array(&finalization_document, "pinned_inputs")
        .iter()
        .map(|row| reconstruct_pinned_input(&schemas, row))
        .collect::<Vec<_>>();
    let finalization = DesignFinalizationManifestV1::new(
        &schemas,
        DesignBasisV1::design_revision(design_revision_id),
        decision_closure_id,
        &root,
        inputs,
    )
    .expect("reconstruct finalization manifest");
    assert_eq!(
        finalization.manifest_id().render(),
        json_string(&finalization_document, "identity")
    );
    assert_document_bytes(&finalization, &finalization_document);

    let handoff =
        CanonicalBuildHandoffV1::project(&finalization, &root).expect("reconstruct handoff");
    assert_eq!(
        handoff.handoff_id().render(),
        json_string(&handoff_document, "identity")
    );
    assert_document_bytes(&handoff, &handoff_document);
    assert_eq!(handoff.components().len(), expected_component_count);
    assert_eq!(
        handoff.pinned_inputs().len(),
        FinalizationInputKindV1::ALL.len()
    );
}

fn reconstruct_components(
    schemas: &maestro::domain::vnext::identity::SchemaClosureV1,
    document: &Value,
) -> Vec<CandidateContractComponentV1> {
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
            let provenance = reconstruct_provenance(json_value(row, "provenance"));
            let component = CandidateContractComponentV1::new(
                schemas,
                kind,
                schema_id,
                fields[3].clone(),
                dependencies,
                provenance,
            )
            .expect("reconstruct component");
            assert_eq!(
                component.component_id().render(),
                json_string(row, "component_id")
            );
            assert_eq!(
                component.canonical_bytes().expect("component bytes"),
                deterministic_cbor_bytes(&canonical),
            );
            component
        })
        .collect()
}

fn reconstruct_provenance(value: &Value) -> ComponentProvenanceV1 {
    match json_string(value, "kind") {
        "decision_materialization" => ComponentProvenanceV1::decision_materialization(
            DecisionResolutionIdV1::parse(json_string(value, "resolution_id"))
                .expect("decision resolution identity"),
            DecisionMaterializationIdV1::parse(json_string(value, "materialization_id"))
                .expect("decision materialization identity"),
        ),
        "design_slot" => ComponentProvenanceV1::design_slot(
            DesignRevisionIdV1::parse(json_string(value, "design_revision_id"))
                .expect("design revision identity"),
            json_u64(value, "slot_tag"),
            DesignSourceBindingIdV1::parse(json_string(value, "source_binding_id"))
                .expect("design source binding identity"),
        )
        .expect("design slot provenance"),
        other => panic!("unsupported emitted provenance kind: {other}"),
    }
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

trait CanonicalBytesV1 {
    fn canonical_bytes_for_test(&self) -> Vec<u8>;
}

impl CanonicalBytesV1 for CandidateContractRootV1 {
    fn canonical_bytes_for_test(&self) -> Vec<u8> {
        self.canonical_bytes().expect("candidate root bytes")
    }
}

impl CanonicalBytesV1 for DesignFinalizationManifestV1 {
    fn canonical_bytes_for_test(&self) -> Vec<u8> {
        self.canonical_bytes().expect("finalization manifest bytes")
    }
}

impl CanonicalBytesV1 for CanonicalBuildHandoffV1 {
    fn canonical_bytes_for_test(&self) -> Vec<u8> {
        self.canonical_bytes().expect("build handoff bytes")
    }
}

fn assert_document_bytes<T: CanonicalBytesV1>(value: &T, document: &Value) {
    let expected = hexadecimal_bytes(json_string(document, "canonical_cbor_hex"));
    assert_eq!(value.canonical_bytes_for_test(), expected);
}

fn deterministic_cbor_bytes(value: &CborValue) -> Vec<u8> {
    maestro::foundation::core::deterministic_cbor::encode(value).expect("deterministic CBOR")
}
