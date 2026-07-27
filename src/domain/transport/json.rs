//! Canonical Packet, Operation, Result, and Action JSON transport.

#![allow(
    dead_code,
    reason = "the canonical JSON adapter remains crate-internal"
)]

use std::collections::BTreeSet;
use std::fmt::Write;

use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::domain::integration::public_literals::{
    ActionAuthorityBasisV1, ActionRequestV1, AgentPacketV1, CeremonyRequestContextV1,
    CeremonyRequestModeV1, CeremonyRequestV1, McpPacketReadEnvelopeV1, McpPacketReadModeV1,
    McpPacketReadRequestV1, OperationRequestV1, OperationResultV1, OperationSemanticOutcomeV1,
    OperationSpecRefV1, OrchestrationAttributionV1, PacketCompletenessV1, PacketProjectionResultV1,
    PacketRecipeAdviceOutcomeV1, PacketRecipeComponentSlotV1, PacketScopeManifestV1,
    ProjectionScopeV1,
};
use crate::domain::orchestration::literals::{
    ExactRecipeSelectionV1, RecipeReturnComponentV1, RecipeSelectionRequestV1,
};

pub(crate) fn encode_packet_read_request(
    request: &McpPacketReadRequestV1,
) -> Result<String, TransportErrorV1> {
    request
        .validate()
        .map_err(|_| TransportErrorV1::InvalidDomainValue)?;
    canonical_json(&packet_request_value(request))
}

pub(crate) fn decode_packet_read_request(
    input: &str,
) -> Result<McpPacketReadRequestV1, TransportErrorV1> {
    let value = decode_json_with_unique_keys(input)?;
    let object = exact_object(
        &value,
        &[
            "schema_version",
            "request_id",
            "repository_locator",
            "authenticated_host_connection_context_ref",
            "projection_scope",
            "expected_release_ref",
            "expected_public_catalog_ref",
            "bounded_response_redaction_profile",
            "read_mode",
        ],
    )?;
    let request = McpPacketReadRequestV1 {
        schema_version: u64_field(object, "schema_version")?,
        request_id: text_field(object, "request_id")?,
        repository_locator: text_field(object, "repository_locator")?,
        authenticated_host_connection_context_ref: text_field(
            object,
            "authenticated_host_connection_context_ref",
        )?,
        projection_scope: decode_projection_scope(field(object, "projection_scope")?)?,
        expected_release_ref: text_field(object, "expected_release_ref")?,
        expected_public_catalog_ref: text_field(object, "expected_public_catalog_ref")?,
        bounded_response_redaction_profile: text_field(
            object,
            "bounded_response_redaction_profile",
        )?,
        read_mode: decode_read_mode(field(object, "read_mode")?)?,
    };
    request
        .validate()
        .map_err(|_| TransportErrorV1::InvalidDomainValue)?;
    Ok(request)
}

pub(crate) fn encode_operation_request(
    request: &OperationRequestV1,
) -> Result<String, TransportErrorV1> {
    request
        .validate()
        .map_err(|_| TransportErrorV1::InvalidDomainValue)?;
    canonical_json(&operation_request_value(request))
}

pub(crate) fn decode_operation_request(
    input: &str,
) -> Result<OperationRequestV1, TransportErrorV1> {
    let value = decode_json_with_unique_keys(input)?;
    let union = exact_object(&value, &["variant", "value"])?;
    let body = field(union, "value")?;
    let request = match text_ref(union, "variant")? {
        "Action" => OperationRequestV1::Action(decode_action(body)?),
        "Ceremony" => OperationRequestV1::Ceremony(decode_ceremony(body)?),
        _ => return Err(TransportErrorV1::UnknownVariant),
    };
    request
        .validate()
        .map_err(|_| TransportErrorV1::InvalidDomainValue)?;
    Ok(request)
}

pub(crate) fn encode_operation_result(
    result: &OperationResultV1,
) -> Result<String, TransportErrorV1> {
    let (variant, body) = match result {
        OperationResultV1::Action(result) => ("Action", &result.0),
        OperationResultV1::Ceremony(result) => ("Ceremony", &result.0),
    };
    let next_packet = body.next_packet.as_ref().map(|packet| packet_value(packet));
    canonical_json(&json!({
        "variant": variant,
        "value": {
            "schema_version": body.schema_version,
            "request_id": body.request_id,
            "operation_spec_ref": body.operation_spec_ref,
            "outcome": outcome_name(body.outcome),
            "before_revision_refs": body.before_revision_refs,
            "after_revision_refs": body.after_revision_refs,
            "transition_receipt_refs": body.transition_receipt_refs,
            "produced_record_refs": body.produced_record_refs,
            "next_packet": next_packet,
            "inspect_ref": body.inspect_ref,
            "replayed_delivery": body.replayed_delivery,
        }
    }))
}

pub(crate) fn encode_packet_read_envelope(
    envelope: &McpPacketReadEnvelopeV1,
) -> Result<String, TransportErrorV1> {
    envelope
        .validate()
        .map_err(|_| TransportErrorV1::InvalidDomainValue)?;
    let value = match envelope {
        McpPacketReadEnvelopeV1::Packet(packet) => {
            json!({"variant": "Packet", "value": packet_value(packet)})
        }
        McpPacketReadEnvelopeV1::SelectionContext(context) => json!({
            "variant": "SelectionContext",
            "value": {
                "schema_version": context.schema_version,
                "resolution_basis_ref": context.resolution_basis_ref,
                "selection_options": context.selection_options.iter().map(|option| json!({
                    "primary_selection": selection_value(&option.primary_selection),
                    "continuation_selection": selection_value(&option.continuation_selection),
                })).collect::<Vec<_>>(),
            }
        }),
        McpPacketReadEnvelopeV1::NoActiveStore {
            bootstrap_route_fact_view,
        } => json!({
            "variant": "NoActiveStore",
            "value": bootstrap_route_fact_view.as_ref().map(|view| json!({
                "schema_version": view.schema_version,
                "bootstrap_context": match view.bootstrap_context {
                    crate::domain::integration::public_literals::BootstrapContextV1::RepositoryBootstrap => "RepositoryBootstrap",
                    crate::domain::integration::public_literals::BootstrapContextV1::InstallationBootstrap => "InstallationBootstrap",
                },
                "resolution_basis_ref": view.resolution_basis_ref,
                "ordered_source_fact_commitments": view.ordered_source_fact_commitments,
                "fact_view_hash": digest_value(&view.fact_view_hash),
            })),
        }),
        McpPacketReadEnvelopeV1::Unavailable { reason_ref } => {
            json!({"variant": "Unavailable", "value": {"reason_ref": reason_ref}})
        }
        McpPacketReadEnvelopeV1::Stale { reason_ref } => {
            json!({"variant": "Stale", "value": {"reason_ref": reason_ref}})
        }
        McpPacketReadEnvelopeV1::Incompatible { reason_ref } => {
            json!({"variant": "Incompatible", "value": {"reason_ref": reason_ref}})
        }
    };
    canonical_json(&value)
}

