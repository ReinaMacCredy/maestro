use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::core::backup::{backup_file_with_timestamp, backup_operation_timestamp};
use crate::core::diff::unified_diff;
use crate::core::error::MaestroError;
use crate::core::fs::{ensure_parent_dir, read_to_string_if_exists};
use crate::core::managed_blocks::{
    remove_managed_block, upsert_managed_block, upsert_managed_json_keys, ManagedBlockFormat,
};
use crate::core::paths::MaestroPaths;
use crate::core::safe_write::write_string_atomic;
use crate::install::hooks::ManagedHookConfig;
use crate::install::lock::{AgentInstall, FileOwnership, MirrorKind};
use crate::install::InstallAgent;
use crate::skills::symlink::{
    create_skill_symlink, remove_skill_symlink_if_owned, skill_symlink_for_agent,
    validate_skill_symlink_destination, SkillSymlink,
};

const JSON_PREVIOUS_VALUE_HASHES: &str = "_maestro_previous_value_hashes";

/// Planned mirror write for an agent install.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirrorPlan {
    /// Repo-relative path.
    pub relative_path: String,
    /// Mirror file format.
    pub kind: MirrorKind,
    /// New content to write.
    pub contents: String,
    /// Managed JSON keys for JSON mirrors.
    pub managed_keys: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct PreparedMirrors {
    pub(crate) install: AgentInstall,
    updates: Vec<MirrorUpdate>,
    skill_symlink: SkillSymlink,
    backup_timestamp: String,
}

#[derive(Debug)]
struct MirrorUpdate {
    relative_path: String,
    path: std::path::PathBuf,
    existing: Option<String>,
    contents: String,
}

#[derive(Clone, Debug)]
struct MirrorRemoval {
    relative_path: String,
    path: PathBuf,
    contents: String,
    next: String,
}

/// Build all mirror writes for an agent.
pub fn mirror_plan(agent: InstallAgent) -> Result<Vec<MirrorPlan>> {
    let mut plans = vec![
        markdown("CLAUDE.md", claude_md_block()),
        markdown("AGENTS.md", agents_md_block()),
        hash(
            ".gitignore",
            gitignore_block(),
            MirrorKind::GitignoreSection,
        ),
    ];

    match agent {
        InstallAgent::Claude => plans.push(hook_config_plan(agent)?),
        InstallAgent::Codex => {
            plans.push(hook_config_plan(agent)?);
            plans.push(hash(
                ".codex/config.toml",
                codex_config_block(),
                MirrorKind::TomlSection,
            ));
        }
    }

    Ok(plans)
}

/// Apply mirrors and return install-lock ownership records.
pub fn apply_mirrors(
    paths: &MaestroPaths,
    agent: InstallAgent,
    installed_at: String,
) -> Result<AgentInstall> {
    let prepared = prepare_mirrors(paths, agent, installed_at, None)?;
    write_prepared_mirrors(paths, &prepared)?;

    Ok(prepared.install)
}

/// Prepare all mirror updates without mutating the repository.
pub(crate) fn prepare_mirrors(
    paths: &MaestroPaths,
    agent: InstallAgent,
    installed_at: String,
    previous_install: Option<&AgentInstall>,
) -> Result<PreparedMirrors> {
    let mut install = AgentInstall::new(installed_at);
    let backup_timestamp = backup_operation_timestamp()?;
    let mut updates = Vec::new();

    for plan in mirror_plan(agent)? {
        let path = managed_mirror_path(paths, &plan.relative_path)?;
        let existing = read_to_string_if_exists(&path)?;
        let previous_values = if plan.kind == MirrorKind::JsonManagedKeys {
            previous_json_values(previous_install, &plan, existing.as_deref())?
        } else {
            BTreeMap::new()
        };
        let contents = contents_for_existing(agent, &plan, existing.as_deref(), &previous_values)?;
        let ownership = ownership_for_plan(&plan, &contents, previous_values);
        install.insert(plan.relative_path.clone(), ownership);

        updates.push(MirrorUpdate {
            relative_path: plan.relative_path,
            path,
            existing,
            contents,
        });
    }
    let skill_symlink = skill_symlink_for_agent(agent);
    validate_skill_symlink_destination(paths, skill_symlink)?;
    install.insert(
        skill_symlink.relative_path,
        FileOwnership::symlink(skill_symlink.target),
    );

    Ok(PreparedMirrors {
        install,
        updates,
        skill_symlink,
        backup_timestamp,
    })
}

