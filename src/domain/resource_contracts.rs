//! Resource contract registry primitives for shipped Maestro artifacts.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceFamily {
    pub id: &'static str,
    pub source_path: &'static str,
    pub ownership_mode: ResourceOwnershipMode,
    pub contract_stamp_source: ContractStampSource,
    pub parser: SemanticKernel,
    pub validator: SemanticKernel,
    pub generated_outputs: &'static [GeneratedOutput],
    pub install_sync_policy: InstallSyncPolicy,
    pub compatibility_baseline: CompatibilityBaselineBehavior,
}

impl ResourceFamily {
    pub fn has_generated_output(&self, output: GeneratedOutput) -> bool {
        self.generated_outputs.contains(&output)
    }

    pub fn outputs_are_derived_from_validated_semantic_model(&self) -> bool {
        !self.generated_outputs.is_empty()
            && !self.parser.owner.trim().is_empty()
            && !self.parser.responsibility.trim().is_empty()
            && !self.validator.owner.trim().is_empty()
            && !self.validator.responsibility.trim().is_empty()
    }

    pub fn requires_semantic_release_check(&self) -> bool {
        self.compatibility_baseline == CompatibilityBaselineBehavior::SemanticReleasedSnapshot
    }
}

pub fn shipped_resource_families() -> &'static [ResourceFamily] {
    SHIPPED_RESOURCE_FAMILIES
}

const SCHEMA_OUTPUTS: &[GeneratedOutput] = &[
    GeneratedOutput::ResourceGuardRows,
    GeneratedOutput::ResourceInventory,
    GeneratedOutput::CompatibilityReport,
    GeneratedOutput::FixtureCoverage,
];

const LOOP_RECIPE_OUTPUTS: &[GeneratedOutput] = &[
    GeneratedOutput::ResourceGuardRows,
    GeneratedOutput::ResourceInventory,
    GeneratedOutput::CompatibilityReport,
    GeneratedOutput::FixtureCoverage,
];

const SKILL_OUTPUTS: &[GeneratedOutput] = &[
    GeneratedOutput::ResourceGuardRows,
    GeneratedOutput::ResourceInventory,
    GeneratedOutput::InstalledMirror,
    GeneratedOutput::CliReference,
];

const HARNESS_OUTPUTS: &[GeneratedOutput] = &[
    GeneratedOutput::ResourceGuardRows,
    GeneratedOutput::ResourceInventory,
    GeneratedOutput::InstalledMirror,
];

const EXECUTABLE_RESOURCE_OUTPUTS: &[GeneratedOutput] = &[
    GeneratedOutput::ResourceGuardRows,
    GeneratedOutput::ResourceInventory,
    GeneratedOutput::InstalledMirror,
];

const PROSE_OUTPUTS: &[GeneratedOutput] = &[
    GeneratedOutput::ResourceGuardRows,
    GeneratedOutput::ResourceInventory,
];

const GENERATED_REFERENCE_OUTPUTS: &[GeneratedOutput] = &[
    GeneratedOutput::CliReference,
    GeneratedOutput::McpReference,
    GeneratedOutput::ResourceInventory,
];

