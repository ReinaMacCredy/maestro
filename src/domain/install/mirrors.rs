use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Error, Result, bail};
use serde_json::{Map, Value};

use crate::domain::skills::symlink::{
    SkillSymlink, create_skill_symlink, validate_skill_symlink_destination,
};
use crate::foundation::core::backup::{backup_file_with_timestamp, backup_operation_timestamp};
use crate::foundation::core::diff::unified_diff;
use crate::foundation::core::fs::{
    create_directory_symlink, ensure_parent_dir, read_to_string_if_exists,
};
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::managed_blocks::{
    ManagedBlockFormat, find_block, remove_managed_block, upsert_managed_block,
    upsert_managed_json_keys,
};
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path, managed_symlink_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::{restore_or_remove, write_string_atomic};

use super::hooks::ManagedHookConfig;
use super::lock::{AgentInstall, FileOwnership, MirrorKind};
use super::{InstallAgent, ensure_uninstallable_install, skill_symlink_for_agent};

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
    managed_json: Option<Map<String, Value>>,
}

#[derive(Debug)]
pub(crate) struct MirrorWriteFailure {
    source: Error,
    rollback_error: Option<Error>,
}

impl MirrorWriteFailure {
    fn rolled_back(source: Error) -> Self {
        Self {
            source,
            rollback_error: None,
        }
    }

    pub(super) fn rollback_failed(source: Error, rollback_error: Error) -> Self {
        Self {
            source,
            rollback_error: Some(rollback_error),
        }
    }

    pub(crate) fn rollback_completed(&self) -> bool {
        self.rollback_error.is_none()
    }

    pub(crate) fn into_error(self) -> Error {
        match self.rollback_error {
            Some(rollback_error) => anyhow::anyhow!(
                "{}; additionally failed to roll back partial mirror writes: {}",
                self.source,
                rollback_error
            ),
            None => self.source,
        }
    }
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
    /// True when maestro created this file (carried from the install lock). Gates
    /// husk removal: only a created-fresh file may be deleted when emptied.
    created_fresh: bool,
}

#[derive(Debug)]
pub(crate) struct MirrorRemovalRollback {
    removals: Vec<MirrorRemoval>,
    symlink_removals: Vec<RemovedSymlink>,
}

impl MirrorRemovalRollback {
    pub(crate) fn rollback(self) -> Result<()> {
        let mut errors = Vec::new();
        if let Err(error) = rollback_mirror_removals(&self.removals) {
            errors.push(format!("failed to roll back file mirrors: {error}"));
        }
        if let Err(error) = rollback_removed_symlinks(&self.symlink_removals) {
            errors.push(format!("failed to roll back symlinks: {error}"));
        }
        if errors.is_empty() {
            return Ok(());
        }

        bail!("{}", errors.join("; "))
    }
}

#[derive(Clone, Debug)]
struct RemovedSymlink {
    path: PathBuf,
    target: String,
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
        let contents = contents_for_existing(&plan, existing.as_deref(), &previous_values)?;
        // `created_fresh` is sticky: it records that maestro created the file from
        // nothing, which decides whether uninstall may delete an emptied residue.
        // On re-install the file already exists, so recomputing it from disk would
        // flip a maestro-created file to "pre-existing" and leave a husk behind.
        // Carry the prior lock's verdict forward when there is one.
        let created_fresh = match previous_install.and_then(|previous| {
            previous.files.get(&plan.relative_path)
        }) {
            Some(ownership) => ownership.created_fresh,
            None => existing.is_none(),
        };
        let ownership =
            ownership_for_plan(&plan, &contents, previous_values, created_fresh)?;
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
) -> std::result::Result<(), MirrorWriteFailure> {
    let mut effects = FilesystemMirrorEffects;
    write_prepared_mirrors_with_effects(paths, prepared, &mut effects)
}

