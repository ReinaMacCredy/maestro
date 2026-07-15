use maestro::domain::vnext::capability::literals::{
    CapabilityInstructionLoadPlanV1, CapabilityMethodResolutionOutcomeV1,
    CapabilityMethodResolutionV1, ContextBudgetMeasurementV1, ContextBudgetProfileV1,
    INSTRUCTION_RESOURCE_PATHS_V1, InstructionResourceRefV1, JOB_METHOD_NEGATIVE_CELLS_V1,
    JOB_METHOD_POSITIVE_CELLS_V1, JobV1, METHOD_NAMES_V1, MethodV1, PUBLIC_SKILL_IDS_V1,
    SKILL_LEDGER_ROWS_V1, exact_review_subset_counts, job_method_is_admitted,
};
use maestro::domain::vnext::integration::public_literals::*;
use maestro::domain::vnext::orchestration::literals::{
    BOUNDED_CONTINUATION_PROFILE_IDS_V1, BoundedContinuationProfileV1, ExactRecipeSelectionV1,
    RECIPE_IDS_V1, RECIPE_MANIFEST_FIELD_NAMES_V1, RECIPE_RETURN_REASON_COUNT_V1, RecipeIdV1,
    RecipeLiteralError, RecipeReturnReasonV1, RecipeSelectionRequestV1,
};

const HEX_ONE: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const HEX_TWO: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn sha(value: &str) -> String {
    format!("sha256:{value}")
}

fn absent_request() -> RecipeSelectionRequestV1 {
    RecipeSelectionRequestV1 {
        schema_version: 1,
        resolution_basis_ref: "candidate:public-catalog:release-one".to_owned(),
        primary_selection: ExactRecipeSelectionV1::Absent,
        continuation_selection: ExactRecipeSelectionV1::Absent,
    }
}

fn recipe_resource(recipe: RecipeIdV1) -> String {
    format!("candidate:orchestration:recipe:{}:v1", recipe.id())
}

fn profile_resource(profile: BoundedContinuationProfileV1) -> String {
    format!(
        "candidate:orchestration:bounded-continuation-profile:{}:v1@sha256:{HEX_TWO}",
        profile.id()
    )
}

fn canonical_selection_context() -> SelectionContextV1 {
    let mut options = Vec::new();
    for primary in std::iter::once(None).chain(RecipeIdV1::PRIMARY.into_iter().map(Some)) {
        for continuation in
            std::iter::once(None).chain(BoundedContinuationProfileV1::ALL.into_iter().map(Some))
        {
            let primary_selection = primary.map_or(ExactRecipeSelectionV1::Absent, |recipe| {
                ExactRecipeSelectionV1::Primary {
                    recipe_resource_ref: recipe_resource(recipe),
                    manifest_content_ref: sha(HEX_ONE),
                }
            });
            let continuation_selection =
                continuation.map_or(ExactRecipeSelectionV1::Absent, |profile| {
                    ExactRecipeSelectionV1::Continuation {
                        recipe_resource_ref: recipe_resource(RecipeIdV1::BoundedContinuation),
                        manifest_content_ref: sha(HEX_ONE),
                        profile_resource_ref: profile_resource(profile),
                    }
                });
            options.push(RecipeSelectionOptionV1 {
                primary_selection,
                continuation_selection,
            });
        }
    }
    SelectionContextV1 {
        schema_version: 1,
        resolution_basis_ref: "candidate:public-catalog:release-one".to_owned(),
        selection_options: options,
    }
}

fn core_binding() -> PacketRecipeBindingV1 {
    let request = absent_request();
    let selection_request_hash = request.semantic_hash().expect("selection hash");
    let recipe_application = request
        .seal("candidate:frontier:one")
        .expect("seal exact Frontier");
    let recipe_application_hash = recipe_application
        .semantic_hash()
        .expect("application hash");
    PacketRecipeBindingV1 {
        schema_version: 1,
        selection_request_hash,
        recipe_application,
        recipe_application_hash,
        component_provenance: Vec::new(),
        advice_provenance: PacketRecipeAdviceProvenanceV1 {
            composition_outcome: PacketRecipeAdviceOutcomeV1::CoreOnly,
            ordered_component_output_hashes: Vec::new(),
            composed_output_hash: [3; 32],
        },
    }
}

fn capability_resource(path: &str) -> InstructionResourceRefV1 {
    InstructionResourceRefV1 {
        logical_path: path.to_owned(),
        resource_ref: format!(
            "candidate:capability:instruction-resource:{path}:v1@sha256:{HEX_ONE}"
        ),
    }
}

