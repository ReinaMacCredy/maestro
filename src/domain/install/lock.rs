use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::InstallAgent;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::read_to_string_if_exists;
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{Compat, INSTALL_LOCK_SCHEMA_VERSION, classify};

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
    /// Transaction state for this agent install.
    #[serde(default)]
    pub state: InstallState,
    /// Files owned by this agent install.
    pub files: BTreeMap<String, FileOwnership>,
}

/// Transaction state for one agent install.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallState {
    /// Lock was written before all managed files were committed.
    Pending,
    /// Uninstall is removing or has removed mirrors and must be retried to finalize the lock.
    Removing,
    /// Managed files were written and the lock can authorize uninstall.
    #[default]
    Committed,
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
    /// True when maestro created this file (it did not exist at install). Only a
    /// created-fresh file may be removed when uninstall empties it; a pre-existing
    /// file the user owned is preserved even when its residue is empty. Absent in
    /// older locks, which deserialize to `false` (the preserve-safe default).
    #[serde(default, skip_serializing_if = "is_false")]
    pub created_fresh: bool,
    /// Optional symlink target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !value
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
        if classify(&lock.schema_version, INSTALL_LOCK_SCHEMA_VERSION) != Compat::Exact {
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

    /// Relative paths another agent already recorded as created-fresh. A second
    /// agent installing into a shared mirror inherits this verdict so a later
    /// uninstall removes the emptied file instead of leaving a husk: by the time
    /// the second agent uninstalls, the first agent's lock entry (and its
    /// created-fresh record) is already gone.
    pub fn paths_created_fresh_by_other_agents(&self, agent: InstallAgent) -> BTreeSet<String> {
        self.agents
            .iter()
            .filter(|(key, _)| key.as_str() != agent.key())
            .flat_map(|(_, install)| {
                install
                    .files
                    .iter()
                    .filter(|(_, ownership)| ownership.created_fresh)
                    .map(|(path, _)| path.clone())
            })
            .collect()
    }
}

impl AgentInstall {
    /// Create an empty install record.
    pub fn new(installed_at: String) -> Self {
        Self {
            installed_at,
            state: InstallState::Committed,
            files: BTreeMap::new(),
        }
    }

    /// Record one file ownership entry.
    pub fn insert(&mut self, path: impl Into<String>, ownership: FileOwnership) {
        self.files.insert(path.into(), ownership);
    }

    /// Mark this install as pending before managed file writes.
    pub fn mark_pending(&mut self) {
        self.state = InstallState::Pending;
    }

    /// Mark this install as removing before managed file removal.
    pub fn mark_removing(&mut self) {
        self.state = InstallState::Removing;
    }

    /// Mark this install as committed after managed file writes complete.
    pub fn mark_committed(&mut self) {
        self.state = InstallState::Committed;
    }
}

impl FileOwnership {
    /// Text mirror ownership for a Maestro-owned managed block or section.
    /// `created_fresh` is true when the file did not exist before install.
    pub fn text(kind: MirrorKind, content: &str, created_fresh: bool) -> Self {
        Self {
            kind,
            content_hash: Some(content_hash(content)),
            managed_keys: Vec::new(),
            previous_values: BTreeMap::new(),
            created_fresh,
            target: None,
        }
    }

    /// JSON managed keys ownership. `created_fresh` is true when the file did not
    /// exist before install.
    pub fn json_keys(
        keys: Vec<String>,
        previous_values: BTreeMap<String, Value>,
        created_fresh: bool,
    ) -> Self {
        Self {
            kind: MirrorKind::JsonManagedKeys,
            content_hash: None,
            managed_keys: keys,
            previous_values,
            created_fresh,
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
            created_fresh: false,
            target: Some(target.into()),
        }
    }

    pub(crate) fn matches_text_content(&self, content: &str) -> bool {
        self.matches_strong_text_content(content) || self.matches_legacy_text_content(content)
    }

    pub(crate) fn matches_legacy_text_content(&self, content: &str) -> bool {
        let Some(content_hash) = self.content_hash.as_deref() else {
            return false;
        };

        content_hash == legacy_content_hash(content)
    }

    pub(crate) fn has_legacy_text_hash(&self) -> bool {
        self.content_hash
            .as_deref()
            .is_some_and(|content_hash| content_hash.starts_with("len:"))
    }

    fn matches_strong_text_content(&self, content: &str) -> bool {
        let Some(content_hash) = self.content_hash.as_deref() else {
            return false;
        };

        content_hash == strong_content_hash(content)
    }
}

fn content_hash(content: &str) -> String {
    strong_content_hash(content)
}

fn strong_content_hash(content: &str) -> String {
    sha256_prefixed(content.as_bytes())
}

fn legacy_content_hash(content: &str) -> String {
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

/// Remove the lockfile if it exists.
pub fn remove_lock_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove install lock {}", path.display()))
        }
    }
}
