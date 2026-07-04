use maestro::domain::resource_contracts::{
    CompatibilityBaselineBehavior, ContractStampSource, GeneratedOutput, InstallSyncPolicy,
    ResourceFamily, ResourceOwnershipMode, SemanticKernel, shipped_resource_families,
};

#[test]
fn resource_family_model_captures_contract_kernel_axes() {
    let family = ResourceFamily {
        id: "schemas",
        source_path: "embedded/schemas",
        ownership_mode: ResourceOwnershipMode::InspectableContractPack,
        contract_stamp_source: ContractStampSource::RustConstants {
            owner: "src/foundation/core/schema.rs",
        },
        parser: SemanticKernel {
            owner: "src/domain/schema_contracts/catalog.rs",
            responsibility: "parse embedded schema packs into typed schema contracts",
        },
        validator: SemanticKernel {
            owner: "src/domain/schema_contracts/validate.rs",
            responsibility: "validate stamps, retired names, supported reads, and fixtures",
        },
        generated_outputs: &[GeneratedOutput::ResourceGuardRows],
        install_sync_policy: InstallSyncPolicy::BinaryServedUserOwnedMirror,
        compatibility_baseline: CompatibilityBaselineBehavior::SemanticReleasedSnapshot,
    };

    assert_eq!(family.id, "schemas");
    assert_eq!(family.source_path, "embedded/schemas");
    assert_eq!(
        family.ownership_mode,
        ResourceOwnershipMode::InspectableContractPack
    );
    assert!(family.has_generated_output(GeneratedOutput::ResourceGuardRows));
    assert_eq!(
        family.compatibility_baseline,
        CompatibilityBaselineBehavior::SemanticReleasedSnapshot
    );
}

#[test]
fn shipped_resource_families_cover_locked_architecture_families() {
    let families = shipped_resource_families();
    let ids = families.iter().map(|family| family.id).collect::<Vec<_>>();

    assert_eq!(
        ids,
        vec![
            "schemas",
            "loop-recipes",
            "skills",
            "harness",
            "hooks",
            "shell",
            "playbook",
            "design",
            "cli-mcp-references",
        ]
    );

    for family in families {
        assert!(!family.source_path.trim().is_empty(), "{}", family.id);
        assert!(!family.parser.owner.trim().is_empty(), "{}", family.id);
        assert!(
            !family.parser.responsibility.trim().is_empty(),
            "{}",
            family.id
        );
        assert!(!family.validator.owner.trim().is_empty(), "{}", family.id);
        assert!(
            !family.validator.responsibility.trim().is_empty(),
            "{}",
            family.id
        );
        assert!(
            !family.generated_outputs.is_empty(),
            "{} must declare derived outputs",
            family.id
        );
    }

    let schemas = families
        .iter()
        .find(|family| family.id == "schemas")
        .expect("schemas family is registered");
    assert_eq!(schemas.source_path, "embedded/schemas");
    assert_eq!(
        schemas.ownership_mode,
        ResourceOwnershipMode::InspectableContractPack
    );
    assert!(matches!(
        schemas.contract_stamp_source,
        ContractStampSource::RustConstants {
            owner: "src/foundation/core/schema.rs"
        }
    ));
    assert_eq!(
        schemas.compatibility_baseline,
        CompatibilityBaselineBehavior::SemanticReleasedSnapshot
    );

    let loop_recipes = families
        .iter()
        .find(|family| family.id == "loop-recipes")
        .expect("loop-recipes family is registered");
    assert_eq!(loop_recipes.source_path, "embedded/loop-recipes");
    assert_eq!(
        loop_recipes.ownership_mode,
        ResourceOwnershipMode::InspectableContractPack
    );
    assert_eq!(loop_recipes.parser.owner, "src/domain/loop_recipes.rs");
    assert_eq!(loop_recipes.validator.owner, "src/domain/loop_recipes.rs");

    let hooks = families
        .iter()
        .find(|family| family.id == "hooks")
        .expect("hooks family is registered");
    assert!(matches!(
        hooks.contract_stamp_source,
        ContractStampSource::LineMarker {
            prefix: "# maestro:hook-version:"
        }
    ));

    let cli_mcp = families
        .iter()
        .find(|family| family.id == "cli-mcp-references")
        .expect("generated CLI/MCP references are registered");
    assert_eq!(
        cli_mcp.ownership_mode,
        ResourceOwnershipMode::GeneratedReferenceOutput
    );
    assert!(cli_mcp.has_generated_output(GeneratedOutput::CliReference));
    assert!(cli_mcp.has_generated_output(GeneratedOutput::McpReference));
}

#[test]
fn generated_outputs_are_secondary_to_validated_semantic_models() {
    for family in shipped_resource_families() {
        assert!(
            family.outputs_are_derived_from_validated_semantic_model(),
            "{} generated outputs must be secondary to parser+validator kernels",
            family.id
        );
    }
}

#[test]
fn release_compatibility_is_semantic_not_hash_only() {
    let families = shipped_resource_families();
    let semantic_release_ids = families
        .iter()
        .filter(|family| family.requires_semantic_release_check())
        .map(|family| family.id)
        .collect::<Vec<_>>();

    assert!(semantic_release_ids.contains(&"schemas"));
    assert!(semantic_release_ids.contains(&"loop-recipes"));
    assert!(semantic_release_ids.contains(&"skills"));
    assert!(semantic_release_ids.contains(&"harness"));
    assert!(semantic_release_ids.contains(&"playbook"));
    assert!(semantic_release_ids.contains(&"design"));
    assert!(
        families.iter().all(|family| family.compatibility_baseline
            != CompatibilityBaselineBehavior::HashAcknowledgementOnly),
        "hash acknowledgement is not compatibility proof"
    );
}
