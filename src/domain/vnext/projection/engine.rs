//! Projection-owned Packet, frontier, recommendation, and replay implementation.

#![allow(
    dead_code,
    reason = "Stage 6 is candidate-only until the separately owned root adapter is activated"
)]

use std::fmt::Write;

use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::capability::generated_catalog::{
    GeneratedCapabilityCatalogV1, GeneratedCatalogErrorV1,
};
use crate::domain::vnext::integration::public_literals::{
    AdvertisedOperationSpecV1, AgentPacketV1, BootstrapRouteFactViewV1, McpPacketReadEnvelopeV1,
    McpPacketReadModeV1, McpPacketReadRequestV1, OperationSpecRefV1, PacketBoundsV1,
    PacketCompletenessV1, PacketProjectionResultV1, PacketRecipeAdviceOutcomeV1,
    PacketRecipeAdviceProvenanceV1, PacketRecipeBindingV1, PacketRecipeComponentProvenanceV1,
    PacketRecipeComponentSlotV1, PacketScopeManifestV1, ProjectionScopeV1, RecipeSelectionOptionV1,
    SelectionContextV1,
};
use crate::domain::vnext::orchestration::literals::{
    ExactRecipeSelectionV1, RecipeComponentOutcomeTagV1, RecipeReturnOccurrenceV1,
    RecipeSelectionRequestV1,
};

const SELECTION_VECTORS_JSON: &str =
    include_str!("../../../../contracts/vnext/public/recipe_selection_application_vectors.v1.json");
