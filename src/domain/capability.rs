use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use regex::Regex;
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
    pub grants_permission: bool,
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
    schema: Option<String>,
    status: Option<String>,
    detail: Option<String>,
    issued_by: Option<String>,
    issued_at: Option<String>,
}

pub fn report(paths: &MaestroPaths, from: Option<&Path>) -> Result<CapabilityReport> {
    let custom_registry = from.is_some();
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

    let metadata = fs::symlink_metadata(&registry_path)
        .with_context(|| format!("failed to inspect {}", registry_path.display()))?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "capability registry {} must not be a symlink",
            registry_path.display()
        );
    }
    if !metadata.is_file() {
        anyhow::bail!(
            "capability registry {} is not a regular file",
            registry_path.display()
        );
    }
    let raw = fs::read_to_string(&registry_path)
        .with_context(|| format!("failed to read {}", registry_path.display()))?;
    let manifest: CapabilityManifest = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", registry_path.display()))?;
    let registry_base_dir = registry_path.parent().unwrap_or(paths.repo_root());
    let file_base_dir = if custom_registry {
        registry_base_dir
    } else {
        paths.repo_root()
    };
    let capabilities = manifest
        .capabilities
        .iter()
        .map(|capability| evaluate_capability(paths, file_base_dir, registry_base_dir, capability))
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
    file_base_dir: &Path,
    registry_base_dir: &Path,
    capability: &CapabilityDeclaration,
) -> CapabilityReadout {
    let providers: Vec<ProviderReadout> = if capability.active {
        capability
            .providers
            .iter()
            .map(|provider| evaluate_provider(paths, file_base_dir, registry_base_dir, provider))
            .collect()
    } else {
        Vec::new()
    };
    let status = aggregate_status(capability.active, &providers);

    CapabilityReadout {
        id: capability.id.clone(),
        active: capability.active,
        grants_permission: false,
        status,
        providers,
    }
}

fn evaluate_provider(
    paths: &MaestroPaths,
    file_base_dir: &Path,
    registry_base_dir: &Path,
    provider: &ProviderDeclaration,
) -> ProviderReadout {
    let kind = provider.kind.trim().to_ascii_lowercase();
    let (status, evidence) = match kind.as_str() {
        "cli" => evaluate_cli_provider(provider),
        "file" => evaluate_file_provider(paths, file_base_dir, provider),
        "host_receipt" => evaluate_receipt_provider(paths, registry_base_dir, provider),
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
    base_dir: &Path,
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
    let resolved = match resolve_scoped_path(paths, base_dir, path) {
        ScopedPath::Allowed(path) => path,
        ScopedPath::Denied { path, detail } => {
            return (
                ProviderStatus::Denied,
                ProviderEvidence {
                    kind: "local_file".to_string(),
                    reference: Some(path.display().to_string()),
                    detail,
                },
            );
        }
    };
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
    paths: &MaestroPaths,
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
    let path = match resolve_scoped_path(paths, base_dir, receipt) {
        ScopedPath::Allowed(path) => path,
        ScopedPath::Denied { path, detail } => {
            return (
                ProviderStatus::Denied,
                ProviderEvidence {
                    kind: "host_receipt".to_string(),
                    reference: Some(path.display().to_string()),
                    detail,
                },
            );
        }
    };
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
    if receipt.schema.as_deref() != Some("maestro.capability-receipt.v1") {
        return (
            ProviderStatus::Unverified,
            ProviderEvidence {
                kind: "host_receipt".to_string(),
                reference,
                detail: "receipt schema missing or unsupported".to_string(),
            },
        );
    }
    if receipt
        .issued_by
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
        || receipt
            .issued_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return (
            ProviderStatus::Unverified,
            ProviderEvidence {
                kind: "host_receipt".to_string(),
                reference,
                detail: "receipt issuer metadata missing".to_string(),
            },
        );
    }
    let status = parse_provider_status(receipt.status.as_deref());
    (
        status,
        ProviderEvidence {
            kind: "host_receipt".to_string(),
            reference,
            detail: redact_sensitive_detail(
                &receipt
                    .detail
                    .unwrap_or_else(|| "host receipt supplied status".to_string()),
            ),
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

enum ScopedPath {
    Allowed(PathBuf),
    Denied { path: PathBuf, detail: String },
}

fn resolve_scoped_path(paths: &MaestroPaths, base_dir: &Path, path: &str) -> ScopedPath {
    let path = Path::new(path);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    };
    match fs::symlink_metadata(&resolved) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return ScopedPath::Denied {
                path: resolved,
                detail: "path is symlinked; capability declarations do not follow symlinks"
                    .to_string(),
            };
        }
        _ => {}
    }

    if is_within_repo(paths.repo_root(), &resolved) {
        ScopedPath::Allowed(resolved)
    } else {
        ScopedPath::Denied {
            path: resolved,
            detail:
                "path is outside repository scope; capability declarations do not grant permission"
                    .to_string(),
        }
    }
}

fn is_within_repo(repo_root: &Path, path: &Path) -> bool {
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let comparable = match path.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            let Some(parent) = path.parent() else {
                return false;
            };
            match parent.canonicalize() {
                Ok(parent) => path
                    .file_name()
                    .map(|name| parent.join(name))
                    .unwrap_or(parent),
                Err(_) => path.to_path_buf(),
            }
        }
    };
    comparable.starts_with(repo_root)
}

fn redact_sensitive_detail(detail: &str) -> String {
    static SECRET_ASSIGNMENT: OnceLock<Regex> = OnceLock::new();
    static BEARER_TOKEN: OnceLock<Regex> = OnceLock::new();
    static COMMON_TOKEN: OnceLock<Regex> = OnceLock::new();
    static PEM_BLOCK: OnceLock<Regex> = OnceLock::new();
    let secret_assignment = SECRET_ASSIGNMENT.get_or_init(|| {
        Regex::new(r"(?i)\b(api[_-]?key|apikey|token|secret|password)\s*[:=]\s*[^\s,;]+")
            .expect("invariant: capability redaction regex compiles")
    });
    let bearer_token = BEARER_TOKEN.get_or_init(|| {
        Regex::new(r"(?i)\b(bearer|authorization)\s+[^,\s;]+")
            .expect("invariant: capability bearer redaction regex compiles")
    });
    let common_token = COMMON_TOKEN.get_or_init(|| {
        Regex::new(r"\b(sk-[A-Za-z0-9_-]{8,}|gh[pousr]_[A-Za-z0-9_]{8,})\b")
            .expect("invariant: capability token redaction regex compiles")
    });
    let pem_block = PEM_BLOCK.get_or_init(|| {
        Regex::new(r"(?s)-----BEGIN [^-]+-----.*?-----END [^-]+-----")
            .expect("invariant: capability PEM redaction regex compiles")
    });
    let detail = secret_assignment
        .replace_all(detail, "$1=[redacted]")
        .into_owned();
    let detail = bearer_token
        .replace_all(&detail, "$1 [redacted]")
        .into_owned();
    let detail = common_token.replace_all(&detail, "[redacted]").into_owned();
    pem_block
        .replace_all(&detail, "[redacted-pem-block]")
        .into_owned()
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
