use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::INSTALL_LOCK_SCHEMA_VERSION;
use crate::install::InstallAgent;

/// `.maestro/install-lock.yaml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InstallLock {
    /// Install lock schema version.
    pub schema_version: String,
    /// Agent install ownership records keyed by agent name.
    pub agents: BTreeMap<String, AgentInstall>,
}

/// Ownership record for one installed agent.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentInstall {
    /// Install timestamp string.
    pub installed_at: String,
    /// Files owned by this agent install.
    pub files: BTreeMap<String, FileOwnership>,
}

/// Ownership information for one mirror file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileOwnership {
    /// Mirror ownership kind.
    pub kind: MirrorKind,
    /// Optional content hash for text-managed files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Optional managed JSON keys.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub managed_keys: Vec<String>,
    /// User-owned JSON values replaced during install, restored during uninstall.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub previous_values: BTreeMap<String, Value>,
    /// Optional symlink target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// Install lock file kinds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorKind {
    /// Markdown managed block.
    MarkdownManagedBlock,
    /// Gitignore managed section.
    GitignoreSection,
    /// TOML managed section.
    TomlSection,
    /// JSON managed top-level keys.
    JsonManagedKeys,
    /// Symlink mirror.
    Symlink,
}

impl InstallLock {
    /// Return an empty V1 install lock.
    pub fn empty() -> Self {
        Self {
            schema_version: INSTALL_LOCK_SCHEMA_VERSION.to_string(),
            agents: BTreeMap::new(),
        }
    }

    /// Load an install lock if it exists, otherwise return an empty one.
    pub fn load(path: &Path) -> Result<Self> {
        let Some(contents) = read_to_string_if_exists(path)? else {
            return Ok(Self::empty());
        };

        let lock: Self = serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse install lock {}", path.display()))?;
        if lock.schema_version != INSTALL_LOCK_SCHEMA_VERSION {
            return Err(MaestroError::SchemaMismatch {
                artifact: path.display().to_string(),
                expected: INSTALL_LOCK_SCHEMA_VERSION,
                found: lock.schema_version,
            }
            .into());
        }

        Ok(lock)
    }

    /// Save the install lock.
    pub fn save(&self, path: &Path) -> Result<()> {
        let contents = serde_yaml::to_string(self)?;
        write_string_atomic(path, &contents)
            .with_context(|| format!("failed to write install lock {}", path.display()))
    }

    /// Replace ownership for one agent.
    pub fn set_agent(&mut self, agent: InstallAgent, install: AgentInstall) {
        self.agents.insert(agent.key().to_string(), install);
    }

    /// Remove ownership for one agent.
    pub fn remove_agent(&mut self, agent: InstallAgent) {
        self.agents.remove(agent.key());
    }
}

impl AgentInstall {
    /// Create an empty install record.
    pub fn new(installed_at: String) -> Self {
        Self {
            installed_at,
            files: BTreeMap::new(),
        }
    }

    /// Record one file ownership entry.
    pub fn insert(&mut self, path: impl Into<String>, ownership: FileOwnership) {
        self.files.insert(path.into(), ownership);
    }
}

impl FileOwnership {
    /// Text managed block ownership.
    pub fn text(kind: MirrorKind, content: &str) -> Self {
        Self {
            kind,
            content_hash: Some(content_hash(content)),
            managed_keys: Vec::new(),
            previous_values: BTreeMap::new(),
            target: None,
        }
    }

    /// JSON managed keys ownership.
    pub fn json_keys(keys: Vec<String>, previous_values: BTreeMap<String, Value>) -> Self {
        Self {
            kind: MirrorKind::JsonManagedKeys,
            content_hash: None,
            managed_keys: keys,
            previous_values,
            target: None,
        }
    }

    /// Symlink ownership.
    pub fn symlink(target: impl Into<String>) -> Self {
        Self {
            kind: MirrorKind::Symlink,
            content_hash: None,
            managed_keys: Vec::new(),
            previous_values: BTreeMap::new(),
            target: Some(target.into()),
        }
    }
}

fn content_hash(content: &str) -> String {
    format!(
        "len:{}:sum:{:016x}",
        content.len(),
        byte_sum(content.as_bytes())
    )
}

fn byte_sum(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0_u64, |sum, byte| sum.wrapping_add(u64::from(*byte)))
}

/// Remove an empty lockfile parent only when the lockfile itself exists.
pub fn remove_lock_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove install lock {}", path.display()))
        }
    }
}
