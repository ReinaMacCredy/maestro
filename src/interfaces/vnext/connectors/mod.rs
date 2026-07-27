#![allow(
    dead_code,
    reason = "Stage 10 descriptors are dormant until cross-stage integration"
)]

//! Stage-10 host and external-connector declarations.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostDescriptorV1 {
    pub profile_id: &'static str,
    pub installation_scope: &'static str,
    pub project_registration: bool,
    pub descriptor_json: &'static str,
}

pub const HOST_DESCRIPTORS_V1: [HostDescriptorV1; 2] = [
    HostDescriptorV1 {
        profile_id: "agents-compatible-cli",
        installation_scope: "global-user-agent-installation",
        project_registration: false,
        descriptor_json: include_str!(
            "../../../../embedded/vnext/hosts/agents-compatible-cli.v1.json"
        ),
    },
    HostDescriptorV1 {
        profile_id: "claude-code",
        installation_scope: "global-user-agent-installation",
        project_registration: false,
        descriptor_json: include_str!("../../../../embedded/vnext/hosts/claude-code.v1.json"),
    },
];

pub fn host_descriptor(profile_id: &str) -> Option<&'static HostDescriptorV1> {
    HOST_DESCRIPTORS_V1
        .iter()
        .find(|descriptor| descriptor.profile_id == profile_id)
}

pub const BOOTSTRAP_WIRING_JSON: &str =
    include_str!("../../../../embedded/vnext/bootstrap/wiring.v1.json");
