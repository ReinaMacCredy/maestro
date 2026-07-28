use std::io::{self, Read};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};

use crate::domain::projection::{ProjectionReadPortV1, read_packet};
use crate::domain::transport::{decode_packet_read_request, encode_packet_read_envelope};
use crate::operations::adapters::LiveProjectionReadProviderV1;

const MAXIMUM_REQUEST_BYTES_V1: u64 = 262_144;

#[derive(Debug, Args)]
pub struct PacketArgs {
    #[command(subcommand)]
    pub command: PacketCommand,
}

#[derive(Debug, Subcommand)]
pub enum PacketCommand {
    /// Read one canonical, bounded Packet projection from repository-local state.
    Read,
}

pub fn run(args: PacketArgs) -> Result<()> {
    match args.command {
        PacketCommand::Read => {
            let input = read_bounded_stdin()?;
            let request = decode_packet_read_request(&input)
                .context("packet read requires one canonical JSON request document")?;
            let provider =
                LiveProjectionReadProviderV1::open_explicit_repository(&request.repository_locator)
                    .context("repository_locator must identify one no-follow repository root")?;
            println!("{}", project_json(&provider, &request)?);
            Ok(())
        }
    }
}

fn project_json(
    provider: &dyn ProjectionReadPortV1,
    request: &crate::domain::integration::public_literals::McpPacketReadRequestV1,
) -> Result<String> {
    let envelope = read_packet(provider, request).context("packet projection was rejected")?;
    encode_packet_read_envelope(&envelope).map_err(Into::into)
}

fn read_bounded_stdin() -> Result<String> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(MAXIMUM_REQUEST_BYTES_V1 + 1)
        .read_to_end(&mut bytes)
        .context("failed to read packet request from stdin")?;
    if bytes.len() as u64 > MAXIMUM_REQUEST_BYTES_V1 {
        bail!("packet request exceeds the 262144-byte input bound");
    }
    String::from_utf8(bytes).context("packet request must be UTF-8 JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::integration::public_literals::McpPacketReadRequestV1;
    use crate::domain::projection::{ProjectionErrorV1, ProjectionReadStateV1};

    struct RefusingPort;

    impl ProjectionReadPortV1 for RefusingPort {
        fn read_once(
            &self,
            _request: &McpPacketReadRequestV1,
        ) -> Result<ProjectionReadStateV1, ProjectionErrorV1> {
            Ok(ProjectionReadStateV1::Unavailable {
                reason_ref: "candidate:projection:test-unavailable:v1".to_owned(),
            })
        }
    }

    #[test]
    fn exact_json_transport_preserves_the_six_outcome_envelope() {
        let request = decode_packet_read_request(
            r#"{"authenticated_host_connection_context_ref":"candidate:host:test:v1","bounded_response_redaction_profile":"repository-local","expected_public_catalog_ref":"candidate:catalog:test:v1","expected_release_ref":"candidate:release:test:v1","projection_scope":{"variant":"Repository"},"read_mode":{"variant":"DiscoverSelectionContextV1"},"repository_locator":"/tmp/repository","request_id":"request-1","schema_version":1}"#,
        )
        .expect("request");
        assert_eq!(
            project_json(&RefusingPort, &request).expect("envelope"),
            "{\"value\":{\"reason_ref\":\"candidate:projection:test-unavailable:v1\"},\"variant\":\"Unavailable\"}\n"
        );
    }
}