fn bootstrap_setup_route() -> JobRouteV1 {
    JobRouteV1 {
        schema_version: 1,
        resolution_basis_ref: "candidate:integration:job-route-basis:release-one".to_owned(),
        input: JobRouteInputV1::Bootstrap {
            exact_bootstrap_fact_view_ref: "candidate:bootstrap-fact-view:one".to_owned(),
            fact_view_hash: [4; 32],
        },
        explicit_read_intent: ExplicitReadIntentV1::None,
        basis: JobRouteBasisV1::Bootstrap,
        outcome: JobRouteOutcomeV1::Selected {
            job: JobV1::Setup,
            reason: JobRouteReasonV1::BootstrapRequired,
            instruction_load_plan: JobInstructionLoadPlanV1 {
                job_resource_ref: capability_resource(JobV1::Setup.resource_path()).resource_ref,
                method_resource_refs: Vec::new(),
                recipe_resource_refs: Vec::new(),
            },
        },
    }
}

fn action_spec() -> ActionSpecRefV1 {
    ActionSpecRefV1 {
        exact_action_spec_ref: "candidate:action-spec:distribution-install".to_owned(),
        exact_schema_id: sha(HEX_ONE),
        exact_core_catalog_ref: "candidate:core-catalog:release-one".to_owned(),
        exact_public_catalog_ref: "candidate:public-catalog:release-one".to_owned(),
    }
}

#[test]
fn recipe_literals_keep_pre_frontier_selection_and_closed_taxonomy() {
    let application = absent_request()
        .seal("candidate:action-frontier:one")
        .expect("application seals one exact Frontier");
    assert_eq!(application.frontier_ref, "candidate:action-frontier:one");
    assert_eq!(RECIPE_IDS_V1.len(), 10);
    assert_eq!(
        BOUNDED_CONTINUATION_PROFILE_IDS_V1,
        ["attended", "unattended"]
    );
    assert_eq!(RECIPE_MANIFEST_FIELD_NAMES_V1.len(), 14);
    assert_eq!(RECIPE_RETURN_REASON_COUNT_V1, 30);
    assert_eq!(RecipeReturnReasonV1::WayfindingHardStop as u8, 30);
    assert!(matches!(
        RecipeSelectionRequestV1 {
            schema_version: 1,
            resolution_basis_ref: "candidate:basis:one".to_owned(),
            primary_selection: ExactRecipeSelectionV1::Continuation {
                recipe_resource_ref: recipe_resource(RecipeIdV1::BoundedContinuation),
                manifest_content_ref: sha(HEX_ONE),
                profile_resource_ref: profile_resource(BoundedContinuationProfileV1::Attended),
            },
            continuation_selection: ExactRecipeSelectionV1::Absent,
        }
        .validate(),
        Err(RecipeLiteralError::InvalidPrimarySelection)
    ));
}

#[test]
fn selection_and_packet_bindings_reject_the_old_false_passes() {
    let context = canonical_selection_context();
    context.validate().expect("exact ordered 30-option product");
    let mut duplicated = context.clone();
    duplicated.selection_options[29] = duplicated.selection_options[0].clone();
    assert!(
        duplicated.validate().is_err(),
        "duplicate selection must reject"
    );

    let binding = core_binding();
    binding.validate().expect("exact core binding");
    let mut zero_request_hash = binding.clone();
    zero_request_hash.selection_request_hash = [0; 32];
    assert!(zero_request_hash.validate().is_err());
    let mut zero_application_hash = binding.clone();
    zero_application_hash.recipe_application_hash = [0; 32];
    assert!(zero_application_hash.validate().is_err());
    let mut zero_composed_hash = binding;
    zero_composed_hash.advice_provenance.composed_output_hash = [0; 32];
    assert!(zero_composed_hash.validate().is_err());
    assert_eq!(MCP_PACKET_READ_ENVELOPE_OUTCOMES_V1.len(), 6);
    assert_eq!(
        GLOBAL_MCP_TOOL_NAMES_V1,
        ["maestro_packet", "maestro_cli_search"]
    );
    assert_eq!(PROJECT_MCP_TOOL_COUNT_V1, 0);
}

