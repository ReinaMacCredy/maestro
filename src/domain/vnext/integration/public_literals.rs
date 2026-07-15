use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::capability::literals::{
    CapabilityInstructionLoadPlanV1, CapabilityMethodResolutionOutcomeV1,
    CapabilityMethodResolutionV1, InternalJobV1, SelectedJobRecipeAdmissionOutcomeV1,
    SelectedJobRecipeAdmissionV1,
};
use crate::domain::vnext::orchestration::literals::{
    BoundedContinuationProfileV1, ExactRecipeSelectionV1, RecipeApplicationV1,
    RecipeComponentOutcomeTagV1, RecipeComponentSlotV1, RecipeIdV1, RecipeReturnOccurrenceV1,
    RecipeSelectionRequestV1,
};

pub const MCP_PACKET_READ_REQUEST_FIELD_NAMES_V1: [&str; 9] = [
    "schema_version",
    "request_id",
    "repository_locator",
    "authenticated_host_connection_context_ref",
    "projection_scope",
    "expected_release_ref",
    "expected_public_catalog_ref",
    "bounded_response_redaction_profile",
    "read_mode",
];
pub const MCP_PACKET_READ_ENVELOPE_OUTCOMES_V1: [&str; 6] = [
    "Packet",
    "SelectionContext",
    "NoActiveStore",
    "Unavailable",
    "Stale",
    "Incompatible",
];
pub const MCP_CLI_SEARCH_REQUEST_FIELDS_V1: [&str; 6] = [
    "schema_version",
    "request_id",
    "query",
    "finite_bound",
    "expected_release_ref",
    "expected_public_catalog_ref",
];
pub const MCP_CLI_SEARCH_ENVELOPE_FIELDS_V1: [&str; 13] = [
    "schema_version",
    "request_id",
    "running_binary_release",
    "binary_digest",
    "binary_version",
    "executable_slot",
    "core_catalog_ref",
    "public_catalog_ref",
    "catalog_snapshot_ref",
    "completeness",
    "bounds",
    "cursor",
    "hits",
];
pub const AGENT_PACKET_FIELD_NAMES_V1: [&str; 19] = [
    "schema_version",
    "packet_id",
    "semantic_audit_hash",
    "as_of_ref",
    "valid_until_ref",
    "visibility_ref",
    "scope_manifest",
    "completeness",
    "bounds",
    "snapshot_manifest_ref",
    "projection_result",
    "blockers",
    "advertised_specs",
    "required_inputs",
    "effect_classes",
    "idempotency_classes",
    "retry_classes",
    "inspect_refs",
    "recipe_binding",
];
pub const GLOBAL_MCP_TOOL_NAMES_V1: [&str; 2] = ["maestro_packet", "maestro_cli_search"];
pub const PROJECT_MCP_TOOL_COUNT_V1: usize = 0;
pub const SETUP_MODE_NAMES_V1: [&str; 7] = [
    "Install",
    "Adopt",
    "Migrate",
    "Update",
    "Repair",
    "Rollback",
    "Uninstall",
];
pub const SETUP_AMBIGUOUS_REASONS_V1: [&str; 1] = ["MultipleEligibleModes"];
pub const SETUP_BLOCKED_REASONS_V1: [&str; 21] = [
    "UnsupportedSchemaOrCatalog",
    "ContextLegalityMismatch",
    "LocalityMismatch",
    "CrossDomainAggregate",
    "StaleFactView",
    "StaleOrUnadvertisedOperation",
    "RecoveryRequired",
    "EffectInDoubt",
    "ConflictingOwnerFacts",
    "UnsafeOrAmbiguousTarget",
    "GenerationUnresolved",
    "MigrationNotReady",
    "TargetOrCustodyUnavailable",
    "DesiredStateUnresolved",
    "NoEligibleSnapshot",
    "AlreadyCurrent",
    "NoEligibleMode",
    "RequestedModeIneligible",
    "OperationModeMismatch",
    "AuthorityUnavailable",
    "CapabilityUnavailable",
];
pub const SETUP_COMPATIBILITY_ACTION_ROWS_V1: usize = 145;
pub const SETUP_COMPATIBILITY_CEREMONY_ROWS_V1: usize = 11;
pub const SKILL_ACTIVATION_OBSERVATION_KIND_TAG_V1: u16 = 12;
pub const SKILL_ACTIVATION_PUBLISH_ACTION_TAG_V1: u16 = 39;
pub const SKILL_ACTIVATION_PREDECESSOR_PUBLISH_ACTION_TAG_V1: u16 = 30;
pub const SKILL_ACTIVATION_OBSERVATION_KIND_COUNT_V1: usize = 43;
pub const SKILL_ACTIVATION_ACTION_LEAF_COUNT_V1: usize = 145;
pub const SKILL_ACTIVATION_EFFECT_ORIGIN_COUNT_V1: usize = 23;
pub const LEGACY_INACTIVE_SKILL_NAMES_V1: [&str; 8] = [
    "ask-maestro",
    "maestro-audit",
    "maestro-card",
    "maestro-design",
    "maestro-research",
    "maestro-setup",
    "maestro-witness",
    "maestro-work",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectionScopeV1 {
    Repository,
    Work { exact_work_ref: String },
}

impl ProjectionScopeV1 {
    fn validate(&self) -> Result<(), PublicLiteralError> {
        match self {
            Self::Repository => Ok(()),
            Self::Work { exact_work_ref } if is_current_ref(exact_work_ref) => Ok(()),
            Self::Work { .. } => Err(PublicLiteralError::InvalidPacketReadRequest),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpPacketReadModeV1 {
    BootstrapNoRecipeV1,
    DiscoverSelectionContextV1,
    ProjectV1(RecipeSelectionRequestV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpPacketReadRequestV1 {
    pub schema_version: u64,
    pub request_id: String,
    pub repository_locator: String,
    pub authenticated_host_connection_context_ref: String,
    pub projection_scope: ProjectionScopeV1,
    pub expected_release_ref: String,
    pub expected_public_catalog_ref: String,
    pub bounded_response_redaction_profile: String,
    pub read_mode: McpPacketReadModeV1,
}

impl McpPacketReadRequestV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        if self.schema_version != 1
            || self.request_id.is_empty()
            || self.repository_locator.is_empty()
            || !is_current_ref(&self.authenticated_host_connection_context_ref)
            || !is_current_ref(&self.expected_release_ref)
            || !is_current_ref(&self.expected_public_catalog_ref)
            || self.bounded_response_redaction_profile.is_empty()
        {
            return Err(PublicLiteralError::InvalidPacketReadRequest);
        }
        self.projection_scope.validate()?;
        match (&self.projection_scope, &self.read_mode) {
            (ProjectionScopeV1::Work { .. }, McpPacketReadModeV1::BootstrapNoRecipeV1) => {
                Err(PublicLiteralError::InvalidPacketReadRequest)
            }
            (_, McpPacketReadModeV1::ProjectV1(request)) => {
                request.validate()?;
                if request.resolution_basis_ref != self.expected_public_catalog_ref {
                    return Err(PublicLiteralError::InvalidPacketReadRequest);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecipeSelectionOptionV1 {
    pub primary_selection: ExactRecipeSelectionV1,
    pub continuation_selection: ExactRecipeSelectionV1,
}

impl RecipeSelectionOptionV1 {
    pub fn validate_against(&self, resolution_basis_ref: &str) -> Result<(), PublicLiteralError> {
        RecipeSelectionRequestV1 {
            schema_version: 1,
            resolution_basis_ref: resolution_basis_ref.to_owned(),
            primary_selection: self.primary_selection.clone(),
            continuation_selection: self.continuation_selection.clone(),
        }
        .validate()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionContextV1 {
    pub schema_version: u64,
    pub resolution_basis_ref: String,
    pub selection_options: Vec<RecipeSelectionOptionV1>,
}

impl SelectionContextV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        if self.schema_version != 1
            || !is_current_ref(&self.resolution_basis_ref)
            || self.selection_options.len() != 30
        {
            return Err(PublicLiteralError::InvalidSelectionContext);
        }

        let expected_primary = std::iter::once(None)
            .chain(RecipeIdV1::PRIMARY.into_iter().map(Some))
            .flat_map(|primary| {
                std::iter::once(None)
                    .chain(BoundedContinuationProfileV1::ALL.into_iter().map(Some))
                    .map(move |continuation| (primary, continuation))
            })
            .collect::<Vec<_>>();
        let actual = self
            .selection_options
            .iter()
            .map(|option| {
                option.validate_against(&self.resolution_basis_ref)?;
                Ok((
                    option.primary_selection.primary_recipe(),
                    option.continuation_selection.continuation_profile(),
                ))
            })
            .collect::<Result<Vec<_>, PublicLiteralError>>()?;
        if actual != expected_primary {
            return Err(PublicLiteralError::InvalidSelectionContext);
        }

        let mut recipe_identities = BTreeMap::new();
        let mut profile_identities = BTreeMap::new();
        for option in &self.selection_options {
            record_recipe_selection_identity(&option.primary_selection, &mut recipe_identities)?;
            record_recipe_selection_identity(
                &option.continuation_selection,
                &mut recipe_identities,
            )?;
            if let ExactRecipeSelectionV1::Continuation {
                profile_resource_ref,
                ..
            } = &option.continuation_selection
            {
                let profile = BoundedContinuationProfileV1::from_resource_ref(profile_resource_ref)
                    .ok_or(PublicLiteralError::InvalidSelectionContext)?;
                if profile_identities
                    .insert(profile, profile_resource_ref.clone())
                    .is_some_and(|prior| prior != *profile_resource_ref)
                {
                    return Err(PublicLiteralError::InvalidSelectionContext);
                }
            }
        }
        if recipe_identities.len() != 10 || profile_identities.len() != 2 {
            return Err(PublicLiteralError::InvalidSelectionContext);
        }
        Ok(())
    }
}

fn record_recipe_selection_identity(
    selection: &ExactRecipeSelectionV1,
    identities: &mut BTreeMap<RecipeIdV1, (String, String)>,
) -> Result<(), PublicLiteralError> {
    let (recipe, resource_ref, manifest_ref) = match selection {
        ExactRecipeSelectionV1::Absent => return Ok(()),
        ExactRecipeSelectionV1::Primary {
            recipe_resource_ref,
            manifest_content_ref,
        }
        | ExactRecipeSelectionV1::Continuation {
            recipe_resource_ref,
            manifest_content_ref,
            ..
        } => (
            RecipeIdV1::from_resource_ref(recipe_resource_ref)
                .ok_or(PublicLiteralError::InvalidSelectionContext)?,
            recipe_resource_ref,
            manifest_content_ref,
        ),
    };
    let identity = (resource_ref.clone(), manifest_ref.clone());
    if identities
        .insert(recipe, identity.clone())
        .is_some_and(|prior| prior != identity)
    {
        return Err(PublicLiteralError::InvalidSelectionContext);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapContextV1 {
    RepositoryBootstrap,
    InstallationBootstrap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapRouteFactViewV1 {
    pub schema_version: u64,
    pub bootstrap_context: BootstrapContextV1,
    pub resolution_basis_ref: String,
    pub ordered_source_fact_commitments: Vec<String>,
    pub fact_view_hash: [u8; 32],
}

impl BootstrapRouteFactViewV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        if self.schema_version != 1
            || !is_current_ref(&self.resolution_basis_ref)
            || self.ordered_source_fact_commitments.is_empty()
            || !is_strictly_ordered_unique(&self.ordered_source_fact_commitments)
            || self.fact_view_hash != self.semantic_hash_without_hash()
        {
            return Err(PublicLiteralError::InvalidBootstrapFactView);
        }
        Ok(())
    }

    pub fn semantic_hash_without_hash(&self) -> [u8; 32] {
        let mut value = Vec::new();
        encode_array_len(4, &mut value);
        encode_u64(self.schema_version, &mut value);
        encode_u64(self.bootstrap_context as u64 + 1, &mut value);
        encode_text(&self.resolution_basis_ref, &mut value);
        encode_text_array(&self.ordered_source_fact_commitments, &mut value);
        domain_hash("maestro.vnext.bootstrap-route-fact-view.v1", &value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketRecipeComponentSlotV1 {
    Primary,
    Continuation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketRecipeComponentProvenanceV1 {
    pub component_slot: PacketRecipeComponentSlotV1,
    pub recipe_return_occurrence: RecipeReturnOccurrenceV1,
    pub component_output_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketRecipeAdviceOutcomeV1 {
    CoreOnly,
    NotApplicable,
    RestrictiveAdvice,
    HardStop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketRecipeAdviceProvenanceV1 {
    pub composition_outcome: PacketRecipeAdviceOutcomeV1,
    pub ordered_component_output_hashes: Vec<[u8; 32]>,
    pub composed_output_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketRecipeBindingV1 {
    pub schema_version: u64,
    pub selection_request_hash: [u8; 32],
    pub recipe_application: RecipeApplicationV1,
    pub recipe_application_hash: [u8; 32],
    pub component_provenance: Vec<PacketRecipeComponentProvenanceV1>,
    pub advice_provenance: PacketRecipeAdviceProvenanceV1,
}

impl PacketRecipeBindingV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        self.recipe_application.validate()?;
        if self.schema_version != 1
            || self.selection_request_hash
                != self
                    .recipe_application
                    .selection_request()
                    .semantic_hash()?
            || self.recipe_application_hash != self.recipe_application.semantic_hash()?
            || self.component_provenance.len() != self.recipe_application.component_count()
            || self.component_provenance.len() > 2
        {
            return Err(PublicLiteralError::InvalidPacketRecipeBinding);
        }

        let expected_slots = match (
            self.recipe_application.primary.is_absent(),
            self.recipe_application.continuation.is_absent(),
        ) {
            (true, true) => Vec::new(),
            (false, true) => vec![PacketRecipeComponentSlotV1::Primary],
            (true, false) => vec![PacketRecipeComponentSlotV1::Continuation],
            (false, false) => vec![
                PacketRecipeComponentSlotV1::Primary,
                PacketRecipeComponentSlotV1::Continuation,
            ],
        };
        let actual_slots = self
            .component_provenance
            .iter()
            .map(|row| row.component_slot)
            .collect::<Vec<_>>();
        if actual_slots != expected_slots {
            return Err(PublicLiteralError::InvalidPacketRecipeBinding);
        }

        for row in &self.component_provenance {
            row.recipe_return_occurrence.validate()?;
            let selection = match row.component_slot {
                PacketRecipeComponentSlotV1::Primary => &self.recipe_application.primary,
                PacketRecipeComponentSlotV1::Continuation => &self.recipe_application.continuation,
            };
            let expected_slot = match row.component_slot {
                PacketRecipeComponentSlotV1::Primary => RecipeComponentSlotV1::Primary,
                PacketRecipeComponentSlotV1::Continuation => RecipeComponentSlotV1::Continuation,
            };
            if row.recipe_return_occurrence.component.slot() != expected_slot
                || !row
                    .recipe_return_occurrence
                    .component
                    .matches_selection(selection)
                || row.recipe_return_occurrence.resolution_basis_ref
                    != self.recipe_application.resolution_basis_ref
                || row.recipe_return_occurrence.frontier_ref != self.recipe_application.frontier_ref
                || is_zero_digest(&row.component_output_hash)
            {
                return Err(PublicLiteralError::InvalidPacketRecipeBinding);
            }
        }

        let component_hashes = self
            .component_provenance
            .iter()
            .map(|row| row.component_output_hash)
            .collect::<Vec<_>>();
        if self.advice_provenance.ordered_component_output_hashes != component_hashes
            || is_zero_digest(&self.advice_provenance.composed_output_hash)
        {
            return Err(PublicLiteralError::InvalidPacketRecipeBinding);
        }
        let outcomes = self
            .component_provenance
            .iter()
            .map(|row| row.recipe_return_occurrence.outcome_tag)
            .collect::<Vec<_>>();
        let valid_composition = if outcomes.is_empty() {
            self.advice_provenance.composition_outcome == PacketRecipeAdviceOutcomeV1::CoreOnly
        } else if outcomes.contains(&RecipeComponentOutcomeTagV1::HardStop) {
            self.advice_provenance.composition_outcome == PacketRecipeAdviceOutcomeV1::HardStop
        } else if outcomes.contains(&RecipeComponentOutcomeTagV1::NotApplicable) {
            matches!(
                self.advice_provenance.composition_outcome,
                PacketRecipeAdviceOutcomeV1::NotApplicable | PacketRecipeAdviceOutcomeV1::HardStop
            )
        } else {
            matches!(
                self.advice_provenance.composition_outcome,
                PacketRecipeAdviceOutcomeV1::RestrictiveAdvice
                    | PacketRecipeAdviceOutcomeV1::HardStop
            )
        };
        if !valid_composition {
            return Err(PublicLiteralError::InvalidPacketRecipeBinding);
        }
        Ok(())
    }

    pub fn is_actionable(&self) -> bool {
        matches!(
            self.advice_provenance.composition_outcome,
            PacketRecipeAdviceOutcomeV1::CoreOnly | PacketRecipeAdviceOutcomeV1::RestrictiveAdvice
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PacketScopeManifestV1 {
    Repository {
        exact_repository_domain_ref: String,
    },
    Work {
        exact_repository_domain_ref: String,
        exact_work_ref: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketCompletenessV1 {
    Complete,
    IncompleteBlocked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketBoundsV1 {
    pub maximum_bytes: u64,
    pub maximum_rows: u64,
    pub maximum_depth: u64,
    pub actual_bytes: u64,
    pub actual_rows: u64,
    pub actual_depth: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PacketProjectionResultV1 {
    Action {
        frontier_ref: String,
        exact_action_spec_ref: String,
    },
    Wave {
        frontier_ref: String,
        exact_wave_ref: String,
        exact_action_spec_refs: Vec<String>,
    },
    PlanningRequired {
        frontier_ref: String,
        exact_inspect_ref: String,
    },
    Inspect {
        frontier_ref: String,
        exact_inspect_ref: String,
    },
    Wait {
        frontier_ref: String,
        exact_reason_ref: String,
    },
    Stop {
        frontier_ref: String,
        exact_reason_ref: String,
    },
}

impl PacketProjectionResultV1 {
    pub fn frontier_ref(&self) -> &str {
        match self {
            Self::Action { frontier_ref, .. }
            | Self::Wave { frontier_ref, .. }
            | Self::PlanningRequired { frontier_ref, .. }
            | Self::Inspect { frontier_ref, .. }
            | Self::Wait { frontier_ref, .. }
            | Self::Stop { frontier_ref, .. } => frontier_ref,
        }
    }

    fn validate(&self) -> bool {
        if !is_current_ref(self.frontier_ref()) {
            return false;
        }
        match self {
            Self::Action {
                exact_action_spec_ref,
                ..
            } => is_current_ref(exact_action_spec_ref),
            Self::Wave {
                exact_wave_ref,
                exact_action_spec_refs,
                ..
            } => {
                is_current_ref(exact_wave_ref)
                    && !exact_action_spec_refs.is_empty()
                    && is_strictly_ordered_unique(exact_action_spec_refs)
            }
            Self::PlanningRequired {
                exact_inspect_ref, ..
            }
            | Self::Inspect {
                exact_inspect_ref, ..
            } => is_current_ref(exact_inspect_ref),
            Self::Wait {
                exact_reason_ref, ..
            }
            | Self::Stop {
                exact_reason_ref, ..
            } => is_current_ref(exact_reason_ref),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdvertisedOperationSpecV1 {
    pub operation_spec: OperationSpecRefV1,
    pub material_dependency_stamp: [u8; 32],
    pub live_guard_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentPacketV1 {
    pub schema_version: u64,
    pub packet_id: String,
    pub semantic_audit_hash: [u8; 32],
    pub as_of_ref: String,
    pub valid_until_ref: String,
    pub visibility_ref: String,
    pub scope_manifest: PacketScopeManifestV1,
    pub completeness: PacketCompletenessV1,
    pub bounds: PacketBoundsV1,
    pub snapshot_manifest_ref: String,
    pub projection_result: PacketProjectionResultV1,
    pub blockers: Vec<String>,
    pub advertised_specs: Vec<AdvertisedOperationSpecV1>,
    pub required_inputs: Vec<String>,
    pub effect_classes: Vec<String>,
    pub idempotency_classes: Vec<String>,
    pub retry_classes: Vec<String>,
    pub inspect_refs: Vec<String>,
    pub recipe_binding: PacketRecipeBindingV1,
}

impl AgentPacketV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        self.recipe_binding.validate()?;
        if self.schema_version != 1
            || !is_current_ref(&self.packet_id)
            || is_zero_digest(&self.semantic_audit_hash)
            || !is_current_ref(&self.as_of_ref)
            || !is_current_ref(&self.valid_until_ref)
            || !is_current_ref(&self.visibility_ref)
            || !is_current_ref(&self.snapshot_manifest_ref)
            || !self.projection_result.validate()
            || self.projection_result.frontier_ref()
                != self.recipe_binding.recipe_application.frontier_ref
            || !self.bounds.validate()
            || !all_unique(&self.blockers)
            || !all_unique(&self.required_inputs)
            || !all_unique(&self.effect_classes)
            || !all_unique(&self.idempotency_classes)
            || !all_unique(&self.retry_classes)
            || !all_unique(&self.inspect_refs)
            || !self.validate_scope()
        {
            return Err(PublicLiteralError::InvalidAgentPacket);
        }
        for spec in &self.advertised_specs {
            if spec.operation_spec.validate().is_err()
                || is_zero_digest(&spec.material_dependency_stamp)
                || !all_unique(&spec.live_guard_refs)
            {
                return Err(PublicLiteralError::InvalidAgentPacket);
            }
        }
        if !self.recipe_binding.is_actionable() && !self.advertised_specs.is_empty() {
            return Err(PublicLiteralError::InvalidAgentPacket);
        }
        Ok(())
    }

    fn validate_scope(&self) -> bool {
        match &self.scope_manifest {
            PacketScopeManifestV1::Repository {
                exact_repository_domain_ref,
            } => is_current_ref(exact_repository_domain_ref),
            PacketScopeManifestV1::Work {
                exact_repository_domain_ref,
                exact_work_ref,
            } => is_current_ref(exact_repository_domain_ref) && is_current_ref(exact_work_ref),
        }
    }
}

impl PacketBoundsV1 {
    fn validate(&self) -> bool {
        self.maximum_bytes > 0
            && self.maximum_rows > 0
            && self.maximum_depth > 0
            && self.actual_bytes <= self.maximum_bytes
            && self.actual_rows <= self.maximum_rows
            && self.actual_depth <= self.maximum_depth
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpPacketReadEnvelopeV1 {
    Packet(Box<AgentPacketV1>),
    SelectionContext(SelectionContextV1),
    NoActiveStore {
        bootstrap_route_fact_view: Option<BootstrapRouteFactViewV1>,
    },
    Unavailable {
        reason_ref: String,
    },
    Stale {
        reason_ref: String,
    },
    Incompatible {
        reason_ref: String,
    },
}

impl McpPacketReadEnvelopeV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        match self {
            Self::Packet(packet) => packet.validate(),
            Self::SelectionContext(context) => context.validate(),
            Self::NoActiveStore {
                bootstrap_route_fact_view: Some(view),
            } => view.validate(),
            Self::NoActiveStore {
                bootstrap_route_fact_view: None,
            } => Ok(()),
            Self::Unavailable { reason_ref }
            | Self::Stale { reason_ref }
            | Self::Incompatible { reason_ref }
                if is_current_ref(reason_ref) =>
            {
                Ok(())
            }
            _ => Err(PublicLiteralError::InvalidPacketReadEnvelope),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpCliSearchRequestV1 {
    pub schema_version: u64,
    pub request_id: String,
    pub query: McpCliSearchQueryV1,
    pub finite_bound: u64,
    pub expected_release_ref: String,
    pub expected_public_catalog_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpCliSearchQueryV1 {
    ExactCommandId(String),
    BoundedFuzzyIntent(String),
}

impl McpCliSearchRequestV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        let query = match &self.query {
            McpCliSearchQueryV1::ExactCommandId(value)
            | McpCliSearchQueryV1::BoundedFuzzyIntent(value) => value,
        };
        if self.schema_version != 1
            || self.request_id.is_empty()
            || query.trim().is_empty()
            || self.finite_bound == 0
            || !is_current_ref(&self.expected_release_ref)
            || !is_current_ref(&self.expected_public_catalog_ref)
        {
            return Err(PublicLiteralError::InvalidCliSearchEnvelope);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum McpCliSearchCompletenessV1 {
    Complete,
    BoundedTruncated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpCliSearchBoundsV1 {
    pub requested_bound: u64,
    pub returned_count: u64,
    pub total_matching_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum McpCliSearchHitV1 {
    PureRead { exact_command_ref: String },
    Action { exact_action_spec_ref: String },
    Ceremony { exact_ceremony_spec_ref: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct McpCliSearchEnvelopeV1 {
    pub schema_version: u64,
    pub request_id: String,
    pub running_binary_release: String,
    pub binary_digest: [u8; 32],
    pub binary_version: String,
    pub executable_slot: String,
    pub core_catalog_ref: String,
    pub public_catalog_ref: String,
    pub catalog_snapshot_ref: String,
    pub completeness: McpCliSearchCompletenessV1,
    pub bounds: McpCliSearchBoundsV1,
    pub cursor: Option<String>,
    pub hits: Vec<McpCliSearchHitV1>,
}

impl McpCliSearchEnvelopeV1 {
    pub fn validate_for(&self, request: &McpCliSearchRequestV1) -> Result<(), PublicLiteralError> {
        request.validate()?;
        let valid_hits = self.hits.iter().all(|hit| match hit {
            McpCliSearchHitV1::PureRead { exact_command_ref }
            | McpCliSearchHitV1::Action {
                exact_action_spec_ref: exact_command_ref,
            }
            | McpCliSearchHitV1::Ceremony {
                exact_ceremony_spec_ref: exact_command_ref,
            } => is_current_ref(exact_command_ref),
        });
        let cursor_parity = match self.completeness {
            McpCliSearchCompletenessV1::Complete => self.cursor.is_none(),
            McpCliSearchCompletenessV1::BoundedTruncated => {
                self.cursor.as_ref().is_some_and(|value| !value.is_empty())
            }
        };
        if self.schema_version != 1
            || self.request_id != request.request_id
            || self.running_binary_release != request.expected_release_ref
            || is_zero_digest(&self.binary_digest)
            || self.binary_version.is_empty()
            || self.executable_slot.is_empty()
            || !is_current_ref(&self.core_catalog_ref)
            || self.public_catalog_ref != request.expected_public_catalog_ref
            || !is_current_ref(&self.catalog_snapshot_ref)
            || self.bounds.requested_bound != request.finite_bound
            || self.bounds.returned_count != self.hits.len() as u64
            || self.bounds.returned_count > self.bounds.requested_bound
            || self.bounds.returned_count > self.bounds.total_matching_count
            || !cursor_parity
            || !valid_hits
        {
            return Err(PublicLiteralError::InvalidCliSearchEnvelope);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum JobRouteReasonV1 {
    BootstrapRequired = 1,
    SetupRequired = 2,
    ExplicitResearch = 3,
    ContextUnknown = 4,
    DesignRequired = 5,
    ExplicitReview = 6,
    ReviewRequired = 7,
    StepRunnable = 8,
    RecoveryRequired = 9,
    ExtensionRequired = 10,
    ConflictingReadIntent = 11,
    ConflictingRouteFacts = 12,
    MissingRouteMapping = 13,
    CanonicalWaitOrStop = 14,
    StaleRouteInput = 15,
    IncompatibleRouteInput = 16,
    UnavailableResourceClosure = 17,
}

pub const JOB_ROUTE_REASONS_V1: [&str; 17] = [
    "BootstrapRequired",
    "SetupRequired",
    "ExplicitResearch",
    "ContextUnknown",
    "DesignRequired",
    "ExplicitReview",
    "ReviewRequired",
    "StepRunnable",
    "RecoveryRequired",
    "ExtensionRequired",
    "ConflictingReadIntent",
    "ConflictingRouteFacts",
    "MissingRouteMapping",
    "CanonicalWaitOrStop",
    "StaleRouteInput",
    "IncompatibleRouteInput",
    "UnavailableResourceClosure",
];

impl JobRouteReasonV1 {
    pub const ALL: [Self; 17] = [
        Self::BootstrapRequired,
        Self::SetupRequired,
        Self::ExplicitResearch,
        Self::ContextUnknown,
        Self::DesignRequired,
        Self::ExplicitReview,
        Self::ReviewRequired,
        Self::StepRunnable,
        Self::RecoveryRequired,
        Self::ExtensionRequired,
        Self::ConflictingReadIntent,
        Self::ConflictingRouteFacts,
        Self::MissingRouteMapping,
        Self::CanonicalWaitOrStop,
        Self::StaleRouteInput,
        Self::IncompatibleRouteInput,
        Self::UnavailableResourceClosure,
    ];

    pub const fn name(self) -> &'static str {
        JOB_ROUTE_REASONS_V1[self as usize - 1]
    }

    pub const fn selected_job(self) -> Option<InternalJobV1> {
        match self {
            Self::BootstrapRequired | Self::SetupRequired => Some(InternalJobV1::Setup),
            Self::ExplicitResearch | Self::ContextUnknown => Some(InternalJobV1::Research),
            Self::DesignRequired => Some(InternalJobV1::Design),
            Self::ExplicitReview | Self::ReviewRequired => Some(InternalJobV1::Review),
            Self::StepRunnable => Some(InternalJobV1::Execute),
            Self::RecoveryRequired => Some(InternalJobV1::Recover),
            Self::ExtensionRequired => Some(InternalJobV1::Adapt),
            Self::ConflictingReadIntent
            | Self::ConflictingRouteFacts
            | Self::MissingRouteMapping
            | Self::CanonicalWaitOrStop
            | Self::StaleRouteInput
            | Self::IncompatibleRouteInput
            | Self::UnavailableResourceClosure => None,
        }
    }

    pub const fn status(self) -> JobRouteStatusV1 {
        match self {
            Self::ConflictingReadIntent | Self::ConflictingRouteFacts => {
                JobRouteStatusV1::Ambiguous
            }
            Self::MissingRouteMapping
            | Self::CanonicalWaitOrStop
            | Self::StaleRouteInput
            | Self::IncompatibleRouteInput
            | Self::UnavailableResourceClosure => JobRouteStatusV1::Blocked,
            _ => JobRouteStatusV1::Selected,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobRouteStatusV1 {
    Selected,
    Ambiguous,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitReadIntentV1 {
    None,
    ResearchReadOnly,
    ReviewReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobRouteBasisV1 {
    Bootstrap,
    RecoveryState,
    ExplicitRequest,
    PacketReason,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobRouteInputV1 {
    Packet {
        exact_packet_ref: String,
        packet_semantic_hash: [u8; 32],
        inherited_valid_until_ref: String,
    },
    Bootstrap {
        exact_bootstrap_fact_view_ref: String,
        fact_view_hash: [u8; 32],
    },
}

impl JobRouteInputV1 {
    fn validate(&self) -> bool {
        match self {
            Self::Packet {
                exact_packet_ref,
                packet_semantic_hash,
                inherited_valid_until_ref,
            } => {
                is_current_ref(exact_packet_ref)
                    && !is_zero_digest(packet_semantic_hash)
                    && is_current_ref(inherited_valid_until_ref)
            }
            Self::Bootstrap {
                exact_bootstrap_fact_view_ref,
                fact_view_hash,
            } => is_current_ref(exact_bootstrap_fact_view_ref) && !is_zero_digest(fact_view_hash),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobInstructionLoadPlanV1 {
    pub job_resource_ref: String,
    pub method_resource_refs: Vec<String>,
    pub recipe_resource_refs: Vec<String>,
}

impl JobInstructionLoadPlanV1 {
    fn validate_for(&self, job: InternalJobV1) -> bool {
        self.method_resource_refs.is_empty()
            && self.recipe_resource_refs.is_empty()
            && self.job_resource_ref.contains(job.resource_path())
            && is_current_ref(&self.job_resource_ref)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobRouteOutcomeV1 {
    Selected {
        job: InternalJobV1,
        reason: JobRouteReasonV1,
        instruction_load_plan: JobInstructionLoadPlanV1,
    },
    Ambiguous(JobRouteReasonV1),
    Blocked(JobRouteReasonV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobRouteV1 {
    pub schema_version: u64,
    pub resolution_basis_ref: String,
    pub input: JobRouteInputV1,
    pub explicit_read_intent: ExplicitReadIntentV1,
    pub basis: JobRouteBasisV1,
    pub outcome: JobRouteOutcomeV1,
}

impl JobRouteV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        if self.schema_version != 1
            || !is_current_ref(&self.resolution_basis_ref)
            || !self.input.validate()
        {
            return Err(PublicLiteralError::InvalidJobRoute);
        }
        match (&self.input, self.basis, &self.outcome) {
            (
                JobRouteInputV1::Bootstrap { .. },
                JobRouteBasisV1::Bootstrap,
                JobRouteOutcomeV1::Selected {
                    job: InternalJobV1::Setup,
                    reason,
                    instruction_load_plan,
                },
            ) if matches!(
                reason,
                JobRouteReasonV1::BootstrapRequired | JobRouteReasonV1::SetupRequired
            ) && instruction_load_plan.validate_for(InternalJobV1::Setup) => {}
            (JobRouteInputV1::Bootstrap { .. }, _, _) => {
                return Err(PublicLiteralError::InvalidJobRoute);
            }
            (
                JobRouteInputV1::Packet { .. },
                _,
                JobRouteOutcomeV1::Selected {
                    job,
                    reason,
                    instruction_load_plan,
                },
            ) if reason.status() == JobRouteStatusV1::Selected
                && reason.selected_job() == Some(*job)
                && instruction_load_plan.validate_for(*job) => {}
            (JobRouteInputV1::Packet { .. }, _, JobRouteOutcomeV1::Ambiguous(reason))
                if reason.status() == JobRouteStatusV1::Ambiguous => {}
            (JobRouteInputV1::Packet { .. }, _, JobRouteOutcomeV1::Blocked(reason))
                if reason.status() == JobRouteStatusV1::Blocked => {}
            _ => return Err(PublicLiteralError::InvalidJobRoute),
        }
        if matches!(
            &self.outcome,
            JobRouteOutcomeV1::Selected {
                reason: JobRouteReasonV1::RecoveryRequired,
                ..
            }
        ) && self.basis != JobRouteBasisV1::RecoveryState
        {
            return Err(PublicLiteralError::InvalidJobRoute);
        }
        if matches!(
            &self.outcome,
            JobRouteOutcomeV1::Selected {
                reason: JobRouteReasonV1::ExplicitResearch | JobRouteReasonV1::ExplicitReview,
                ..
            }
        ) && self.basis != JobRouteBasisV1::ExplicitRequest
        {
            return Err(PublicLiteralError::InvalidJobRoute);
        }
        Ok(())
    }

    pub fn selected_job(&self) -> Option<InternalJobV1> {
        match self.outcome {
            JobRouteOutcomeV1::Selected { job, .. } => Some(job),
            JobRouteOutcomeV1::Ambiguous(_) | JobRouteOutcomeV1::Blocked(_) => None,
        }
    }

    pub fn semantic_hash(&self) -> Result<[u8; 32], PublicLiteralError> {
        self.validate()?;
        let mut value = Vec::new();
        encode_array_len(6, &mut value);
        encode_u64(self.schema_version, &mut value);
        encode_text(&self.resolution_basis_ref, &mut value);
        encode_job_route_input(&self.input, &mut value);
        encode_u64(self.explicit_read_intent as u64 + 1, &mut value);
        encode_u64(self.basis as u64 + 1, &mut value);
        encode_job_route_outcome(&self.outcome, &mut value);
        Ok(domain_hash("maestro.vnext.job-route.v1", &value))
    }

    pub fn semantic_ref(&self) -> Result<String, PublicLiteralError> {
        Ok(format!("sha256:{}", hex_digest(&self.semantic_hash()?)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobGuidanceEnvelopeV1 {
    PacketGuidance {
        exact_packet: Box<AgentPacketV1>,
        exact_route: JobRouteV1,
    },
    BootstrapGuidance {
        exact_bootstrap_fact_view: BootstrapRouteFactViewV1,
        exact_route: JobRouteV1,
    },
}

pub type JobGuidanceV1 = JobGuidanceEnvelopeV1;

impl JobGuidanceEnvelopeV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        match self {
            Self::PacketGuidance {
                exact_packet,
                exact_route,
            } => {
                exact_packet.validate()?;
                exact_route.validate()?;
                match &exact_route.input {
                    JobRouteInputV1::Packet {
                        exact_packet_ref,
                        packet_semantic_hash,
                        inherited_valid_until_ref,
                    } if exact_packet_ref == &exact_packet.packet_id
                        && packet_semantic_hash == &exact_packet.semantic_audit_hash
                        && inherited_valid_until_ref == &exact_packet.valid_until_ref =>
                    {
                        Ok(())
                    }
                    _ => Err(PublicLiteralError::InvalidJobGuidance),
                }
            }
            Self::BootstrapGuidance {
                exact_bootstrap_fact_view,
                exact_route,
            } => {
                exact_bootstrap_fact_view.validate()?;
                exact_route.validate()?;
                match &exact_route.input {
                    JobRouteInputV1::Bootstrap { fact_view_hash, .. }
                        if fact_view_hash == &exact_bootstrap_fact_view.fact_view_hash =>
                    {
                        Ok(())
                    }
                    _ => Err(PublicLiteralError::InvalidJobGuidance),
                }
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationSpecRefV1 {
    Action(ActionSpecRefV1),
    Ceremony(CeremonySpecRefV1),
}

impl OperationSpecRefV1 {
    fn validate(&self) -> Result<(), PublicLiteralError> {
        match self {
            Self::Action(value) if value.validate() => Ok(()),
            Self::Ceremony(value) if value.validate() => Ok(()),
            _ => Err(PublicLiteralError::InvalidOperationEnvelope),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionSpecRefV1 {
    pub exact_action_spec_ref: String,
    pub exact_schema_id: String,
    pub exact_core_catalog_ref: String,
    pub exact_public_catalog_ref: String,
}

impl ActionSpecRefV1 {
    fn validate(&self) -> bool {
        is_current_ref(&self.exact_action_spec_ref)
            && is_sha256_ref(&self.exact_schema_id)
            && is_current_ref(&self.exact_core_catalog_ref)
            && is_current_ref(&self.exact_public_catalog_ref)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CeremonySpecRefV1 {
    pub exact_ceremony_spec_ref: String,
    pub exact_schema_id: String,
    pub exact_core_catalog_ref: String,
    pub exact_public_catalog_ref: String,
}

impl CeremonySpecRefV1 {
    fn validate(&self) -> bool {
        is_current_ref(&self.exact_ceremony_spec_ref)
            && is_sha256_ref(&self.exact_schema_id)
            && is_current_ref(&self.exact_core_catalog_ref)
            && is_current_ref(&self.exact_public_catalog_ref)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionAuthorityBasisV1 {
    Ordinary {
        verified_principal_ref: String,
        current_session_ref: String,
        live_grant_refs: Vec<String>,
        required_mandate_refs: Vec<String>,
    },
    BootstrapControl {
        exact_bootstrap_scope_ref: String,
        current_executor_assertion_ref: String,
    },
    ContinuityMaintenance {
        exact_cma_branch_ref: String,
        maintenance_executor_assertion_ref: String,
        applicability_ref: String,
        phase_slot_ref: String,
    },
}

impl ActionAuthorityBasisV1 {
    fn validate(&self) -> bool {
        match self {
            Self::Ordinary {
                verified_principal_ref,
                current_session_ref,
                live_grant_refs,
                required_mandate_refs,
            } => {
                is_current_ref(verified_principal_ref)
                    && is_current_ref(current_session_ref)
                    && is_strictly_ordered_unique(live_grant_refs)
                    && is_strictly_ordered_unique(required_mandate_refs)
            }
            Self::BootstrapControl {
                exact_bootstrap_scope_ref,
                current_executor_assertion_ref,
            } => {
                is_current_ref(exact_bootstrap_scope_ref)
                    && is_current_ref(current_executor_assertion_ref)
            }
            Self::ContinuityMaintenance {
                exact_cma_branch_ref,
                maintenance_executor_assertion_ref,
                applicability_ref,
                phase_slot_ref,
            } => {
                is_current_ref(exact_cma_branch_ref)
                    && is_current_ref(maintenance_executor_assertion_ref)
                    && is_current_ref(applicability_ref)
                    && is_current_ref(phase_slot_ref)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CeremonyRequestModeV1 {
    Initiate,
    RecoverReserved,
    ResolveResult,
    Withdraw,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CeremonyRequestContextV1 {
    NoStore {
        protected_realm_ref: String,
        genesis_candidate_ref: String,
    },
    PreStore {
        protected_carrier_ref: String,
        candidate_seal_ref: String,
        expected_old_token_ref: String,
    },
}

impl CeremonyRequestContextV1 {
    fn validate(&self) -> bool {
        match self {
            Self::NoStore {
                protected_realm_ref,
                genesis_candidate_ref,
            } => is_current_ref(protected_realm_ref) && is_current_ref(genesis_candidate_ref),
            Self::PreStore {
                protected_carrier_ref,
                candidate_seal_ref,
                expected_old_token_ref,
            } => {
                is_current_ref(protected_carrier_ref)
                    && is_current_ref(candidate_seal_ref)
                    && is_current_ref(expected_old_token_ref)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrchestrationAttributionV1 {
    pub exact_packet_recipe_binding_ref: String,
    pub exact_application_ref: String,
    pub component_output_hashes: Vec<[u8; 32]>,
    pub composed_advice_hash: [u8; 32],
}

impl OrchestrationAttributionV1 {
    fn validate(&self) -> bool {
        is_current_ref(&self.exact_packet_recipe_binding_ref)
            && is_current_ref(&self.exact_application_ref)
            && self.component_output_hashes.len() <= 2
            && self
                .component_output_hashes
                .iter()
                .all(|hash| !is_zero_digest(hash))
            && !is_zero_digest(&self.composed_advice_hash)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionRequestV1 {
    pub schema_version: u64,
    pub request_id: String,
    pub idempotency_key: String,
    pub semantic_request_hash: [u8; 32],
    pub selected_packet_semantic_hash: [u8; 32],
    pub action_spec: ActionSpecRefV1,
    pub material_dependency_stamp: [u8; 32],
    pub exact_store_generation_ref: String,
    pub exact_authority_epoch_ref: String,
    pub valid_until_ref: String,
    pub authority_basis: ActionAuthorityBasisV1,
    pub typed_input_cbor: Vec<u8>,
    pub evidence_refs: Vec<String>,
    pub prerequisite_receipt_refs: Vec<String>,
    pub orchestration_attribution: Option<OrchestrationAttributionV1>,
}

impl ActionRequestV1 {
    fn validate(&self) -> bool {
        self.schema_version == 1
            && !self.request_id.is_empty()
            && !self.idempotency_key.is_empty()
            && !is_zero_digest(&self.semantic_request_hash)
            && !is_zero_digest(&self.selected_packet_semantic_hash)
            && self.action_spec.validate()
            && !is_zero_digest(&self.material_dependency_stamp)
            && is_current_ref(&self.exact_store_generation_ref)
            && is_current_ref(&self.exact_authority_epoch_ref)
            && is_current_ref(&self.valid_until_ref)
            && self.authority_basis.validate()
            && !self.typed_input_cbor.is_empty()
            && is_strictly_ordered_unique(&self.evidence_refs)
            && is_strictly_ordered_unique(&self.prerequisite_receipt_refs)
            && self
                .orchestration_attribution
                .as_ref()
                .is_none_or(OrchestrationAttributionV1::validate)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CeremonyRequestV1 {
    pub schema_version: u64,
    pub request_id: String,
    pub idempotency_key: String,
    pub semantic_request_hash: [u8; 32],
    pub ceremony_spec: CeremonySpecRefV1,
    pub request_mode: CeremonyRequestModeV1,
    pub context: CeremonyRequestContextV1,
    pub branch_authority_ref: String,
    pub expected_carrier_token_ref: String,
    pub typed_input_cbor: Vec<u8>,
    pub prerequisite_receipt_refs: Vec<String>,
    pub orchestration_attribution: Option<OrchestrationAttributionV1>,
}

impl CeremonyRequestV1 {
    fn validate(&self) -> bool {
        self.schema_version == 1
            && !self.request_id.is_empty()
            && !self.idempotency_key.is_empty()
            && !is_zero_digest(&self.semantic_request_hash)
            && self.ceremony_spec.validate()
            && self.context.validate()
            && is_current_ref(&self.branch_authority_ref)
            && is_current_ref(&self.expected_carrier_token_ref)
            && !self.typed_input_cbor.is_empty()
            && is_strictly_ordered_unique(&self.prerequisite_receipt_refs)
            && self
                .orchestration_attribution
                .as_ref()
                .is_none_or(OrchestrationAttributionV1::validate)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationRequestV1 {
    Action(ActionRequestV1),
    Ceremony(CeremonyRequestV1),
}

impl OperationRequestV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        if match self {
            Self::Action(request) => request.validate(),
            Self::Ceremony(request) => request.validate(),
        } {
            Ok(())
        } else {
            Err(PublicLiteralError::InvalidOperationEnvelope)
        }
    }

    fn request_id(&self) -> &str {
        match self {
            Self::Action(request) => &request.request_id,
            Self::Ceremony(request) => &request.request_id,
        }
    }

    fn operation_spec_ref(&self) -> &str {
        match self {
            Self::Action(request) => &request.action_spec.exact_action_spec_ref,
            Self::Ceremony(request) => &request.ceremony_spec.exact_ceremony_spec_ref,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationSemanticOutcomeV1 {
    Committed,
    NoOp,
    Rejected,
    Stale,
    Conflict,
    Unavailable,
    InDoubt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationResultBodyV1 {
    pub schema_version: u64,
    pub request_id: String,
    pub operation_spec_ref: String,
    pub outcome: OperationSemanticOutcomeV1,
    pub before_revision_refs: Vec<String>,
    pub after_revision_refs: Vec<String>,
    pub transition_receipt_refs: Vec<String>,
    pub produced_record_refs: Vec<String>,
    pub next_packet: Option<Box<AgentPacketV1>>,
    pub inspect_ref: Option<String>,
    pub replayed_delivery: bool,
}

impl OperationResultBodyV1 {
    fn validate_for(&self, request: &OperationRequestV1) -> bool {
        self.schema_version == 1
            && self.request_id == request.request_id()
            && self.operation_spec_ref == request.operation_spec_ref()
            && is_strictly_ordered_unique(&self.before_revision_refs)
            && is_strictly_ordered_unique(&self.after_revision_refs)
            && is_strictly_ordered_unique(&self.transition_receipt_refs)
            && is_strictly_ordered_unique(&self.produced_record_refs)
            && self
                .next_packet
                .as_ref()
                .is_none_or(|packet| packet.validate().is_ok())
            && self
                .inspect_ref
                .as_ref()
                .is_none_or(|value| is_current_ref(value))
            && !(self.next_packet.is_some() && self.inspect_ref.is_some())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionResultV1(pub OperationResultBodyV1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CeremonyResultV1(pub OperationResultBodyV1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationResultV1 {
    Action(ActionResultV1),
    Ceremony(CeremonyResultV1),
}

impl OperationResultV1 {
    pub fn validate_for(&self, request: &OperationRequestV1) -> Result<(), PublicLiteralError> {
        let valid = match (self, request) {
            (Self::Action(result), OperationRequestV1::Action(_)) => result.0.validate_for(request),
            (Self::Ceremony(result), OperationRequestV1::Ceremony(_)) => {
                result.0.validate_for(request)
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(PublicLiteralError::InvalidOperationEnvelope)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SetupModeV1 {
    Install = 1,
    Adopt = 2,
    Migrate = 3,
    Update = 4,
    Repair = 5,
    Rollback = 6,
    Uninstall = 7,
}

impl SetupModeV1 {
    pub const ALL: [Self; 7] = [
        Self::Install,
        Self::Adopt,
        Self::Migrate,
        Self::Update,
        Self::Repair,
        Self::Rollback,
        Self::Uninstall,
    ];

    pub const fn name(self) -> &'static str {
        SETUP_MODE_NAMES_V1[self as usize - 1]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupModeRequestV1 {
    Require(SetupModeV1),
    AcceptUniqueEligible,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActiveStoreDomainV1 {
    Repository { repository_domain_ref: String },
    Installation { installation_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveStoreAcquisitionV1 {
    pub domain: ActiveStoreDomainV1,
    pub store_ref: String,
    pub store_generation_ref: String,
    pub authority_epoch_ref: String,
    pub current_packet_or_material_stamp: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreStoreAcquisitionV1 {
    pub inactive_destination_ref: String,
    pub branch_bundle_ref: String,
    pub candidate_seal_ref: String,
    pub protected_carrier_ref: String,
    pub expected_old_token_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoStoreInstallationGenesisV1 {
    pub protected_realm_ref: String,
    pub genesis_candidate_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcquisitionContextV1 {
    ActiveStore(ActiveStoreAcquisitionV1),
    PreStore(PreStoreAcquisitionV1),
    NoStoreInstallationGenesis(NoStoreInstallationGenesisV1),
}

impl AcquisitionContextV1 {
    fn validate(&self) -> bool {
        match self {
            Self::ActiveStore(value) => {
                let domain = match &value.domain {
                    ActiveStoreDomainV1::Repository {
                        repository_domain_ref,
                    } => repository_domain_ref,
                    ActiveStoreDomainV1::Installation { installation_id } => installation_id,
                };
                is_current_ref(domain)
                    && is_current_ref(&value.store_ref)
                    && is_current_ref(&value.store_generation_ref)
                    && is_current_ref(&value.authority_epoch_ref)
                    && !is_zero_digest(&value.current_packet_or_material_stamp)
            }
            Self::PreStore(value) => [
                &value.inactive_destination_ref,
                &value.branch_bundle_ref,
                &value.candidate_seal_ref,
                &value.protected_carrier_ref,
                &value.expected_old_token_ref,
            ]
            .into_iter()
            .all(|value| is_current_ref(value)),
            Self::NoStoreInstallationGenesis(value) => {
                is_current_ref(&value.protected_realm_ref)
                    && is_current_ref(&value.genesis_candidate_ref)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceOwnerV1 {
    Distribution,
    Installation,
    Repository,
    Migration,
    Persistence,
    Authority,
    Execution,
    Effect,
    Projection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceOwnerFactCommitmentV1 {
    pub owner: SourceOwnerV1,
    pub exact_fact_ref: String,
    pub fact_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupPlanIntentV1 {
    Install,
    Update,
    Repair,
    Migrate,
    Rollback,
    Uninstall,
}

impl SetupPlanIntentV1 {
    const fn mode(self) -> SetupModeV1 {
        match self {
            Self::Install => SetupModeV1::Install,
            Self::Update => SetupModeV1::Update,
            Self::Repair => SetupModeV1::Repair,
            Self::Migrate => SetupModeV1::Migrate,
            Self::Rollback => SetupModeV1::Rollback,
            Self::Uninstall => SetupModeV1::Uninstall,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupCeremonyV1 {
    InstallationContextGenesis,
    RepositoryV1Cutover,
    InstallationV1Cutover,
    RecoverRepositoryStoreGeneration,
    RecoverInstallationStoreGeneration,
    ActivateVerifiedRepositoryGeneration,
    ActivateVerifiedInstallationGeneration,
    RecoverPreStoreBinarySlot,
    RecoverPreStoreWriterCohort,
    EstablishRepositoryRecoveryAdmission,
    EstablishInstallationRecoveryAdmission,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetupAdvertisedOperationBindingV1 {
    Action {
        operation_spec: ActionSpecRefV1,
        exact_typed_subject_ref: String,
        material_dependency_stamp: [u8; 32],
        meaning: SetupActionMeaningV1,
    },
    Ceremony {
        operation_spec: CeremonySpecRefV1,
        exact_protected_carrier_ref: String,
        ceremony: SetupCeremonyV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupActionMeaningV1 {
    DistributionPlan(SetupPlanIntentV1),
    AdoptManagedRegion,
    TransferWholeFileCustody,
    RollbackDistributionTransaction,
    RecoverDistributionTransaction,
    ReconciliationOrRecovery,
    NonDistribution,
}

impl SetupAdvertisedOperationBindingV1 {
    fn validate_context(&self, context: &AcquisitionContextV1) -> bool {
        match (self, context) {
            (
                Self::Action {
                    operation_spec,
                    exact_typed_subject_ref,
                    material_dependency_stamp,
                    ..
                },
                AcquisitionContextV1::ActiveStore(_),
            ) => {
                operation_spec.validate()
                    && is_current_ref(exact_typed_subject_ref)
                    && !is_zero_digest(material_dependency_stamp)
            }
            (
                Self::Ceremony {
                    operation_spec,
                    exact_protected_carrier_ref,
                    ..
                },
                AcquisitionContextV1::PreStore(_)
                | AcquisitionContextV1::NoStoreInstallationGenesis(_),
            ) => operation_spec.validate() && is_current_ref(exact_protected_carrier_ref),
            _ => false,
        }
    }

    fn validates_mode(&self, context: &AcquisitionContextV1, mode: SetupModeV1) -> bool {
        match (self, context) {
            (
                Self::Action {
                    meaning: SetupActionMeaningV1::DistributionPlan(intent),
                    ..
                },
                AcquisitionContextV1::ActiveStore(_),
            ) => intent.mode() == mode,
            (
                Self::Action {
                    meaning:
                        SetupActionMeaningV1::AdoptManagedRegion
                        | SetupActionMeaningV1::TransferWholeFileCustody,
                    ..
                },
                AcquisitionContextV1::ActiveStore(_),
            ) => mode == SetupModeV1::Adopt,
            (
                Self::Ceremony {
                    ceremony: SetupCeremonyV1::InstallationContextGenesis,
                    ..
                },
                AcquisitionContextV1::NoStoreInstallationGenesis(_),
            ) => matches!(
                mode,
                SetupModeV1::Install | SetupModeV1::Adopt | SetupModeV1::Migrate
            ),
            (
                Self::Ceremony {
                    ceremony:
                        SetupCeremonyV1::RepositoryV1Cutover | SetupCeremonyV1::InstallationV1Cutover,
                    ..
                },
                AcquisitionContextV1::PreStore(_),
            ) => mode == SetupModeV1::Migrate,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetupFactViewV1 {
    pub schema_version: u64,
    pub resolver_resource_and_release_catalog_closure: String,
    pub acquisition_context: AcquisitionContextV1,
    pub locality_subject: String,
    pub source_owner_fact_commitments: Vec<SourceOwnerFactCommitmentV1>,
    pub advertised_operation_binding: SetupAdvertisedOperationBindingV1,
}

impl SetupFactViewV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        if self.schema_version != 1
            || !is_current_ref(&self.resolver_resource_and_release_catalog_closure)
            || !self.acquisition_context.validate()
            || !is_current_ref(&self.locality_subject)
            || self.source_owner_fact_commitments.is_empty()
            || !self
                .advertised_operation_binding
                .validate_context(&self.acquisition_context)
        {
            return Err(PublicLiteralError::InvalidSetupFactView);
        }
        let mut prior: Option<(SourceOwnerV1, &str)> = None;
        for row in &self.source_owner_fact_commitments {
            let key = (row.owner, row.exact_fact_ref.as_str());
            if !is_current_ref(&row.exact_fact_ref)
                || is_zero_digest(&row.fact_hash)
                || prior.is_some_and(|value| value >= key)
            {
                return Err(PublicLiteralError::InvalidSetupFactView);
            }
            prior = Some(key);
        }
        Ok(())
    }

    pub fn semantic_hash(&self) -> Result<[u8; 32], PublicLiteralError> {
        self.validate()?;
        let mut value = Vec::new();
        encode_array_len(6, &mut value);
        encode_u64(self.schema_version, &mut value);
        encode_text(
            &self.resolver_resource_and_release_catalog_closure,
            &mut value,
        );
        encode_acquisition_context(&self.acquisition_context, &mut value);
        encode_text(&self.locality_subject, &mut value);
        encode_array_len(self.source_owner_fact_commitments.len(), &mut value);
        for row in &self.source_owner_fact_commitments {
            encode_array_len(3, &mut value);
            encode_u64(row.owner as u64 + 1, &mut value);
            encode_text(&row.exact_fact_ref, &mut value);
            encode_bytes(&row.fact_hash, &mut value);
        }
        encode_setup_operation_binding(&self.advertised_operation_binding, &mut value);
        Ok(domain_hash("maestro.vnext.setup-fact-view.v1", &value))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupModeAmbiguousReasonV1 {
    MultipleEligibleModes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupModeBlockedReasonV1 {
    UnsupportedSchemaOrCatalog,
    ContextLegalityMismatch,
    LocalityMismatch,
    CrossDomainAggregate,
    StaleFactView,
    StaleOrUnadvertisedOperation,
    RecoveryRequired,
    EffectInDoubt,
    ConflictingOwnerFacts,
    UnsafeOrAmbiguousTarget,
    GenerationUnresolved,
    MigrationNotReady,
    TargetOrCustodyUnavailable,
    DesiredStateUnresolved,
    NoEligibleSnapshot,
    AlreadyCurrent,
    NoEligibleMode,
    RequestedModeIneligible,
    OperationModeMismatch,
    AuthorityUnavailable,
    CapabilityUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetupModeResolutionOutcomeV1 {
    Selected {
        mode: SetupModeV1,
        validated_operation_binding: SetupAdvertisedOperationBindingV1,
    },
    Ambiguous(SetupModeAmbiguousReasonV1),
    Blocked(SetupModeBlockedReasonV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetupModeResolutionV1 {
    pub schema_version: u64,
    pub resolver_resource_and_release_catalog_closure: String,
    pub fact_view_commitment: [u8; 32],
    pub request_commitment: [u8; 32],
    pub advertised_operation_binding: SetupAdvertisedOperationBindingV1,
    pub outcome: SetupModeResolutionOutcomeV1,
}

impl SetupModeResolutionV1 {
    pub fn validate_against(
        &self,
        fact_view: &SetupFactViewV1,
        request: SetupModeRequestV1,
        canonical_eligible_modes: &[SetupModeV1],
    ) -> Result<(), PublicLiteralError> {
        fact_view.validate()?;
        if self.schema_version != 1
            || self.resolver_resource_and_release_catalog_closure
                != fact_view.resolver_resource_and_release_catalog_closure
            || self.fact_view_commitment != fact_view.semantic_hash()?
            || self.request_commitment != setup_request_hash(request)
            || self.advertised_operation_binding != fact_view.advertised_operation_binding
            || !is_strictly_ordered_unique(canonical_eligible_modes)
            || self.outcome
                != resolve_setup_outcome(
                    &fact_view.acquisition_context,
                    &fact_view.advertised_operation_binding,
                    request,
                    canonical_eligible_modes,
                )
        {
            return Err(PublicLiteralError::InvalidSetupResolution);
        }
        Ok(())
    }
}

pub fn resolve_setup_mode(
    fact_view: &SetupFactViewV1,
    request: SetupModeRequestV1,
    canonical_eligible_modes: &[SetupModeV1],
) -> Result<SetupModeResolutionV1, PublicLiteralError> {
    fact_view.validate()?;
    if !is_strictly_ordered_unique(canonical_eligible_modes) {
        return Err(PublicLiteralError::InvalidSetupResolution);
    }
    let resolution = SetupModeResolutionV1 {
        schema_version: 1,
        resolver_resource_and_release_catalog_closure: fact_view
            .resolver_resource_and_release_catalog_closure
            .clone(),
        fact_view_commitment: fact_view.semantic_hash()?,
        request_commitment: setup_request_hash(request),
        advertised_operation_binding: fact_view.advertised_operation_binding.clone(),
        outcome: resolve_setup_outcome(
            &fact_view.acquisition_context,
            &fact_view.advertised_operation_binding,
            request,
            canonical_eligible_modes,
        ),
    };
    resolution.validate_against(fact_view, request, canonical_eligible_modes)?;
    Ok(resolution)
}

fn resolve_setup_outcome(
    context: &AcquisitionContextV1,
    operation: &SetupAdvertisedOperationBindingV1,
    request: SetupModeRequestV1,
    eligible_modes: &[SetupModeV1],
) -> SetupModeResolutionOutcomeV1 {
    let candidate = match request {
        SetupModeRequestV1::Require(mode) if eligible_modes.contains(&mode) => mode,
        SetupModeRequestV1::Require(_) => {
            return SetupModeResolutionOutcomeV1::Blocked(
                SetupModeBlockedReasonV1::RequestedModeIneligible,
            );
        }
        SetupModeRequestV1::AcceptUniqueEligible if eligible_modes.is_empty() => {
            return SetupModeResolutionOutcomeV1::Blocked(SetupModeBlockedReasonV1::NoEligibleMode);
        }
        SetupModeRequestV1::AcceptUniqueEligible if eligible_modes.len() > 1 => {
            return SetupModeResolutionOutcomeV1::Ambiguous(
                SetupModeAmbiguousReasonV1::MultipleEligibleModes,
            );
        }
        SetupModeRequestV1::AcceptUniqueEligible => eligible_modes[0],
    };
    if operation.validates_mode(context, candidate) {
        SetupModeResolutionOutcomeV1::Selected {
            mode: candidate,
            validated_operation_binding: operation.clone(),
        }
    } else {
        SetupModeResolutionOutcomeV1::Blocked(SetupModeBlockedReasonV1::OperationModeMismatch)
    }
}

fn setup_request_hash(request: SetupModeRequestV1) -> [u8; 32] {
    let mut value = Vec::new();
    match request {
        SetupModeRequestV1::Require(mode) => {
            encode_array_len(2, &mut value);
            encode_u64(1, &mut value);
            encode_u64(mode as u64, &mut value);
        }
        SetupModeRequestV1::AcceptUniqueEligible => {
            encode_array_len(1, &mut value);
            encode_u64(2, &mut value);
        }
    }
    domain_hash("maestro.vnext.setup-mode-request.v1", &value)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillActivationStoreDomainV1 {
    Repository {
        repository_domain_ref: String,
        store_generation_ref: String,
    },
    Installation {
        installation_id: String,
        store_generation_ref: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillActivationBootstrapContextV1 {
    RepositoryBootstrap { exact_bootstrap_context_ref: String },
    InstallationBootstrap { exact_bootstrap_context_ref: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillActivationAcquisitionContextV1 {
    ActiveStore(SkillActivationStoreDomainV1),
    Bootstrap(SkillActivationBootstrapContextV1),
}

impl SkillActivationAcquisitionContextV1 {
    fn validate(&self) -> bool {
        match self {
            Self::ActiveStore(SkillActivationStoreDomainV1::Repository {
                repository_domain_ref,
                store_generation_ref,
            }) => is_current_ref(repository_domain_ref) && is_current_ref(store_generation_ref),
            Self::ActiveStore(SkillActivationStoreDomainV1::Installation {
                installation_id,
                store_generation_ref,
            }) => is_current_ref(installation_id) && is_current_ref(store_generation_ref),
            Self::Bootstrap(
                SkillActivationBootstrapContextV1::RepositoryBootstrap {
                    exact_bootstrap_context_ref,
                }
                | SkillActivationBootstrapContextV1::InstallationBootstrap {
                    exact_bootstrap_context_ref,
                },
            ) => is_current_ref(exact_bootstrap_context_ref),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillActivationSubjectV1 {
    pub schema_version: u64,
    pub activation_acquisition_id: String,
    pub acquisition_context: SkillActivationAcquisitionContextV1,
    pub release_ref: String,
    pub root_skill_resource_ref: String,
}

impl SkillActivationSubjectV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        if self.schema_version != 1
            || !is_nominal_id(&self.activation_acquisition_id)
            || !self.acquisition_context.validate()
            || !is_current_ref(&self.release_ref)
            || !self
                .root_skill_resource_ref
                .contains("skills/maestro/SKILL.md")
            || !is_current_ref(&self.root_skill_resource_ref)
        {
            return Err(PublicLiteralError::InvalidSkillActivationSubject);
        }
        Ok(())
    }

    pub fn semantic_hash(&self) -> Result<[u8; 32], PublicLiteralError> {
        self.validate()?;
        let mut value = Vec::new();
        encode_array_len(5, &mut value);
        encode_u64(self.schema_version, &mut value);
        encode_text(&self.activation_acquisition_id, &mut value);
        encode_activation_context(&self.acquisition_context, &mut value);
        encode_text(&self.release_ref, &mut value);
        encode_text(&self.root_skill_resource_ref, &mut value);
        Ok(domain_hash(
            "maestro.vnext.skill-activation-subject.v1",
            &value,
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PacketRecipeResolutionOutcomeV1 {
    NoRecipe,
    Admitted(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillActivationRecipeResolutionV1 {
    BootstrapNoRecipe,
    PacketAdmission {
        exact_admission: SelectedJobRecipeAdmissionV1,
        outcome: PacketRecipeResolutionOutcomeV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedResourceClosureV1 {
    pub job_resource_ref: String,
    pub direct_method_resource_refs: Vec<String>,
    pub tdd_child_resource_refs: Vec<String>,
    pub research_example_resource_ref: Option<String>,
    pub recipe_resource_refs: Vec<String>,
    pub closure_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillActivationPayloadV1 {
    pub selected_route: JobRouteV1,
    pub capability_resolution: CapabilityMethodResolutionV1,
    pub recipe_resolution: SkillActivationRecipeResolutionV1,
    pub context_budget_profile_ref: String,
    pub loaded_resource_closure: LoadedResourceClosureV1,
}

impl SkillActivationPayloadV1 {
    pub fn validate_against(
        &self,
        subject: &SkillActivationSubjectV1,
    ) -> Result<(), PublicLiteralError> {
        subject.validate()?;
        self.selected_route.validate()?;
        let selected_job = self
            .selected_route
            .selected_job()
            .ok_or(PublicLiteralError::InvalidSkillActivationPayload)?;
        let route_ref = self.selected_route.semantic_ref()?;
        let plan = validate_selected_capability_resolution(
            &self.capability_resolution,
            selected_job,
            &route_ref,
        )?;
        let recipe_refs = validate_activation_recipe_resolution(
            &self.recipe_resolution,
            &self.selected_route,
            &route_ref,
        )?;
        if !is_current_ref(&self.context_budget_profile_ref)
            || !validate_loaded_resource_closure(&self.loaded_resource_closure, plan, &recipe_refs)
            || self.loaded_resource_closure.closure_digest
                != self.expected_closure_digest(subject)?
        {
            return Err(PublicLiteralError::InvalidSkillActivationPayload);
        }
        match (
            &subject.acquisition_context,
            &self.selected_route.input,
            &self.recipe_resolution,
            selected_job,
        ) {
            (
                SkillActivationAcquisitionContextV1::Bootstrap(_),
                JobRouteInputV1::Bootstrap { .. },
                SkillActivationRecipeResolutionV1::BootstrapNoRecipe,
                InternalJobV1::Setup,
            ) => {}
            (
                SkillActivationAcquisitionContextV1::ActiveStore(_),
                JobRouteInputV1::Packet { .. },
                SkillActivationRecipeResolutionV1::PacketAdmission { .. },
                _,
            ) => {}
            _ => return Err(PublicLiteralError::InvalidSkillActivationPayload),
        }
        Ok(())
    }

    pub fn semantic_hash(
        &self,
        subject: &SkillActivationSubjectV1,
    ) -> Result<[u8; 32], PublicLiteralError> {
        self.validate_against_without_digest(subject)?;
        let mut value = Vec::new();
        encode_activation_payload(self, &mut value)?;
        Ok(domain_hash(
            "maestro.vnext.skill-activation-payload.v1",
            &value,
        ))
    }

    fn validate_against_without_digest(
        &self,
        subject: &SkillActivationSubjectV1,
    ) -> Result<(), PublicLiteralError> {
        subject.validate()?;
        self.selected_route.validate()?;
        let selected_job = self
            .selected_route
            .selected_job()
            .ok_or(PublicLiteralError::InvalidSkillActivationPayload)?;
        let route_ref = self.selected_route.semantic_ref()?;
        let plan = validate_selected_capability_resolution(
            &self.capability_resolution,
            selected_job,
            &route_ref,
        )?;
        let recipe_refs = validate_activation_recipe_resolution(
            &self.recipe_resolution,
            &self.selected_route,
            &route_ref,
        )?;
        if !is_current_ref(&self.context_budget_profile_ref)
            || !validate_loaded_resource_closure_without_digest(
                &self.loaded_resource_closure,
                plan,
                &recipe_refs,
            )
        {
            return Err(PublicLiteralError::InvalidSkillActivationPayload);
        }
        Ok(())
    }

    fn expected_closure_digest(
        &self,
        subject: &SkillActivationSubjectV1,
    ) -> Result<[u8; 32], PublicLiteralError> {
        self.validate_against_without_digest(subject)?;
        let mut value = Vec::new();
        encode_array_len(7, &mut value);
        encode_text(&subject.root_skill_resource_ref, &mut value);
        encode_bytes(&self.selected_route.semantic_hash()?, &mut value);
        encode_capability_resolution(&self.capability_resolution, &mut value);
        encode_recipe_resolution(&self.recipe_resolution, &mut value);
        encode_text(&self.context_budget_profile_ref, &mut value);
        encode_loaded_closure_without_digest(&self.loaded_resource_closure, &mut value);
        encode_text(&subject.release_ref, &mut value);
        Ok(domain_hash(
            "maestro.vnext.skill-activation-loaded-closure.v1",
            &value,
        ))
    }
}

fn validate_selected_capability_resolution<'a>(
    resolution: &'a CapabilityMethodResolutionV1,
    selected_job: InternalJobV1,
    route_ref: &str,
) -> Result<&'a CapabilityInstructionLoadPlanV1, PublicLiteralError> {
    let plan = match &resolution.outcome {
        CapabilityMethodResolutionOutcomeV1::Selected(plan) => plan,
        CapabilityMethodResolutionOutcomeV1::Ambiguous(_)
        | CapabilityMethodResolutionOutcomeV1::Blocked(_) => {
            return Err(PublicLiteralError::InvalidSkillActivationPayload);
        }
    };
    if resolution.schema_version != 1
        || !is_current_ref(&resolution.resolution_basis_ref)
        || resolution.exact_selected_job_route_ref != route_ref
        || !is_current_ref(&resolution.exact_intent_ref)
        || plan.selected_job_resource_ref.logical_path != selected_job.resource_path()
        || plan.selected_job_resource_ref.validate().is_err()
        || plan.direct_method_resource_refs.len() > 4
        || plan.tdd_child_resource_refs.len() > 5
        || plan
            .direct_method_resource_refs
            .iter()
            .any(|resource| resource.validate().is_err())
        || plan
            .tdd_child_resource_refs
            .iter()
            .any(|resource| resource.validate().is_err())
        || plan
            .research_example_resource_ref
            .as_ref()
            .is_some_and(|resource| resource.validate().is_err())
    {
        return Err(PublicLiteralError::InvalidSkillActivationPayload);
    }
    let direct_methods = plan
        .direct_method_resource_refs
        .iter()
        .map(|resource| resource.method)
        .collect::<Vec<_>>();
    if !is_strictly_ordered_unique(&direct_methods)
        || direct_methods.iter().any(|method| {
            !crate::domain::vnext::capability::literals::job_method_is_admitted(
                selected_job,
                *method,
            )
        })
        || (!plan.tdd_child_resource_refs.is_empty()
            && (selected_job != InternalJobV1::Execute
                || !direct_methods
                    .contains(&crate::domain::vnext::capability::literals::DirectMethodV1::Tdd)))
        || (plan.research_example_resource_ref.is_some() && selected_job != InternalJobV1::Research)
    {
        return Err(PublicLiteralError::InvalidSkillActivationPayload);
    }
    Ok(plan)
}

fn validate_activation_recipe_resolution(
    resolution: &SkillActivationRecipeResolutionV1,
    route: &JobRouteV1,
    route_ref: &str,
) -> Result<Vec<String>, PublicLiteralError> {
    match resolution {
        SkillActivationRecipeResolutionV1::BootstrapNoRecipe
            if matches!(route.input, JobRouteInputV1::Bootstrap { .. })
                && route.selected_job() == Some(InternalJobV1::Setup) =>
        {
            Ok(Vec::new())
        }
        SkillActivationRecipeResolutionV1::PacketAdmission {
            exact_admission,
            outcome,
        } if matches!(route.input, JobRouteInputV1::Packet { .. })
            && is_current_ref(&exact_admission.resolution_basis_ref)
            && is_current_ref(&exact_admission.exact_packet_application_ref)
            && exact_admission.exact_selected_job_route_ref == route_ref =>
        {
            match (&exact_admission.outcome, outcome) {
                (
                    SelectedJobRecipeAdmissionOutcomeV1::NoRecipe,
                    PacketRecipeResolutionOutcomeV1::NoRecipe,
                ) => Ok(Vec::new()),
                (
                    SelectedJobRecipeAdmissionOutcomeV1::Admitted(admitted),
                    PacketRecipeResolutionOutcomeV1::Admitted(exact),
                ) if admitted == exact
                    && (1..=2).contains(&exact.len())
                    && is_strictly_ordered_unique(exact)
                    && exact
                        .iter()
                        .all(|value| RecipeIdV1::from_resource_ref(value).is_some()) =>
                {
                    Ok(exact.clone())
                }
                _ => Err(PublicLiteralError::InvalidSkillActivationPayload),
            }
        }
        _ => Err(PublicLiteralError::InvalidSkillActivationPayload),
    }
}

fn validate_loaded_resource_closure(
    closure: &LoadedResourceClosureV1,
    plan: &CapabilityInstructionLoadPlanV1,
    recipe_refs: &[String],
) -> bool {
    !is_zero_digest(&closure.closure_digest)
        && validate_loaded_resource_closure_without_digest(closure, plan, recipe_refs)
}

fn validate_loaded_resource_closure_without_digest(
    closure: &LoadedResourceClosureV1,
    plan: &CapabilityInstructionLoadPlanV1,
    recipe_refs: &[String],
) -> bool {
    let direct = plan
        .direct_method_resource_refs
        .iter()
        .map(|value| value.instruction_resource.resource_ref.clone())
        .collect::<Vec<_>>();
    let children = plan
        .tdd_child_resource_refs
        .iter()
        .map(|value| value.resource_ref.clone())
        .collect::<Vec<_>>();
    let example = plan
        .research_example_resource_ref
        .as_ref()
        .map(|value| value.resource_ref.clone());
    closure.job_resource_ref == plan.selected_job_resource_ref.resource_ref
        && closure.direct_method_resource_refs == direct
        && closure.tdd_child_resource_refs == children
        && closure.research_example_resource_ref == example
        && closure.recipe_resource_refs == recipe_refs
        && closure.direct_method_resource_refs.len() <= 4
        && closure.tdd_child_resource_refs.len() <= 5
        && closure.recipe_resource_refs.len() <= 2
        && is_strictly_ordered_unique(&closure.direct_method_resource_refs)
        && is_strictly_ordered_unique(&closure.tdd_child_resource_refs)
        && is_strictly_ordered_unique(&closure.recipe_resource_refs)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillActivationCandidateV1 {
    pub schema_version: u64,
    pub subject: SkillActivationSubjectV1,
    pub payload: SkillActivationPayloadV1,
    pub subject_commitment: [u8; 32],
    pub payload_commitment: [u8; 32],
    pub candidate_commitment: [u8; 32],
}

impl SkillActivationCandidateV1 {
    pub fn create(
        subject: SkillActivationSubjectV1,
        mut payload: SkillActivationPayloadV1,
    ) -> Result<Self, PublicLiteralError> {
        payload.loaded_resource_closure.closure_digest =
            payload.expected_closure_digest(&subject)?;
        let subject_commitment = subject.semantic_hash()?;
        let payload_commitment = payload.semantic_hash(&subject)?;
        let candidate_commitment = activation_candidate_hash(
            &subject_commitment,
            &payload_commitment,
            &subject.activation_acquisition_id,
        );
        Ok(Self {
            schema_version: 1,
            subject,
            payload,
            subject_commitment,
            payload_commitment,
            candidate_commitment,
        })
    }

    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        self.subject.validate()?;
        self.payload.validate_against(&self.subject)?;
        let expected_subject = self.subject.semantic_hash()?;
        let expected_payload = self.payload.semantic_hash(&self.subject)?;
        if self.schema_version != 1
            || self.subject_commitment != expected_subject
            || self.payload_commitment != expected_payload
            || self.candidate_commitment
                != activation_candidate_hash(
                    &expected_subject,
                    &expected_payload,
                    &self.subject.activation_acquisition_id,
                )
        {
            return Err(PublicLiteralError::InvalidSkillActivationCandidate);
        }
        Ok(())
    }
}

fn activation_candidate_hash(
    subject_commitment: &[u8; 32],
    payload_commitment: &[u8; 32],
    acquisition_id: &str,
) -> [u8; 32] {
    let mut value = Vec::new();
    encode_array_len(3, &mut value);
    encode_bytes(subject_commitment, &mut value);
    encode_bytes(payload_commitment, &mut value);
    encode_text(acquisition_id, &mut value);
    domain_hash("maestro.vnext.skill-activation-candidate.v1", &value)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyNewlineStateV1 {
    Terminated,
    AbsentAtEndOfFile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyActivationParseStatusV1 {
    CompleteRecognized,
    CompleteUnknown,
    Malformed,
    InvalidUtf8,
    TrailingPartial,
    SourceUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacySkillActivationDispositionV1 {
    MappedHistoricalNonBearer,
    OpaquePreserved,
    Quarantined,
    UnavailablePreexistingLoss,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacySkillActivationImportReasonV1 {
    RecognizedHistoricalActivation,
    UnknownSchemaOrSpelling,
    MalformedRecord,
    InvalidUtf8,
    TrailingPartialRecord,
    PreexistingSourceLoss,
    RetiredSkillName,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacySkillActivationImportV1 {
    pub schema_version: u64,
    pub source_format: String,
    pub source_file_hash: [u8; 32],
    pub source_path_bytes: Vec<u8>,
    pub record_ordinal: u64,
    pub byte_start: u64,
    pub byte_length: u64,
    pub newline_state: LegacyNewlineStateV1,
    pub raw_record_hash: [u8; 32],
    pub parse_status: LegacyActivationParseStatusV1,
    pub raw_event_spelling: Option<Vec<u8>>,
    pub skill_name: Option<Vec<u8>>,
    pub session_annotation: Option<Vec<u8>>,
    pub agent_runtime_annotation: Option<Vec<u8>>,
    pub activation_mode_annotation: Option<Vec<u8>>,
    pub timestamp_annotations: Vec<Vec<u8>>,
    pub disposition: LegacySkillActivationDispositionV1,
    pub reason: LegacySkillActivationImportReasonV1,
}

impl LegacySkillActivationImportV1 {
    pub fn validate(&self) -> Result<(), PublicLiteralError> {
        let disposition_matches = match (self.parse_status, self.disposition, self.reason) {
            (
                LegacyActivationParseStatusV1::CompleteRecognized,
                LegacySkillActivationDispositionV1::MappedHistoricalNonBearer,
                LegacySkillActivationImportReasonV1::RecognizedHistoricalActivation
                | LegacySkillActivationImportReasonV1::RetiredSkillName,
            ) => self
                .raw_event_spelling
                .as_deref()
                .is_some_and(|value| value == b"SkillActivation" || value == b"skill_activation"),
            (
                LegacyActivationParseStatusV1::CompleteUnknown,
                LegacySkillActivationDispositionV1::OpaquePreserved,
                LegacySkillActivationImportReasonV1::UnknownSchemaOrSpelling,
            ) => true,
            (
                LegacyActivationParseStatusV1::Malformed,
                LegacySkillActivationDispositionV1::Quarantined,
                LegacySkillActivationImportReasonV1::MalformedRecord,
            )
            | (
                LegacyActivationParseStatusV1::InvalidUtf8,
                LegacySkillActivationDispositionV1::Quarantined,
                LegacySkillActivationImportReasonV1::InvalidUtf8,
            )
            | (
                LegacyActivationParseStatusV1::TrailingPartial,
                LegacySkillActivationDispositionV1::Quarantined,
                LegacySkillActivationImportReasonV1::TrailingPartialRecord,
            )
            | (
                LegacyActivationParseStatusV1::SourceUnavailable,
                LegacySkillActivationDispositionV1::UnavailablePreexistingLoss,
                LegacySkillActivationImportReasonV1::PreexistingSourceLoss,
            ) => true,
            _ => false,
        };
        if self.schema_version != 1
            || self.source_format != "FORMAT-RUN-EVENT-V1"
            || is_zero_digest(&self.source_file_hash)
            || self.source_path_bytes.is_empty()
            || self.byte_length == 0
            || is_zero_digest(&self.raw_record_hash)
            || !disposition_matches
            || !all_unique_bytes(&self.timestamp_annotations)
            || (self.parse_status == LegacyActivationParseStatusV1::TrailingPartial
                && self.newline_state != LegacyNewlineStateV1::AbsentAtEndOfFile)
        {
            return Err(PublicLiteralError::InvalidLegacySkillActivationImport);
        }
        if self.reason == LegacySkillActivationImportReasonV1::RetiredSkillName {
            let name = self
                .skill_name
                .as_deref()
                .and_then(|value| std::str::from_utf8(value).ok());
            if !name.is_some_and(|value| LEGACY_INACTIVE_SKILL_NAMES_V1.contains(&value)) {
                return Err(PublicLiteralError::InvalidLegacySkillActivationImport);
            }
        }
        Ok(())
    }

    pub fn rerun_identity(&self) -> Result<[u8; 32], PublicLiteralError> {
        self.validate()?;
        let mut value = Vec::new();
        encode_array_len(3, &mut value);
        encode_bytes(&self.source_file_hash, &mut value);
        encode_u64(self.byte_start, &mut value);
        encode_u64(self.byte_length, &mut value);
        Ok(domain_hash(
            "maestro.vnext.legacy-skill-activation-import-range.v1",
            &value,
        ))
    }
}

fn encode_job_route_input(input: &JobRouteInputV1, output: &mut Vec<u8>) {
    match input {
        JobRouteInputV1::Packet {
            exact_packet_ref,
            packet_semantic_hash,
            inherited_valid_until_ref,
        } => {
            encode_array_len(4, output);
            encode_u64(1, output);
            encode_text(exact_packet_ref, output);
            encode_bytes(packet_semantic_hash, output);
            encode_text(inherited_valid_until_ref, output);
        }
        JobRouteInputV1::Bootstrap {
            exact_bootstrap_fact_view_ref,
            fact_view_hash,
        } => {
            encode_array_len(3, output);
            encode_u64(2, output);
            encode_text(exact_bootstrap_fact_view_ref, output);
            encode_bytes(fact_view_hash, output);
        }
    }
}

fn encode_job_route_outcome(outcome: &JobRouteOutcomeV1, output: &mut Vec<u8>) {
    match outcome {
        JobRouteOutcomeV1::Selected {
            job,
            reason,
            instruction_load_plan,
        } => {
            encode_array_len(6, output);
            encode_u64(1, output);
            encode_u64(*job as u64, output);
            encode_u64(*reason as u64, output);
            encode_text(&instruction_load_plan.job_resource_ref, output);
            encode_text_array(&instruction_load_plan.method_resource_refs, output);
            encode_text_array(&instruction_load_plan.recipe_resource_refs, output);
        }
        JobRouteOutcomeV1::Ambiguous(reason) => {
            encode_array_len(2, output);
            encode_u64(2, output);
            encode_u64(*reason as u64, output);
        }
        JobRouteOutcomeV1::Blocked(reason) => {
            encode_array_len(2, output);
            encode_u64(3, output);
            encode_u64(*reason as u64, output);
        }
    }
}

fn encode_acquisition_context(context: &AcquisitionContextV1, output: &mut Vec<u8>) {
    match context {
        AcquisitionContextV1::ActiveStore(value) => {
            encode_array_len(6, output);
            encode_u64(1, output);
            match &value.domain {
                ActiveStoreDomainV1::Repository {
                    repository_domain_ref,
                } => {
                    encode_u64(1, output);
                    encode_text(repository_domain_ref, output);
                }
                ActiveStoreDomainV1::Installation { installation_id } => {
                    encode_u64(2, output);
                    encode_text(installation_id, output);
                }
            }
            encode_text(&value.store_ref, output);
            encode_text(&value.store_generation_ref, output);
            encode_text(&value.authority_epoch_ref, output);
            encode_bytes(&value.current_packet_or_material_stamp, output);
        }
        AcquisitionContextV1::PreStore(value) => {
            encode_array_len(6, output);
            encode_u64(2, output);
            encode_text(&value.inactive_destination_ref, output);
            encode_text(&value.branch_bundle_ref, output);
            encode_text(&value.candidate_seal_ref, output);
            encode_text(&value.protected_carrier_ref, output);
            encode_text(&value.expected_old_token_ref, output);
        }
        AcquisitionContextV1::NoStoreInstallationGenesis(value) => {
            encode_array_len(3, output);
            encode_u64(3, output);
            encode_text(&value.protected_realm_ref, output);
            encode_text(&value.genesis_candidate_ref, output);
        }
    }
}

fn encode_setup_operation_binding(
    binding: &SetupAdvertisedOperationBindingV1,
    output: &mut Vec<u8>,
) {
    match binding {
        SetupAdvertisedOperationBindingV1::Action {
            operation_spec,
            exact_typed_subject_ref,
            material_dependency_stamp,
            meaning,
        } => {
            encode_array_len(8, output);
            encode_u64(1, output);
            encode_text(&operation_spec.exact_action_spec_ref, output);
            encode_text(&operation_spec.exact_schema_id, output);
            encode_text(&operation_spec.exact_core_catalog_ref, output);
            encode_text(&operation_spec.exact_public_catalog_ref, output);
            encode_text(exact_typed_subject_ref, output);
            encode_bytes(material_dependency_stamp, output);
            let (tag, parameter) = match meaning {
                SetupActionMeaningV1::DistributionPlan(intent) => (1, *intent as u64 + 1),
                SetupActionMeaningV1::AdoptManagedRegion => (2, 0),
                SetupActionMeaningV1::TransferWholeFileCustody => (3, 0),
                SetupActionMeaningV1::RollbackDistributionTransaction => (4, 0),
                SetupActionMeaningV1::RecoverDistributionTransaction => (5, 0),
                SetupActionMeaningV1::ReconciliationOrRecovery => (6, 0),
                SetupActionMeaningV1::NonDistribution => (7, 0),
            };
            encode_array_len(2, output);
            encode_u64(tag, output);
            encode_u64(parameter, output);
        }
        SetupAdvertisedOperationBindingV1::Ceremony {
            operation_spec,
            exact_protected_carrier_ref,
            ceremony,
        } => {
            encode_array_len(7, output);
            encode_u64(2, output);
            encode_text(&operation_spec.exact_ceremony_spec_ref, output);
            encode_text(&operation_spec.exact_schema_id, output);
            encode_text(&operation_spec.exact_core_catalog_ref, output);
            encode_text(&operation_spec.exact_public_catalog_ref, output);
            encode_text(exact_protected_carrier_ref, output);
            encode_u64(*ceremony as u64 + 1, output);
        }
    }
}

fn encode_activation_context(context: &SkillActivationAcquisitionContextV1, output: &mut Vec<u8>) {
    match context {
        SkillActivationAcquisitionContextV1::ActiveStore(
            SkillActivationStoreDomainV1::Repository {
                repository_domain_ref,
                store_generation_ref,
            },
        ) => {
            encode_array_len(4, output);
            encode_u64(1, output);
            encode_u64(1, output);
            encode_text(repository_domain_ref, output);
            encode_text(store_generation_ref, output);
        }
        SkillActivationAcquisitionContextV1::ActiveStore(
            SkillActivationStoreDomainV1::Installation {
                installation_id,
                store_generation_ref,
            },
        ) => {
            encode_array_len(4, output);
            encode_u64(1, output);
            encode_u64(2, output);
            encode_text(installation_id, output);
            encode_text(store_generation_ref, output);
        }
        SkillActivationAcquisitionContextV1::Bootstrap(
            SkillActivationBootstrapContextV1::RepositoryBootstrap {
                exact_bootstrap_context_ref,
            },
        ) => {
            encode_array_len(3, output);
            encode_u64(2, output);
            encode_u64(1, output);
            encode_text(exact_bootstrap_context_ref, output);
        }
        SkillActivationAcquisitionContextV1::Bootstrap(
            SkillActivationBootstrapContextV1::InstallationBootstrap {
                exact_bootstrap_context_ref,
            },
        ) => {
            encode_array_len(3, output);
            encode_u64(2, output);
            encode_u64(2, output);
            encode_text(exact_bootstrap_context_ref, output);
        }
    }
}

fn encode_activation_payload(
    payload: &SkillActivationPayloadV1,
    output: &mut Vec<u8>,
) -> Result<(), PublicLiteralError> {
    encode_array_len(5, output);
    encode_bytes(&payload.selected_route.semantic_hash()?, output);
    encode_capability_resolution(&payload.capability_resolution, output);
    encode_recipe_resolution(&payload.recipe_resolution, output);
    encode_text(&payload.context_budget_profile_ref, output);
    encode_loaded_closure_with_digest(&payload.loaded_resource_closure, output);
    Ok(())
}

fn encode_capability_resolution(resolution: &CapabilityMethodResolutionV1, output: &mut Vec<u8>) {
    encode_array_len(5, output);
    encode_u64(resolution.schema_version, output);
    encode_text(&resolution.resolution_basis_ref, output);
    encode_text(&resolution.exact_selected_job_route_ref, output);
    encode_text(&resolution.exact_intent_ref, output);
    match &resolution.outcome {
        CapabilityMethodResolutionOutcomeV1::Selected(plan) => {
            encode_array_len(5, output);
            encode_u64(1, output);
            encode_text(&plan.selected_job_resource_ref.resource_ref, output);
            encode_text_array(
                &plan
                    .direct_method_resource_refs
                    .iter()
                    .map(|value| value.instruction_resource.resource_ref.clone())
                    .collect::<Vec<_>>(),
                output,
            );
            encode_text_array(
                &plan
                    .tdd_child_resource_refs
                    .iter()
                    .map(|value| value.resource_ref.clone())
                    .collect::<Vec<_>>(),
                output,
            );
            match &plan.research_example_resource_ref {
                Some(value) => {
                    encode_array_len(2, output);
                    encode_u64(1, output);
                    encode_text(&value.resource_ref, output);
                }
                None => {
                    encode_array_len(1, output);
                    encode_u64(2, output);
                }
            }
        }
        CapabilityMethodResolutionOutcomeV1::Ambiguous(_) => {
            encode_array_len(1, output);
            encode_u64(2, output);
        }
        CapabilityMethodResolutionOutcomeV1::Blocked(reason) => {
            encode_array_len(2, output);
            encode_u64(3, output);
            encode_u64(*reason as u64 + 1, output);
        }
    }
}

fn encode_recipe_resolution(resolution: &SkillActivationRecipeResolutionV1, output: &mut Vec<u8>) {
    match resolution {
        SkillActivationRecipeResolutionV1::BootstrapNoRecipe => {
            encode_array_len(1, output);
            encode_u64(1, output);
        }
        SkillActivationRecipeResolutionV1::PacketAdmission {
            exact_admission,
            outcome,
        } => {
            encode_array_len(5, output);
            encode_u64(2, output);
            encode_text(&exact_admission.resolution_basis_ref, output);
            encode_text(&exact_admission.exact_packet_application_ref, output);
            encode_text(&exact_admission.exact_selected_job_route_ref, output);
            match outcome {
                PacketRecipeResolutionOutcomeV1::NoRecipe => {
                    encode_array_len(1, output);
                    encode_u64(1, output);
                }
                PacketRecipeResolutionOutcomeV1::Admitted(values) => {
                    encode_array_len(2, output);
                    encode_u64(2, output);
                    encode_text_array(values, output);
                }
            }
        }
    }
}

fn encode_loaded_closure_without_digest(closure: &LoadedResourceClosureV1, output: &mut Vec<u8>) {
    encode_array_len(5, output);
    encode_text(&closure.job_resource_ref, output);
    encode_text_array(&closure.direct_method_resource_refs, output);
    encode_text_array(&closure.tdd_child_resource_refs, output);
    match &closure.research_example_resource_ref {
        Some(value) => {
            encode_array_len(2, output);
            encode_u64(1, output);
            encode_text(value, output);
        }
        None => {
            encode_array_len(1, output);
            encode_u64(2, output);
        }
    }
    encode_text_array(&closure.recipe_resource_refs, output);
}

fn encode_loaded_closure_with_digest(closure: &LoadedResourceClosureV1, output: &mut Vec<u8>) {
    encode_array_len(2, output);
    encode_loaded_closure_without_digest(closure, output);
    encode_bytes(&closure.closure_digest, output);
}

fn is_current_ref(value: &str) -> bool {
    !value.is_empty()
        && !value.contains("latest")
        && !value.contains("fallback")
        && !value.chars().any(char::is_whitespace)
}

fn is_nominal_id(value: &str) -> bool {
    is_current_ref(value) && !value.starts_with("sha256:") && value.len() >= 8
}

fn is_sha256_ref(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(is_nonzero_lower_hex_digest)
}

fn is_nonzero_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn is_zero_digest(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn all_unique(values: &[String]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

fn all_unique_bytes(values: &[Vec<u8>]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

fn is_strictly_ordered_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn domain_hash(domain: &str, canonical_value: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain.as_bytes());
    hasher.update((canonical_value.len() as u64).to_be_bytes());
    hasher.update(canonical_value);
    hasher.finalize().into()
}

fn hex_digest(value: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in value {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn encode_text_array(values: &[String], output: &mut Vec<u8>) {
    encode_array_len(values.len(), output);
    for value in values {
        encode_text(value, output);
    }
}

fn encode_array_len(value: usize, output: &mut Vec<u8>) {
    encode_head(4, value as u64, output);
}

fn encode_u64(value: u64, output: &mut Vec<u8>) {
    encode_head(0, value, output);
}

fn encode_text(value: &str, output: &mut Vec<u8>) {
    encode_head(3, value.len() as u64, output);
    output.extend_from_slice(value.as_bytes());
}

fn encode_bytes(value: &[u8], output: &mut Vec<u8>) {
    encode_head(2, value.len() as u64, output);
    output.extend_from_slice(value);
}

fn encode_head(major: u8, value: u64, output: &mut Vec<u8>) {
    match value {
        0..=23 => output.push((major << 5) | value as u8),
        24..=0xff => output.extend_from_slice(&[(major << 5) | 24, value as u8]),
        0x100..=0xffff => {
            output.push((major << 5) | 25);
            output.extend_from_slice(&(value as u16).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            output.push((major << 5) | 26);
            output.extend_from_slice(&(value as u32).to_be_bytes());
        }
        _ => {
            output.push((major << 5) | 27);
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PublicLiteralError {
    #[error("McpPacketReadRequestV1 must use one exact context-disjoint read mode")]
    InvalidPacketReadRequest,
    #[error("McpPacketReadEnvelopeV1 must preserve one exact six-outcome payload")]
    InvalidPacketReadEnvelope,
    #[error("SelectionContextV1 must preserve the distinct canonical ordered 10 by 3 product")]
    InvalidSelectionContext,
    #[error("BootstrapRouteFactViewV1 must bind an exact nonzero semantic commitment")]
    InvalidBootstrapFactView,
    #[error(
        "PacketRecipeBindingV1 must reproduce request/application hashes and exact same-Frontier provenance"
    )]
    InvalidPacketRecipeBinding,
    #[error("AgentPacketV1 must bind its complete nonzero public projection and Recipe closure")]
    InvalidAgentPacket,
    #[error("McpCliSearch request/envelope bounds and cursor are inconsistent")]
    InvalidCliSearchEnvelope,
    #[error("JobRouteV1 is not one exact typed total route row")]
    InvalidJobRoute,
    #[error("JobGuidanceEnvelopeV1 does not preserve exact Packet/bootstrap and route identity")]
    InvalidJobGuidance,
    #[error(
        "OperationRequestV1/OperationResultV1 must preserve the disjoint Action/Ceremony branch"
    )]
    InvalidOperationEnvelope,
    #[error("SetupFactViewV1 must be a coherent typed single-locality owner-fact join")]
    InvalidSetupFactView,
    #[error(
        "SetupModeResolutionV1 must reproduce the facts-first request and Operation validation"
    )]
    InvalidSetupResolution,
    #[error("SkillActivationSubjectV1 must bind one nominal acquisition and typed context")]
    InvalidSkillActivationSubject,
    #[error("SkillActivationPayloadV1 must contain only complete Selected same-Release closures")]
    InvalidSkillActivationPayload,
    #[error("SkillActivationCandidateV1 commitments do not reproduce canonical bytes")]
    InvalidSkillActivationCandidate,
    #[error("LegacySkillActivationImportV1 is not byte-total or uses a promoting disposition")]
    InvalidLegacySkillActivationImport,
    #[error(transparent)]
    Recipe(#[from] crate::domain::vnext::orchestration::literals::RecipeLiteralError),
    #[error(transparent)]
    Capability(#[from] crate::domain::vnext::capability::literals::CapabilityLiteralError),
}