/// Write a prepared mirror plan after caller-owned metadata has been persisted.
pub(crate) fn write_prepared_mirrors(
    paths: &MaestroPaths,
    prepared: &PreparedMirrors,
) -> Result<()> {
    let mut written = Vec::new();
    for update in &prepared.updates {
        if let Err(error) = write_mirror_update(paths, prepared, update) {
            rollback_mirror_updates(&written)?;
            return Err(error);
        }
        if update.existing.as_deref() != Some(update.contents.as_str()) {
            written.push(update);
        }
    }
    if let Err(error) = create_skill_symlink(paths, prepared.skill_symlink) {
        rollback_mirror_updates(&written)?;
        return Err(error);
    }

    Ok(())
}

fn write_mirror_update(
    paths: &MaestroPaths,
    prepared: &PreparedMirrors,
    update: &MirrorUpdate,
) -> Result<()> {
    if update.existing.as_deref() == Some(update.contents.as_str()) {
        return Ok(());
    }

    if update.path.exists() {
        backup_file_with_timestamp(paths, &update.path, "install", &prepared.backup_timestamp)?;
    }
    ensure_parent_dir(&update.path)?;
    write_string_atomic(&update.path, &update.contents)
        .with_context(|| format!("failed to write mirror {}", update.path.display()))?;
    println!(
        "{}",
        unified_diff(
            &update.relative_path,
            update.existing.as_deref().unwrap_or(""),
            &update.contents
        )
    );

    Ok(())
}

fn rollback_mirror_updates(written: &[&MirrorUpdate]) -> Result<()> {
    for update in written.iter().rev() {
        match &update.existing {
            Some(contents) => write_string_atomic(&update.path, contents)
                .with_context(|| format!("failed to roll back mirror {}", update.path.display()))?,
            None => match fs::remove_file(&update.path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to remove rolled-back mirror {}",
                            update.path.display()
                        )
                    });
                }
            },
        }
    }

    Ok(())
}

fn ownership_for_plan(
    plan: &MirrorPlan,
    contents: &str,
    previous_values: BTreeMap<String, Value>,
) -> FileOwnership {
    match plan.kind {
        MirrorKind::MarkdownManagedBlock
        | MirrorKind::GitignoreSection
        | MirrorKind::TomlSection => FileOwnership::text(plan.kind.clone(), contents),
        MirrorKind::JsonManagedKeys => {
            FileOwnership::json_keys(plan.managed_keys.clone(), previous_values)
        }
        MirrorKind::Symlink => {
            unreachable!("symlink mirrors are not written by text mirror planner")
        }
    }
}

fn contents_for_existing(
    agent: InstallAgent,
    plan: &MirrorPlan,
    existing: Option<&str>,
    previous_values: &BTreeMap<String, Value>,
) -> Result<String> {
    match plan.kind {
        MirrorKind::MarkdownManagedBlock if plan.relative_path == "CLAUDE.md" => Ok(
            upsert_managed_block(existing, ManagedBlockFormat::Markdown, claude_md_block()),
        ),
        MirrorKind::MarkdownManagedBlock if plan.relative_path == "AGENTS.md" => Ok(
            upsert_managed_block(existing, ManagedBlockFormat::Markdown, agents_md_block()),
        ),
        MirrorKind::GitignoreSection => Ok(upsert_managed_block(
            existing,
            ManagedBlockFormat::HashComment,
            gitignore_block(),
        )),
        MirrorKind::TomlSection => Ok(upsert_managed_block(
            existing,
            ManagedBlockFormat::HashComment,
            codex_config_block(),
        )),
        MirrorKind::JsonManagedKeys => {
            let object = ManagedHookConfig::for_agent(agent)
                .contents
                .as_object()
                .cloned()
                .context("managed JSON mirror must be an object")?;
            let contents = upsert_managed_json_keys(existing, object)?;
            add_previous_value_hashes(contents, previous_values)
        }
        MirrorKind::MarkdownManagedBlock | MirrorKind::Symlink => Ok(plan.contents.clone()),
    }
}

