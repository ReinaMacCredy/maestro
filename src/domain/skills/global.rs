use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::domain::skills::catalog::{frontmatter_version, skills};
use crate::foundation::core::fs::{create_directory_symlink, ensure_dir};
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::safe_write::{write_atomic, write_string_atomic};
use crate::foundation::core::schema::GLOBAL_SKILLS_LOCK_SCHEMA_VERSION;

const LOCK_FILE_NAME: &str = "skills-lock.yaml";
const REMEDIATION: &str = "move the path aside or restore the Maestro-managed target, then rerun `maestro sync --global-skills`";

const SUPPORTED_ROOTS: &[SupportedRoot] = &[
    SupportedRoot {
        agent: "codex",
        display_suffix: ".agents/skills",
    },
    SupportedRoot {
        agent: "claude",
        display_suffix: ".claude/skills",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SupportedRoot {
    agent: &'static str,
    display_suffix: &'static str,
}

/// Preflighted global skill install/sync work.
#[derive(Debug)]
pub struct PreparedGlobalSkills {
    home_dir: PathBuf,
    cache_dir: PathBuf,
    lock_path: PathBuf,
    roots: Vec<RootPlan>,
    skills: Vec<SkillPlan>,
    links: Vec<LinkPlan>,
}

/// Result of applying the global skill cache and links.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalSkillsOutcome {
    /// User home directory used for this operation.
    pub home_dir: PathBuf,
    /// Maestro-owned skill cache path.
    pub cache_dir: PathBuf,
    /// Global ownership lock path.
    pub lock_path: PathBuf,
    /// Agent roots Maestro touched or verified.
    pub roots: Vec<GlobalSkillRootOutcome>,
    /// Shipped skill names written or verified in the cache.
    pub skill_names: Vec<String>,
    /// Number of per-agent skill symlinks written or verified.
    pub link_count: usize,
}

/// One supported global agent skill root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalSkillRootOutcome {
    /// Supported agent key, e.g. `codex`.
    pub agent: String,
    /// User-facing root path, e.g. `~/.agents/skills` expanded to an absolute path.
    pub display_path: PathBuf,
    /// Canonical target used by the filesystem, especially when the root is a symlink.
    pub resolved_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RootPlan {
    agent: String,
    display_path: PathBuf,
    state: RootState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootState {
    Missing,
    ExistingDirectory,
    ExistingSymlink,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SkillPlan {
    name: String,
    version: Option<String>,
    files: Vec<SkillFilePlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SkillFilePlan {
    relative_path: String,
    path: PathBuf,
    contents: &'static [u8],
    hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkPlan {
    agent: String,
    link_path: PathBuf,
    target_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct GlobalSkillsLock {
    schema_version: String,
    cache_dir: PathBuf,
    skills: BTreeMap<String, GlobalSkillRecord>,
    roots: BTreeMap<String, GlobalRootRecord>,
    links: BTreeMap<String, GlobalLinkRecord>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct GlobalSkillRecord {
    version: Option<String>,
    files: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct GlobalRootRecord {
    display_path: PathBuf,
    resolved_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct GlobalLinkRecord {
    agent: String,
    skill: String,
    display_path: PathBuf,
    target_path: PathBuf,
    resolved_root: PathBuf,
}

/// Preflight global skills using the current user's home directory.
pub fn prepare_global_skills() -> Result<PreparedGlobalSkills> {
    prepare_global_skills_at(&home_dir()?)
}

/// Preflight global skills under an explicit home directory.
pub fn prepare_global_skills_at(home_dir: &Path) -> Result<PreparedGlobalSkills> {
    let cache_dir = cache_dir(home_dir);
    let lock_path = lock_path(home_dir);
    let previous_lock = load_lock_if_exists(&lock_path)?;
    let roots = prepare_roots(home_dir)?;
    let skills = prepare_cache_skills(&cache_dir, previous_lock.as_ref())?;
    let links = prepare_links(&roots, &skills, &cache_dir)?;

    Ok(PreparedGlobalSkills {
        home_dir: home_dir.to_path_buf(),
        cache_dir,
        lock_path,
        roots,
        skills,
        links,
    })
}

/// Sync global skills using the current user's home directory.
pub fn sync_global_skills() -> Result<GlobalSkillsOutcome> {
    let prepared = prepare_global_skills()?;
    write_prepared_global_skills(prepared)
}

/// Sync global skills under an explicit home directory.
pub fn sync_global_skills_at(home_dir: &Path) -> Result<GlobalSkillsOutcome> {
    let prepared = prepare_global_skills_at(home_dir)?;
    write_prepared_global_skills(prepared)
}

/// Refresh global skills only when a global lock already exists.
pub fn sync_global_skills_if_locked(
    home_override: Option<&Path>,
) -> Result<Option<GlobalSkillsOutcome>> {
    let Some(prepared) = prepare_global_skills_if_locked(home_override)? else {
        return Ok(None);
    };
    write_prepared_global_skills(prepared).map(Some)
}

/// Preflight global skills only when a global lock already exists.
pub fn prepare_global_skills_if_locked(
    home_override: Option<&Path>,
) -> Result<Option<PreparedGlobalSkills>> {
    let home_dir = match home_override {
        Some(home_dir) => home_dir.to_path_buf(),
        None => home_dir()?,
    };
    if !lock_path(&home_dir).exists() {
        return Ok(None);
    }

    prepare_global_skills_at(&home_dir).map(Some)
}

/// Apply a preflighted global skill sync.
pub fn write_prepared_global_skills(prepared: PreparedGlobalSkills) -> Result<GlobalSkillsOutcome> {
    let mut rollback = Rollback::default();
    match write_prepared_inner(&prepared, &mut rollback) {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            if let Err(rollback_error) = rollback.rollback() {
                return Err(anyhow!(
                    "{error}; additionally failed to roll back global skill writes: {rollback_error}"
                ));
            }
            Err(error)
        }
    }
}

/// Render a successful global skill sync.
pub fn render_global_skills_outcome(outcome: &GlobalSkillsOutcome) -> String {
    let mut out = String::new();
    out.push_str("global Maestro skills synced for all supported agents:\n");
    out.push_str(&format!("cache: {}\n", outcome.cache_dir.display()));
    out.push_str(&format!("lock: {}\n", outcome.lock_path.display()));
    for root in &outcome.roots {
        out.push_str(&format!(
            "{} root: {} (resolved: {})\n",
            root.agent,
            root.display_path.display(),
            root.resolved_path.display()
        ));
    }
    out.push_str("codex legacy root: ~/.codex/skills skipped (not managed in v1)\n");
    out.push_str(&format!(
        "skills: {} ({})\n",
        outcome.skill_names.len(),
        outcome.skill_names.join(", ")
    ));
    out.push_str(&format!("links: {}\n", outcome.link_count));
    out
}

/// Render a dry-run global skill sync.
pub fn render_global_skills_dry_run(prepared: &PreparedGlobalSkills) -> String {
    let mut out = String::new();
    out.push_str("global Maestro skills would sync for all supported agents:\n");
    out.push_str(&format!("cache: {}\n", prepared.cache_dir.display()));
    out.push_str(&format!("lock: {}\n", prepared.lock_path.display()));
    for root in &prepared.roots {
        out.push_str(&format!(
            "{} root: {}\n",
            root.agent,
            root.display_path.display()
        ));
    }
    out.push_str("codex legacy root: ~/.codex/skills skipped (not managed in v1)\n");
    out.push_str(&format!("skills: {}\n", prepared.skills.len()));
    out.push_str(&format!("links: {}\n", prepared.links.len()));
    out
}

fn write_prepared_inner(
    prepared: &PreparedGlobalSkills,
    rollback: &mut Rollback,
) -> Result<GlobalSkillsOutcome> {
    ensure_dir_tracked(&prepared.cache_dir, rollback)?;
    for skill in &prepared.skills {
        ensure_dir_tracked(&prepared.cache_dir.join(&skill.name), rollback)?;
        for file in &skill.files {
            write_skill_file(file, rollback)?;
        }
    }

    let roots = write_roots(prepared, rollback)?;
    for link in &prepared.links {
        write_link(link, rollback)?;
    }

    let lock = build_lock(prepared, &roots);
    write_lock(&prepared.lock_path, &lock, rollback)?;

    Ok(GlobalSkillsOutcome {
        home_dir: prepared.home_dir.clone(),
        cache_dir: prepared.cache_dir.clone(),
        lock_path: prepared.lock_path.clone(),
        roots,
        skill_names: prepared
            .skills
            .iter()
            .map(|skill| skill.name.clone())
            .collect(),
        link_count: prepared.links.len(),
    })
}

fn prepare_roots(home_dir: &Path) -> Result<Vec<RootPlan>> {
    SUPPORTED_ROOTS
        .iter()
        .map(|root| prepare_root(home_dir, root))
        .collect()
}

fn prepare_root(home_dir: &Path, root: &SupportedRoot) -> Result<RootPlan> {
    let display_path = home_dir.join(root.display_suffix);
    let state = match fs::symlink_metadata(&display_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let resolved = display_path.canonicalize().with_context(|| {
                format!(
                    "global skill root {} is a symlink but does not resolve",
                    display_path.display()
                )
            })?;
            if !resolved.is_dir() {
                bail!(
                    "global skill root {} resolves to {}, which is not a directory",
                    display_path.display(),
                    resolved.display()
                );
            }
            RootState::ExistingSymlink
        }
        Ok(metadata) if metadata.is_dir() => RootState::ExistingDirectory,
        Ok(_) => {
            bail!(
                "refusing global skill install: supported root {} exists and is not a directory; remediation: {REMEDIATION}",
                display_path.display(),
            );
        }
        Err(error) if error.kind() == ErrorKind::NotFound => RootState::Missing,
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect global skill root {}",
                    display_path.display()
                )
            });
        }
    };

    Ok(RootPlan {
        agent: root.agent.to_string(),
        display_path,
        state,
    })
}

fn prepare_cache_skills(
    cache_dir: &Path,
    previous_lock: Option<&GlobalSkillsLock>,
) -> Result<Vec<SkillPlan>> {
    let mut plans = Vec::new();
    for skill in skills() {
        let skill_dir = cache_dir.join(skill.name);
        match fs::symlink_metadata(&skill_dir) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                bail!(
                    "refusing global skill install: cache skill path {} exists and is not a directory; remediation: {REMEDIATION}",
                    skill_dir.display()
                );
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to inspect global skill cache {}",
                        skill_dir.display()
                    )
                });
            }
        }

        let mut file_plans = Vec::new();
        for file in &skill.files {
            let path = skill_dir.join(file.relative_path);
            let hash = sha256_prefixed(file.contents);
            preflight_cache_file(skill.name, file.relative_path, &path, &hash, previous_lock)?;
            file_plans.push(SkillFilePlan {
                relative_path: file.relative_path.to_string(),
                path,
                contents: file.contents,
                hash,
            });
        }

        plans.push(SkillPlan {
            name: skill.name.to_string(),
            version: frontmatter_version(skill.skill_md()),
            files: file_plans,
        });
    }
    Ok(plans)
}

