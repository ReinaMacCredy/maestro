use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::foundation::core::paths::MaestroPaths;

const CAPABILITY_REPORT_SCHEMA: &str = "maestro.capability.v1";
const DEFAULT_REGISTRY_FILE: &str = "capabilities.yml";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityReport {
    pub version: u32,
    pub schema: &'static str,
    pub registry: RegistryReadout,
    pub capabilities: Vec<CapabilityReadout>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RegistryReadout {
    pub path: String,
    pub present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityReadout {
    pub id: String,
    pub active: bool,
    pub status: CapabilityStatus,
    pub providers: Vec<ProviderReadout>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Inactive,
    Present,
    Missing,
    Denied,
    Unverified,
}

impl CapabilityStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Present => "present",
            Self::Missing => "missing",
            Self::Denied => "denied",
            Self::Unverified => "unverified",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderReadout {
    pub name: String,
    pub kind: String,
    pub status: ProviderStatus,
    pub evidence: ProviderEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
    Present,
    Missing,
    Denied,
    Unverified,
}

impl ProviderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::Denied => "denied",
            Self::Unverified => "unverified",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderEvidence {
    pub kind: String,
    pub reference: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CapabilityManifest {
    #[serde(default)]
    capabilities: Vec<CapabilityDeclaration>,
}

#[derive(Clone, Debug, Deserialize)]
struct CapabilityDeclaration {
    #[serde(default)]
    id: String,
    #[serde(default = "default_active")]
    active: bool,
    #[serde(default)]
    providers: Vec<ProviderDeclaration>,
}

#[derive(Clone, Debug, Deserialize)]
struct ProviderDeclaration {
    #[serde(default)]
    name: String,
    #[serde(default)]
    kind: String,
    command: Option<String>,
    path: Option<String>,
    receipt: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct HostReceipt {
    status: Option<String>,
    detail: Option<String>,
}

pub fn report(paths: &MaestroPaths, from: Option<&Path>) -> Result<CapabilityReport> {
    let registry_path = from
        .map(Path::to_path_buf)
        .unwrap_or_else(|| paths.maestro_dir().join(DEFAULT_REGISTRY_FILE));
    let registry = RegistryReadout {
        path: registry_path.display().to_string(),
        present: registry_path.exists(),
    };
    if !registry.present {
        return Ok(CapabilityReport {
            version: 1,
            schema: CAPABILITY_REPORT_SCHEMA,
            registry,
            capabilities: Vec::new(),
        });
    }

    let raw = fs::read_to_string(&registry_path)
        .with_context(|| format!("failed to read {}", registry_path.display()))?;
    let manifest: CapabilityManifest = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", registry_path.display()))?;
    let base_dir = registry_path.parent().unwrap_or(paths.repo_root());
    let capabilities = manifest
        .capabilities
        .iter()
        .map(|capability| evaluate_capability(paths, base_dir, capability))
        .collect();

    Ok(CapabilityReport {
        version: 1,
        schema: CAPABILITY_REPORT_SCHEMA,
        registry,
        capabilities,
    })
}

fn evaluate_capability(
    paths: &MaestroPaths,
    base_dir: &Path,
    capability: &CapabilityDeclaration,
) -> CapabilityReadout {
    let providers: Vec<ProviderReadout> = if capability.active {
        capability
            .providers
            .iter()
            .map(|provider| evaluate_provider(paths, base_dir, provider))
            .collect()
    } else {
        Vec::new()
    };
    let status = aggregate_status(capability.active, &providers);

    CapabilityReadout {
        id: capability.id.clone(),
        active: capability.active,
        status,
        providers,
    }
}

fn evaluate_provider(
    paths: &MaestroPaths,
    base_dir: &Path,
    provider: &ProviderDeclaration,
) -> ProviderReadout {
    let kind = provider.kind.trim().to_ascii_lowercase();
    let (status, evidence) = match kind.as_str() {
        "cli" => evaluate_cli_provider(provider),
        "file" => evaluate_file_provider(paths, provider),
        "host_receipt" => evaluate_receipt_provider(base_dir, provider),
        _ => (
            ProviderStatus::Unverified,
            ProviderEvidence {
                kind: "declaration".to_string(),
                reference: None,
                detail: format!("unknown provider kind {}", provider.kind),
            },
        ),
    };

    ProviderReadout {
        name: provider.name.clone(),
        kind,
        status,
        evidence,
    }
}

fn evaluate_cli_provider(provider: &ProviderDeclaration) -> (ProviderStatus, ProviderEvidence) {
    let Some(command) = provider
        .command
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return (
            ProviderStatus::Unverified,
            ProviderEvidence {
                kind: "declaration".to_string(),
                reference: None,
                detail: "cli provider missing command".to_string(),
            },
        );
    };
    match resolve_command(command) {
        Some(path) => (
            ProviderStatus::Present,
            ProviderEvidence {
                kind: "local_command".to_string(),
                reference: Some(path.display().to_string()),
                detail: "command found on local filesystem".to_string(),
            },
        ),
        None => (
            ProviderStatus::Missing,
            ProviderEvidence {
                kind: "local_command".to_string(),
                reference: Some(command.to_string()),
                detail: "command not found on PATH".to_string(),
            },
        ),
    }
}

fn evaluate_file_provider(
    paths: &MaestroPaths,
    provider: &ProviderDeclaration,
) -> (ProviderStatus, ProviderEvidence) {
    let Some(path) = provider
        .path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return (
            ProviderStatus::Unverified,
            ProviderEvidence {
                kind: "declaration".to_string(),
                reference: None,
                detail: "file provider missing path".to_string(),
            },
        );
    };
    let resolved = resolve_repo_path(paths, path);
    if resolved.exists() {
        (
            ProviderStatus::Present,
            ProviderEvidence {
                kind: "local_file".to_string(),
                reference: Some(resolved.display().to_string()),
                detail: "path exists".to_string(),
            },
        )
    } else {
        (
            ProviderStatus::Missing,
            ProviderEvidence {
                kind: "local_file".to_string(),
                reference: Some(resolved.display().to_string()),
                detail: "path does not exist".to_string(),
            },
        )
    }
}

fn evaluate_receipt_provider(
    base_dir: &Path,
    provider: &ProviderDeclaration,
) -> (ProviderStatus, ProviderEvidence) {
    let Some(receipt) = provider
        .receipt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return (
            ProviderStatus::Unverified,
            ProviderEvidence {
                kind: "declaration".to_string(),
                reference: None,
                detail: "host_receipt provider missing receipt".to_string(),
            },
        );
    };
    let path = base_dir.join(receipt);
    let reference = Some(path.display().to_string());
    let Ok(raw) = fs::read_to_string(&path) else {
        return (
            ProviderStatus::Missing,
            ProviderEvidence {
                kind: "host_receipt".to_string(),
                reference,
                detail: "receipt file missing or unreadable".to_string(),
            },
        );
    };
    let receipt: HostReceipt = match serde_yaml::from_str(&raw) {
        Ok(receipt) => receipt,
        Err(error) => {
            return (
                ProviderStatus::Unverified,
                ProviderEvidence {
                    kind: "host_receipt".to_string(),
                    reference,
                    detail: format!("receipt parse error: {error}"),
                },
            );
        }
    };
    let status = parse_provider_status(receipt.status.as_deref());
    (
        status,
        ProviderEvidence {
            kind: "host_receipt".to_string(),
            reference,
            detail: receipt
                .detail
                .unwrap_or_else(|| "host receipt supplied status".to_string()),
        },
    )
}

fn aggregate_status(active: bool, providers: &[ProviderReadout]) -> CapabilityStatus {
    if !active {
        return CapabilityStatus::Inactive;
    }
    if providers
        .iter()
        .any(|provider| provider.status == ProviderStatus::Present)
    {
        return CapabilityStatus::Present;
    }
    if providers
        .iter()
        .any(|provider| provider.status == ProviderStatus::Denied)
    {
        return CapabilityStatus::Denied;
    }
    if providers
        .iter()
        .any(|provider| provider.status == ProviderStatus::Unverified)
    {
        return CapabilityStatus::Unverified;
    }
    CapabilityStatus::Missing
}

fn parse_provider_status(status: Option<&str>) -> ProviderStatus {
    match status.map(|status| status.trim().to_ascii_lowercase().replace('-', "_")) {
        Some(status) if status == "present" => ProviderStatus::Present,
        Some(status) if status == "missing" => ProviderStatus::Missing,
        Some(status) if status == "denied" => ProviderStatus::Denied,
        Some(status) if status == "unverified" => ProviderStatus::Unverified,
        _ => ProviderStatus::Unverified,
    }
}

fn resolve_command(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.components().count() > 1 {
        return executable_file(path).then(|| path.to_path_buf());
    }
    env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .find_map(|dir| {
            let candidate = Path::new(dir).join(command);
            executable_file(&candidate).then_some(candidate)
        })
}

fn resolve_repo_path(paths: &MaestroPaths, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        paths.repo_root().join(path)
    }
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    executable_mode(&metadata)
}

#[cfg(unix)]
fn executable_mode(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable_mode(_metadata: &fs::Metadata) -> bool {
    true
}

fn default_active() -> bool {
    true
}