const SELECTION_VECTORS_SHA256: &str =
    "d286a98d5d6d7146652a5a114bef15ef15e30fe49bab29ed996100c6d2357635";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProjectionReadStateV1 {
    Active(Box<ProjectionSnapshotV1>),
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

pub(crate) trait ProjectionReadPortV1 {
    fn read_once(
        &self,
        request: &McpPacketReadRequestV1,
    ) -> Result<ProjectionReadStateV1, ProjectionErrorV1>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecipeComponentProjectionV1 {
    pub occurrence: RecipeReturnOccurrenceV1,
    pub component_output_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectionSnapshotV1 {
    pub running_release_ref: String,
    pub public_catalog_ref: String,
    pub repository_locator: String,
    pub authenticated_host_connection_context_ref: String,
    pub exact_repository_domain_ref: String,
    pub frontier_ref: String,
    pub as_of_ref: String,
    pub valid_until_ref: String,
    pub visibility_ref: String,
    pub snapshot_manifest_ref: String,
    pub projection_result: PacketProjectionResultV1,
    pub completeness: PacketCompletenessV1,
    pub blockers: Vec<String>,
    pub required_inputs: Vec<String>,
    pub effect_classes: Vec<String>,
    pub idempotency_classes: Vec<String>,
    pub retry_classes: Vec<String>,
    pub inspect_refs: Vec<String>,
    pub live_guard_refs: Vec<String>,
    pub recipe_components: Vec<RecipeComponentProjectionV1>,
    pub maximum_bytes: u64,
    pub maximum_rows: u64,
    pub maximum_depth: u64,
}

pub(crate) fn read_packet(
    port: &dyn ProjectionReadPortV1,
    request: &McpPacketReadRequestV1,
) -> Result<McpPacketReadEnvelopeV1, ProjectionErrorV1> {
    request
        .validate()
        .map_err(|_| ProjectionErrorV1::InvalidRequest)?;
    let state = port.read_once(request)?;
    let envelope = match state {
        ProjectionReadStateV1::NoActiveStore {
            bootstrap_route_fact_view,
        } => McpPacketReadEnvelopeV1::NoActiveStore {
            bootstrap_route_fact_view,
        },
        ProjectionReadStateV1::Unavailable { reason_ref } => {
            McpPacketReadEnvelopeV1::Unavailable { reason_ref }
        }
        ProjectionReadStateV1::Stale { reason_ref } => {
            McpPacketReadEnvelopeV1::Stale { reason_ref }
        }
        ProjectionReadStateV1::Incompatible { reason_ref } => {
            McpPacketReadEnvelopeV1::Incompatible { reason_ref }
        }
        ProjectionReadStateV1::Active(snapshot) => {
            if !is_current(request, &snapshot) {
                McpPacketReadEnvelopeV1::Stale {
                    reason_ref: "candidate:projection:snapshot-currentness-mismatch:v1".to_owned(),
                }
            } else {
                match &request.read_mode {
                    McpPacketReadModeV1::BootstrapNoRecipeV1 => {
                        McpPacketReadEnvelopeV1::Incompatible {
                            reason_ref: "candidate:projection:active-store-forbids-bootstrap:v1"
                                .to_owned(),
                        }
                    }
                    McpPacketReadModeV1::DiscoverSelectionContextV1 => {
                        McpPacketReadEnvelopeV1::SelectionContext(selection_context(
                            &request.expected_public_catalog_ref,
                        )?)
                    }
                    McpPacketReadModeV1::ProjectV1(selection) => McpPacketReadEnvelopeV1::Packet(
                        Box::new(project_packet(request, selection, *snapshot)?),
                    ),
                }
            }
        }
    };
    envelope
        .validate()
        .map_err(|_| ProjectionErrorV1::InvalidProjection)?;
    Ok(envelope)
}

fn is_current(request: &McpPacketReadRequestV1, snapshot: &ProjectionSnapshotV1) -> bool {
    snapshot.running_release_ref == request.expected_release_ref
        && snapshot.public_catalog_ref == request.expected_public_catalog_ref
        && snapshot.repository_locator == request.repository_locator
        && snapshot.authenticated_host_connection_context_ref
            == request.authenticated_host_connection_context_ref
        && snapshot.projection_result.frontier_ref() == snapshot.frontier_ref
}

fn selection_context(resolution_basis_ref: &str) -> Result<SelectionContextV1, ProjectionErrorV1> {
    verify_selection_vectors()?;
    let document: Value = serde_json::from_str(SELECTION_VECTORS_JSON)
        .map_err(|_| ProjectionErrorV1::FrozenSelectionVectors)?;
    let vectors = document
        .get("vectors")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == 30)
        .ok_or(ProjectionErrorV1::FrozenSelectionVectors)?;
    let selection_options = vectors
        .iter()
        .map(|row| {
            let request = row
                .get("selection_request")
                .ok_or(ProjectionErrorV1::FrozenSelectionVectors)?;
            Ok(RecipeSelectionOptionV1 {
                primary_selection: parse_selection(
                    request
                        .get("primary_selection")
                        .ok_or(ProjectionErrorV1::FrozenSelectionVectors)?,
                    false,
                )?,
                continuation_selection: parse_selection(
                    request
                        .get("continuation_selection")
                        .ok_or(ProjectionErrorV1::FrozenSelectionVectors)?,
                    true,
                )?,
            })
        })
        .collect::<Result<Vec<_>, ProjectionErrorV1>>()?;
    let context = SelectionContextV1 {
        schema_version: 1,
        resolution_basis_ref: resolution_basis_ref.to_owned(),
        selection_options,
    };
    context
        .validate()
        .map_err(|_| ProjectionErrorV1::FrozenSelectionVectors)?;
    Ok(context)
}

fn parse_selection(
    value: &Value,
    continuation: bool,
) -> Result<ExactRecipeSelectionV1, ProjectionErrorV1> {
    match value.get("variant").and_then(Value::as_str) {
        Some("Absent") => Ok(ExactRecipeSelectionV1::Absent),
        Some("Present") => {
            let fields = value
                .get("value")
                .and_then(Value::as_array)
                .ok_or(ProjectionErrorV1::FrozenSelectionVectors)?;
            let text = |index| {
                fields
                    .get(index)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or(ProjectionErrorV1::FrozenSelectionVectors)
            };
            if continuation && fields.len() == 3 {
                Ok(ExactRecipeSelectionV1::Continuation {
                    recipe_resource_ref: text(0)?,
                    manifest_content_ref: text(1)?,
                    profile_resource_ref: text(2)?,
                })
            } else if !continuation && fields.len() == 2 {
                Ok(ExactRecipeSelectionV1::Primary {
                    recipe_resource_ref: text(0)?,
                    manifest_content_ref: text(1)?,
                })
            } else {
                Err(ProjectionErrorV1::FrozenSelectionVectors)
            }
        }
        _ => Err(ProjectionErrorV1::FrozenSelectionVectors),
    }
}

fn verify_selection_vectors() -> Result<(), ProjectionErrorV1> {
    let actual = lower_hex(&Sha256::digest(SELECTION_VECTORS_JSON.as_bytes()));
    if actual == SELECTION_VECTORS_SHA256 {
        Ok(())
    } else {
        Err(ProjectionErrorV1::FrozenSelectionVectors)
    }
}

fn validate_project_selection(
    current_resolution_basis_ref: &str,
    selection: &RecipeSelectionRequestV1,
) -> Result<(), ProjectionErrorV1> {
    if selection.resolution_basis_ref != current_resolution_basis_ref {
        return Err(ProjectionErrorV1::ForeignSelectionBasis);
    }
    let discovered = selection_context(current_resolution_basis_ref)?;
    if !discovered.selection_options.iter().any(|option| {
        option.primary_selection == selection.primary_selection
            && option.continuation_selection == selection.continuation_selection
    }) {
        return Err(ProjectionErrorV1::NonMemberSelection);
    }
    Ok(())
}

fn project_packet(
    request: &McpPacketReadRequestV1,
    selection: &RecipeSelectionRequestV1,
    snapshot: ProjectionSnapshotV1,
) -> Result<AgentPacketV1, ProjectionErrorV1> {
    validate_project_selection(&request.expected_public_catalog_ref, selection)?;
    let application = selection.clone().seal(snapshot.frontier_ref.clone())?;
    let expected_components = application.component_count();
    if snapshot.recipe_components.len() != expected_components {
        return Err(ProjectionErrorV1::RecipeProvenanceMismatch);
    }
    let expected_slots = match (
        application.primary.is_absent(),
        application.continuation.is_absent(),
    ) {
        (true, true) => Vec::new(),
        (false, true) => vec![PacketRecipeComponentSlotV1::Primary],
        (true, false) => vec![PacketRecipeComponentSlotV1::Continuation],
        (false, false) => vec![
            PacketRecipeComponentSlotV1::Primary,
            PacketRecipeComponentSlotV1::Continuation,
        ],
    };
    let component_provenance = snapshot
        .recipe_components
        .iter()
        .zip(expected_slots)
        .map(
            |(component, component_slot)| PacketRecipeComponentProvenanceV1 {
                component_slot,
                recipe_return_occurrence: component.occurrence.clone(),
                component_output_hash: component.component_output_hash,
            },
        )
        .collect::<Vec<_>>();
    let component_hashes = component_provenance
        .iter()
        .map(|row| row.component_output_hash)
        .collect::<Vec<_>>();
    let composition_outcome = compose_outcome(&component_provenance);
    let composed_output_hash = composed_hash(composition_outcome, &component_hashes);
    let recipe_binding = PacketRecipeBindingV1 {
        schema_version: 1,
        selection_request_hash: selection.semantic_hash()?,
        recipe_application_hash: application.semantic_hash()?,
        recipe_application: application,
        component_provenance,
        advice_provenance: PacketRecipeAdviceProvenanceV1 {
            composition_outcome,
            ordered_component_output_hashes: component_hashes,
            composed_output_hash,
        },
    };
    recipe_binding
        .validate()
        .map_err(|_| ProjectionErrorV1::RecipeProvenanceMismatch)?;

    let catalog = GeneratedCapabilityCatalogV1::load_frozen()?;
    let advertised_specs = advertised_specs(
        &catalog,
        &snapshot.projection_result,
        recipe_binding.is_actionable(),
        &snapshot.live_guard_refs,
    )?;
    let scope_manifest = match &request.projection_scope {
        ProjectionScopeV1::Repository => PacketScopeManifestV1::Repository {
            exact_repository_domain_ref: snapshot.exact_repository_domain_ref.clone(),
        },
        ProjectionScopeV1::Work { exact_work_ref } => PacketScopeManifestV1::Work {
            exact_repository_domain_ref: snapshot.exact_repository_domain_ref.clone(),
            exact_work_ref: exact_work_ref.clone(),
        },
    };
    let actual_rows = 1
        + snapshot.blockers.len()
        + advertised_specs.len()
        + snapshot.required_inputs.len()
        + snapshot.effect_classes.len()
        + snapshot.idempotency_classes.len()
        + snapshot.retry_classes.len()
        + snapshot.inspect_refs.len()
        + snapshot.recipe_components.len();
    let bounds = PacketBoundsV1 {
        maximum_bytes: snapshot.maximum_bytes,
        maximum_rows: snapshot.maximum_rows,
        maximum_depth: snapshot.maximum_depth,
        actual_bytes: 0,
        actual_rows: u64::try_from(actual_rows).map_err(|_| ProjectionErrorV1::BoundExceeded)?,
        actual_depth: 6,
    };
    if bounds.actual_bytes > bounds.maximum_bytes
        || bounds.actual_rows > bounds.maximum_rows
        || bounds.actual_depth > bounds.maximum_depth
    {
        return Err(ProjectionErrorV1::BoundExceeded);
    }
    let mut packet = AgentPacketV1 {
        schema_version: 1,
        packet_id: packet_id(&snapshot.frontier_ref, selection)?,
        semantic_audit_hash: [0; 32],
        as_of_ref: snapshot.as_of_ref,
        valid_until_ref: snapshot.valid_until_ref,
        visibility_ref: snapshot.visibility_ref,
        scope_manifest,
        completeness: snapshot.completeness,
        bounds,
        snapshot_manifest_ref: snapshot.snapshot_manifest_ref,
        projection_result: snapshot.projection_result,
        blockers: snapshot.blockers,
        advertised_specs,
        required_inputs: snapshot.required_inputs,
        effect_classes: snapshot.effect_classes,
        idempotency_classes: snapshot.idempotency_classes,
        retry_classes: snapshot.retry_classes,
        inspect_refs: snapshot.inspect_refs,
        recipe_binding,
    };
    for _ in 0..4 {
        let encoded_len = crate::domain::vnext::transport::encoded_packet_len(&packet)?;
        if packet.bounds.actual_bytes == encoded_len {
            break;
        }
        packet.bounds.actual_bytes = encoded_len;
    }
    if packet.bounds.actual_bytes > packet.bounds.maximum_bytes {
        return Err(ProjectionErrorV1::BoundExceeded);
    }
    packet.semantic_audit_hash = packet_semantic_hash(&packet);
    packet
        .validate()
        .map_err(|_| ProjectionErrorV1::InvalidProjection)?;
    Ok(packet)
}

fn compose_outcome(
    components: &[PacketRecipeComponentProvenanceV1],
) -> PacketRecipeAdviceOutcomeV1 {
    if components.is_empty() {
        PacketRecipeAdviceOutcomeV1::CoreOnly
    } else if components.iter().any(|row| {
        row.recipe_return_occurrence.outcome_tag == RecipeComponentOutcomeTagV1::HardStop
    }) {
        PacketRecipeAdviceOutcomeV1::HardStop
    } else if components.iter().any(|row| {
        row.recipe_return_occurrence.outcome_tag == RecipeComponentOutcomeTagV1::NotApplicable
    }) {
        PacketRecipeAdviceOutcomeV1::NotApplicable
    } else {
        PacketRecipeAdviceOutcomeV1::RestrictiveAdvice
    }
}

fn composed_hash(outcome: PacketRecipeAdviceOutcomeV1, component_hashes: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = DomainHasherV1::new(b"maestro.vnext.recipe-composition.v1");
    hasher.u64(outcome as u64 + 1);
    for hash in component_hashes {
        hasher.bytes(hash);
    }
    hasher.finish()
}

fn advertised_specs(
    catalog: &GeneratedCapabilityCatalogV1,
    result: &PacketProjectionResultV1,
    actionable: bool,
    live_guard_refs: &[String],
) -> Result<Vec<AdvertisedOperationSpecV1>, ProjectionErrorV1> {
    if !actionable {
        return Ok(Vec::new());
    }
    let refs = match result {
        PacketProjectionResultV1::Action {
            exact_action_spec_ref,
            ..
        } => vec![exact_action_spec_ref.as_str()],
        PacketProjectionResultV1::Wave {
            exact_action_spec_refs,
            ..
        } => exact_action_spec_refs.iter().map(String::as_str).collect(),
        PacketProjectionResultV1::PlanningRequired { .. }
        | PacketProjectionResultV1::Inspect { .. }
        | PacketProjectionResultV1::Wait { .. }
        | PacketProjectionResultV1::Stop { .. } => Vec::new(),
    };
    refs.into_iter()
        .map(|descriptor_ref| {
            let entry = catalog
                .action(descriptor_ref)
                .ok_or(ProjectionErrorV1::UncataloguedProjection)?;
            Ok(AdvertisedOperationSpecV1 {
                operation_spec: entry.operation_spec_ref(),
                material_dependency_stamp: entry.material_dependency_stamp(),
                live_guard_refs: live_guard_refs.to_vec(),
            })
        })
        .collect()
}

fn packet_id(
    frontier_ref: &str,
    selection: &RecipeSelectionRequestV1,
) -> Result<String, ProjectionErrorV1> {
    let mut hasher = DomainHasherV1::new(b"maestro.vnext.packet-id.v1");
    hasher.text(frontier_ref);
    hasher.bytes(&selection.semantic_hash()?);
    Ok(format!("sha256:{}", lower_hex(&hasher.finish())))
}

pub(crate) fn packet_semantic_hash(packet: &AgentPacketV1) -> [u8; 32] {
    let mut hash = DomainHasherV1::new(b"maestro.vnext.agent-packet.semantic-audit.v1");
    hash.u64(packet.schema_version);
    hash.text(&packet.packet_id);
    hash.text(&packet.as_of_ref);
    hash.text(&packet.valid_until_ref);
    hash.text(&packet.visibility_ref);
    match &packet.scope_manifest {
        PacketScopeManifestV1::Repository {
            exact_repository_domain_ref,
        } => {
            hash.u64(1);
            hash.text(exact_repository_domain_ref);
        }
        PacketScopeManifestV1::Work {
            exact_repository_domain_ref,
            exact_work_ref,
        } => {
            hash.u64(2);
            hash.text(exact_repository_domain_ref);
            hash.text(exact_work_ref);
        }
    }
    hash.u64(packet.completeness as u64 + 1);
    for value in [
        packet.bounds.maximum_bytes,
        packet.bounds.maximum_rows,
        packet.bounds.maximum_depth,
        packet.bounds.actual_bytes,
        packet.bounds.actual_rows,
        packet.bounds.actual_depth,
    ] {
        hash.u64(value);
    }
    hash.text(&packet.snapshot_manifest_ref);
    hash_projection(&mut hash, &packet.projection_result);
    for values in [
        &packet.blockers,
        &packet.required_inputs,
        &packet.effect_classes,
        &packet.idempotency_classes,
        &packet.retry_classes,
        &packet.inspect_refs,
    ] {
        hash.strings(values);
    }
    hash.u64(packet.advertised_specs.len() as u64);
    for spec in &packet.advertised_specs {
        hash.text(operation_ref(&spec.operation_spec));
        hash.bytes(&spec.material_dependency_stamp);
        hash.strings(&spec.live_guard_refs);
    }
    hash.bytes(&packet.recipe_binding.selection_request_hash);
    hash.bytes(&packet.recipe_binding.recipe_application_hash);
    hash.u64(packet.recipe_binding.component_provenance.len() as u64);
    for component in &packet.recipe_binding.component_provenance {
        hash.u64(component.component_slot as u64 + 1);
        hash.text(&component.recipe_return_occurrence.resolution_basis_ref);
        hash.text(&component.recipe_return_occurrence.frontier_ref);
        hash.u64(component.recipe_return_occurrence.outcome_tag as u64);
        hash.text(
            &component
                .recipe_return_occurrence
                .return_reason_ref
                .recipe_return_reason_resource_ref,
        );
        hash.bytes(&component.component_output_hash);
    }
    hash.u64(packet.recipe_binding.advice_provenance.composition_outcome as u64 + 1);
    for component in &packet
        .recipe_binding
        .advice_provenance
        .ordered_component_output_hashes
    {
        hash.bytes(component);
    }
    hash.bytes(&packet.recipe_binding.advice_provenance.composed_output_hash);
    hash.finish()
}

fn operation_ref(spec: &OperationSpecRefV1) -> &str {
    match spec {
        OperationSpecRefV1::Action(value) => &value.exact_action_spec_ref,
        OperationSpecRefV1::Ceremony(value) => &value.exact_ceremony_spec_ref,
    }
}

fn hash_projection(hash: &mut DomainHasherV1, result: &PacketProjectionResultV1) {
    hash.text(result.frontier_ref());
    match result {
        PacketProjectionResultV1::Action {
            exact_action_spec_ref,
            ..
        } => {
            hash.u64(1);
            hash.text(exact_action_spec_ref);
        }
        PacketProjectionResultV1::Wave {
            exact_wave_ref,
            exact_action_spec_refs,
            ..
        } => {
            hash.u64(2);
            hash.text(exact_wave_ref);
            hash.strings(exact_action_spec_refs);
        }
        PacketProjectionResultV1::PlanningRequired {
            exact_inspect_ref, ..
        } => {
            hash.u64(3);
            hash.text(exact_inspect_ref);
        }
        PacketProjectionResultV1::Inspect {
            exact_inspect_ref, ..
        } => {
            hash.u64(4);
            hash.text(exact_inspect_ref);
        }
        PacketProjectionResultV1::Wait {
            exact_reason_ref, ..
        } => {
            hash.u64(5);
            hash.text(exact_reason_ref);
        }
        PacketProjectionResultV1::Stop {
            exact_reason_ref, ..
        } => {
            hash.u64(6);
            hash.text(exact_reason_ref);
        }
    }
}

struct DomainHasherV1(Sha256);

impl DomainHasherV1 {
    fn new(domain: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_be_bytes());
        hasher.update(domain);
        Self(hasher)
    }

    fn u64(&mut self, value: u64) {
        self.0.update(value.to_be_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        self.0.update(value);
    }

    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn strings(&mut self, values: &[String]) {
        self.u64(values.len() as u64);
        for value in values {
            self.text(value);
        }
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}")
            .expect("invariant: writing hexadecimal into a String cannot fail");
    }
    encoded
}

#[derive(Debug, Error)]
pub(crate) enum ProjectionErrorV1 {
    #[error("the Packet read request is invalid")]
    InvalidRequest,
    #[error("the frozen selection discovery vectors are invalid")]
    FrozenSelectionVectors,
    #[error("the Project selection basis is stale or foreign to the current catalog")]
    ForeignSelectionBasis,
    #[error("the Project selection pair is not one of the 30 frozen discovery options")]
    NonMemberSelection,
    #[error("the recipe component provenance does not match the sealed application")]
    RecipeProvenanceMismatch,
    #[error("the projection names an Action outside the generated catalog")]
    UncataloguedProjection,
    #[error("the projected Packet exceeds its declared finite bound")]
    BoundExceeded,
    #[error("the projected Packet is internally inconsistent")]
    InvalidProjection,
    #[error("the owner projection read failed: {0}")]
    Read(String),
    #[error(transparent)]
    Catalog(#[from] GeneratedCatalogErrorV1),
    #[error(transparent)]
    Recipe(#[from] crate::domain::vnext::orchestration::literals::RecipeLiteralError),
    #[error(transparent)]
    Transport(#[from] crate::domain::vnext::transport::TransportErrorV1),
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    struct PureNoStorePort {
        reads: Cell<usize>,
    }

    impl ProjectionReadPortV1 for PureNoStorePort {
        fn read_once(
            &self,
            _request: &McpPacketReadRequestV1,
        ) -> Result<ProjectionReadStateV1, ProjectionErrorV1> {
            self.reads.set(self.reads.get() + 1);
            Ok(ProjectionReadStateV1::NoActiveStore {
                bootstrap_route_fact_view: None,
            })
        }
    }

    #[test]
    fn frozen_selection_discovery_is_exact_and_ordered() {
        let context = selection_context(
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .expect("selection vectors");
        assert_eq!(context.selection_options.len(), 30);
        assert!(context.validate().is_ok());
    }

    #[test]
    fn project_selection_rejects_foreign_basis_and_non_member_pair() {
        let basis = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let context = selection_context(basis).expect("selection vectors");
        let option = &context.selection_options[3];
        let member = RecipeSelectionRequestV1 {
            schema_version: 1,
            resolution_basis_ref: basis.to_owned(),
            primary_selection: option.primary_selection.clone(),
            continuation_selection: option.continuation_selection.clone(),
        };
        assert!(validate_project_selection(basis, &member).is_ok());

        let mut foreign = member.clone();
        foreign.resolution_basis_ref =
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned();
        assert!(matches!(
            validate_project_selection(basis, &foreign),
            Err(ProjectionErrorV1::ForeignSelectionBasis)
        ));

        let mut non_member = member;
        let ExactRecipeSelectionV1::Primary {
            manifest_content_ref,
            ..
        } = &mut non_member.primary_selection
        else {
            panic!("fixture option 4 is a primary selection");
        };
        *manifest_content_ref =
            "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_owned();
        assert!(non_member.validate().is_ok());
        assert!(matches!(
            validate_project_selection(basis, &non_member),
            Err(ProjectionErrorV1::NonMemberSelection)
        ));
    }

    #[test]
    fn every_generated_action_can_be_advertised_without_owner_semantics() {
        let catalog = GeneratedCapabilityCatalogV1::load_frozen().expect("catalog");
        for entry in catalog.actions() {
            let result = PacketProjectionResultV1::Action {
                frontier_ref: "candidate:frontier:current:v1".to_owned(),
                exact_action_spec_ref: entry.descriptor_ref().to_owned(),
            };
            let advertised =
                advertised_specs(&catalog, &result, true, &[]).expect("advertised action");
            assert_eq!(advertised.len(), 1);
            assert_eq!(
                operation_ref(&advertised[0].operation_spec),
                entry.descriptor_ref()
            );
        }
    }

    #[test]
    fn non_actionable_recipe_output_advertises_nothing() {
        let catalog = GeneratedCapabilityCatalogV1::load_frozen().expect("catalog");
        let result = PacketProjectionResultV1::Action {
            frontier_ref: "candidate:frontier:current:v1".to_owned(),
            exact_action_spec_ref: catalog.actions()[0].descriptor_ref().to_owned(),
        };
        assert!(
            advertised_specs(&catalog, &result, false, &[])
                .expect("filter")
                .is_empty()
        );
    }

    #[test]
    fn packet_read_is_one_read_with_no_write_or_network_port() {
        let port = PureNoStorePort {
            reads: Cell::new(0),
        };
        let request = McpPacketReadRequestV1 {
            schema_version: 1,
            request_id: "request-bootstrap".to_owned(),
            repository_locator: "/repo".to_owned(),
            authenticated_host_connection_context_ref: "candidate:host-connection:authenticated"
                .to_owned(),
            projection_scope: ProjectionScopeV1::Repository,
            expected_release_ref: "candidate:release:current".to_owned(),
            expected_public_catalog_ref: "candidate:catalog:public".to_owned(),
            bounded_response_redaction_profile: "bounded-default".to_owned(),
            read_mode: McpPacketReadModeV1::BootstrapNoRecipeV1,
        };
        assert!(matches!(
            read_packet(&port, &request).expect("read"),
            McpPacketReadEnvelopeV1::NoActiveStore { .. }
        ));
        assert_eq!(port.reads.get(), 1);
    }
}