fn preflight_cache_file(
    skill_name: &str,
    relative_path: &str,
    path: &Path,
    expected_hash: &str,
    previous_lock: Option<&GlobalSkillsLock>,
) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            let contents = fs::read(path)
                .with_context(|| format!("failed to read global skill file {}", path.display()))?;
            let actual_hash = sha256_prefixed(&contents);
            if actual_hash == expected_hash
                || previous_lock
                    .and_then(|lock| previous_file_hash(lock, skill_name, relative_path))
                    .is_some_and(|previous_hash| previous_hash == actual_hash)
            {
                return Ok(());
            }

            bail!(
                "refusing global skill install: {} differs from the embedded skill and is not recorded as Maestro-managed in {}; remediation: {REMEDIATION}",
                path.display(),
                path_for_message(previous_lock)
            );
        }
        Ok(_) => {
            bail!(
                "refusing global skill install: cache file path {} exists and is not a regular file; remediation: {REMEDIATION}",
                path.display()
            );
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("failed to inspect global skill file {}", path.display())),
    }
}

fn prepare_links(
    roots: &[RootPlan],
    skills: &[SkillPlan],
    cache_dir: &Path,
) -> Result<Vec<LinkPlan>> {
    let mut links = Vec::new();
    for root in roots {
        for skill in skills {
            let link_path = root.display_path.join(&skill.name);
            let target_path = cache_dir.join(&skill.name);
            preflight_link(&link_path, &target_path)?;
            links.push(LinkPlan {
                agent: root.agent.clone(),
                link_path,
                target_path,
            });
        }
    }
    Ok(links)
}