/// Remove managed mirror content for files recorded in the install lock.
pub fn remove_mirrors(
    paths: &MaestroPaths,
    agent: InstallAgent,
    install: &AgentInstall,
    still_owned_paths: &BTreeSet<String>,
) -> Result<()> {
    let backup_timestamp = backup_operation_timestamp()?;
    validate_install_ownership(agent, install)?;
    let mut removals = Vec::new();
    let mut symlink_removals = Vec::new();

    for (relative_path, ownership) in &install.files {
        if still_owned_paths.contains(relative_path) {
            continue;
        }
        if ownership.kind == MirrorKind::Symlink {
            let path = managed_symlink_path(paths, relative_path)?;
            symlink_removals.push((path, ownership));
            continue;
        }
        let path = managed_mirror_path(paths, relative_path)?;
        let Some(contents) = read_to_string_if_exists(&path)? else {
            continue;
        };

        let next = match ownership.kind {
            MirrorKind::MarkdownManagedBlock => {
                remove_managed_block(&contents, ManagedBlockFormat::Markdown)
            }
            MirrorKind::GitignoreSection | MirrorKind::TomlSection => {
                remove_managed_block(&contents, ManagedBlockFormat::HashComment)
            }
            MirrorKind::JsonManagedKeys => remove_json_keys_with_restore(&contents, ownership)?,
            MirrorKind::Symlink => unreachable!("symlink ownership is handled before file reads"),
        };
        if next == contents {
            continue;
        }
        removals.push(MirrorRemoval {
            relative_path: relative_path.clone(),
            path,
            contents,
            next,
        });
    }

    write_mirror_removals(paths, &removals, &backup_timestamp)?;
    if let Err(error) = remove_symlinks(&symlink_removals) {
        rollback_mirror_removals(&removals)?;
        return Err(error);
    }

    Ok(())
}

fn write_mirror_removals(
    paths: &MaestroPaths,
    removals: &[MirrorRemoval],
    backup_timestamp: &str,
) -> Result<()> {
    let mut written = Vec::new();
    for removal in removals {
        if let Err(error) = write_mirror_removal(paths, removal, backup_timestamp) {
            rollback_mirror_removals(&written)?;
            return Err(error);
        }
        written.push(removal.clone());
    }

    Ok(())
}

fn write_mirror_removal(
    paths: &MaestroPaths,
    removal: &MirrorRemoval,
    backup_timestamp: &str,
) -> Result<()> {
    backup_file_with_timestamp(paths, &removal.path, "uninstall", backup_timestamp)?;

    write_string_atomic(&removal.path, &removal.next)
        .with_context(|| format!("failed to uninstall mirror {}", removal.path.display()))?;
    println!(
        "{}",
        unified_diff(&removal.relative_path, &removal.contents, &removal.next)
    );

    Ok(())
}

fn rollback_mirror_removals(removals: &[MirrorRemoval]) -> Result<()> {
    for removal in removals.iter().rev() {
        write_string_atomic(&removal.path, &removal.contents).with_context(|| {
            format!(
                "failed to roll back uninstalled mirror {}",
                removal.path.display()
            )
        })?;
    }

    Ok(())
}

fn remove_symlinks(symlink_removals: &[(PathBuf, &FileOwnership)]) -> Result<()> {
    for (path, ownership) in symlink_removals {
        remove_symlink_if_owned(path, ownership)?;
    }

    Ok(())
}

fn validate_install_ownership(agent: InstallAgent, install: &AgentInstall) -> Result<()> {
    let mut allowed = BTreeMap::new();
    for plan in mirror_plan(agent)? {
        allowed.insert(plan.relative_path.clone(), plan);
    }
    let skill_symlink = skill_symlink_for_agent(agent);

    for (relative_path, ownership) in &install.files {
        if relative_path == skill_symlink.relative_path {
            if ownership.kind != MirrorKind::Symlink
                || ownership.target.as_deref() != Some(skill_symlink.target)
            {
                bail!(
                    "install lock entry is not an expected mirror for {}: {}",
                    agent.key(),
                    relative_path
                );
            }
            continue;
        }

        let Some(plan) = allowed.get(relative_path) else {
            bail!(
                "install lock entry is not an expected mirror for {}: {}",
                agent.key(),
                relative_path
            );
        };
        if plan.kind != ownership.kind {
            bail!(
                "install lock entry is not an expected mirror for {}: {}",
                agent.key(),
                relative_path
            );
        }
        if ownership.kind == MirrorKind::JsonManagedKeys {
            let expected_keys = plan.managed_keys.iter().collect::<BTreeSet<_>>();
            let owned_keys = ownership.managed_keys.iter().collect::<BTreeSet<_>>();
            let previous_keys = ownership.previous_values.keys().collect::<BTreeSet<_>>();
            if owned_keys != expected_keys || !previous_keys.is_subset(&expected_keys) {
                bail!(
                    "install lock entry has unexpected managed JSON keys for {}: {}",
                    agent.key(),
                    relative_path
                );
            }
        }
    }

    Ok(())
}

