//! Stage-6 CLI and JSON transport adapter.

use serde_json::json;

use crate::domain::capability::generated_catalog::{
    GeneratedCapabilityCatalogV1, OperationCatalogKindV1,
};
use crate::domain::integration::public_literals::{OperationResultV1, OperationSemanticOutcomeV1};
use crate::domain::projection::{LegacySuccessorSurfaceV1, ProjectionReadPortV1, read_packet};
use crate::domain::transport::{
    decode_operation_request, decode_packet_read_request, encode_operation_result,
    encode_packet_read_envelope,
};
use crate::operations::CutoverGovernedOperationAssemblyV1;
use crate::operations::action::{
    ActionSubmissionServiceV1, GovernedOperationPortV1, OperationKindV1, OperationResultReadPortV1,
    PreparedOperationV1,
};
use crate::operations::adapters::legacy_successor_refusal;

pub(super) fn refuse_legacy_successor_route(
    surface: LegacySuccessorSurfaceV1<'_>,
) -> anyhow::Result<()> {
    let refusal = legacy_successor_refusal(surface)
        .ok_or_else(|| anyhow::anyhow!("the requested route is not a frozen legacy surface"))?;
    anyhow::bail!("{}; use {}", refusal.code, refusal.canonical_replacement)
}

pub(super) fn refuse_legacy_recipe_route(recipe: &str) -> anyhow::Result<()> {
    let surface = LegacySuccessorSurfaceV1::Recipe(recipe);
    if legacy_successor_refusal(surface).is_some() {
        refuse_legacy_successor_route(surface)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Stage6CliOutputV1 {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: u8,
}

impl Stage6CliOutputV1 {
    fn success(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            exit_code: 0,
        }
    }

    fn failure(exit_code: u8, message: impl Into<String>) -> Self {
        Self {
            stdout: String::new(),
            stderr: format!("{}\n", message.into()),
            exit_code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Stage6CliCommandV1 {
    CapabilityCatalog {
        json: bool,
    },
    OperationPrepare {
        kind: OperationKindV1,
        name: String,
        json: bool,
    },
    OperationSubmit {
        json: bool,
    },
    OperationResult {
        request_id: String,
        json: bool,
    },
    PacketRead {
        json: bool,
    },
}

#[expect(
    dead_code,
    reason = "the canonical Stage-6 adapter entry remains crate-internal until the frozen transport replaces legacy CLI parsing"
)]
pub(crate) fn run(
    args: &[String],
    input_json: Option<&str>,
    projection_port: &dyn ProjectionReadPortV1,
    operation_assembly: CutoverGovernedOperationAssemblyV1<'_>,
    result_port: &dyn OperationResultReadPortV1,
) -> Stage6CliOutputV1 {
    let operation_port = match operation_assembly.into_port() {
        Ok(port) => port,
        Err(error) => return Stage6CliOutputV1::failure(1, error.to_string()),
    };
    run_with_operation_port(
        args,
        input_json,
        projection_port,
        &operation_port,
        result_port,
    )
}

fn run_with_operation_port(
    args: &[String],
    input_json: Option<&str>,
    projection_port: &dyn ProjectionReadPortV1,
    operation_port: &dyn GovernedOperationPortV1,
    result_port: &dyn OperationResultReadPortV1,
) -> Stage6CliOutputV1 {
    let command = match parse(args) {
        Ok(command) => command,
        Err(message) => return Stage6CliOutputV1::failure(2, message),
    };
    let service = match ActionSubmissionServiceV1::load() {
        Ok(service) => service,
        Err(error) => return Stage6CliOutputV1::failure(1, error.to_string()),
    };
    match command {
        Stage6CliCommandV1::CapabilityCatalog { json } => capability_catalog(json),
        Stage6CliCommandV1::OperationPrepare { kind, name, json } => {
            match service.prepare_named(kind, &name) {
                Ok(prepared) => render_prepared(prepared, json),
                Err(error) => Stage6CliOutputV1::failure(2, error.to_string()),
            }
        }
        Stage6CliCommandV1::OperationSubmit { json } => {
            let Some(input) = input_json else {
                return Stage6CliOutputV1::failure(
                    2,
                    "operation submit requires one canonical JSON request document",
                );
            };
            let request = match decode_operation_request(input) {
                Ok(request) => request,
                Err(error) => return Stage6CliOutputV1::failure(2, error.to_string()),
            };
            match service.submit(operation_port, &request) {
                Ok(result) => render_result(&result, json),
                Err(error) => Stage6CliOutputV1::failure(1, error.to_string()),
            }
        }
        Stage6CliCommandV1::OperationResult { request_id, json } => {
            match service.read_result(result_port, &request_id) {
                Ok(Some(result)) => render_result(&result, json),
                Ok(None) => Stage6CliOutputV1::failure(
                    6,
                    format!("no durable Result is available for request {request_id}"),
                ),
                Err(error) => Stage6CliOutputV1::failure(1, error.to_string()),
            }
        }
        Stage6CliCommandV1::PacketRead { json } => {
            let Some(input) = input_json else {
                return Stage6CliOutputV1::failure(
                    2,
                    "packet read requires one canonical JSON request document",
                );
            };
            let request = match decode_packet_read_request(input) {
                Ok(request) => request,
                Err(error) => return Stage6CliOutputV1::failure(2, error.to_string()),
            };
            match read_packet(projection_port, &request) {
                Ok(envelope) if json => match encode_packet_read_envelope(&envelope) {
                    Ok(encoded) => Stage6CliOutputV1::success(encoded),
                    Err(error) => Stage6CliOutputV1::failure(1, error.to_string()),
                },
                Ok(envelope) => Stage6CliOutputV1::success(format!("{envelope:?}\n")),
                Err(error) => Stage6CliOutputV1::failure(1, error.to_string()),
            }
        }
    }
}

fn parse(args: &[String]) -> Result<Stage6CliCommandV1, &'static str> {
    let (args, json) = strip_json(args)?;
    match args {
        [root, command] if root == "capability" && command == "catalog" => {
            Ok(Stage6CliCommandV1::CapabilityCatalog { json })
        }
        [root, command, kind, name] if root == "operation" && command == "prepare" => {
            let kind = match kind.as_str() {
                "action" => OperationKindV1::Action,
                "ceremony" => OperationKindV1::Ceremony,
                _ => return Err("operation prepare kind must be action or ceremony"),
            };
            Ok(Stage6CliCommandV1::OperationPrepare {
                kind,
                name: name.clone(),
                json,
            })
        }
        [root, command] if root == "operation" && command == "submit" => {
            Ok(Stage6CliCommandV1::OperationSubmit { json })
        }
        [root, command, request_id] if root == "operation" && command == "result" => {
            Ok(Stage6CliCommandV1::OperationResult {
                request_id: request_id.clone(),
                json,
            })
        }
        [root, command] if root == "packet" && command == "read" => {
            Ok(Stage6CliCommandV1::PacketRead { json })
        }
        _ => Err("expected capability catalog, operation prepare|submit|result, or packet read"),
    }
}

fn strip_json(args: &[String]) -> Result<(&[String], bool), &'static str> {
    let json_positions = args
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (value == "--json").then_some(index))
        .collect::<Vec<_>>();
    match json_positions.as_slice() {
        [] => Ok((args, false)),
        [index] if *index + 1 == args.len() => Ok((&args[..*index], true)),
        _ => Err("--json may appear once, as the final argument"),
    }
}