fn preflight_link(link_path: &Path, target_path: &Path) -> Result<()> {
    match fs::symlink_metadata(link_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let existing_target = fs::read_link(link_path)
                .with_context(|| format!("failed to read symlink {}", link_path.display()))?;
            if existing_target != target_path {
                bail!(
                    "refusing global skill install: {} points to {}, expected {}; remediation: {REMEDIATION}",
                    link_path.display(),
                    existing_target.display(),
                    target_path.display()
                );
            }
            Ok(())
        }
        Ok(_) => {
            bail!(
                "refusing global skill install: {} exists and is not the expected Maestro-managed symlink to {}; remediation: {REMEDIATION}",
                link_path.display(),
                target_path.display()
            );
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to inspect global skill link {}",
                link_path.display()
            )
        }),
    }
}

fn write_skill_file(file: &SkillFilePlan, rollback: &mut Rollback) -> Result<()> {
    ensure_dir_tracked(
        file.path
            .parent()
            .with_context(|| format!("global skill file has no parent: {}", file.path.display()))?,
        rollback,
    )?;
    let previous = match fs::read(&file.path) {
        Ok(contents) if contents == file.contents => return Ok(()),
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| {
                format!("failed to read global skill file {}", file.path.display())
            });
        }
    };

    rollback.track_file_write(file.path.clone(), previous);
    write_atomic(&file.path, file.contents)
        .with_context(|| format!("failed to write global skill file {}", file.path.display()))
}