fn write_prepared_mirrors_with_effects(
    paths: &MaestroPaths,
    prepared: &PreparedMirrors,
    effects: &mut impl MirrorEffects,
) -> std::result::Result<(), MirrorWriteFailure> {
    let mut written = Vec::new();
    for update in &prepared.updates {
        if let Err(error) = effects.write_mirror_update(paths, prepared, update) {
            return Err(rollback_write_failure(effects, error, &written));
        }
        if update.existing.as_deref() != Some(update.contents.as_str()) {
            written.push(update);
        }
    }
    if let Err(error) = effects.create_skill_symlink(paths, prepared.skill_symlink) {
        return Err(rollback_write_failure(effects, error, &written));
    }

    Ok(())
}

fn rollback_write_failure(
    effects: &mut impl MirrorEffects,
    error: Error,
    written: &[&MirrorUpdate],
) -> MirrorWriteFailure {
    match effects.rollback_mirror_updates(written) {
        Ok(()) => MirrorWriteFailure::rolled_back(error),
        Err(rollback_error) => MirrorWriteFailure::rollback_failed(error, rollback_error),
    }
}

trait MirrorEffects {
    fn write_mirror_update(
        &mut self,
        paths: &MaestroPaths,
        prepared: &PreparedMirrors,
        update: &MirrorUpdate,
    ) -> Result<()>;

    fn create_skill_symlink(&mut self, paths: &MaestroPaths, symlink: SkillSymlink) -> Result<()>;

    fn rollback_mirror_updates(&mut self, written: &[&MirrorUpdate]) -> Result<()>;
}

struct FilesystemMirrorEffects;

impl MirrorEffects for FilesystemMirrorEffects {
    fn write_mirror_update(
        &mut self,
        paths: &MaestroPaths,
        prepared: &PreparedMirrors,
        update: &MirrorUpdate,
    ) -> Result<()> {
        write_mirror_update(paths, prepared, update)
    }

    fn create_skill_symlink(&mut self, paths: &MaestroPaths, symlink: SkillSymlink) -> Result<()> {
        create_skill_symlink(paths, symlink)
    }

    fn rollback_mirror_updates(&mut self, written: &[&MirrorUpdate]) -> Result<()> {
        rollback_mirror_updates(written)
    }
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
        restore_or_remove(
            &update.path,
            update.existing.as_deref(),
            || format!("failed to roll back mirror {}", update.path.display()),
            || {
                format!(
                    "failed to remove rolled-back mirror {}",
                    update.path.display()
                )
            },
        )?;
    }

    Ok(())
}

fn ownership_for_plan(
    plan: &MirrorPlan,
    contents: &str,
    previous_values: BTreeMap<String, Value>,
    created_fresh: bool,
) -> Result<FileOwnership> {
    match plan.kind {
        MirrorKind::MarkdownManagedBlock
        | MirrorKind::GitignoreSection
        | MirrorKind::TomlSection => Ok(FileOwnership::text(
            plan.kind.clone(),
            text_ownership_content(&plan.kind, contents)?,
            created_fresh,
        )),
        MirrorKind::JsonManagedKeys => Ok(FileOwnership::json_keys(
            plan.managed_keys.clone(),
            previous_values,
            created_fresh,
        )),
        MirrorKind::Symlink => {
            unreachable!("symlink mirrors are not written by text mirror planner")
        }
    }
}

fn contents_for_existing(
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
            let object = plan
                .managed_json
                .clone()
                .context("managed JSON mirror must be an object")?;
            let contents = upsert_managed_json_keys(existing, object)?;
            add_previous_value_hashes(contents, previous_values)
        }
        MirrorKind::MarkdownManagedBlock | MirrorKind::Symlink => Ok(plan.contents.clone()),
    }
}