#[test]
fn job_route_setup_and_mcp_search_are_typed_total_contracts() {
    let route = bootstrap_setup_route();
    route.validate().expect("exact bootstrap Setup route");
    let mut wrong_job = route.clone();
    wrong_job.outcome = JobRouteOutcomeV1::Selected {
        job: JobV1::Execute,
        reason: JobRouteReasonV1::BootstrapRequired,
        instruction_load_plan: JobInstructionLoadPlanV1 {
            job_resource_ref: capability_resource(JobV1::Execute.resource_path()).resource_ref,
            method_resource_refs: Vec::new(),
            recipe_resource_refs: Vec::new(),
        },
    };
    assert!(wrong_job.validate().is_err());
    assert_eq!(JOB_ROUTE_REASONS_V1.len(), 17);

    let fact_view = SetupFactViewV1 {
        schema_version: 1,
        resolver_resource_and_release_catalog_closure: "candidate:setup-resolver:release-one"
            .to_owned(),
        acquisition_context: AcquisitionContextV1::ActiveStore(ActiveStoreAcquisitionV1 {
            domain: ActiveStoreDomainV1::Repository {
                repository_domain_ref: "candidate:repository-domain:one".to_owned(),
            },
            store_ref: "candidate:store:one".to_owned(),
            store_generation_ref: "candidate:generation:one".to_owned(),
            authority_epoch_ref: "candidate:authority-epoch:one".to_owned(),
            current_packet_or_material_stamp: [5; 32],
        }),
        locality_subject: "candidate:distribution-target:one".to_owned(),
        source_owner_fact_commitments: vec![SourceOwnerFactCommitmentV1 {
            owner: SourceOwnerV1::Distribution,
            exact_fact_ref: "candidate:distribution-fact:one".to_owned(),
            fact_hash: [6; 32],
        }],
        advertised_operation_binding: SetupAdvertisedOperationBindingV1::Action {
            operation_spec: action_spec(),
            exact_typed_subject_ref: "candidate:distribution-plan:one".to_owned(),
            material_dependency_stamp: [7; 32],
            meaning: SetupActionMeaningV1::DistributionPlan(SetupPlanIntentV1::Install),
        },
    };
    let resolution = resolve_setup_mode(
        &fact_view,
        SetupModeRequestV1::AcceptUniqueEligible,
        &[SetupModeV1::Install],
    )
    .expect("facts-first singleton selection");
    resolution
        .validate_against(
            &fact_view,
            SetupModeRequestV1::AcceptUniqueEligible,
            &[SetupModeV1::Install],
        )
        .expect("exact commitments");
    let ambiguous = resolve_setup_mode(
        &fact_view,
        SetupModeRequestV1::AcceptUniqueEligible,
        &[SetupModeV1::Install, SetupModeV1::Repair],
    )
    .expect("coherent alternatives are ambiguous before Operation validation");
    assert!(matches!(
        ambiguous.outcome,
        SetupModeResolutionOutcomeV1::Ambiguous(SetupModeAmbiguousReasonV1::MultipleEligibleModes)
    ));

    let request = McpCliSearchRequestV1 {
        schema_version: 1,
        request_id: "search-one".to_owned(),
        query: McpCliSearchQueryV1::BoundedFuzzyIntent("inspect work".to_owned()),
        finite_bound: 1,
        expected_release_ref: "candidate:release:one".to_owned(),
        expected_public_catalog_ref: "candidate:public-catalog:one".to_owned(),
    };
    let envelope = McpCliSearchEnvelopeV1 {
        schema_version: 1,
        request_id: "search-one".to_owned(),
        running_binary_release: "candidate:release:one".to_owned(),
        binary_digest: [8; 32],
        binary_version: "0.107.0".to_owned(),
        executable_slot: "candidate:executable-slot:one".to_owned(),
        core_catalog_ref: "candidate:core-catalog:one".to_owned(),
        public_catalog_ref: "candidate:public-catalog:one".to_owned(),
        catalog_snapshot_ref: "candidate:catalog-snapshot:one".to_owned(),
        completeness: McpCliSearchCompletenessV1::BoundedTruncated,
        bounds: McpCliSearchBoundsV1 {
            requested_bound: 1,
            returned_count: 1,
            total_matching_count: 2,
        },
        cursor: Some("cursor:next-page".to_owned()),
        hits: vec![McpCliSearchHitV1::PureRead {
            exact_command_ref: "candidate:command:inspect".to_owned(),
        }],
    };
    envelope.validate_for(&request).expect("cursor parity");
    let mut missing_cursor = envelope;
    missing_cursor.cursor = None;
    assert!(missing_cursor.validate_for(&request).is_err());

    assert_eq!(SETUP_BLOCKED_REASONS_V1.len(), 21);
    assert_eq!(SETUP_COMPATIBILITY_ACTION_ROWS_V1, 145);
    assert_eq!(SETUP_COMPATIBILITY_CEREMONY_ROWS_V1, 11);
}