pub(crate) fn encoded_packet_len(packet: &AgentPacketV1) -> Result<u64, TransportErrorV1> {
    u64::try_from(canonical_json(&packet_value(packet))?.len())
        .map_err(|_| TransportErrorV1::DocumentTooLarge)
}

fn packet_request_value(request: &McpPacketReadRequestV1) -> Value {
    json!({
        "schema_version": request.schema_version,
        "request_id": request.request_id,
        "repository_locator": request.repository_locator,
        "authenticated_host_connection_context_ref": request.authenticated_host_connection_context_ref,
        "projection_scope": projection_scope_value(&request.projection_scope),
        "expected_release_ref": request.expected_release_ref,
        "expected_public_catalog_ref": request.expected_public_catalog_ref,
        "bounded_response_redaction_profile": request.bounded_response_redaction_profile,
        "read_mode": read_mode_value(&request.read_mode),
    })
}

fn projection_scope_value(scope: &ProjectionScopeV1) -> Value {
    match scope {
        ProjectionScopeV1::Repository => json!({"variant": "Repository"}),
        ProjectionScopeV1::Work { exact_work_ref } => {
            json!({"variant": "Work", "value": {"exact_work_ref": exact_work_ref}})
        }
    }
}

fn decode_projection_scope(value: &Value) -> Result<ProjectionScopeV1, TransportErrorV1> {
    let object = value.as_object().ok_or(TransportErrorV1::ExpectedObject)?;
    match text_ref(object, "variant")? {
        "Repository" => {
            exact_keys(object, &["variant"])?;
            Ok(ProjectionScopeV1::Repository)
        }
        "Work" => {
            exact_keys(object, &["variant", "value"])?;
            let body = exact_object(field(object, "value")?, &["exact_work_ref"])?;
            Ok(ProjectionScopeV1::Work {
                exact_work_ref: text_field(body, "exact_work_ref")?,
            })
        }
        _ => Err(TransportErrorV1::UnknownVariant),
    }
}

fn read_mode_value(mode: &McpPacketReadModeV1) -> Value {
    match mode {
        McpPacketReadModeV1::BootstrapNoRecipeV1 => {
            json!({"variant": "BootstrapNoRecipeV1"})
        }
        McpPacketReadModeV1::DiscoverSelectionContextV1 => {
            json!({"variant": "DiscoverSelectionContextV1"})
        }
        McpPacketReadModeV1::ProjectV1(selection) => {
            json!({"variant": "ProjectV1", "value": selection_request_value(selection)})
        }
    }
}

fn decode_read_mode(value: &Value) -> Result<McpPacketReadModeV1, TransportErrorV1> {
    let object = value.as_object().ok_or(TransportErrorV1::ExpectedObject)?;
    match text_ref(object, "variant")? {
        "BootstrapNoRecipeV1" => {
            exact_keys(object, &["variant"])?;
            Ok(McpPacketReadModeV1::BootstrapNoRecipeV1)
        }
        "DiscoverSelectionContextV1" => {
            exact_keys(object, &["variant"])?;
            Ok(McpPacketReadModeV1::DiscoverSelectionContextV1)
        }
        "ProjectV1" => {
            exact_keys(object, &["variant", "value"])?;
            Ok(McpPacketReadModeV1::ProjectV1(decode_selection_request(
                field(object, "value")?,
            )?))
        }
        _ => Err(TransportErrorV1::UnknownVariant),
    }
}

fn selection_request_value(request: &RecipeSelectionRequestV1) -> Value {
    json!({
        "schema_version": request.schema_version,
        "resolution_basis_ref": request.resolution_basis_ref,
        "primary_selection": selection_value(&request.primary_selection),
        "continuation_selection": selection_value(&request.continuation_selection),
    })
}

fn decode_selection_request(value: &Value) -> Result<RecipeSelectionRequestV1, TransportErrorV1> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "resolution_basis_ref",
            "primary_selection",
            "continuation_selection",
        ],
    )?;
    Ok(RecipeSelectionRequestV1 {
        schema_version: u64_field(object, "schema_version")?,
        resolution_basis_ref: text_field(object, "resolution_basis_ref")?,
        primary_selection: decode_selection(field(object, "primary_selection")?, false)?,
        continuation_selection: decode_selection(field(object, "continuation_selection")?, true)?,
    })
}

fn selection_value(selection: &ExactRecipeSelectionV1) -> Value {
    match selection {
        ExactRecipeSelectionV1::Absent => json!({"variant": "Absent"}),
        ExactRecipeSelectionV1::Primary {
            recipe_resource_ref,
            manifest_content_ref,
        } => json!({
            "variant": "Present",
            "value": [recipe_resource_ref, manifest_content_ref],
        }),
        ExactRecipeSelectionV1::Continuation {
            recipe_resource_ref,
            manifest_content_ref,
            profile_resource_ref,
        } => json!({
            "variant": "Present",
            "value": [recipe_resource_ref, manifest_content_ref, profile_resource_ref],
        }),
    }
}