/// Remove managed mirror content for files recorded in the install lock.
pub(crate) fn remove_mirrors(
    paths: &MaestroPaths,
    agent: InstallAgent,
    install: &AgentInstall,
    still_owned_paths: &BTreeSet<String>,
) -> Result<MirrorRemovalRollback> {
    let backup_timestamp = backup_operation_timestamp()?;
    let expected_plans = expected_mirror_plans(agent)?;
    validate_install_ownership(agent, install, &expected_plans)?;
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
        let plan = expected_plans.get(relative_path).with_context(|| {
            format!("install lock entry is not an expected mirror: {relative_path}")
        })?;
        let next = match ownership.kind {
            MirrorKind::MarkdownManagedBlock => {
                match remove_text_mirror_content(
                    relative_path,
                    ownership,
                    plan,
                    &contents,
                    ManagedBlockFormat::Markdown,
                    install.state == super::InstallState::Removing,
                )? {
                    Some(next) => next,
                    None => continue,
                }
            }
            MirrorKind::GitignoreSection | MirrorKind::TomlSection => {
                match remove_text_mirror_content(
                    relative_path,
                    ownership,
                    plan,
                    &contents,
                    ManagedBlockFormat::HashComment,
                    install.state == super::InstallState::Removing,
                )? {
                    Some(next) => next,
                    None => continue,
                }
            }
            MirrorKind::JsonManagedKeys => match remove_json_keys_with_restore(
                &contents,
                ownership,
                install.state == super::InstallState::Removing,
            )? {
                Some(next) => next,
                None => continue,
            },
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
            created_fresh: ownership.created_fresh,
        });
    }

    write_mirror_removals(paths, &removals, &backup_timestamp)?;
    let symlink_removals = match remove_symlinks(&symlink_removals) {
        Ok(symlink_removals) => symlink_removals,
        Err(error) => {
            rollback_mirror_removals(&removals)?;
            return Err(error);
        }
    };

    Ok(MirrorRemovalRollback {
        removals,
        symlink_removals,
    })
}

fn remove_symlinks(symlink_removals: &[(PathBuf, &FileOwnership)]) -> Result<Vec<RemovedSymlink>> {
    let mut removed = Vec::new();
    for (path, ownership) in symlink_removals {
        match remove_symlink_if_owned(path, ownership) {
            Ok(Some(symlink)) => removed.push(symlink),
            Ok(None) => {}
            Err(error) => {
                rollback_removed_symlinks(&removed)?;
                return Err(error);
            }
        }
    }

    Ok(removed)
}

fn remove_symlink_if_owned(
    path: &Path,
    ownership: &FileOwnership,
) -> Result<Option<RemovedSymlink>> {
    let Some(expected_target) = ownership.target.as_deref() else {
        return Ok(None);
    };
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::read_link(path)
                .with_context(|| format!("failed to read symlink {}", path.display()))?;
            if target != Path::new(expected_target) {
                return Ok(None);
            }
            fs::remove_file(path)
                .with_context(|| format!("failed to remove symlink {}", path.display()))?;
            Ok(Some(RemovedSymlink {
                path: path.to_path_buf(),
                target: expected_target.to_string(),
            }))
        }
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn rollback_removed_symlinks(symlinks: &[RemovedSymlink]) -> Result<()> {
    for symlink in symlinks.iter().rev() {
        match fs::symlink_metadata(&symlink.path) {
            Ok(_) => {
                bail!(
                    "failed to roll back removed symlink {} because the path now exists",
                    symlink.path.display()
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to inspect removed symlink {}",
                        symlink.path.display()
                    )
                });
            }
        }
        ensure_parent_dir(&symlink.path)?;
        create_directory_symlink(Path::new(&symlink.target), &symlink.path)
            .with_context(|| format!("failed to roll back symlink {}", symlink.path.display()))?;
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

    if is_empty_residue(&removal.next) && removal.created_fresh {
        // Stripping maestro's managed content left no user content behind, AND
        // maestro created this file at install (it did not pre-exist). Remove the
        // husk instead of leaving a 0-byte file or bare `{}` behind (T6.5). A file
        // the user already had is never deleted here, even when its residue is
        // empty (e.g. a pre-existing empty `{}` settings file): that falls to the
        // `else` branch, which restores it. This honors the locked husk-safety
        // rule -- delete only what maestro created fresh, preserve pre-existing.
        fs::remove_file(&removal.path).with_context(|| {
            format!("failed to remove emptied mirror {}", removal.path.display())
        })?;
        // The file is gone; printing a `contents -> {}` diff would tell an
        // agent the file now holds `{}`. Announce the removal instead.
        println!("removed {}", removal.relative_path);
    } else {
        write_string_atomic(&removal.path, &removal.next)
            .with_context(|| format!("failed to uninstall mirror {}", removal.path.display()))?;
        println!(
            "{}",
            unified_diff(&removal.relative_path, &removal.contents, &removal.next)
        );
    }

    Ok(())
}