fn write_roots(
    prepared: &PreparedGlobalSkills,
    rollback: &mut Rollback,
) -> Result<Vec<GlobalSkillRootOutcome>> {
    let mut roots = Vec::new();
    for root in &prepared.roots {
        match root.state {
            RootState::Missing => ensure_dir_tracked(&root.display_path, rollback)?,
            RootState::ExistingDirectory => ensure_dir(&root.display_path)?,
            RootState::ExistingSymlink => {}
        }
        let resolved_path = root.display_path.canonicalize().with_context(|| {
            format!(
                "failed to resolve global skill root {} after creation",
                root.display_path.display()
            )
        })?;
        roots.push(GlobalSkillRootOutcome {
            agent: root.agent.clone(),
            display_path: root.display_path.clone(),
            resolved_path,
        });
    }
    Ok(roots)
}

fn write_link(link: &LinkPlan, rollback: &mut Rollback) -> Result<()> {
    match fs::symlink_metadata(&link.link_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => return Ok(()),
        Ok(_) => {
            bail!(
                "refusing global skill install: {} exists and is not a symlink; remediation: {REMEDIATION}",
                link.link_path.display()
            );
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect global skill link {}",
                    link.link_path.display()
                )
            });
        }
    }

    ensure_dir_tracked(
        link.link_path.parent().with_context(|| {
            format!(
                "global skill link has no parent: {}",
                link.link_path.display()
            )
        })?,
        rollback,
    )?;
    create_directory_symlink(&link.target_path, &link.link_path).with_context(|| {
        format!(
            "failed to create global skill symlink {} -> {}",
            link.link_path.display(),
            link.target_path.display()
        )
    })?;
    rollback.track_created_link(link.link_path.clone());
    Ok(())
}

fn write_lock(lock_path: &Path, lock: &GlobalSkillsLock, rollback: &mut Rollback) -> Result<()> {
    let previous = match fs::read(lock_path) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| {
                format!("failed to read global skills lock {}", lock_path.display())
            });
        }
    };
    let contents = serde_yaml::to_string(lock).context("failed to serialize global skills lock")?;
    if previous.as_deref() == Some(contents.as_bytes()) {
        return Ok(());
    }

    rollback.track_file_write(lock_path.to_path_buf(), previous);
    write_string_atomic(lock_path, &contents)
        .with_context(|| format!("failed to write global skills lock {}", lock_path.display()))
}

fn build_lock(
    prepared: &PreparedGlobalSkills,
    roots: &[GlobalSkillRootOutcome],
) -> GlobalSkillsLock {
    let mut skill_records = BTreeMap::new();
    for skill in &prepared.skills {
        let files = skill
            .files
            .iter()
            .map(|file| (file.relative_path.clone(), file.hash.clone()))
            .collect();
        skill_records.insert(
            skill.name.clone(),
            GlobalSkillRecord {
                version: skill.version.clone(),
                files,
            },
        );
    }

    let root_records = roots
        .iter()
        .map(|root| {
            (
                root.agent.clone(),
                GlobalRootRecord {
                    display_path: root.display_path.clone(),
                    resolved_path: root.resolved_path.clone(),
                },
            )
        })
        .collect();

    let mut link_records = BTreeMap::new();
    for link in &prepared.links {
        let skill = link
            .target_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("invariant: global skill target path ends with a UTF-8 skill name")
            .to_string();
        let resolved_root = roots
            .iter()
            .find(|root| root.agent == link.agent)
            .map(|root| root.resolved_path.clone())
            .expect("invariant: every link belongs to a supported root");
        link_records.insert(
            format!("{}:{}", link.agent, skill),
            GlobalLinkRecord {
                agent: link.agent.clone(),
                skill,
                display_path: link.link_path.clone(),
                target_path: link.target_path.clone(),
                resolved_root,
            },
        );
    }

    GlobalSkillsLock {
        schema_version: GLOBAL_SKILLS_LOCK_SCHEMA_VERSION.to_string(),
        cache_dir: prepared.cache_dir.clone(),
        skills: skill_records,
        roots: root_records,
        links: link_records,
    }
}