fn decode_selection(
    value: &Value,
    continuation: bool,
) -> Result<ExactRecipeSelectionV1, TransportErrorV1> {
    let object = value.as_object().ok_or(TransportErrorV1::ExpectedObject)?;
    match text_ref(object, "variant")? {
        "Absent" => {
            exact_keys(object, &["variant"])?;
            Ok(ExactRecipeSelectionV1::Absent)
        }
        "Present" => {
            exact_keys(object, &["variant", "value"])?;
            let values = field(object, "value")?
                .as_array()
                .ok_or(TransportErrorV1::ExpectedArray)?;
            let text = |index| {
                values
                    .get(index)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or(TransportErrorV1::ExpectedString)
            };
            if continuation && values.len() == 3 {
                Ok(ExactRecipeSelectionV1::Continuation {
                    recipe_resource_ref: text(0)?,
                    manifest_content_ref: text(1)?,
                    profile_resource_ref: text(2)?,
                })
            } else if !continuation && values.len() == 2 {
                Ok(ExactRecipeSelectionV1::Primary {
                    recipe_resource_ref: text(0)?,
                    manifest_content_ref: text(1)?,
                })
            } else {
                Err(TransportErrorV1::InvalidUnionShape)
            }
        }
        _ => Err(TransportErrorV1::UnknownVariant),
    }
}

fn operation_request_value(request: &OperationRequestV1) -> Value {
    match request {
        OperationRequestV1::Action(action) => {
            json!({"variant": "Action", "value": action_value(action)})
        }
        OperationRequestV1::Ceremony(ceremony) => {
            json!({"variant": "Ceremony", "value": ceremony_value(ceremony)})
        }
    }
}

fn action_value(action: &ActionRequestV1) -> Value {
    json!({
        "schema_version": action.schema_version,
        "request_id": action.request_id,
        "idempotency_key": action.idempotency_key,
        "semantic_request_hash": digest_value(&action.semantic_request_hash),
        "selected_packet_semantic_hash": digest_value(&action.selected_packet_semantic_hash),
        "action_spec": {
            "exact_action_spec_ref": action.action_spec.exact_action_spec_ref,
            "exact_schema_id": action.action_spec.exact_schema_id,
            "exact_core_catalog_ref": action.action_spec.exact_core_catalog_ref,
            "exact_public_catalog_ref": action.action_spec.exact_public_catalog_ref,
        },
        "material_dependency_stamp": digest_value(&action.material_dependency_stamp),
        "exact_store_generation_ref": action.exact_store_generation_ref,
        "exact_authority_epoch_ref": action.exact_authority_epoch_ref,
        "valid_until_ref": action.valid_until_ref,
        "authority_basis": authority_value(&action.authority_basis),
        "typed_input_cbor": action.typed_input_cbor,
        "evidence_refs": action.evidence_refs,
        "prerequisite_receipt_refs": action.prerequisite_receipt_refs,
        "orchestration_attribution": action.orchestration_attribution.as_ref().map(attribution_value),
    })
}

fn decode_action(value: &Value) -> Result<ActionRequestV1, TransportErrorV1> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "request_id",
            "idempotency_key",
            "semantic_request_hash",
            "selected_packet_semantic_hash",
            "action_spec",
            "material_dependency_stamp",
            "exact_store_generation_ref",
            "exact_authority_epoch_ref",
            "valid_until_ref",
            "authority_basis",
            "typed_input_cbor",
            "evidence_refs",
            "prerequisite_receipt_refs",
            "orchestration_attribution",
        ],
    )?;
    let spec = exact_object(
        field(object, "action_spec")?,
        &[
            "exact_action_spec_ref",
            "exact_schema_id",
            "exact_core_catalog_ref",
            "exact_public_catalog_ref",
        ],
    )?;
    Ok(ActionRequestV1 {
        schema_version: u64_field(object, "schema_version")?,
        request_id: text_field(object, "request_id")?,
        idempotency_key: text_field(object, "idempotency_key")?,
        semantic_request_hash: digest_field(object, "semantic_request_hash")?,
        selected_packet_semantic_hash: digest_field(object, "selected_packet_semantic_hash")?,
        action_spec: crate::domain::integration::public_literals::ActionSpecRefV1 {
            exact_action_spec_ref: text_field(spec, "exact_action_spec_ref")?,
            exact_schema_id: text_field(spec, "exact_schema_id")?,
            exact_core_catalog_ref: text_field(spec, "exact_core_catalog_ref")?,
            exact_public_catalog_ref: text_field(spec, "exact_public_catalog_ref")?,
        },
        material_dependency_stamp: digest_field(object, "material_dependency_stamp")?,
        exact_store_generation_ref: text_field(object, "exact_store_generation_ref")?,
        exact_authority_epoch_ref: text_field(object, "exact_authority_epoch_ref")?,
        valid_until_ref: text_field(object, "valid_until_ref")?,
        authority_basis: decode_authority(field(object, "authority_basis")?)?,
        typed_input_cbor: byte_array(field(object, "typed_input_cbor")?)?,
        evidence_refs: string_array(field(object, "evidence_refs")?)?,
        prerequisite_receipt_refs: string_array(field(object, "prerequisite_receipt_refs")?)?,
        orchestration_attribution: decode_optional_attribution(field(
            object,
            "orchestration_attribution",
        )?)?,
    })
}

fn authority_value(authority: &ActionAuthorityBasisV1) -> Value {
    match authority {
        ActionAuthorityBasisV1::Ordinary {
            verified_principal_ref,
            current_session_ref,
            live_grant_refs,
            required_mandate_refs,
        } => json!({"variant": "Ordinary", "value": {
            "verified_principal_ref": verified_principal_ref,
            "current_session_ref": current_session_ref,
            "live_grant_refs": live_grant_refs,
            "required_mandate_refs": required_mandate_refs,
        }}),
        ActionAuthorityBasisV1::BootstrapControl {
            exact_bootstrap_scope_ref,
            current_executor_assertion_ref,
        } => json!({"variant": "BootstrapControl", "value": {
            "exact_bootstrap_scope_ref": exact_bootstrap_scope_ref,
            "current_executor_assertion_ref": current_executor_assertion_ref,
        }}),
        ActionAuthorityBasisV1::ContinuityMaintenance {
            exact_cma_branch_ref,
            maintenance_executor_assertion_ref,
            applicability_ref,
            phase_slot_ref,
        } => json!({"variant": "ContinuityMaintenance", "value": {
            "exact_cma_branch_ref": exact_cma_branch_ref,
            "maintenance_executor_assertion_ref": maintenance_executor_assertion_ref,
            "applicability_ref": applicability_ref,
            "phase_slot_ref": phase_slot_ref,
        }}),
    }
}