/// True when stripping maestro's managed content emptied the file: pure
/// whitespace (text mirrors) or an empty JSON object (json mirrors). A file that
/// reduces to this held only maestro's block, so uninstall removes it instead of
/// leaving a husk; a real user file leaves non-empty residue and is preserved.
fn is_empty_residue(next: &str) -> bool {
    let trimmed = next.trim();
    trimmed.is_empty() || trimmed == "{}"
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

fn expected_mirror_plans(agent: InstallAgent) -> Result<BTreeMap<String, MirrorPlan>> {
    mirror_plan(agent).map(|plans| {
        plans
            .into_iter()
            .map(|plan| (plan.relative_path.clone(), plan))
            .collect()
    })
}

fn validate_install_ownership(
    agent: InstallAgent,
    install: &AgentInstall,
    allowed: &BTreeMap<String, MirrorPlan>,
) -> Result<()> {
    ensure_uninstallable_install(agent, install)?;

    let skill_symlink = skill_symlink_for_agent(agent);
    let owned_paths = install.files.keys().cloned().collect::<BTreeSet<_>>();
    let mut expected_paths = allowed.keys().cloned().collect::<BTreeSet<_>>();
    expected_paths.insert(skill_symlink.relative_path.to_string());
    if owned_paths != expected_paths && !is_legacy_symlink_only_install(install, skill_symlink) {
        bail!(
            "install lock for {} does not match the expected mirror set",
            agent.key()
        );
    }

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

fn is_legacy_symlink_only_install(
    install: &AgentInstall,
    skill_symlink: crate::domain::skills::symlink::SkillSymlink,
) -> bool {
    install.files.len() == 1
        && install
            .files
            .get(skill_symlink.relative_path)
            .is_some_and(|ownership| {
                ownership.kind == MirrorKind::Symlink
                    && ownership.target.as_deref() == Some(skill_symlink.target)
            })
}

fn verify_text_content_ownership(
    relative_path: &str,
    ownership: &FileOwnership,
    plan: &MirrorPlan,
    contents: &str,
) -> Result<()> {
    match ownership.kind {
        MirrorKind::MarkdownManagedBlock
        | MirrorKind::GitignoreSection
        | MirrorKind::TomlSection => {
            let owned_content = text_ownership_content(&ownership.kind, contents)?;
            if !ownership.matches_text_content(owned_content)
                && !legacy_full_file_lock_still_owns_current_block(ownership, plan, owned_content)?
            {
                bail!(
                    "refusing to uninstall {} because the current contents do not match the install lock",
                    relative_path
                );
            }
        }
        MirrorKind::JsonManagedKeys | MirrorKind::Symlink => {}
    }

    Ok(())
}

fn remove_text_mirror_content(
    relative_path: &str,
    ownership: &FileOwnership,
    plan: &MirrorPlan,
    contents: &str,
    format: ManagedBlockFormat,
    removing_retry: bool,
) -> Result<Option<String>> {
    match verify_text_content_ownership(relative_path, ownership, plan, contents) {
        Ok(()) => Ok(Some(remove_managed_block(contents, format))),
        Err(error) => {
            if removing_retry && marked_block_for_format(contents, format).is_none() {
                return Ok(None);
            }
            Err(error)
        }
    }
}

fn legacy_full_file_lock_still_owns_current_block(
    ownership: &FileOwnership,
    plan: &MirrorPlan,
    owned_content: &str,
) -> Result<bool> {
    if !ownership.has_legacy_text_hash() {
        return Ok(false);
    }
    Ok(owned_content == text_ownership_content(&plan.kind, &plan.contents)?)
}

fn text_ownership_content<'a>(kind: &MirrorKind, contents: &'a str) -> Result<&'a str> {
    let Some((start_marker, end_marker)) = text_markers(kind) else {
        bail!("install lock entry is not a text mirror");
    };
    let Some(block) = marked_block(contents, start_marker, end_marker) else {
        bail!("managed text mirror is missing Maestro ownership markers");
    };
    Ok(block)
}