fn capability_catalog(json_output: bool) -> Stage6CliOutputV1 {
    let catalog = match GeneratedCapabilityCatalogV1::load_frozen() {
        Ok(catalog) => catalog,
        Err(error) => return Stage6CliOutputV1::failure(1, error.to_string()),
    };
    if json_output {
        let rows = catalog
            .actions()
            .iter()
            .chain(catalog.ceremonies())
            .map(|entry| {
                json!({
                    "ordinal": entry.ordinal(),
                    "name": entry.name(),
                    "kind": match entry.kind() {
                        OperationCatalogKindV1::Action => "action",
                        OperationCatalogKindV1::Ceremony => "ceremony",
                    },
                    "owner": entry.owner().name(),
                    "owner_profile_ref": entry.owner_profile_ref(),
                    "descriptor_ref": entry.descriptor_ref(),
                    "descriptor": entry.descriptor(),
                })
            })
            .collect::<Vec<_>>();
        let value = json!({
            "schema": "maestro.vnext.capability-catalog.v1",
            "grammar_ref": catalog.grammar_ref(),
            "action_count": catalog.actions().len(),
            "ceremony_count": catalog.ceremonies().len(),
            "operation_count": catalog.operation_count(),
            "owner_relation_row_count": catalog.owner_relation_row_count(),
            "operations": rows,
        });
        match serde_json::to_string(&value) {
            Ok(value) => Stage6CliOutputV1::success(format!("{value}\n")),
            Err(error) => Stage6CliOutputV1::failure(1, error.to_string()),
        }
    } else {
        Stage6CliOutputV1::success(format!(
            "145 Actions, 11 Ceremonies, 156 governed Operations, 444 owner-relation rows\ncatalog {}\n",
            catalog.grammar_ref()
        ))
    }
}