fn remove_symlink_if_owned(path: &Path, ownership: &FileOwnership) -> Result<()> {
    let Some(expected_target) = ownership.target.as_deref() else {
        return Ok(());
    };
    remove_skill_symlink_if_owned(path, expected_target)
}

fn managed_mirror_path(paths: &MaestroPaths, relative_path: &str) -> Result<PathBuf> {
    let relative = Path::new(relative_path);
    reject_unsafe_relative_path(relative)?;
    reject_symlinked_path_components(paths.repo_root(), relative)?;
    Ok(paths.repo_root().join(relative))
}

fn managed_symlink_path(paths: &MaestroPaths, relative_path: &str) -> Result<PathBuf> {
    let relative = Path::new(relative_path);
    reject_unsafe_relative_path(relative)?;
    reject_symlinked_parent_components(paths.repo_root(), relative)?;
    Ok(paths.repo_root().join(relative))
}

fn reject_unsafe_relative_path(relative: &Path) -> Result<()> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MaestroError::OutsideRepository {
            path: relative.to_path_buf(),
        }
        .into());
    }

    Ok(())
}

fn reject_symlinked_path_components(repo_root: &Path, relative: &Path) -> Result<()> {
    let mut current = repo_root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(MaestroError::ManagedPathContainsSymlink { path: current }.into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect {}", current.display()));
            }
        }
    }

    Ok(())
}

fn reject_symlinked_parent_components(repo_root: &Path, relative: &Path) -> Result<()> {
    let Some(parent) = relative.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    reject_symlinked_path_components(repo_root, parent)
}

fn markdown(relative_path: &str, body: &str) -> MirrorPlan {
    MirrorPlan {
        relative_path: relative_path.to_string(),
        kind: MirrorKind::MarkdownManagedBlock,
        contents: upsert_managed_block(None, ManagedBlockFormat::Markdown, body),
        managed_keys: Vec::new(),
    }
}

fn hash(relative_path: &str, body: &str, kind: MirrorKind) -> MirrorPlan {
    MirrorPlan {
        relative_path: relative_path.to_string(),
        kind,
        contents: upsert_managed_block(None, ManagedBlockFormat::HashComment, body),
        managed_keys: Vec::new(),
    }
}

fn json_keys(relative_path: &str, value: Value, managed_keys: Vec<String>) -> Result<MirrorPlan> {
    let object = value
        .as_object()
        .cloned()
        .context("managed JSON mirror must be an object")?;
    Ok(MirrorPlan {
        relative_path: relative_path.to_string(),
        kind: MirrorKind::JsonManagedKeys,
        contents: upsert_managed_json_keys(None, object)?,
        managed_keys,
    })
}

fn hook_config_plan(agent: InstallAgent) -> Result<MirrorPlan> {
    let config = ManagedHookConfig::for_agent(agent);
    json_keys(config.relative_path, config.contents, config.managed_keys)
}

fn claude_md_block() -> &'static str {
    "# Maestro Harness Protocol\n@.maestro/harness/HARNESS.md"
}

fn agents_md_block() -> &'static str {
    "# Maestro Harness Protocol\nRead .maestro/harness/HARNESS.md first before working in this repo."
}

fn gitignore_block() -> &'static str {
    "# Maestro local-only paths\n.maestro/runs/\n.maestro/backups/\n.maestro/install-lock.yaml\n.maestro/tasks/*/evidence/\n.maestro/tasks/*/local/\n\n# Local agent settings\n.claude/settings.local.json\n.codex/hooks.json"
}

fn codex_config_block() -> &'static str {
    "# Maestro MCP config is installed in a later phase when `maestro mcp serve` exists."
}