fn load_lock_if_exists(path: &Path) -> Result<Option<GlobalSkillsLock>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read global skills lock {}", path.display()));
        }
    };
    let lock: GlobalSkillsLock = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse global skills lock {}", path.display()))?;
    if lock.schema_version != GLOBAL_SKILLS_LOCK_SCHEMA_VERSION {
        bail!(
            "global skills lock {} has schema {}, expected {}",
            path.display(),
            lock.schema_version,
            GLOBAL_SKILLS_LOCK_SCHEMA_VERSION
        );
    }
    Ok(Some(lock))
}

fn previous_file_hash<'a>(
    lock: &'a GlobalSkillsLock,
    skill_name: &str,
    relative_path: &str,
) -> Option<&'a str> {
    lock.skills
        .get(skill_name)?
        .files
        .get(relative_path)
        .map(String::as_str)
}

fn path_for_message(lock: Option<&GlobalSkillsLock>) -> String {
    match lock {
        Some(_) => "the global skills lock".to_string(),
        None => "a missing global skills lock".to_string(),
    }
}

fn ensure_dir_tracked(path: &Path, rollback: &mut Rollback) -> Result<()> {
    let mut missing = Vec::new();
    let mut current = path;
    loop {
        match fs::symlink_metadata(current) {
            Ok(metadata) if metadata.is_dir() => break,
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let resolved = current.canonicalize().with_context(|| {
                    format!("directory symlink {} does not resolve", current.display())
                })?;
                if resolved.is_dir() {
                    break;
                }
                bail!(
                    "refusing global skill install: {} resolves to {}, which is not a directory; remediation: {REMEDIATION}",
                    current.display(),
                    resolved.display()
                );
            }
            Ok(_) => {
                bail!(
                    "refusing global skill install: {} exists and is not a directory; remediation: {REMEDIATION}",
                    current.display()
                );
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                missing.push(current.to_path_buf());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect directory {}", current.display()));
            }
        }

        let Some(parent) = current
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            break;
        };
        current = parent;
    }

    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))?;
    for directory in missing.into_iter().rev() {
        rollback.track_created_dir(directory);
    }
    Ok(())
}

fn home_dir() -> Result<PathBuf> {
    let Some(home) = env::var_os("HOME") else {
        bail!("HOME is not set; cannot install global Maestro skills");
    };
    if home.is_empty() {
        bail!("HOME is empty; cannot install global Maestro skills");
    }
    Ok(PathBuf::from(home))
}

fn cache_dir(home_dir: &Path) -> PathBuf {
    home_dir.join(".maestro").join("skills")
}

fn lock_path(home_dir: &Path) -> PathBuf {
    home_dir.join(".maestro").join(LOCK_FILE_NAME)
}

#[derive(Default)]
struct Rollback {
    file_writes: Vec<(PathBuf, Option<Vec<u8>>)>,
    created_links: Vec<PathBuf>,
    created_dirs: Vec<PathBuf>,
}

impl Rollback {
    fn track_file_write(&mut self, path: PathBuf, previous: Option<Vec<u8>>) {
        self.file_writes.push((path, previous));
    }

    fn track_created_link(&mut self, path: PathBuf) {
        self.created_links.push(path);
    }

    fn track_created_dir(&mut self, path: PathBuf) {
        self.created_dirs.push(path);
    }

