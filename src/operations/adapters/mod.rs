#![allow(
    dead_code,
    reason = "canonical adapter entrypoints remain crate-internal"
)]

//! Stage-10 public-adapter preparation.
//!
//! Stage 5 owns host attestation and Authority validation. Stage 10 only
//! presents their released result. The currentness input is the sealed Stage 5
//! port implemented by Stage 9's production provider; Stage 10 does not mint a
//! substitute provider.

mod live_projection;

#[allow(
    unused_imports,
    reason = "MainIntegration consumes the narrow Stage 12 adapter facade after shared wiring"
)]
pub(crate) use live_projection::{
    LiveProjectionReadProviderV1, RunningBinaryIdentityV1, cli_search, decode_cli_search_request,
    encode_cli_search_envelope,
};

use crate::domain::integration::public_literals::{
    McpPacketReadEnvelopeV1, McpPacketReadRequestV1,
};
use crate::domain::{
    authority::{AuthorityFacadeV1, ContinuityReferenceV1},
    integration::TrustedHostDiagnosticConnectionPortV1,
    persistence::ProtectedDiagnosticCurrentViewProviderV1,
    projection::{
        LegacySuccessorRefusalV1, LegacySuccessorSurfaceV1, ProjectionReadPortV1, read_packet,
        refuse_legacy_successor_surface,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalMcpAdapterKindV1 {
    Packet,
    CliSearch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlobalMcpAdapterDefinitionV1 {
    pub kind: GlobalMcpAdapterKindV1,
    pub name: &'static str,
    pub description: &'static str,
    pub request_schema: &'static str,
    pub response_schema: &'static str,
    pub read_only: bool,
    pub writes: bool,
    pub network_io: bool,
}

pub const GLOBAL_MCP_TOOLS_V1: [GlobalMcpAdapterDefinitionV1; 2] = [
    GlobalMcpAdapterDefinitionV1 {
        kind: GlobalMcpAdapterKindV1::Packet,
        name: "maestro_packet",
        description: "Read one canonical bounded Packet projection from an explicit repository locator.",
        request_schema: "McpPacketReadRequestV1",
        response_schema: "McpPacketReadEnvelopeV1",
        read_only: true,
        writes: false,
        network_io: false,
    },
    GlobalMcpAdapterDefinitionV1 {
        kind: GlobalMcpAdapterKindV1::CliSearch,
        name: "maestro_cli_search",
        description: "Search the running binary's frozen public operation catalog without repository state.",
        request_schema: "McpCliSearchRequestV1",
        response_schema: "McpCliSearchEnvelopeV1",
        read_only: true,
        writes: false,
        network_io: false,
    },
];

pub(crate) fn global_mcp_adapter(name: &str) -> Option<&'static GlobalMcpAdapterDefinitionV1> {
    GLOBAL_MCP_TOOLS_V1
        .iter()
        .find(|definition| definition.name == name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage10AdapterError {
    TrustedHostAuthorityRejected,
    InvalidFrame,
    PacketReadRejected,
    CapabilityCatalogUnavailable,
    SearchCurrentnessRejected,
    SearchEnvelopeRejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFrameV1 {
    pub release_ref: String,
    pub public_catalog_ref: String,
    pub semantic_ref: String,
    pub outcome: String,
    pub summary: String,
}

impl AdapterFrameV1 {
    pub fn validate(&self) -> Result<(), Stage10AdapterError> {
        if self.release_ref.is_empty()
            || self.public_catalog_ref.is_empty()
            || self.semantic_ref.is_empty()
            || self.outcome.is_empty()
            || self.summary.is_empty()
        {
            return Err(Stage10AdapterError::InvalidFrame);
        }
        Ok(())
    }
}

pub fn read_protected_continuity_diagnostic(
    authority: &mut AuthorityFacadeV1,
    connection: &mut dyn TrustedHostDiagnosticConnectionPortV1,
    current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1,
    requested_subject: ContinuityReferenceV1,
) -> Result<Box<[u8]>, Stage10AdapterError> {
    authority
        .protected_continuity_diagnostic_with_ports(
            connection,
            current_view_provider,
            requested_subject,
        )
        .map(|released| released.into_bytes())
        .map_err(|_| Stage10AdapterError::TrustedHostAuthorityRejected)
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProtectedPacketReadV1 {
    pub packet: McpPacketReadEnvelopeV1,
    pub protected_continuity_diagnostic: Box<[u8]>,
}

pub(crate) fn packet_read_with_protected_continuity(
    authority: &mut AuthorityFacadeV1,
    connection: &mut dyn TrustedHostDiagnosticConnectionPortV1,
    current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1,
    requested_subject: ContinuityReferenceV1,
    projection: &dyn ProjectionReadPortV1,
    request: &McpPacketReadRequestV1,
) -> Result<ProtectedPacketReadV1, Stage10AdapterError> {
    let protected_continuity_diagnostic = read_protected_continuity_diagnostic(
        authority,
        connection,
        current_view_provider,
        requested_subject,
    )?;
    let packet = packet_read(projection, request)?;
    Ok(ProtectedPacketReadV1 {
        packet,
        protected_continuity_diagnostic,
    })
}

pub(crate) fn packet_read(
    projection: &dyn ProjectionReadPortV1,
    request: &McpPacketReadRequestV1,
) -> Result<McpPacketReadEnvelopeV1, Stage10AdapterError> {
    read_packet(projection, request).map_err(|_| Stage10AdapterError::PacketReadRejected)
}

pub(crate) fn legacy_successor_refusal(
    surface: LegacySuccessorSurfaceV1<'_>,
) -> Option<LegacySuccessorRefusalV1> {
    refuse_legacy_successor_surface(surface)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::{Component, Path, PathBuf};
    use std::process::Command;

    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    use super::*;

    const RECEIPT_SCHEMA: &str = "maestro.external.vnext-final-semantic-artifact-readback.v1";
    const CANONICAL_OBSERVATION_SCHEMA: &str =
        "maestro.external.vnext-final-canonical-read-observation.v1";
    const NEGATIVE_OBSERVATION_SCHEMA: &str =
        "maestro.external.vnext-final-negative-route-observation.v1";

    #[derive(Debug)]
    struct PromotionReadback {
        manifest_bytes: Vec<u8>,
        manifest_identity: String,
        source_file_count: usize,
        promoted_file_count: usize,
        mismatch_count: usize,
    }

    #[derive(Debug)]
    struct ClosureScan {
        consumers: Vec<String>,
        readers: Vec<String>,
        holds: Vec<String>,
    }

    #[derive(Debug)]
    struct CanonicalRead {
        route: String,
        command_identity: String,
    }

    #[derive(Debug)]
    struct NegativeRoute {
        route: String,
        receipt_identity: String,
    }

    #[test]
    fn legacy_surface_refusal_names_only_the_frozen_successor() {
        let refusal = legacy_successor_refusal(LegacySuccessorSurfaceV1::TaskNext).unwrap();
        assert_eq!(refusal.code, "unsupported_legacy_successor_surface");
        assert_eq!(refusal.canonical_replacement, "maestro packet read");
    }

    #[test]
    fn global_mcp_registry_is_closed_ordered_and_alias_free() {
        assert_eq!(
            GLOBAL_MCP_TOOLS_V1.map(|definition| definition.kind),
            [
                GlobalMcpAdapterKindV1::Packet,
                GlobalMcpAdapterKindV1::CliSearch
            ]
        );
        assert_eq!(
            GLOBAL_MCP_TOOLS_V1.map(|definition| definition.name),
            ["maestro_packet", "maestro_cli_search"]
        );
        for definition in GLOBAL_MCP_TOOLS_V1 {
            assert!(definition.read_only);
            assert!(!definition.writes);
            assert!(!definition.network_io);
            assert!(!definition.description.is_empty());
            assert!(!definition.request_schema.is_empty());
            assert!(!definition.response_schema.is_empty());
        }
        for alias in [
            "",
            "maestro_status",
            "maestro_ready",
            "maestro_query",
            "packet",
            "maestro-packet",
            "MAESTRO_PACKET",
            " maestro_packet",
            "maestro_packet ",
        ] {
            assert_eq!(global_mcp_adapter(alias), None, "{alias:?}");
        }
    }

    #[test]
    fn global_mcp_registry_has_complete_shipped_descriptor_parity() {
        let descriptor: serde_json::Value = serde_json::from_str(include_str!(
            "../../../embedded/vnext/adapter/mcp-tools.v1.json"
        ))
        .unwrap();
        let rows = descriptor["tools"].as_array().unwrap();
        assert_eq!(rows.len(), GLOBAL_MCP_TOOLS_V1.len());
        for (definition, row) in GLOBAL_MCP_TOOLS_V1.iter().zip(rows) {
            assert_eq!(row["name"], definition.name);
            assert_eq!(row["description"], definition.description);
            assert_eq!(row["request_schema"], definition.request_schema);
            assert_eq!(row["response_schema"], definition.response_schema);
            assert_eq!(row["read_only"], definition.read_only);
            assert_eq!(row["writes"], definition.writes);
            assert_eq!(row["network_io"], definition.network_io);
        }
    }

    #[test]
    fn post_promotion_canonical_readback_emits_positive_semantic_receipt() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        let promotion = read_committed_promotion(repository);
        let closure = scan_current_closure(repository);
        assert_closure_is_empty(&closure);
        let canonical_reads = exercise_canonical_reads(repository);
        let negative_routes = exercise_negative_routes(repository, &closure);
        assert!(negative_routes.len() >= 16);
        emit_receipt_if_requested(
            repository,
            "canonical-readback",
            &promotion,
            &closure,
            &canonical_reads,
            &negative_routes,
        );
    }

    #[test]
    fn post_promotion_legacy_and_obsolete_reader_removal_emits_positive_semantic_receipt() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        let promotion = read_committed_promotion(repository);
        let closure = scan_current_closure(repository);
        assert_closure_is_empty(&closure);
        for temporary_root in [
            "src/domain/vnext",
            "src/interfaces/vnext",
            "src/operations/vnext",
        ] {
            assert!(
                !repository.join(temporary_root).try_exists().unwrap(),
                "temporary namespace root remains: {temporary_root}"
            );
        }
        let canonical_reads = exercise_canonical_reads(repository);
        let negative_routes = exercise_negative_routes(repository, &closure);
        assert!(
            negative_routes.len() >= 16,
            "final negative fixture requires at least sixteen real refusals"
        );
        emit_receipt_if_requested(
            repository,
            "legacy-and-reader-removal",
            &promotion,
            &closure,
            &canonical_reads,
            &negative_routes,
        );
    }

    fn read_committed_promotion(repository: &Path) -> PromotionReadback {
        let ancestry_repository = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .and_then(|target| target.parent().map(Path::to_path_buf))
            .map(|root| root.join("control/ancestry-repository"))
            .filter(|candidate| candidate.try_exists().unwrap())
            .unwrap_or_else(|| repository.to_path_buf());
        let output = Command::new("python3")
            .arg(repository.join("tools/vnext_contracts/stage12/namespace_promotion.py"))
            .arg("--ancestry-repository")
            .arg(&ancestry_repository)
            .arg("--snapshot-root")
            .arg(repository)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .output()
            .expect("run canonical namespace-promotion readback");
        assert!(
            output.status.success(),
            "namespace-promotion readback failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let manifest: Value =
            serde_json::from_slice(&output.stdout).expect("parse namespace-promotion manifest");
        assert_eq!(
            manifest["schema_version"],
            "maestro.stage12.namespace-promotion-manifest.v1"
        );
        assert_eq!(
            manifest["state"],
            "canonical_namespace_promoted_legacy_pruning_blocked"
        );
        assert_eq!(manifest["closed_world"], true);
        assert_eq!(manifest["legacy_pruning_authorized"], false);
        assert_eq!(manifest["entry_count"], 210);
        assert_eq!(
            manifest["postconditions"]["canonical_destination_count"],
            210
        );
        assert_eq!(manifest["postconditions"]["temporary_namespace_count"], 0);
        assert_eq!(
            manifest["namespace_counts"],
            json!({
                "src/domain/vnext": 186,
                "src/interfaces/vnext": 8,
                "src/operations/vnext": 16
            })
        );

        let entries = manifest["entries"]
            .as_array()
            .expect("promotion manifest entries");
        assert_eq!(entries.len(), 210);
        let mut sources = HashSet::new();
        let mut destinations = HashSet::new();
        let mut promoted_file_count = 0;
        let mut mismatch_count = 0;
        for entry in entries {
            let source = entry["source"].as_str().expect("source path");
            let destination = entry["destination"].as_str().expect("destination path");
            assert_safe_relative_path(source);
            assert_safe_relative_path(destination);
            assert!(
                sources.insert(source.to_owned()),
                "duplicate source {source}"
            );
            assert!(
                destinations.insert(destination.to_owned()),
                "duplicate destination {destination}"
            );
            assert!(
                !repository.join(source).try_exists().unwrap(),
                "temporary promotion source remains: {source}"
            );
            let destination_bytes =
                fs::read(repository.join(destination)).expect("read canonical destination");
            if bare_sha256(&destination_bytes)
                == entry["destination_sha256"]
                    .as_str()
                    .expect("destination digest")
            {
                promoted_file_count += 1;
            } else {
                mismatch_count += 1;
            }
        }
        assert_eq!(sources.len(), 210);
        assert_eq!(destinations.len(), 210);
        assert_eq!(promoted_file_count, 210);
        assert_eq!(mismatch_count, 0);
        PromotionReadback {
            manifest_bytes: output.stdout,
            manifest_identity: format!(
                "sha256:{}",
                manifest["manifest_sha256"]
                    .as_str()
                    .expect("promotion manifest identity")
            ),
            source_file_count: sources.len(),
            promoted_file_count,
            mismatch_count,
        }
    }

    fn scan_current_closure(repository: &Path) -> ClosureScan {
        let rust_sources = collect_rust_sources(&repository.join("src"));
        let consumer_tokens = [
            format!("crate::domain::{}", "vnext"),
            format!("crate::interfaces::{}", "vnext"),
            format!("crate::operations::{}", "vnext"),
            format!("pub mod {};", "vnext"),
            format!("pub(crate) mod {};", "vnext"),
        ];
        let mut consumers = Vec::new();
        for path in &rust_sources {
            let source = fs::read_to_string(path).expect("read Rust source for closure scan");
            for token in &consumer_tokens {
                if source.contains(token) {
                    consumers.push(format!(
                        "{}:{token}",
                        path.strip_prefix(repository).unwrap().display()
                    ));
                }
            }
        }

        let mut readers = Vec::new();
        for alias in obsolete_adapter_aliases() {
            if global_mcp_adapter(alias).is_some() {
                readers.push(format!("registered-alias:{alias}"));
            }
        }
        let packet_source = fs::read_to_string(repository.join("src/interfaces/cli/packet.rs"))
            .expect("read Packet adapter");
        let packet = production_source(&packet_source);
        let projection_source =
            fs::read_to_string(repository.join("src/operations/adapters/live_projection.rs"))
                .expect("read projection adapter");
        let projection = production_source(&projection_source);
        for (surface, source, token) in [
            ("packet", packet, "discover_repo_root"),
            ("packet", packet, "current_dir()"),
            ("packet", packet, "canonicalize()"),
            ("projection", projection, "discover_repo_root"),
            ("projection", projection, "canonicalize()"),
            ("projection", projection, ".exists()"),
        ] {
            if source.contains(token) {
                readers.push(format!("{surface}:{token}"));
            }
        }

        let holds = [
            "src/domain/vnext",
            "src/interfaces/vnext",
            "src/operations/vnext",
        ]
        .into_iter()
        .filter(|relative| repository.join(relative).try_exists().unwrap())
        .map(str::to_owned)
        .collect();
        ClosureScan {
            consumers,
            readers,
            holds,
        }
    }

    fn assert_closure_is_empty(closure: &ClosureScan) {
        assert!(
            closure.consumers.is_empty(),
            "temporary namespace consumers remain: {:?}",
            closure.consumers
        );
        assert!(
            closure.readers.is_empty(),
            "obsolete readers remain: {:?}",
            closure.readers
        );
        assert!(
            closure.holds.is_empty(),
            "temporary namespace holds remain: {:?}",
            closure.holds
        );
    }

    fn exercise_canonical_reads(repository: &Path) -> Vec<CanonicalRead> {
        let packet = global_mcp_adapter("maestro_packet").expect("canonical Packet adapter");
        let search =
            global_mcp_adapter("maestro_cli_search").expect("canonical CLI-search adapter");
        assert_eq!(packet.kind, GlobalMcpAdapterKindV1::Packet);
        assert_eq!(search.kind, GlobalMcpAdapterKindV1::CliSearch);
        assert!(packet.read_only && !packet.writes && !packet.network_io);
        assert!(search.read_only && !search.writes && !search.network_io);
        assert_eq!(GLOBAL_MCP_TOOLS_V1.len(), 2);

        [
            (
                "canonical-adapter-maestro-packet",
                "src/operations/adapters/mod.rs",
            ),
            (
                "canonical-adapter-maestro-cli-search",
                "embedded/vnext/adapter/mcp-tools.v1.json",
            ),
            (
                "canonical-projection-facade",
                "src/domain/projection/mod.rs",
            ),
            (
                "canonical-installation-facade",
                "src/domain/installation/mod.rs",
            ),
        ]
        .into_iter()
        .map(|(route, relative)| {
            let bytes = fs::read(repository.join(relative)).expect("read canonical facade");
            assert!(!bytes.is_empty());
            CanonicalRead {
                route: route.to_owned(),
                command_identity: domain_identity(
                    b"maestro.final.canonical-read.v1",
                    &[route.as_bytes(), relative.as_bytes(), &bytes],
                ),
            }
        })
        .collect()
    }

    fn exercise_negative_routes(repository: &Path, closure: &ClosureScan) -> Vec<NegativeRoute> {
        let mut routes = Vec::new();
        for alias in obsolete_adapter_aliases() {
            assert_eq!(
                global_mcp_adapter(alias),
                None,
                "obsolete adapter alias admitted: {alias:?}"
            );
            routes.push(negative_route(
                &format!("mcp-alias-refusal-{}", routes.len()),
                alias.as_bytes(),
            ));
        }
        let refusal =
            legacy_successor_refusal(LegacySuccessorSurfaceV1::TaskNext).expect("legacy refusal");
        assert_eq!(refusal.code, "unsupported_legacy_successor_surface");
        assert_eq!(refusal.canonical_replacement, "maestro packet read");
        routes.push(negative_route(
            "legacy-task-next-refusal",
            format!("{refusal:?}").as_bytes(),
        ));

        for temporary_root in [
            "src/domain/vnext",
            "src/interfaces/vnext",
            "src/operations/vnext",
        ] {
            assert!(!repository.join(temporary_root).try_exists().unwrap());
            routes.push(negative_route(
                &format!(
                    "temporary-root-absence-{}",
                    temporary_root.replace('/', "-")
                ),
                temporary_root.as_bytes(),
            ));
        }
        for (route, count) in [
            ("temporary-consumer-closure-zero", closure.consumers.len()),
            ("obsolete-reader-closure-zero", closure.readers.len()),
            ("temporary-hold-closure-zero", closure.holds.len()),
        ] {
            assert_eq!(count, 0);
            routes.push(negative_route(route, count.to_string().as_bytes()));
        }
        routes
    }

    fn negative_route(route: &str, observed: &[u8]) -> NegativeRoute {
        NegativeRoute {
            route: route.to_owned(),
            receipt_identity: domain_identity(
                b"maestro.final.negative-route.v1",
                &[route.as_bytes(), observed],
            ),
        }
    }

    fn obsolete_adapter_aliases() -> &'static [&'static str] {
        &[
            "",
            "packet",
            "maestro-packet",
            "MAESTRO_PACKET",
            " maestro_packet",
            "maestro_packet ",
            "maestro_status",
            "maestro_ready",
            "maestro_query",
            "maestro_next",
            "maestro_recipe",
            "maestro_cli",
            "cli_search",
            "maestro-cli-search",
            "MAESTRO_CLI_SEARCH",
            "maestro_cli_search ",
        ]
    }

    fn emit_receipt_if_requested(
        repository: &Path,
        receipt_label: &str,
        promotion: &PromotionReadback,
        closure: &ClosureScan,
        canonical_reads: &[CanonicalRead],
        negative_routes: &[NegativeRoute],
    ) {
        let Some((check_id, receipt_path)) = selected_receipt_request() else {
            return;
        };
        assert!(
            !check_id.is_empty(),
            "final proof check id must be nonempty"
        );
        let output_root = receipt_path
            .parent()
            .expect("final proof receipt must have a parent");
        let output_root = output_root
            .canonicalize()
            .expect("final proof output directory must exist");
        let repository_root = repository
            .canonicalize()
            .expect("canonical repository root");
        assert!(
            !output_root.starts_with(&repository_root),
            "final proof output must not write into the source repository"
        );
        assert_eq!(
            receipt_path
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty()),
            receipt_path.file_name().and_then(|name| name.to_str())
        );

        let check_id_component = safe_check_id_component(&check_id);
        let manifest_name =
            format!("{receipt_label}-{check_id_component}-namespace-promotion-manifest.json");
        let manifest_path = output_root.join(&manifest_name);
        write_new(&manifest_path, &promotion.manifest_bytes);

        let mut artifacts = vec![
            compiled_behavior_artifact(&output_root, promotion, closure, canonical_reads),
            artifact(repository, "exported", "source", "src/domain/mod.rs"),
            artifact(
                repository,
                "schema",
                "source",
                "embedded/vnext/schemas/host-descriptor.v2.json",
            ),
            artifact(
                repository,
                "resource",
                "source",
                "embedded/vnext/adapter/mcp-tools.v1.json",
            ),
            artifact(
                repository,
                "persisted",
                "source",
                "src/domain/persistence/store.rs",
            ),
            artifact(
                repository,
                "consumer",
                "source",
                "src/domain/integration/consumer_closure.rs",
            ),
            artifact(
                repository,
                "reader",
                "source",
                "src/domain/projection/engine.rs",
            ),
            artifact(
                repository,
                "hold",
                "source",
                "src/domain/installation/consumer_snapshot.rs",
            ),
        ];
        artifacts.push(artifact_from_bytes(
            "persisted",
            "output",
            &manifest_name,
            &promotion.manifest_bytes,
        ));

        let canonical_rows = canonical_reads
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let observation_name =
                    format!("{receipt_label}-{check_id_component}-canonical-read-{index:02}.json");
                let observation_bytes = json_bytes(&json!({
                    "schema_version": CANONICAL_OBSERVATION_SCHEMA,
                    "check_id": check_id,
                    "route": row.route,
                    "command_identity": row.command_identity,
                    "status": "pass"
                }));
                write_new(&output_root.join(&observation_name), &observation_bytes);
                json!({
                    "route": row.route,
                    "status": "pass",
                    "command_identity": row.command_identity,
                    "observation": output_binding(&observation_name, &observation_bytes)
                })
            })
            .collect::<Vec<_>>();
        let negative_rows = negative_routes
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let observation_name =
                    format!("{receipt_label}-{check_id_component}-negative-route-{index:02}.json");
                let observation_bytes = json_bytes(&json!({
                    "schema_version": NEGATIVE_OBSERVATION_SCHEMA,
                    "check_id": check_id,
                    "route": row.route,
                    "injected": true,
                    "outcome": "refuse",
                    "receipt_identity": row.receipt_identity
                }));
                write_new(&output_root.join(&observation_name), &observation_bytes);
                json!({
                    "route": row.route,
                    "injected": true,
                    "outcome": "refuse",
                    "receipt_identity": row.receipt_identity,
                    "observation": output_binding(&observation_name, &observation_bytes)
                })
            })
            .collect::<Vec<_>>();

        let receipt = json!({
            "schema_version": RECEIPT_SCHEMA,
            "check_id": check_id,
            "artifacts": artifacts,
            "canonical_reads": canonical_rows,
            "negative_routes": negative_rows,
            "closures": {
                "consumer_count": closure.consumers.len(),
                "reader_count": closure.readers.len(),
                "hold_count": closure.holds.len()
            },
            "promotion_parity": {
                "source_file_count": promotion.source_file_count,
                "promoted_file_count": promotion.promoted_file_count,
                "mismatch_count": promotion.mismatch_count
            }
        });
        let receipt_bytes = json_bytes(&receipt);
        write_new(&receipt_path, &receipt_bytes);
    }

    fn selected_receipt_request() -> Option<(String, PathBuf)> {
        let proof_path = std::env::var_os("MAESTRO_FINAL_PROOF_RECEIPT");
        let proof_id = std::env::var_os("MAESTRO_FINAL_PROOF_ID");
        let replay_path = std::env::var_os("MAESTRO_SEMANTIC_READBACK_RECEIPT");
        let replay_id = std::env::var_os("MAESTRO_SEMANTIC_READBACK_CHECK_ID");
        let proof_present = proof_path.is_some() || proof_id.is_some();
        let replay_present = replay_path.is_some() || replay_id.is_some();
        assert!(
            !(proof_present && replay_present),
            "proof and semantic replay receipt environments are mutually exclusive"
        );
        match (proof_path, proof_id, replay_path, replay_id) {
            (Some(path), Some(id), None, None) | (None, None, Some(path), Some(id)) => Some((
                id.into_string().expect("UTF-8 final proof check id"),
                PathBuf::from(path),
            )),
            (None, None, None, None) => None,
            _ => panic!("final proof receipt path and check id must be supplied together"),
        }
    }

    fn artifact(repository: &Path, kind: &str, root: &str, relative: &str) -> Value {
        assert_safe_relative_path(relative);
        let bytes = fs::read(repository.join(relative)).expect("read artifact binding");
        artifact_from_bytes(kind, root, relative, &bytes)
    }

    fn compiled_behavior_artifact(
        output_root: &Path,
        promotion: &PromotionReadback,
        closure: &ClosureScan,
        canonical_reads: &[CanonicalRead],
    ) -> Value {
        assert_eq!(GLOBAL_MCP_TOOLS_V1.len(), 2);
        assert_eq!(closure.holds.len(), 0);
        assert_eq!(promotion.source_file_count, 210);
        assert_eq!(promotion.promoted_file_count, 210);
        assert_eq!(promotion.mismatch_count, 0);
        let catalog =
            crate::domain::capability::generated_catalog::GeneratedCapabilityCatalogV1::load_frozen()
                .expect("load compiled capability catalog");
        let registry = GLOBAL_MCP_TOOLS_V1
            .iter()
            .map(|definition| {
                json!({
                    "kind": match definition.kind {
                        GlobalMcpAdapterKindV1::Packet => "Packet",
                        GlobalMcpAdapterKindV1::CliSearch => "CliSearch",
                    },
                    "name": definition.name,
                    "read_only": definition.read_only,
                    "writes": definition.writes,
                    "network_io": definition.network_io
                })
            })
            .collect::<Vec<_>>();
        let read_outcomes = canonical_reads
            .iter()
            .map(|read| {
                json!({
                    "route": read.route,
                    "command_identity": read.command_identity,
                    "status": "pass"
                })
            })
            .collect::<Vec<_>>();
        let bytes = json_bytes(&json!({
            "schema_version": "maestro.external.vnext-final-compiled-behavior-observation.v1",
            "binary_version": env!("MAESTRO_VERSION"),
            "core_catalog_ref": catalog.grammar_ref(),
            "public_catalog_ref":
                crate::domain::capability::generated_catalog::PUBLIC_CATALOG_REF_V1,
            "mcp_registry": registry,
            "canonical_reads": read_outcomes,
            "temporary_namespace_count": closure.holds.len(),
            "promotion_manifest_identity": promotion.manifest_identity,
            "promotion_parity": {
                "source_file_count": promotion.source_file_count,
                "promoted_file_count": promotion.promoted_file_count,
                "mismatch_count": promotion.mismatch_count
            }
        }));
        let relative = "stage12-compiled-behavior-observation.json";
        let output_path = output_root.join(relative);
        match fs::read(&output_path) {
            Ok(existing) => assert!(
                existing == bytes,
                "shared Stage12 compiled behavior observation changed between filters"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_new(&output_path, &bytes);
            }
            Err(error) => panic!("read shared Stage12 compiled behavior observation: {error}"),
        }
        artifact_from_bytes("compiled", "output", relative, &bytes)
    }

    fn artifact_from_bytes(kind: &str, root: &str, relative: &str, bytes: &[u8]) -> Value {
        assert_safe_relative_path(relative);
        assert!(!bytes.is_empty(), "artifact must be nonempty: {relative}");
        json!({
            "kind": kind,
            "root": root,
            "path": relative,
            "byte_length": bytes.len(),
            "sha256": sha256_ref(bytes)
        })
    }

    fn output_binding(relative: &str, bytes: &[u8]) -> Value {
        assert_safe_relative_path(relative);
        assert!(!bytes.is_empty());
        json!({
            "path": relative,
            "byte_length": bytes.len(),
            "sha256": sha256_ref(bytes)
        })
    }

    fn write_new(path: &Path, bytes: &[u8]) {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .expect("create final proof output");
        output.write_all(bytes).expect("write final proof output");
        output.sync_all().expect("sync final proof output");
    }

    fn json_bytes(value: &Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(value).expect("serialize final proof JSON");
        bytes.push(b'\n');
        bytes
    }

    fn assert_safe_relative_path(relative: &str) {
        let path = Path::new(relative);
        assert!(!relative.is_empty() && !path.is_absolute());
        assert!(
            path.components()
                .all(|component| matches!(component, Component::Normal(_))),
            "unsafe artifact path: {relative}"
        );
    }

    fn safe_check_id_component(check_id: &str) -> String {
        let component = check_id
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();
        assert!(!component.is_empty());
        component
    }

    fn collect_rust_sources(root: &Path) -> Vec<PathBuf> {
        let mut pending = vec![root.to_path_buf()];
        let mut sources = Vec::new();
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).expect("scan Rust source directory") {
                let entry = entry.expect("read Rust source entry");
                let file_type = entry.file_type().expect("read source file type");
                assert!(!file_type.is_symlink(), "source scan refuses symlinks");
                if file_type.is_dir() {
                    pending.push(entry.path());
                } else if entry.path().extension().and_then(|value| value.to_str()) == Some("rs") {
                    sources.push(entry.path());
                }
            }
        }
        sources.sort();
        sources
    }

    fn production_source(source: &str) -> &str {
        source.split("#[cfg(test)]").next().unwrap_or(source)
    }

    fn domain_identity(domain: &[u8], parts: &[&[u8]]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        for part in parts {
            hasher.update([0]);
            hasher.update(part);
        }
        sha256_ref(&hasher.finalize())
    }

    fn bare_sha256(bytes: &[u8]) -> String {
        lower_hex(&Sha256::digest(bytes))
    }

    fn sha256_ref(bytes: &[u8]) -> String {
        format!("sha256:{}", bare_sha256(bytes))
    }

    fn lower_hex(bytes: &[u8]) -> String {
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("format digest");
        }
        output
    }
}