fn previous_json_values(
    previous_install: Option<&AgentInstall>,
    plan: &MirrorPlan,
    existing: Option<&str>,
) -> Result<BTreeMap<String, Value>> {
    let Some(existing) = existing.filter(|contents| !contents.trim().is_empty()) else {
        return Ok(BTreeMap::new());
    };
    if existing == plan.contents {
        return Ok(BTreeMap::new());
    }
    let Value::Object(object) =
        serde_json::from_str::<Value>(existing).context("failed to parse existing JSON mirror")?
    else {
        bail!("managed JSON mirror must be an object");
    };

    if let Some(previous_ownership) = previous_install
        .and_then(|install| install.files.get(&plan.relative_path))
        .filter(|ownership| ownership.kind == MirrorKind::JsonManagedKeys)
    {
        if validate_previous_value_hashes(&object, previous_ownership).is_ok() {
            return Ok(previous_ownership.previous_values.clone());
        }
    }

    let mut previous_values = BTreeMap::new();
    for key in &plan.managed_keys {
        if let Some(value) = object.get(key) {
            previous_values.insert(key.clone(), value.clone());
        }
    }

    Ok(previous_values)
}

fn remove_json_keys_with_restore(existing: &str, ownership: &FileOwnership) -> Result<String> {
    let Value::Object(mut object) =
        serde_json::from_str::<Value>(existing).context("failed to parse existing JSON mirror")?
    else {
        bail!("managed JSON mirror must be an object");
    };
    validate_previous_value_hashes(&object, ownership)?;

    for key in &ownership.managed_keys {
        object.remove(key);
    }
    object.remove("_maestro_managed_keys");
    object.remove(JSON_PREVIOUS_VALUE_HASHES);
    for (key, value) in &ownership.previous_values {
        object.insert(key.clone(), value.clone());
    }

    let mut formatted = serde_json::to_string_pretty(&Value::Object(object))?;
    formatted.push('\n');
    Ok(formatted)
}

fn add_previous_value_hashes(
    contents: String,
    previous_values: &BTreeMap<String, Value>,
) -> Result<String> {
    if previous_values.is_empty() {
        return Ok(contents);
    }
    let Value::Object(mut object) =
        serde_json::from_str::<Value>(&contents).context("failed to parse managed JSON mirror")?
    else {
        bail!("managed JSON mirror must be an object");
    };
    let hashes = previous_values
        .iter()
        .map(|(key, value)| Ok((key.clone(), Value::String(json_value_fingerprint(value)?))))
        .collect::<Result<Map<String, Value>>>()?;
    object.insert(
        JSON_PREVIOUS_VALUE_HASHES.to_string(),
        Value::Object(hashes),
    );

    let mut formatted = serde_json::to_string_pretty(&Value::Object(object))?;
    formatted.push('\n');
    Ok(formatted)
}

fn validate_previous_value_hashes(
    object: &Map<String, Value>,
    ownership: &FileOwnership,
) -> Result<()> {
    if ownership.previous_values.is_empty() {
        return Ok(());
    }
    let Some(Value::Object(hashes)) = object.get(JSON_PREVIOUS_VALUE_HASHES) else {
        bail!("managed JSON restore metadata is missing");
    };
    for (key, value) in &ownership.previous_values {
        let expected = json_value_fingerprint(value)?;
        if hashes.get(key).and_then(Value::as_str) != Some(expected.as_str()) {
            bail!("managed JSON restore metadata does not match install lock");
        }
    }

    Ok(())
}

fn json_value_fingerprint(value: &Value) -> Result<String> {
    let encoded = serde_json::to_string(value)?;
    let digest = Sha256::digest(encoded.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(format!("sha256:{hex}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{rollback_mirror_removals, MirrorRemoval};

    #[test]
    fn rollback_mirror_removals_restores_written_files() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("maestro-mirror-rollback-{nanos}"));
        fs::create_dir(&root).expect("invariant: temp root should be creatable");
        let path = root.join("AGENTS.md");
        fs::write(&path, "removed\n").expect("invariant: changed mirror should be writable");
        let removal = MirrorRemoval {
            relative_path: "AGENTS.md".to_string(),
            path: path.clone(),
            contents: "original\n".to_string(),
            next: "removed\n".to_string(),
        };

        rollback_mirror_removals(&[removal]).expect("invariant: rollback should succeed");

        let contents = fs::read_to_string(&path).expect("invariant: rolled back file is readable");
        assert_eq!(contents, "original\n");
        fs::remove_dir_all(root).expect("invariant: temp root should be removable");
    }
}