fn render_prepared(prepared: PreparedOperationV1, json_output: bool) -> Stage6CliOutputV1 {
    let spec_ref = match &prepared.operation_spec {
        crate::domain::integration::public_literals::OperationSpecRefV1::Action(spec) => {
            &spec.exact_action_spec_ref
        }
        crate::domain::integration::public_literals::OperationSpecRefV1::Ceremony(spec) => {
            &spec.exact_ceremony_spec_ref
        }
    };
    if json_output {
        let value = json!({
            "schema": "maestro.vnext.operation-preparation.v1",
            "ordinal": prepared.ordinal,
            "name": prepared.name,
            "owner": prepared.owner.name(),
            "operation_spec_ref": spec_ref,
            "material_dependency_stamp": hex_digest(&prepared.material_dependency_stamp),
            "ceremony_context": prepared.ceremony_context.map(|value| match value {
                crate::domain::capability::generated_catalog::CeremonyContextKindV1::NoStore => "NoStore",
                crate::domain::capability::generated_catalog::CeremonyContextKindV1::PreStore => "PreStore",
            }),
        });
        match serde_json::to_string(&value) {
            Ok(value) => Stage6CliOutputV1::success(format!("{value}\n")),
            Err(error) => Stage6CliOutputV1::failure(1, error.to_string()),
        }
    } else {
        Stage6CliOutputV1::success(format!(
            "{}\nowner {}\nspec {}\n",
            prepared.name,
            prepared.owner.name(),
            spec_ref
        ))
    }
}

fn render_result(result: &OperationResultV1, json_output: bool) -> Stage6CliOutputV1 {
    let body = match result {
        OperationResultV1::Action(result) => &result.0,
        OperationResultV1::Ceremony(result) => &result.0,
    };
    let exit_code = outcome_exit(body.outcome);
    if json_output {
        match encode_operation_result(result) {
            Ok(stdout) => Stage6CliOutputV1 {
                stdout,
                stderr: String::new(),
                exit_code,
            },
            Err(error) => Stage6CliOutputV1::failure(1, error.to_string()),
        }
    } else {
        Stage6CliOutputV1 {
            stdout: format!(
                "{} {}{}\n",
                body.request_id,
                outcome_name(body.outcome),
                if body.replayed_delivery {
                    " (replay)"
                } else {
                    ""
                }
            ),
            stderr: String::new(),
            exit_code,
        }
    }
}