fn decode_authority(value: &Value) -> Result<ActionAuthorityBasisV1, TransportErrorV1> {
    let union = exact_object(value, &["variant", "value"])?;
    let body = field(union, "value")?;
    match text_ref(union, "variant")? {
        "Ordinary" => {
            let object = exact_object(
                body,
                &[
                    "verified_principal_ref",
                    "current_session_ref",
                    "live_grant_refs",
                    "required_mandate_refs",
                ],
            )?;
            Ok(ActionAuthorityBasisV1::Ordinary {
                verified_principal_ref: text_field(object, "verified_principal_ref")?,
                current_session_ref: text_field(object, "current_session_ref")?,
                live_grant_refs: string_array(field(object, "live_grant_refs")?)?,
                required_mandate_refs: string_array(field(object, "required_mandate_refs")?)?,
            })
        }
        "BootstrapControl" => {
            let object = exact_object(
                body,
                &[
                    "exact_bootstrap_scope_ref",
                    "current_executor_assertion_ref",
                ],
            )?;
            Ok(ActionAuthorityBasisV1::BootstrapControl {
                exact_bootstrap_scope_ref: text_field(object, "exact_bootstrap_scope_ref")?,
                current_executor_assertion_ref: text_field(
                    object,
                    "current_executor_assertion_ref",
                )?,
            })
        }
        "ContinuityMaintenance" => {
            let object = exact_object(
                body,
                &[
                    "exact_cma_branch_ref",
                    "maintenance_executor_assertion_ref",
                    "applicability_ref",
                    "phase_slot_ref",
                ],
            )?;
            Ok(ActionAuthorityBasisV1::ContinuityMaintenance {
                exact_cma_branch_ref: text_field(object, "exact_cma_branch_ref")?,
                maintenance_executor_assertion_ref: text_field(
                    object,
                    "maintenance_executor_assertion_ref",
                )?,
                applicability_ref: text_field(object, "applicability_ref")?,
                phase_slot_ref: text_field(object, "phase_slot_ref")?,
            })
        }
        _ => Err(TransportErrorV1::UnknownVariant),
    }
}

fn ceremony_value(ceremony: &CeremonyRequestV1) -> Value {
    json!({
        "schema_version": ceremony.schema_version,
        "request_id": ceremony.request_id,
        "idempotency_key": ceremony.idempotency_key,
        "semantic_request_hash": digest_value(&ceremony.semantic_request_hash),
        "ceremony_spec": {
            "exact_ceremony_spec_ref": ceremony.ceremony_spec.exact_ceremony_spec_ref,
            "exact_schema_id": ceremony.ceremony_spec.exact_schema_id,
            "exact_core_catalog_ref": ceremony.ceremony_spec.exact_core_catalog_ref,
            "exact_public_catalog_ref": ceremony.ceremony_spec.exact_public_catalog_ref,
        },
        "request_mode": ceremony_mode_name(ceremony.request_mode),
        "context": ceremony_context_value(&ceremony.context),
        "branch_authority_ref": ceremony.branch_authority_ref,
        "expected_carrier_token_ref": ceremony.expected_carrier_token_ref,
        "typed_input_cbor": ceremony.typed_input_cbor,
        "prerequisite_receipt_refs": ceremony.prerequisite_receipt_refs,
        "orchestration_attribution": ceremony.orchestration_attribution.as_ref().map(attribution_value),
    })
}

fn decode_ceremony(value: &Value) -> Result<CeremonyRequestV1, TransportErrorV1> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "request_id",
            "idempotency_key",
            "semantic_request_hash",
            "ceremony_spec",
            "request_mode",
            "context",
            "branch_authority_ref",
            "expected_carrier_token_ref",
            "typed_input_cbor",
            "prerequisite_receipt_refs",
            "orchestration_attribution",
        ],
    )?;
    let spec = exact_object(
        field(object, "ceremony_spec")?,
        &[
            "exact_ceremony_spec_ref",
            "exact_schema_id",
            "exact_core_catalog_ref",
            "exact_public_catalog_ref",
        ],
    )?;
    Ok(CeremonyRequestV1 {
        schema_version: u64_field(object, "schema_version")?,
        request_id: text_field(object, "request_id")?,
        idempotency_key: text_field(object, "idempotency_key")?,
        semantic_request_hash: digest_field(object, "semantic_request_hash")?,
        ceremony_spec: crate::domain::integration::public_literals::CeremonySpecRefV1 {
            exact_ceremony_spec_ref: text_field(spec, "exact_ceremony_spec_ref")?,
            exact_schema_id: text_field(spec, "exact_schema_id")?,
            exact_core_catalog_ref: text_field(spec, "exact_core_catalog_ref")?,
            exact_public_catalog_ref: text_field(spec, "exact_public_catalog_ref")?,
        },
        request_mode: decode_ceremony_mode(text_ref(object, "request_mode")?)?,
        context: decode_ceremony_context(field(object, "context")?)?,
        branch_authority_ref: text_field(object, "branch_authority_ref")?,
        expected_carrier_token_ref: text_field(object, "expected_carrier_token_ref")?,
        typed_input_cbor: byte_array(field(object, "typed_input_cbor")?)?,
        prerequisite_receipt_refs: string_array(field(object, "prerequisite_receipt_refs")?)?,
        orchestration_attribution: decode_optional_attribution(field(
            object,
            "orchestration_attribution",
        )?)?,
    })
}

fn ceremony_mode_name(mode: CeremonyRequestModeV1) -> &'static str {
    match mode {
        CeremonyRequestModeV1::Initiate => "Initiate",
        CeremonyRequestModeV1::RecoverReserved => "RecoverReserved",
        CeremonyRequestModeV1::ResolveResult => "ResolveResult",
        CeremonyRequestModeV1::Withdraw => "Withdraw",
    }
}