fn text_markers(kind: &MirrorKind) -> Option<(&'static str, &'static str)> {
    let format = match kind {
        MirrorKind::MarkdownManagedBlock => ManagedBlockFormat::Markdown,
        MirrorKind::GitignoreSection | MirrorKind::TomlSection => ManagedBlockFormat::HashComment,
        MirrorKind::JsonManagedKeys | MirrorKind::Symlink => return None,
    };
    Some(format.markers())
}

fn marked_block<'a>(contents: &'a str, start_marker: &str, end_marker: &str) -> Option<&'a str> {
    find_block(contents, start_marker, end_marker).map(|(start, end)| &contents[start..end])
}

fn marked_block_for_format(contents: &str, format: ManagedBlockFormat) -> Option<&str> {
    let (start_marker, end_marker) = format.markers();
    marked_block(contents, start_marker, end_marker)
}

fn managed_mirror_path(paths: &MaestroPaths, relative_path: &str) -> Result<PathBuf> {
    managed_path(paths, relative_path, SymlinkPolicy::RejectAllComponents)
}

fn markdown(relative_path: &str, body: &str) -> MirrorPlan {
    MirrorPlan {
        relative_path: relative_path.to_string(),
        kind: MirrorKind::MarkdownManagedBlock,
        contents: upsert_managed_block(None, ManagedBlockFormat::Markdown, body),
        managed_keys: Vec::new(),
        managed_json: None,
    }
}

fn hash(relative_path: &str, body: &str, kind: MirrorKind) -> MirrorPlan {
    MirrorPlan {
        relative_path: relative_path.to_string(),
        kind,
        contents: upsert_managed_block(None, ManagedBlockFormat::HashComment, body),
        managed_keys: Vec::new(),
        managed_json: None,
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
        contents: upsert_managed_json_keys(None, object.clone())?,
        managed_keys,
        managed_json: Some(object),
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
    "# Maestro local-only paths\n.maestro/runs/\n.maestro/backups/\n.maestro/install-lock.yaml\n.maestro/tasks/*/evidence/\n.maestro/tasks/*/local/\n.maestro/archive/**/evidence/\n.maestro/archive/**/local/\n.maestro/archive/**/runs/\n\n# Local agent settings\n.claude/settings.local.json\n.claude/skills\n.codex/hooks.json\n.codex/skills"
}

fn codex_config_block() -> &'static str {
    "# Maestro ships an MCP server: `maestro mcp serve` (stdio).\n\
     # Maestro does not wire it into Codex for you. To expose it, add an MCP\n\
     # server entry pointing at that command, e.g.:\n\
     #   [mcp_servers.maestro]\n\
     #   command = \"maestro\"\n\
     #   args = [\"mcp\", \"serve\"]"
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
        validate_previous_value_hashes(&object, previous_ownership)?;
        return Ok(previous_ownership.previous_values.clone());
    }
    if object.contains_key(JSON_PREVIOUS_VALUE_HASHES) {
        bail!("managed JSON restore metadata has no validated install lock snapshot");
    }

    let mut previous_values = BTreeMap::new();
    for key in &plan.managed_keys {
        if let Some(value) = object.get(key) {
            previous_values.insert(key.clone(), value.clone());
        }
    }

    Ok(previous_values)
}