const SHIPPED_RESOURCE_FAMILIES: &[ResourceFamily] = &[
    ResourceFamily {
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
        generated_outputs: SCHEMA_OUTPUTS,
        install_sync_policy: InstallSyncPolicy::BinaryServedReadOnly,
        compatibility_baseline: CompatibilityBaselineBehavior::SemanticReleasedSnapshot,
    },
    ResourceFamily {
        id: "loop-recipes",
        source_path: "embedded/loop-recipes",
        ownership_mode: ResourceOwnershipMode::InspectableContractPack,
        contract_stamp_source: ContractStampSource::ResourceField {
            field: "schema_version",
        },
        parser: SemanticKernel {
            owner: "src/domain/loop_recipes.rs",
            responsibility: "parse maestro.recipe.v2/v3 plus exact embedded profiles into typed recipe contracts",
        },
        validator: SemanticKernel {
            owner: "src/domain/loop_recipes.rs",
            responsibility: "validate typed profile composition and safety monotonicity across phases, transitions, triggers, and return conditions",
        },
        generated_outputs: LOOP_RECIPE_OUTPUTS,
        install_sync_policy: InstallSyncPolicy::BinaryServedReadOnly,
        compatibility_baseline: CompatibilityBaselineBehavior::SemanticReleasedSnapshot,
    },
    ResourceFamily {
        id: "skills",
        source_path: "embedded/skills",
        ownership_mode: ResourceOwnershipMode::InspectableProseOrTemplate,
        contract_stamp_source: ContractStampSource::ResourceFrontmatter { field: "version" },
        parser: SemanticKernel {
            owner: "src/domain/skills/catalog.rs",
            responsibility: "parse bundled skill trees and frontmatter versions",
        },
        validator: SemanticKernel {
            owner: "src/domain/skills/global.rs",
            responsibility: "validate install ownership, unmanaged drift, and global skill sync",
        },
        generated_outputs: SKILL_OUTPUTS,
        install_sync_policy: InstallSyncPolicy::BinaryServedUserOwnedMirror,
        compatibility_baseline: CompatibilityBaselineBehavior::SemanticReleasedSnapshot,
    },
    ResourceFamily {
        id: "harness",
        source_path: "embedded/harness",
        ownership_mode: ResourceOwnershipMode::InspectableProseOrTemplate,
        contract_stamp_source: ContractStampSource::ResourceFrontmatter { field: "version" },
        parser: SemanticKernel {
            owner: "src/domain/harness/templates.rs",
            responsibility: "serve bundled Harness protocol and recovery templates",
        },
        validator: SemanticKernel {
            owner: "src/domain/harness/extract.rs",
            responsibility: "preserve explicit install/update, backup, and no-silent-mutation behavior",
        },
        generated_outputs: HARNESS_OUTPUTS,
        install_sync_policy: InstallSyncPolicy::BinaryServedUserOwnedMirror,
        compatibility_baseline: CompatibilityBaselineBehavior::SemanticReleasedSnapshot,
    },
    ResourceFamily {
        id: "hooks",
        source_path: "embedded/hooks",
        ownership_mode: ResourceOwnershipMode::ExecutableResourceBytes,
        contract_stamp_source: ContractStampSource::LineMarker {
            prefix: "# maestro:hook-version:",
        },
        parser: SemanticKernel {
            owner: "src/domain/extraction/hook_script.rs",
            responsibility: "extract shipped hook script bytes with managed placeholders",
        },
        validator: SemanticKernel {
            owner: "src/domain/run/record.rs",
            responsibility: "normalize hook events and preserve append tolerance",
        },
        generated_outputs: EXECUTABLE_RESOURCE_OUTPUTS,
        install_sync_policy: InstallSyncPolicy::BinaryServedUserOwnedMirror,
        compatibility_baseline: CompatibilityBaselineBehavior::SemanticReleasedSnapshot,
    },
    ResourceFamily {
        id: "shell",
        source_path: "embedded/shell",
        ownership_mode: ResourceOwnershipMode::ExecutableResourceBytes,
        contract_stamp_source: ContractStampSource::ResourceManifest {
            owner: "src/domain/extraction/mod.rs",
        },
        parser: SemanticKernel {
            owner: "src/domain/extraction/mod.rs",
            responsibility: "extract bundled shell resources through the shared resource extractor",
        },
        validator: SemanticKernel {
            owner: "src/domain/extraction/extract.rs",
            responsibility: "enforce backup, symlink, and rollback safety for extracted resources",
        },
        generated_outputs: EXECUTABLE_RESOURCE_OUTPUTS,
        install_sync_policy: InstallSyncPolicy::BinaryServedUserOwnedMirror,
        compatibility_baseline: CompatibilityBaselineBehavior::RuntimeValidationOnly,
    },
    ResourceFamily {
        id: "playbook",
        source_path: "embedded/playbook",
        ownership_mode: ResourceOwnershipMode::InspectableProseOrTemplate,
        contract_stamp_source: ContractStampSource::None,
        parser: SemanticKernel {
            owner: "src/domain/playbook.rs",
            responsibility: "serve bundled code playbooks by token without repo initialization",
        },
        validator: SemanticKernel {
            owner: "src/domain/playbook.rs",
            responsibility: "fail loudly on unknown language tokens and expose valid playbook tokens",
        },
        generated_outputs: PROSE_OUTPUTS,
        install_sync_policy: InstallSyncPolicy::BinaryServedReadOnly,
        compatibility_baseline: CompatibilityBaselineBehavior::SemanticReleasedSnapshot,
    },
    ResourceFamily {
        id: "design",
        source_path: "embedded/design",
        ownership_mode: ResourceOwnershipMode::InspectableProseOrTemplate,
        contract_stamp_source: ContractStampSource::ResourceManifest {
            owner: "src/domain/design.rs",
        },
        parser: SemanticKernel {
            owner: "src/domain/design.rs",
            responsibility: "serve bundled DESIGN.md styles and upstream source metadata",
        },
        validator: SemanticKernel {
            owner: "src/domain/design.rs",
            responsibility: "validate style tokens, source metadata, and design template availability",
        },
        generated_outputs: PROSE_OUTPUTS,
        install_sync_policy: InstallSyncPolicy::BinaryServedUserOwnedMirror,
        compatibility_baseline: CompatibilityBaselineBehavior::SemanticReleasedSnapshot,
    },
    ResourceFamily {
        id: "cli-mcp-references",
        source_path: "src/interfaces",
        ownership_mode: ResourceOwnershipMode::GeneratedReferenceOutput,
        contract_stamp_source: ContractStampSource::GeneratedFromCode,
        parser: SemanticKernel {
            owner: "src/interfaces/cli/reference.rs",
            responsibility: "derive CLI reference text from the clap command model",
        },
        validator: SemanticKernel {
            owner: "tests/cli_reference_freshness.rs",
            responsibility: "verify generated CLI references are fresh against code-owned commands",
        },
        generated_outputs: GENERATED_REFERENCE_OUTPUTS,
        install_sync_policy: InstallSyncPolicy::GeneratedReferenceOnly,
        compatibility_baseline: CompatibilityBaselineBehavior::GeneratedFromCode,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOwnershipMode {
    InspectableContractPack,
    InspectableProseOrTemplate,
    ExecutableResourceBytes,
    GeneratedReferenceOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractStampSource {
    RustConstants { owner: &'static str },
    ResourceFrontmatter { field: &'static str },
    LineMarker { prefix: &'static str },
    ResourceField { field: &'static str },
    ResourceManifest { owner: &'static str },
    GeneratedFromCode,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticKernel {
    pub owner: &'static str,
    pub responsibility: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedOutput {
    ResourceGuardRows,
    ResourceInventory,
    CliReference,
    McpReference,
    CompatibilityReport,
    FixtureCoverage,
    InstalledMirror,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallSyncPolicy {
    BinaryServedUserOwnedMirror,
    BinaryServedReadOnly,
    GeneratedReferenceOnly,
    NotInstalled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityBaselineBehavior {
    SemanticReleasedSnapshot,
    RuntimeValidationOnly,
    HashAcknowledgementOnly,
    GeneratedFromCode,
}