fn decode_ceremony_mode(value: &str) -> Result<CeremonyRequestModeV1, TransportErrorV1> {
    match value {
        "Initiate" => Ok(CeremonyRequestModeV1::Initiate),
        "RecoverReserved" => Ok(CeremonyRequestModeV1::RecoverReserved),
        "ResolveResult" => Ok(CeremonyRequestModeV1::ResolveResult),
        "Withdraw" => Ok(CeremonyRequestModeV1::Withdraw),
        _ => Err(TransportErrorV1::UnknownVariant),
    }
}

fn ceremony_context_value(context: &CeremonyRequestContextV1) -> Value {
    match context {
        CeremonyRequestContextV1::NoStore {
            protected_realm_ref,
            genesis_candidate_ref,
        } => json!({"variant": "NoStore", "value": {
            "protected_realm_ref": protected_realm_ref,
            "genesis_candidate_ref": genesis_candidate_ref,
        }}),
        CeremonyRequestContextV1::PreStore {
            protected_carrier_ref,
            candidate_seal_ref,
            expected_old_token_ref,
        } => json!({"variant": "PreStore", "value": {
            "protected_carrier_ref": protected_carrier_ref,
            "candidate_seal_ref": candidate_seal_ref,
            "expected_old_token_ref": expected_old_token_ref,
        }}),
    }
}

fn decode_ceremony_context(value: &Value) -> Result<CeremonyRequestContextV1, TransportErrorV1> {
    let union = exact_object(value, &["variant", "value"])?;
    match text_ref(union, "variant")? {
        "NoStore" => {
            let body = exact_object(
                field(union, "value")?,
                &["protected_realm_ref", "genesis_candidate_ref"],
            )?;
            Ok(CeremonyRequestContextV1::NoStore {
                protected_realm_ref: text_field(body, "protected_realm_ref")?,
                genesis_candidate_ref: text_field(body, "genesis_candidate_ref")?,
            })
        }
        "PreStore" => {
            let body = exact_object(
                field(union, "value")?,
                &[
                    "protected_carrier_ref",
                    "candidate_seal_ref",
                    "expected_old_token_ref",
                ],
            )?;
            Ok(CeremonyRequestContextV1::PreStore {
                protected_carrier_ref: text_field(body, "protected_carrier_ref")?,
                candidate_seal_ref: text_field(body, "candidate_seal_ref")?,
                expected_old_token_ref: text_field(body, "expected_old_token_ref")?,
            })
        }
        _ => Err(TransportErrorV1::UnknownVariant),
    }
}

fn attribution_value(value: &OrchestrationAttributionV1) -> Value {
    json!({
        "exact_packet_recipe_binding_ref": value.exact_packet_recipe_binding_ref,
        "exact_application_ref": value.exact_application_ref,
        "component_output_hashes": value.component_output_hashes.iter().map(digest_value).collect::<Vec<_>>(),
        "composed_advice_hash": digest_value(&value.composed_advice_hash),
    })
}

fn decode_optional_attribution(
    value: &Value,
) -> Result<Option<OrchestrationAttributionV1>, TransportErrorV1> {
    if value.is_null() {
        return Ok(None);
    }
    let object = exact_object(
        value,
        &[
            "exact_packet_recipe_binding_ref",
            "exact_application_ref",
            "component_output_hashes",
            "composed_advice_hash",
        ],
    )?;
    let component_output_hashes = field(object, "component_output_hashes")?
        .as_array()
        .ok_or(TransportErrorV1::ExpectedArray)?
        .iter()
        .map(parse_digest)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(OrchestrationAttributionV1 {
        exact_packet_recipe_binding_ref: text_field(object, "exact_packet_recipe_binding_ref")?,
        exact_application_ref: text_field(object, "exact_application_ref")?,
        component_output_hashes,
        composed_advice_hash: digest_field(object, "composed_advice_hash")?,
    }))
}

fn packet_value(packet: &AgentPacketV1) -> Value {
    json!({
        "schema_version": packet.schema_version,
        "packet_id": packet.packet_id,
        "semantic_audit_hash": digest_value(&packet.semantic_audit_hash),
        "as_of_ref": packet.as_of_ref,
        "valid_until_ref": packet.valid_until_ref,
        "visibility_ref": packet.visibility_ref,
        "scope_manifest": packet_scope_value(&packet.scope_manifest),
        "completeness": match packet.completeness {
            PacketCompletenessV1::Complete => "Complete",
            PacketCompletenessV1::IncompleteBlocked => "IncompleteBlocked",
        },
        "bounds": {
            "maximum_bytes": packet.bounds.maximum_bytes,
            "maximum_rows": packet.bounds.maximum_rows,
            "maximum_depth": packet.bounds.maximum_depth,
            "actual_bytes": packet.bounds.actual_bytes,
            "actual_rows": packet.bounds.actual_rows,
            "actual_depth": packet.bounds.actual_depth,
        },
        "snapshot_manifest_ref": packet.snapshot_manifest_ref,
        "projection_result": projection_result_value(&packet.projection_result),
        "blockers": packet.blockers,
        "advertised_specs": packet.advertised_specs.iter().map(|spec| json!({
            "operation_spec": operation_spec_value(&spec.operation_spec),
            "material_dependency_stamp": digest_value(&spec.material_dependency_stamp),
            "live_guard_refs": spec.live_guard_refs,
        })).collect::<Vec<_>>(),
        "required_inputs": packet.required_inputs,
        "effect_classes": packet.effect_classes,
        "idempotency_classes": packet.idempotency_classes,
        "retry_classes": packet.retry_classes,
        "inspect_refs": packet.inspect_refs,
        "recipe_binding": {
            "schema_version": packet.recipe_binding.schema_version,
            "selection_request_hash": digest_value(&packet.recipe_binding.selection_request_hash),
            "recipe_application": {
                "schema_version": packet.recipe_binding.recipe_application.schema_version,
                "resolution_basis_ref": packet.recipe_binding.recipe_application.resolution_basis_ref,
                "frontier_ref": packet.recipe_binding.recipe_application.frontier_ref,
                "primary": selection_value(&packet.recipe_binding.recipe_application.primary),
                "continuation": selection_value(&packet.recipe_binding.recipe_application.continuation),
            },
            "recipe_application_hash": digest_value(&packet.recipe_binding.recipe_application_hash),
            "component_provenance": packet.recipe_binding.component_provenance.iter().map(|component| json!({
                "component_slot": match component.component_slot {
                    PacketRecipeComponentSlotV1::Primary => "Primary",
                    PacketRecipeComponentSlotV1::Continuation => "Continuation",
                },
                "recipe_return_occurrence": return_occurrence_value(&component.recipe_return_occurrence),
                "component_output_hash": digest_value(&component.component_output_hash),
            })).collect::<Vec<_>>(),
            "advice_provenance": {
                "composition_outcome": match packet.recipe_binding.advice_provenance.composition_outcome {
                    PacketRecipeAdviceOutcomeV1::CoreOnly => "CoreOnly",
                    PacketRecipeAdviceOutcomeV1::NotApplicable => "NotApplicable",
                    PacketRecipeAdviceOutcomeV1::RestrictiveAdvice => "RestrictiveAdvice",
                    PacketRecipeAdviceOutcomeV1::HardStop => "HardStop",
                },
                "ordered_component_output_hashes": packet.recipe_binding.advice_provenance.ordered_component_output_hashes.iter().map(digest_value).collect::<Vec<_>>(),
                "composed_output_hash": digest_value(&packet.recipe_binding.advice_provenance.composed_output_hash),
            },
        },
    })
}