fn outcome_exit(outcome: OperationSemanticOutcomeV1) -> u8 {
    match outcome {
        OperationSemanticOutcomeV1::Committed | OperationSemanticOutcomeV1::NoOp => 0,
        OperationSemanticOutcomeV1::Rejected => 3,
        OperationSemanticOutcomeV1::Stale => 4,
        OperationSemanticOutcomeV1::Conflict => 5,
        OperationSemanticOutcomeV1::Unavailable => 6,
        OperationSemanticOutcomeV1::InDoubt => 7,
    }
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

fn hex_digest(value: &[u8; 32]) -> String {
    let mut encoded = String::from("sha256:");
    for byte in value {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}")
            .expect("invariant: writing hexadecimal into a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::integration::public_literals::{McpPacketReadRequestV1, OperationRequestV1};
    use crate::domain::projection::{ProjectionErrorV1, ProjectionReadStateV1};
    use crate::operations::action::{
        ActionSubmissionErrorV1, OwnerAdmissionV1, OwnerSubmissionOutcomeV1,
    };

    struct RefusingProjectionPort;

    impl ProjectionReadPortV1 for RefusingProjectionPort {
        fn read_once(
            &self,
            _request: &McpPacketReadRequestV1,
        ) -> Result<ProjectionReadStateV1, ProjectionErrorV1> {
            Err(ProjectionErrorV1::InvalidRequest)
        }
    }

    struct UnavailableOperationPort;

    impl GovernedOperationPortV1 for UnavailableOperationPort {
        fn submit(
            &self,
            _request: &OperationRequestV1,
            _admission: &OwnerAdmissionV1,
        ) -> Result<OwnerSubmissionOutcomeV1, ActionSubmissionErrorV1> {
            Ok(OwnerSubmissionOutcomeV1::OwnerUnavailable { inspect_ref: None })
        }
    }

    struct EmptyResultReadPort;

    impl OperationResultReadPortV1 for EmptyResultReadPort {
        fn read_result(
            &self,
            _request_id: &str,
        ) -> Result<
            Option<(
                OperationRequestV1,
                crate::domain::integration::public_literals::OperationResultV1,
            )>,
            ActionSubmissionErrorV1,
        > {
            Ok(None)
        }
    }

    #[test]
    fn operation_result_route_executes_through_the_read_port() {
        let output = run_with_operation_port(
            &["operation".into(), "result".into(), "request-1".into()],
            None,
            &RefusingProjectionPort,
            &UnavailableOperationPort,
            &EmptyResultReadPort,
        );
        assert_eq!(output.exit_code, 6);
        assert!(output.stderr.contains("no durable Result is available"));
    }

    #[test]
    fn parser_exposes_exact_stage6_roots() {
        assert_eq!(
            parse(&["operation".into(), "submit".into(), "--json".into()]),
            Ok(Stage6CliCommandV1::OperationSubmit { json: true })
        );
        assert!(parse(&["operation".into(), "ceremony".into()]).is_err());
        assert!(parse(&["operation".into(), "submit".into(), "extra".into()]).is_err());
    }

    #[test]
    fn all_seven_outcomes_have_stable_exit_codes() {
        assert_eq!(outcome_exit(OperationSemanticOutcomeV1::Committed), 0);
        assert_eq!(outcome_exit(OperationSemanticOutcomeV1::NoOp), 0);
        assert_eq!(outcome_exit(OperationSemanticOutcomeV1::Rejected), 3);
        assert_eq!(outcome_exit(OperationSemanticOutcomeV1::Stale), 4);
        assert_eq!(outcome_exit(OperationSemanticOutcomeV1::Conflict), 5);
        assert_eq!(outcome_exit(OperationSemanticOutcomeV1::Unavailable), 6);
        assert_eq!(outcome_exit(OperationSemanticOutcomeV1::InDoubt), 7);
    }

    #[test]
    fn catalog_output_closes_145_plus_11() {
        let output = capability_catalog(true);
        assert_eq!(output.exit_code, 0);
        let value: serde_json::Value = serde_json::from_str(&output.stdout).expect("catalog json");
        assert_eq!(value["action_count"], 145);
        assert_eq!(value["ceremony_count"], 11);
        assert_eq!(value["operation_count"], 156);
        assert_eq!(value["owner_relation_row_count"], 444);
    }
}