fn remove_json_keys_with_restore(
    existing: &str,
    ownership: &FileOwnership,
    removing_retry: bool,
) -> Result<Option<String>> {
    let Value::Object(mut object) =
        serde_json::from_str::<Value>(existing).context("failed to parse existing JSON mirror")?
    else {
        bail!("managed JSON mirror must be an object");
    };
    if removing_retry && json_restore_already_applied(&object, ownership) {
        return Ok(None);
    }
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
    Ok(Some(formatted))
}

fn json_restore_already_applied(object: &Map<String, Value>, ownership: &FileOwnership) -> bool {
    !object.contains_key("_maestro_managed_keys")
        && !object.contains_key(JSON_PREVIOUS_VALUE_HASHES)
        && ownership
            .managed_keys
            .iter()
            .all(|key| ownership.previous_values.contains_key(key) || !object.contains_key(key))
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
    Ok(sha256_prefixed(encoded.as_bytes()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use anyhow::{Result, bail};

    use super::{
        MirrorEffects, MirrorRemoval, MirrorUpdate, PreparedMirrors, rollback_mirror_removals,
        write_prepared_mirrors_with_effects,
    };
    use crate::domain::install::AgentInstall;
    use crate::domain::skills::symlink::SkillSymlink;
    use crate::foundation::core::paths::MaestroPaths;

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
            created_fresh: false,
        };

        rollback_mirror_removals(&[removal]).expect("invariant: rollback should succeed");

        let contents = fs::read_to_string(&path).expect("invariant: rolled back file is readable");
        assert_eq!(contents, "original\n");
        fs::remove_dir_all(root).expect("invariant: temp root should be removable");
    }

    #[test]
    fn write_prepared_mirrors_reports_rollback_failure() {
        let root = temp_root("maestro-mirror-effects-test");
        let paths = MaestroPaths::new(root.clone());
        let prepared = PreparedMirrors {
            install: AgentInstall::new("test".to_string()),
            updates: vec![
                MirrorUpdate {
                    relative_path: "AGENTS.md".to_string(),
                    path: root.join("AGENTS.md"),
                    existing: Some("old\n".to_string()),
                    contents: "new\n".to_string(),
                },
                MirrorUpdate {
                    relative_path: "CLAUDE.md".to_string(),
                    path: root.join("CLAUDE.md"),
                    existing: None,
                    contents: "later\n".to_string(),
                },
            ],
            skill_symlink: SkillSymlink {
                relative_path: ".codex/skills",
                target: "../.maestro/skills",
            },
            backup_timestamp: "test".to_string(),
        };
        let mut effects = FailingRollbackEffects::default();

        let failure = write_prepared_mirrors_with_effects(&paths, &prepared, &mut effects)
            .expect_err("invariant: second write and rollback should fail");

        assert!(!failure.rollback_completed());
        assert!(failure.into_error().to_string().contains("rollback failed"));
        assert_eq!(effects.write_attempts, 2);
        assert_eq!(effects.rollback_count, 1);

        fs::remove_dir_all(root).expect("invariant: temp root should be removable");
    }

    #[derive(Default)]
    struct FailingRollbackEffects {
        write_attempts: usize,
        rollback_count: usize,
    }

    impl MirrorEffects for FailingRollbackEffects {
        fn write_mirror_update(
            &mut self,
            _paths: &MaestroPaths,
            _prepared: &PreparedMirrors,
            _update: &MirrorUpdate,
        ) -> Result<()> {
            self.write_attempts += 1;
            if self.write_attempts == 2 {
                bail!("write failed");
            }
            Ok(())
        }

        fn create_skill_symlink(
            &mut self,
            _paths: &MaestroPaths,
            _symlink: SkillSymlink,
        ) -> Result<()> {
            Ok(())
        }

        fn rollback_mirror_updates(&mut self, written: &[&MirrorUpdate]) -> Result<()> {
            self.rollback_count += 1;
            assert_eq!(written.len(), 1);
            bail!("rollback failed");
        }
    }

    fn temp_root(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir(&root).expect("invariant: temp root should be creatable");
        root
    }
}