fn packet_scope_value(scope: &PacketScopeManifestV1) -> Value {
    match scope {
        PacketScopeManifestV1::Repository {
            exact_repository_domain_ref,
        } => json!({
            "variant": "Repository",
            "value": {"exact_repository_domain_ref": exact_repository_domain_ref},
        }),
        PacketScopeManifestV1::Work {
            exact_repository_domain_ref,
            exact_work_ref,
        } => json!({
            "variant": "Work",
            "value": {
                "exact_repository_domain_ref": exact_repository_domain_ref,
                "exact_work_ref": exact_work_ref,
            },
        }),
    }
}

fn projection_result_value(result: &PacketProjectionResultV1) -> Value {
    match result {
        PacketProjectionResultV1::Action {
            frontier_ref,
            exact_action_spec_ref,
        } => json!({"variant": "Action", "value": {
            "frontier_ref": frontier_ref,
            "exact_action_spec_ref": exact_action_spec_ref,
        }}),
        PacketProjectionResultV1::Wave {
            frontier_ref,
            exact_wave_ref,
            exact_action_spec_refs,
        } => json!({"variant": "Wave", "value": {
            "frontier_ref": frontier_ref,
            "exact_wave_ref": exact_wave_ref,
            "exact_action_spec_refs": exact_action_spec_refs,
        }}),
        PacketProjectionResultV1::PlanningRequired {
            frontier_ref,
            exact_inspect_ref,
        } => json!({"variant": "PlanningRequired", "value": {
            "frontier_ref": frontier_ref,
            "exact_inspect_ref": exact_inspect_ref,
        }}),
        PacketProjectionResultV1::Inspect {
            frontier_ref,
            exact_inspect_ref,
        } => json!({"variant": "Inspect", "value": {
            "frontier_ref": frontier_ref,
            "exact_inspect_ref": exact_inspect_ref,
        }}),
        PacketProjectionResultV1::Wait {
            frontier_ref,
            exact_reason_ref,
        } => json!({"variant": "Wait", "value": {
            "frontier_ref": frontier_ref,
            "exact_reason_ref": exact_reason_ref,
        }}),
        PacketProjectionResultV1::Stop {
            frontier_ref,
            exact_reason_ref,
        } => json!({"variant": "Stop", "value": {
            "frontier_ref": frontier_ref,
            "exact_reason_ref": exact_reason_ref,
        }}),
    }
}

fn operation_spec_value(spec: &OperationSpecRefV1) -> Value {
    match spec {
        OperationSpecRefV1::Action(value) => json!({"variant": "Action", "value": {
            "exact_action_spec_ref": value.exact_action_spec_ref,
            "exact_schema_id": value.exact_schema_id,
            "exact_core_catalog_ref": value.exact_core_catalog_ref,
            "exact_public_catalog_ref": value.exact_public_catalog_ref,
        }}),
        OperationSpecRefV1::Ceremony(value) => json!({"variant": "Ceremony", "value": {
            "exact_ceremony_spec_ref": value.exact_ceremony_spec_ref,
            "exact_schema_id": value.exact_schema_id,
            "exact_core_catalog_ref": value.exact_core_catalog_ref,
            "exact_public_catalog_ref": value.exact_public_catalog_ref,
        }}),
    }
}

fn return_occurrence_value(
    occurrence: &crate::domain::orchestration::literals::RecipeReturnOccurrenceV1,
) -> Value {
    json!({
        "schema_version": occurrence.schema_version,
        "resolution_basis_ref": occurrence.resolution_basis_ref,
        "frontier_ref": occurrence.frontier_ref,
        "component": match &occurrence.component {
            RecipeReturnComponentV1::Primary {
                recipe_resource_ref,
                manifest_content_ref,
            } => json!({"variant": "Primary", "value": {
                "recipe_resource_ref": recipe_resource_ref,
                "manifest_content_ref": manifest_content_ref,
            }}),
            RecipeReturnComponentV1::Continuation {
                recipe_resource_ref,
                manifest_content_ref,
                profile_resource_ref,
            } => json!({"variant": "Continuation", "value": {
                "recipe_resource_ref": recipe_resource_ref,
                "manifest_content_ref": manifest_content_ref,
                "profile_resource_ref": profile_resource_ref,
            }}),
        },
        "outcome_tag": occurrence.outcome_tag.name(),
        "return_reason_ref": {
            "recipe_return_reason_resource_ref": occurrence.return_reason_ref.recipe_return_reason_resource_ref,
            "reason": occurrence.return_reason_ref.reason.name(),
        },
    })
}