#[test]
fn capability_and_activation_literals_preserve_exact_selected_closures() {
    assert_eq!(PUBLIC_SKILL_IDS_V1, ["maestro"]);
    assert_eq!(INSTRUCTION_RESOURCE_PATHS_V1.len(), 31);
    assert_eq!(METHOD_NAMES_V1.len(), 17);
    assert_eq!(JOB_METHOD_POSITIVE_CELLS_V1, 19);
    assert_eq!(JOB_METHOD_NEGATIVE_CELLS_V1, 100);
    assert_eq!(SKILL_LEDGER_ROWS_V1, 35);
    assert!(job_method_is_admitted(JobV1::Execute, MethodV1::Tdd));
    assert!(!job_method_is_admitted(JobV1::Setup, MethodV1::Tdd));
    assert_eq!(exact_review_subset_counts(), (13, 27));

    let resource = capability_resource(INSTRUCTION_RESOURCE_PATHS_V1[0]);
    let profile = ContextBudgetProfileV1 {
        schema_version: 1,
        profile_id: "agents-compatible-cli".to_owned(),
        release_ref: "candidate:release:one".to_owned(),
        host_ref: "candidate:host:agents-compatible-cli".to_owned(),
        renderer_or_meter_ref: "candidate:meter:utf8-bytes:v1".to_owned(),
        measurement_procedure_ref: "candidate:procedure:release-host-measurement:v1".to_owned(),
        admitted_resource_refs: vec![resource.resource_ref.clone()],
        maximum_utf8_bytes: 4096,
        maximum_host_observed_units: 4096,
        measurements: vec![ContextBudgetMeasurementV1 {
            closure_ref: "candidate:closure:root-only".to_owned(),
            ordered_resource_refs: vec![resource.resource_ref.clone()],
            utf8_bytes: 1024,
            host_observed_units: 1024,
        }],
    };
    profile.validate().expect("Release/host evidence profile");

    let route = bootstrap_setup_route();
    let route_ref = route.semantic_ref().expect("route commitment");
    let job_resource = capability_resource(JobV1::Setup.resource_path());
    let capability_resolution = CapabilityMethodResolutionV1 {
        schema_version: 1,
        resolution_basis_ref: "candidate:capability-resolution:one".to_owned(),
        exact_selected_job_route_ref: route_ref,
        exact_intent_ref: "candidate:capability-intent:job-only".to_owned(),
        outcome: CapabilityMethodResolutionOutcomeV1::Selected(CapabilityInstructionLoadPlanV1 {
            selected_job_resource_ref: job_resource.clone(),
            direct_method_resource_refs: Vec::new(),
            tdd_child_resource_refs: Vec::new(),
            research_example_resource_ref: None,
        }),
    };
    let subject = SkillActivationSubjectV1 {
        schema_version: 1,
        activation_acquisition_id: "activation-acquisition-0001".to_owned(),
        acquisition_context: SkillActivationAcquisitionContextV1::Bootstrap(
            SkillActivationBootstrapContextV1::RepositoryBootstrap {
                exact_bootstrap_context_ref: "candidate:bootstrap-context:one".to_owned(),
            },
        ),
        release_ref: "candidate:release:one".to_owned(),
        root_skill_resource_ref: capability_resource("skills/maestro/SKILL.md").resource_ref,
    };
    let payload = SkillActivationPayloadV1 {
        selected_route: route,
        capability_resolution,
        recipe_resolution: SkillActivationRecipeResolutionV1::BootstrapNoRecipe,
        context_budget_profile_ref: "candidate:context-budget-profile:agents-cli".to_owned(),
        loaded_resource_closure: LoadedResourceClosureV1 {
            job_resource_ref: job_resource.resource_ref,
            direct_method_resource_refs: Vec::new(),
            tdd_child_resource_refs: Vec::new(),
            research_example_resource_ref: None,
            recipe_resource_refs: Vec::new(),
            closure_digest: [0; 32],
        },
    };
    let candidate = SkillActivationCandidateV1::create(subject, payload)
        .expect("complete Selected bootstrap closure creates a candidate");
    candidate
        .validate()
        .expect("candidate commitments reproduce");
    let mut zeroed = candidate;
    zeroed.candidate_commitment = [0; 32];
    assert!(zeroed.validate().is_err());
    assert_eq!(SKILL_ACTIVATION_OBSERVATION_KIND_TAG_V1, 12);
    assert_eq!(SKILL_ACTIVATION_PUBLISH_ACTION_TAG_V1, 39);
    assert_eq!(SKILL_ACTIVATION_PREDECESSOR_PUBLISH_ACTION_TAG_V1, 30);
    assert_eq!(SKILL_ACTIVATION_OBSERVATION_KIND_COUNT_V1, 43);
    assert_eq!(SKILL_ACTIVATION_ACTION_LEAF_COUNT_V1, 145);
    assert_eq!(SKILL_ACTIVATION_EFFECT_ORIGIN_COUNT_V1, 23);
}