    fn rollback(self) -> Result<()> {
        for link in self.created_links.into_iter().rev() {
            remove_file_if_exists(&link)
                .with_context(|| format!("failed to roll back symlink {}", link.display()))?;
        }
        for (path, previous) in self.file_writes.into_iter().rev() {
            match previous {
                Some(contents) => write_atomic(&path, &contents)
                    .with_context(|| format!("failed to restore {}", path.display()))?,
                None => remove_file_if_exists(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?,
            }
        }
        for directory in self.created_dirs.into_iter().rev() {
            match fs::remove_dir(&directory) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty
                    ) => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to remove {}", directory.display()));
                }
            }
        }
        Ok(())
    }
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn sync_writes_global_cache_lock_and_supported_agent_links() {
        let home = temp_home("global-skills-sync");

        let outcome = sync_global_skills_at(&home).expect("global sync should succeed");

        assert!(outcome.cache_dir.join("maestro-task/SKILL.md").is_file());
        assert!(outcome.lock_path.is_file());
        assert_eq!(outcome.roots.len(), 2);
        assert!(home.join(".agents/skills/maestro-task").is_symlink());
        assert!(home.join(".claude/skills/maestro-task").is_symlink());
        assert!(!home.join(".codex/skills/maestro-task").exists());

        let lock = load_lock_if_exists(&outcome.lock_path)
            .expect("lock should load")
            .expect("lock should exist");
        assert_eq!(lock.schema_version, GLOBAL_SKILLS_LOCK_SCHEMA_VERSION);
        assert!(lock.skills.contains_key("maestro-task"));
        assert!(lock.roots.contains_key("codex"));
        assert!(lock.roots.contains_key("claude"));
        assert!(lock.links.contains_key("codex:maestro-task"));
    }

    #[test]
    fn preflight_refuses_user_owned_root_collision_before_writes() {
        let home = temp_home("global-skills-collision");
        fs::create_dir_all(home.join(".agents/skills")).expect("root should be creatable");
        fs::write(home.join(".agents/skills/maestro-task"), "user skill\n")
            .expect("collision should be writable");

        let error = prepare_global_skills_at(&home).expect_err("collision should fail preflight");

        assert!(error.to_string().contains("refusing global skill install"));
        assert!(!home.join(".maestro/skills/maestro-task/SKILL.md").exists());
        assert!(!home.join(".maestro/skills-lock.yaml").exists());
    }

    #[test]
    fn write_failure_rolls_back_new_cache_files_and_links() {
        let home = temp_home("global-skills-rollback");
        let prepared = prepare_global_skills_at(&home).expect("preflight should succeed");
        fs::create_dir_all(home.join(".claude/skills")).expect("root should be creatable");
        fs::write(home.join(".claude/skills/maestro-task"), "late collision\n")
            .expect("late collision should be writable");

        let error =
            write_prepared_global_skills(prepared).expect_err("late collision should fail write");

        assert!(!error.to_string().contains("failed to roll back"));
        assert!(
            !home.join(".maestro/skills/maestro-task/SKILL.md").exists(),
            "new cache files should be rolled back"
        );
        assert!(
            !home.join(".agents/skills/maestro-task").exists(),
            "new codex links should be rolled back"
        );
        assert!(
            !home.join(".maestro/skills-lock.yaml").exists(),
            "new global lock should be rolled back"
        );
        assert_eq!(
            fs::read_to_string(home.join(".claude/skills/maestro-task"))
                .expect("late collision should survive"),
            "late collision\n"
        );
    }

    #[test]
    fn sync_refreshes_managed_drift_but_refuses_unmanaged_cache_edits() {
        let home = temp_home("global-skills-managed-drift");
        sync_global_skills_at(&home).expect("initial sync should succeed");
        let task = home.join(".maestro/skills/maestro-task/SKILL.md");
        fs::write(&task, "user edit\n").expect("skill should be writable");

        let error = prepare_global_skills_at(&home).expect_err("user edit should be refused");

        assert!(
            error
                .to_string()
                .contains("not recorded as Maestro-managed")
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_supported_roots_are_accepted_and_record_resolved_targets() {
        let home = temp_home("global-skills-symlink-root");
        let dotfiles = home.join("dotfiles/agents");
        fs::create_dir_all(dotfiles.join("claude-skills")).expect("target should be creatable");
        fs::create_dir_all(home.join(".claude")).expect("parent should be creatable");
        std::os::unix::fs::symlink(dotfiles.join("claude-skills"), home.join(".claude/skills"))
            .expect("root symlink should be creatable");

        let outcome = sync_global_skills_at(&home).expect("global sync should succeed");

        let claude = outcome
            .roots
            .iter()
            .find(|root| root.agent == "claude")
            .expect("claude root should be recorded");
        assert_eq!(
            claude.resolved_path,
            dotfiles
                .join("claude-skills")
                .canonicalize()
                .expect("target should canonicalize")
        );
        assert!(home.join(".claude/skills/maestro-task").is_symlink());
    }

    fn temp_home(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock should be after epoch")
            .as_nanos();
        let home = env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir(&home).expect("temp home should be creatable");
        home
    }
}