fn outcome_name(outcome: OperationSemanticOutcomeV1) -> &'static str {
    match outcome {
        OperationSemanticOutcomeV1::Committed => "committed",
        OperationSemanticOutcomeV1::NoOp => "no_op",
        OperationSemanticOutcomeV1::Rejected => "rejected",
        OperationSemanticOutcomeV1::Stale => "stale",
        OperationSemanticOutcomeV1::Conflict => "conflict",
        OperationSemanticOutcomeV1::Unavailable => "unavailable",
        OperationSemanticOutcomeV1::InDoubt => "in_doubt",
    }
}

/// Parses untrusted JSON while refusing duplicate object keys anywhere in the
/// document. `serde_json::from_str` keeps the last duplicate silently, which
/// would let a duplicated field pass the exact-field-set gate with a smuggled
/// second value.
fn decode_json_with_unique_keys(input: &str) -> Result<Value, TransportErrorV1> {
    struct UniqueKeyValueSeed;

    impl<'de> serde::de::DeserializeSeed<'de> for UniqueKeyValueSeed {
        type Value = Value;

        fn deserialize<D>(self, deserializer: D) -> Result<Value, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            deserializer.deserialize_any(UniqueKeyValueVisitor)
        }
    }

    struct UniqueKeyValueVisitor;

    impl<'de> serde::de::Visitor<'de> for UniqueKeyValueVisitor {
        type Value = Value;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a JSON value whose objects have unique keys")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
            Ok(Value::Bool(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
            Ok(Value::from(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
            Ok(Value::from(value))
        }

        fn visit_f64<E>(self, value: f64) -> Result<Value, E>
        where
            E: serde::de::Error,
        {
            serde_json::Number::from_f64(value)
                .map(Value::Number)
                .ok_or_else(|| E::custom("non-finite JSON number"))
        }

        fn visit_str<E>(self, value: &str) -> Result<Value, E> {
            Ok(Value::String(value.to_owned()))
        }

        fn visit_string<E>(self, value: String) -> Result<Value, E> {
            Ok(Value::String(value))
        }

        fn visit_unit<E>(self) -> Result<Value, E> {
            Ok(Value::Null)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut items = Vec::new();
            while let Some(item) = seq.next_element_seed(UniqueKeyValueSeed)? {
                items.push(item);
            }
            Ok(Value::Array(items))
        }

        fn visit_map<A>(self, mut entries: A) -> Result<Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut object = Map::new();
            while let Some(key) = entries.next_key::<String>()? {
                let value = entries.next_value_seed(UniqueKeyValueSeed)?;
                if object.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate object key"));
                }
            }
            Ok(Value::Object(object))
        }
    }

    let mut deserializer = serde_json::Deserializer::from_str(input);
    let value = serde::de::DeserializeSeed::deserialize(UniqueKeyValueSeed, &mut deserializer)?;
    deserializer.end()?;
    Ok(value)
}

fn exact_object<'value>(
    value: &'value Value,
    expected: &[&str],
) -> Result<&'value Map<String, Value>, TransportErrorV1> {
    let object = value.as_object().ok_or(TransportErrorV1::ExpectedObject)?;
    exact_keys(object, expected)?;
    Ok(object)
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str]) -> Result<(), TransportErrorV1> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(TransportErrorV1::UnknownOrMissingField)
    }
}

fn field<'value>(
    object: &'value Map<String, Value>,
    name: &str,
) -> Result<&'value Value, TransportErrorV1> {
    object
        .get(name)
        .ok_or(TransportErrorV1::UnknownOrMissingField)
}

fn text_ref<'value>(
    object: &'value Map<String, Value>,
    name: &str,
) -> Result<&'value str, TransportErrorV1> {
    field(object, name)?
        .as_str()
        .ok_or(TransportErrorV1::ExpectedString)
}

fn text_field(object: &Map<String, Value>, name: &str) -> Result<String, TransportErrorV1> {
    Ok(text_ref(object, name)?.to_owned())
}

fn u64_field(object: &Map<String, Value>, name: &str) -> Result<u64, TransportErrorV1> {
    field(object, name)?
        .as_u64()
        .ok_or(TransportErrorV1::ExpectedUnsigned)
}

fn string_array(value: &Value) -> Result<Vec<String>, TransportErrorV1> {
    value
        .as_array()
        .ok_or(TransportErrorV1::ExpectedArray)?
        .iter()
        .map(|row| {
            row.as_str()
                .map(str::to_owned)
                .ok_or(TransportErrorV1::ExpectedString)
        })
        .collect()
}

fn byte_array(value: &Value) -> Result<Vec<u8>, TransportErrorV1> {
    value
        .as_array()
        .ok_or(TransportErrorV1::ExpectedArray)?
        .iter()
        .map(|row| {
            row.as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or(TransportErrorV1::InvalidByte)
        })
        .collect()
}

fn digest_field(object: &Map<String, Value>, name: &str) -> Result<[u8; 32], TransportErrorV1> {
    parse_digest(field(object, name)?)
}

fn parse_digest(value: &Value) -> Result<[u8; 32], TransportErrorV1> {
    let text = value
        .as_str()
        .and_then(|value| value.strip_prefix("sha256:"))
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or(TransportErrorV1::InvalidDigest)?;
    let mut bytes = [0; 32];
    for (index, chunk) in text.as_bytes().chunks_exact(2).enumerate() {
        let encoded = std::str::from_utf8(chunk).map_err(|_| TransportErrorV1::InvalidDigest)?;
        bytes[index] =
            u8::from_str_radix(encoded, 16).map_err(|_| TransportErrorV1::InvalidDigest)?;
    }
    if bytes == [0; 32] {
        return Err(TransportErrorV1::InvalidDigest);
    }
    Ok(bytes)
}

fn digest_value(value: &[u8; 32]) -> String {
    format!("sha256:{}", lower_hex(value))
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}")
            .expect("invariant: writing hexadecimal into a String cannot fail");
    }
    encoded
}

fn canonical_json(value: &Value) -> Result<String, TransportErrorV1> {
    Ok(format!("{}\n", serde_json::to_string(value)?))
}