#[test]
fn operation_and_legacy_import_envelopes_keep_disjoint_non_promoting_branches() {
    let request = OperationRequestV1::Action(ActionRequestV1 {
        schema_version: 1,
        request_id: "operation-request-one".to_owned(),
        idempotency_key: "idempotency-key-one".to_owned(),
        semantic_request_hash: [9; 32],
        selected_packet_semantic_hash: [10; 32],
        action_spec: action_spec(),
        material_dependency_stamp: [11; 32],
        exact_store_generation_ref: "candidate:generation:one".to_owned(),
        exact_authority_epoch_ref: "candidate:authority-epoch:one".to_owned(),
        valid_until_ref: "candidate:validity:one".to_owned(),
        authority_basis: ActionAuthorityBasisV1::Ordinary {
            verified_principal_ref: "candidate:principal:one".to_owned(),
            current_session_ref: "candidate:session:one".to_owned(),
            live_grant_refs: Vec::new(),
            required_mandate_refs: Vec::new(),
        },
        typed_input_cbor: vec![0x81, 0x01],
        evidence_refs: Vec::new(),
        prerequisite_receipt_refs: Vec::new(),
        orchestration_attribution: None,
    });
    request.validate().expect("typed Action request");
    let result = OperationResultV1::Action(ActionResultV1(OperationResultBodyV1 {
        schema_version: 1,
        request_id: "operation-request-one".to_owned(),
        operation_spec_ref: "candidate:action-spec:distribution-install".to_owned(),
        outcome: OperationSemanticOutcomeV1::Committed,
        before_revision_refs: Vec::new(),
        after_revision_refs: vec!["candidate:revision:one".to_owned()],
        transition_receipt_refs: vec!["candidate:receipt:one".to_owned()],
        produced_record_refs: Vec::new(),
        next_packet: None,
        inspect_ref: None,
        replayed_delivery: false,
    }));
    result
        .validate_for(&request)
        .expect("matching Action result");
    let wrong_branch = match result {
        OperationResultV1::Action(ActionResultV1(body)) => {
            OperationResultV1::Ceremony(CeremonyResultV1(body))
        }
        OperationResultV1::Ceremony(_) => unreachable!(),
    };
    assert!(wrong_branch.validate_for(&request).is_err());

    let import = LegacySkillActivationImportV1 {
        schema_version: 1,
        source_format: "FORMAT-RUN-EVENT-V1".to_owned(),
        source_file_hash: [12; 32],
        source_path_bytes: b".maestro/runs/events.jsonl".to_vec(),
        record_ordinal: 0,
        byte_start: 0,
        byte_length: 128,
        newline_state: LegacyNewlineStateV1::Terminated,
        raw_record_hash: [13; 32],
        parse_status: LegacyActivationParseStatusV1::CompleteRecognized,
        raw_event_spelling: Some(b"SkillActivation".to_vec()),
        skill_name: Some(b"maestro-audit".to_vec()),
        session_annotation: Some(b"session-old".to_vec()),
        agent_runtime_annotation: Some(b"codex".to_vec()),
        activation_mode_annotation: Some(b"agent_selected".to_vec()),
        timestamp_annotations: vec![b"2026-07-14T00:00:00Z".to_vec()],
        disposition: LegacySkillActivationDispositionV1::MappedHistoricalNonBearer,
        reason: LegacySkillActivationImportReasonV1::RetiredSkillName,
    };
    import.validate().expect("non-promoting historical import");
    assert_ne!(import.rerun_identity().expect("range identity"), [0; 32]);
    assert_eq!(LEGACY_INACTIVE_SKILL_NAMES_V1.len(), 8);
}