#[derive(Debug, Error)]
pub(crate) enum TransportErrorV1 {
    #[error("JSON document is malformed")]
    Json(#[from] serde_json::Error),
    #[error("JSON object has an unknown or missing field")]
    UnknownOrMissingField,
    #[error("JSON value must be an object")]
    ExpectedObject,
    #[error("JSON value must be an array")]
    ExpectedArray,
    #[error("JSON value must be a string")]
    ExpectedString,
    #[error("JSON value must be an unsigned integer")]
    ExpectedUnsigned,
    #[error("JSON byte value is outside 0..=255")]
    InvalidByte,
    #[error("JSON digest must be a non-zero lowercase sha256 reference")]
    InvalidDigest,
    #[error("JSON union uses an unknown variant")]
    UnknownVariant,
    #[error("JSON union has the wrong payload cardinality")]
    InvalidUnionShape,
    #[error("decoded JSON does not satisfy the frozen domain contract")]
    InvalidDomainValue,
    #[error("canonical JSON document length exceeds the supported transport")]
    DocumentTooLarge,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_request() -> OperationRequestV1 {
        let catalog =
            crate::domain::capability::generated_catalog::GeneratedCapabilityCatalogV1::load_frozen()
                .expect("catalog");
        let entry = &catalog.actions()[0];
        let OperationSpecRefV1::Action(action_spec) = entry.operation_spec_ref() else {
            panic!("action catalog entry");
        };
        OperationRequestV1::Action(ActionRequestV1 {
            schema_version: 1,
            request_id: "request-action-1".to_owned(),
            idempotency_key: "idempotency-action-1".to_owned(),
            semantic_request_hash: [1; 32],
            selected_packet_semantic_hash: [2; 32],
            action_spec,
            material_dependency_stamp: entry.material_dependency_stamp(),
            exact_store_generation_ref: "candidate:store:generation:current".to_owned(),
            exact_authority_epoch_ref: "candidate:authority:epoch:current".to_owned(),
            valid_until_ref: "candidate:frontier:valid-until".to_owned(),
            authority_basis: ActionAuthorityBasisV1::Ordinary {
                verified_principal_ref: "candidate:principal:verified".to_owned(),
                current_session_ref: "candidate:session:current".to_owned(),
                live_grant_refs: Vec::new(),
                required_mandate_refs: Vec::new(),
            },
            typed_input_cbor: vec![0xa0],
            evidence_refs: Vec::new(),
            prerequisite_receipt_refs: Vec::new(),
            orchestration_attribution: None,
        })
    }

    fn packet_request() -> McpPacketReadRequestV1 {
        McpPacketReadRequestV1 {
            schema_version: 1,
            request_id: "request-1".to_owned(),
            repository_locator: "/repo".to_owned(),
            authenticated_host_connection_context_ref: "candidate:host-connection:authenticated"
                .to_owned(),
            projection_scope: ProjectionScopeV1::Repository,
            expected_release_ref: "candidate:release:current".to_owned(),
            expected_public_catalog_ref: "candidate:catalog:public".to_owned(),
            bounded_response_redaction_profile: "bounded-default".to_owned(),
            read_mode: McpPacketReadModeV1::DiscoverSelectionContextV1,
        }
    }

    #[test]
    fn packet_request_round_trips_canonical_json() {
        let expected = packet_request();
        let encoded = encode_packet_read_request(&expected).expect("encode");
        assert_eq!(
            decode_packet_read_request(&encoded).expect("decode"),
            expected
        );
        assert!(encoded.ends_with('\n'));
    }

    #[test]
    fn packet_request_rejects_unknown_fields() {
        let encoded = encode_packet_read_request(&packet_request()).expect("encode");
        let mut value: Value = serde_json::from_str(&encoded).expect("json");
        value
            .as_object_mut()
            .expect("object")
            .insert("future_field".to_owned(), Value::Bool(true));
        assert!(matches!(
            decode_packet_read_request(&value.to_string()),
            Err(TransportErrorV1::UnknownOrMissingField)
        ));
    }

    #[test]
    fn action_request_round_trips_without_losing_authority_or_bytes() {
        let expected = action_request();
        let encoded = encode_operation_request(&expected).expect("encode");
        assert_eq!(
            decode_operation_request(&encoded).expect("decode"),
            expected
        );
    }

    #[test]
    fn operation_request_rejects_unknown_leaf_and_mode() {
        let encoded = encode_operation_request(&action_request()).expect("encode");
        let mut unknown_field: Value = serde_json::from_str(&encoded).expect("json");
        unknown_field["value"]
            .as_object_mut()
            .expect("action")
            .insert("future_leaf".to_owned(), Value::Null);
        assert!(matches!(
            decode_operation_request(&unknown_field.to_string()),
            Err(TransportErrorV1::UnknownOrMissingField)
        ));

        let mut unknown_mode: Value = serde_json::from_str(&encoded).expect("json");
        unknown_mode["variant"] = Value::String("FutureOperation".to_owned());
        assert!(matches!(
            decode_operation_request(&unknown_mode.to_string()),
            Err(TransportErrorV1::UnknownVariant)
        ));
    }

    #[test]
    fn duplicate_object_keys_are_refused_at_top_level_and_nested() {
        let encoded = encode_packet_read_request(&packet_request()).expect("encode");
        let top_level = encoded.replacen('{', "{\"request_id\":\"smuggled\",", 1);
        assert!(matches!(
            decode_packet_read_request(&top_level),
            Err(TransportErrorV1::Json(_))
        ));

        let encoded = encode_operation_request(&action_request()).expect("encode");
        let nested = encoded.replacen("\"value\":{", "\"value\":{\"request_id\":\"smuggled\",", 1);
        assert_ne!(nested, encoded);
        assert!(matches!(
            decode_operation_request(&nested),
            Err(TransportErrorV1::Json(_))
        ));
    }

    #[test]
    fn digest_parser_rejects_zero_and_noncanonical_hex() {
        assert!(parse_digest(&Value::String(format!("sha256:{}", "0".repeat(64)))).is_err());
        assert!(parse_digest(&Value::String(format!("sha256:{}", "A".repeat(64)))).is_err());
    }
}
