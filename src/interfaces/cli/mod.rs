use std::env;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

use serde::Serialize;

use crate::domain::feature::FeatureStatus;
use crate::domain::feature::FeatureView;
use crate::domain::feature::handoff_gap;
use crate::domain::feature::read_sidecar_text;
use crate::domain::proof;
use crate::domain::run;
use crate::domain::task::{TaskRecord, TaskState};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;
use crate::interfaces::hooks::record;

pub mod active;
pub mod archive;
pub mod card;
pub mod conflict;
pub mod decision;
pub mod design;
pub mod doctor;
pub mod event;
pub mod feature;
pub mod grep;
pub mod harness;
pub mod hook;
pub mod index;
pub mod init;
pub mod install;
pub mod lean;
pub mod loop_recipes;
pub mod mcp;
pub mod memory;
pub mod migrate;
pub mod mission_control;
pub mod msg;
pub mod playbook;
pub mod qa;
pub mod query;
pub mod reference;
pub mod resume;
pub mod scorer;
pub mod session;
pub mod shell_init;
pub mod status;
pub mod sync;
pub mod task;
mod task_id;
pub mod uninstall;
pub mod update;
pub mod verify;
pub mod version;
pub mod watch;
pub mod worktree;

pub(crate) fn recovery_label(hint: Option<&str>) -> String {
    match hint {
        Some(hint) => format!("fix: {hint}"),
        None => "fix: run maestro doctor".to_string(),
    }
}

pub(crate) fn shell_word(value: &str) -> String {
    if !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b'.' | b'_' | b'-' | b'=' | b':' | b'@')
        })
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Next-step label for a feature, shared by `status` and `feature list` so the
/// hint never diverges between the two surfaces.
pub(crate) fn feature_next_label(paths: &MaestroPaths, view: &FeatureView) -> String {
    feature_next_label_and_command(paths, view).0.to_string()
}

/// Concrete next command for detail surfaces where a compact table label is too
/// easy to misread as a contradictory lifecycle state.
pub(crate) fn feature_next_command(paths: &MaestroPaths, view: &FeatureView) -> String {
    feature_next_label_and_command(paths, view).1
}

fn feature_next_label_and_command(
    paths: &MaestroPaths,
    view: &FeatureView,
) -> (&'static str, String) {
    match view.status {
        FeatureStatus::Proposed if view.acceptance.is_empty() || view.affected_areas.is_empty() => {
            (
                "template: set_contract",
                format!(
                    "maestro feature set {} --acceptance \"<criterion>\" --area \"<surface>\"",
                    view.id
                ),
            )
        }
        FeatureStatus::Proposed if !handoff_is_fresh(paths, view) => (
            "run: finalize_feature",
            format!("maestro feature finalize {}", view.id),
        ),
        FeatureStatus::Proposed if !qa_baseline_present(paths, &view.id) => (
            "template: qa_baseline",
            format!(
                "maestro qa baseline {} --observed \"<current behavior>\"",
                view.id
            ),
        ),
        FeatureStatus::Proposed => (
            "run: accept_feature",
            format!("maestro feature accept {}", view.id),
        ),
        FeatureStatus::Ready => (
            "run: prepare_feature",
            format!("maestro feature prepare {} --draft", view.id),
        ),
        FeatureStatus::InProgress
            if view.counts.total > 0 && view.counts.total == view.counts.verified =>
        {
            (
                "template: close_feature",
                format!("maestro feature close {} --outcome \"<outcome>\"", view.id),
            )
        }
        FeatureStatus::InProgress => (
            "run: resolve_tasks",
            format!("maestro feature show {}", view.id),
        ),
        FeatureStatus::Closed | FeatureStatus::Cancelled => (
            "run: archive_feature",
            format!("maestro feature archive {}", view.id),
        ),
    }
}

fn handoff_is_fresh(paths: &MaestroPaths, view: &FeatureView) -> bool {
    matches!(handoff_gap(paths, &view.id), Ok(None))
}

fn qa_baseline_present(paths: &MaestroPaths, id: &str) -> bool {
    read_sidecar_text(paths, id, "qa.md")
        .ok()
        .flatten()
        .is_some_and(|contents| !contents.trim().is_empty())
}

/// Working-tree git readout shared by `resume` and `status` so the rendered git
/// line and clean-worktree note never diverge between the two surfaces.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct GitReadout {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub code_other_dirty: usize,
    pub maestro_dirty: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divergence: Option<GitDivergenceReadout>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct GitDivergenceReadout {
    pub shared_branch: String,
    pub ahead: usize,
    pub behind: usize,
}

/// Read the working-tree git state, returning `None` (not an error) when the
/// repo is not a git repository so the readout never breaks the caller.
pub(crate) fn git_readout(paths: &MaestroPaths) -> Option<GitReadout> {
    git::snapshot(paths.repo_root())
        .ok()
        .map(|snapshot| GitReadout {
            branch: snapshot.branch,
            code_other_dirty: snapshot.code_other_dirty,
            maestro_dirty: snapshot.maestro_dirty,
            divergence: git::branch_divergence(paths.repo_root())
                .ok()
                .flatten()
                .map(|divergence| GitDivergenceReadout {
                    shared_branch: divergence.shared_branch,
                    ahead: divergence.ahead,
                    behind: divergence.behind,
                }),
        })
}

pub(crate) fn render_git_line(git: &GitReadout) -> String {
    let branch = git.branch.as_deref().unwrap_or("detached");
    format!(
        "git: {branch}, {} code/other + {} maestro-card uncommitted",
        git.code_other_dirty, git.maestro_dirty
    )
}

pub(crate) fn stale_merge_advisory(git: &GitReadout) -> Option<String> {
    let divergence = git.divergence.as_ref()?;
    if divergence.behind == 0 {
        return None;
    }
    Some(format!(
        "[stale] {} moved {} commit{} since this worktree forked; rebase before merging",
        divergence.shared_branch,
        divergence.behind,
        if divergence.behind == 1 { "" } else { "s" }
    ))
}

pub(crate) fn merge_busy_advisory(holder: &str) -> String {
    format!("[merge-busy] {holder} is merging back; wait before rebase+fast-forward")
}

/// The contextual clean-worktree note, shown only when the next verb is
/// close/verify-shaped and uncommitted code/other changes exist.
pub(crate) fn clean_worktree_note(code_other_dirty: usize) -> String {
    format!(
        "note: commit the {code_other_dirty} uncommitted code/other change(s) before the close/verify step so proof reflects the intended tree"
    )
}

/// Shorten a git commit oid to its 7-char display prefix, leaving short
/// sentinels (e.g. `<none>`) intact.
pub(crate) fn short_commit(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// The named repair for a stale proof: a refresh framed as "HEAD moved, likely
/// no code change", naming the commit drift (`old->new`) when the stale reason
/// carries one. Shared so `resume`/`status` and `feature close --dry-run` name
/// the same repair.
pub(crate) fn stale_proof_repair(
    task_id: &str,
    stale_reasons: &[proof::ProofStaleReason],
) -> String {
    match stale_reasons
        .iter()
        .find(|reason| reason.field == "verified_commit")
    {
        Some(reason) => format!(
            "proof: stale ({}->{}); refresh (HEAD moved, likely no code change): maestro task verify {task_id}",
            short_commit(&reason.actual),
            short_commit(&reason.expected),
        ),
        None => format!("proof: stale; refresh: maestro task verify {task_id}"),
    }
}

/// Pure concern-only proof predicate: the actionable proof line for a focal
/// task, or `None` when the proof is in no state the user must act on. A proof
/// is a concern only when it is stale, failed, or missing on a task that still
/// needs verification; an accepted proof, or a missing proof in any other
/// state, is normal and renders nothing.
fn proof_concern_line_from(
    task_id: &str,
    kind: &proof::ProofStatusKind,
    state: &TaskState,
    stale_reasons: &[proof::ProofStaleReason],
) -> Option<String> {
    match kind {
        proof::ProofStatusKind::Stale => Some(stale_proof_repair(task_id, stale_reasons)),
        proof::ProofStatusKind::Failed => Some(format!(
            "proof: failed; fix, then re-verify: maestro task verify {task_id}"
        )),
        proof::ProofStatusKind::Missing if *state == TaskState::NeedsVerification => Some(format!(
            "proof: missing; bind proof: maestro task verify {task_id}"
        )),
        proof::ProofStatusKind::Missing | proof::ProofStatusKind::Accepted => None,
    }
}

/// The concern-only proof line for a focal task on `resume`/`status`, or `None`
/// when there is nothing to act on. Pre-gates on the states where proof can be
/// a concern so unrelated focal tasks never read proof; a soft proof read
/// failure renders no line rather than breaking the surface.
pub(crate) fn proof_concern_line(paths: &MaestroPaths, task: &TaskRecord) -> Option<String> {
    if !matches!(
        task.state,
        TaskState::NeedsVerification | TaskState::Verified
    ) {
        return None;
    }
    let status = proof::proof_status(paths, &task.id).ok()?;
    proof_concern_line_from(&task.id, &status.kind, &task.state, &status.stale_reasons)
}

#[derive(Debug, Parser)]
#[command(
    name = "maestro",
    about = "Local-first agent harness CLI",
    version = env!("MAESTRO_VERSION"),
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: RootCommand,
}

#[derive(Debug, Subcommand)]
pub enum RootCommand {
    #[command(
        about = "Scaffold .maestro/ and extract bundled resources into this repo",
        after_help = "Examples:\n  maestro init                 # scaffold .maestro/ (refuses if it already exists)\n  maestro init --yes           # idempotent: create what's missing, keep local edits\n  maestro init --dry-run       # preview the tree and extraction, write nothing\n  maestro init --force         # overwrite existing files, backing them up first"
    )]
    Init(InitArgs),
    #[command(about = "Install maestro hooks and config for an agent (claude, codex, droid)")]
    Install(AgentArgs),
    #[command(
        about = "Upgrade the maestro binary and refresh bundled resources",
        after_help = "Examples:\n  maestro upgrade               # upgrade to the latest release and refresh resources\n  maestro upgrade --check       # report whether an update is available, install nothing\n  maestro upgrade --force       # reinstall the latest even when already up to date"
    )]
    Upgrade(UpgradeArgs),
    #[command(
        about = "Resync bundled resources to this binary's versions (offline)",
        after_help = "Examples:\n  maestro sync                 # resync repo bundled resources to this binary, preserving edits\n  maestro sync --global-skills # resync user-level Maestro skill cache and links\n  maestro sync --dry-run       # preview the resync, write nothing"
    )]
    Sync(SyncArgs),
    #[command(
        about = "List or initialize a repo-root DESIGN.md from shipped design guides",
        after_help = "Examples:\n  maestro design list\n  maestro design init --dry-run\n  maestro design init --style awesome:voltagent\n  maestro design init --force"
    )]
    Design(DesignArgs),
    #[command(
        hide = true,
        about = "Migrate v1 Maestro artifacts to the reduced v2 layout"
    )]
    MigrateV2,
    #[command(
        hide = true,
        about = "Fold the legacy v2 trees (features/tasks/decisions/backlog) into the card store",
        after_help = "Examples:\n  maestro migrate              # snapshot .maestro, then mint cards from the legacy trees"
    )]
    Migrate,
    #[command(about = "Remove maestro hooks and config for an agent")]
    Uninstall(AgentArgs),
    #[command(about = "Diagnose the maestro installation and report problems")]
    Doctor,
    #[command(about = "Print the shell init snippet for maestro")]
    ShellInit,
    #[command(
        about = "Show the repo's current agent handoff and next action",
        after_help = "Examples:\n  maestro status\n  maestro status --json"
    )]
    Status(StatusArgs),
    #[command(
        about = "Show or run the next safe agent action",
        after_help = "Examples:\n  maestro next\n  maestro next --json\n  maestro next --run\n  maestro next --loop --max-steps 5"
    )]
    Next(NextArgs),
    #[command(
        about = "Print a clean-session resume packet from current repo artifacts",
        after_help = "Examples:\n  maestro resume\n  maestro resume --full\n  maestro resume --handoff --write"
    )]
    Resume(ResumeArgs),
    #[command(about = "Manage tasks: create, claim, complete, verify, and query")]
    Task(TaskArgs),
    #[command(about = "Record run events from the agent harness")]
    Event(EventArgs),
    #[command(about = "Manage features: the product contract and its lifecycle")]
    Feature(FeatureArgs),
    #[command(about = "Record feature QA baseline and slice evidence")]
    Qa(QaArgs),
    #[command(about = "Record passive worktree handoff and cleanup ledger facts")]
    Worktree(WorktreeArgs),
    #[command(
        about = "Search Maestro memory and source with the indexed grep engine",
        after_help = "Examples:\n  maestro grep runtime\n  maestro grep --json 'runtime corpus:memory type:decision'\n  maestro grep 'why agent runtime'"
    )]
    Grep(GrepArgs),
    #[command(
        about = "Manage inspectable Memory candidates and suggestions",
        after_help = "Examples:\n  maestro memory suggest list\n  maestro memory suggest create --source-ref run_event:run-1 --signal-type failure --summary \"remember the fix\"\n  maestro memory create --from msug-abc123"
    )]
    Memory(MemoryArgs),
    #[command(
        about = "Run typed scorer contracts and inspect Memory receipts",
        after_help = "Examples:\n  maestro scorer run mem-abc#gate.scorer_contract\n  maestro scorer list --memory mem-abc\n  maestro scorer show mem-abc#rcpt-123"
    )]
    Scorer(ScorerArgs),
    #[command(about = "Create, show, and list decision cards in the card store")]
    Decision(DecisionArgs),
    #[command(
        about = "Work the card store: ready, list, show, create, update, close, claim, assign, note, dep, archive",
        after_help = "Examples:\n  maestro card ready              # claimable work (ready + unblocked)\n  maestro card show task-0a1b2c\n  maestro card create \"<title>\" -t task"
    )]
    Card(CardArgs),
    #[command(
        hide = true,
        about = "List workable cards with no open blockers (card store)",
        after_help = "Examples:\n  maestro ready                # every unblocked task/bug/chore\n  maestro ready --json\n  maestro ready agent-cli-ux   # only those parented to a feature"
    )]
    Ready(ReadyArgs),
    #[command(
        hide = true,
        about = "List cards filtered by parent, type, assignee, or coarse status (card store)",
        after_help = "Examples:\n  maestro list --parent agent-cli-ux\n  maestro list --json --type bug --status open\n  maestro list --assignee claude#s1"
    )]
    List(ListArgs),
    #[command(
        hide = true,
        about = "Author dependency edges between cards (card store)"
    )]
    Dep(DepArgs),
    #[command(
        about = "Show what other live sessions are doing (cross-session awareness)",
        after_help = "Examples:\n  maestro active               # live sessions, newest first\n  maestro active --all         # include stale sessions beyond the window\n  maestro active release task-abc --reason \"handoff\""
    )]
    Active(ActiveArgs),
    #[command(
        about = "Show the joined activity, lifecycle, and proof story for one session",
        after_help = "Examples:\n  maestro session show 019f180a-fcf7-7891-ae54-59654b1a56ff\n  maestro session show 019f180a-fcf7-7891-ae54-59654b1a56ff --json"
    )]
    Session(SessionArgs),
    #[command(
        about = "Author non-blocking related links between cards (card store)",
        after_help = "Examples:\n  maestro link add task-a task-b\n  maestro link remove task-b task-a"
    )]
    Link(LinkArgs),
    #[command(
        about = "Send and read messages on a linked-card channel (pull-only)",
        after_help = "Examples:\n  maestro msg send task-b \"ready for review\"\n  maestro msg read              # unread across every linked partner\n  maestro msg read task-b       # one partner\n  maestro msg list              # channel overview"
    )]
    Msg(MsgArgs),
    #[command(
        about = "Flag a work conflict on a peer card so it holds off (no link, no git)",
        after_help = "Examples:\n  maestro conflict task-b \"taking login.rs, worktreeing it\"\n  maestro conflict --clear task-b   # retract once merged back"
    )]
    Conflict(ConflictArgs),
    #[command(
        hide = true,
        about = "Archive a feature card and its child cards (card store)",
        after_help = "Examples:\n  maestro archive csv-export   # archives the feature card + every parent=csv-export card\n  maestro archive --loose      # sweeps closed loose tasks/ideas + superseded decisions"
    )]
    Archive(ArchiveArgs),
    #[command(
        hide = true,
        about = "Claim a workable card for this session (card store)",
        after_help = "Examples:\n  maestro claim task-0a1b2c   # take an unclaimed task/bug/chore\n  MAESTRO_SESSION=mine maestro claim task-0a1b2c"
    )]
    Claim(ClaimArgs),
    #[command(
        hide = true,
        about = "Suggest an owner for a workable card (advisory; never blocks a claim)",
        after_help = "Examples:\n  maestro assign task-0a1b2c codex   # advisory routing hint, not a claim\n  maestro assign task-0a1b2c none    # clear the hint\n  maestro assign task-0a1b2c --clear"
    )]
    Assign(AssignArgs),
    #[command(
        hide = true,
        about = "Append a dated note to a card's notes.md (card store)",
        after_help = "Examples:\n  maestro note task-0a1b2c \"chose option B; A breaks on reparent\""
    )]
    Note(NoteArgs),
    #[command(
        hide = true,
        about = "Create a card of any type (card store)",
        after_help = "Examples:\n  maestro create -t task \"Add CSV export\" --parent csv-export\n  maestro create -t bug \"Fix ordering race\"\n  maestro create -t feature \"CSV export\""
    )]
    Create(CreateArgs),
    #[command(
        hide = true,
        about = "Show a card's header, edges, and body (card store)"
    )]
    Show(ShowArgs),
    #[command(
        hide = true,
        about = "Update a card's status, title, description, or claim (card store)",
        after_help = "Examples:\n  maestro update task-add-csv-export-0a1b --status needs_verification\n  maestro update task-add-csv-export-0a1b --claim\n  maestro update task-add-csv-export-0a1b --title \"New title\""
    )]
    Update(UpdateArgs),
    #[command(hide = true, about = "Close a card: status -> closed (card store)")]
    Close(CloseArgs),
    #[command(
        about = "List, show, apply, unapply, dismiss, and measure harness improvement suggestions"
    )]
    Harness(HarnessArgs),
    #[command(about = "Query computed read models (matrix, friction, backlog)")]
    Query(QueryArgs),
    #[command(about = "Maintain the local text index that accelerates list --grep")]
    Index(IndexArgs),
    #[command(about = "Run or inspect the MCP server (serve, tools)")]
    Mcp(McpArgs),
    #[command(about = "Hook entry points invoked by the agent harness")]
    Hook(HookArgs),
    #[command(
        hide = true,
        about = "Read-only Mission Control dashboard (preview, JSON, render-check)",
        after_help = "Examples:\n  maestro mission-control --preview --size 120x40 --format plain\n  maestro mission-control --json\n  maestro mission-control --render-check --size 120x40"
    )]
    MissionControl(MissionControlArgs),
    #[command(
        about = "Live dependency-tree board (bare) or a one-shot snapshot; optional feature-id focuses one feature",
        after_help = "Examples:\n  maestro watch                 # live board, all features with open work\n  maestro watch <feature-id>    # live board focused on one feature\n  maestro watch snapshot        # render one frame and exit\n  maestro watch snapshot <feature-id>"
    )]
    Watch(WatchArgs),
    #[command(hide = true, about = "Verify a task against its recorded proof")]
    Verify { id: Option<String> },
    #[command(
        about = "Print a language code styleguide, or the index with no language",
        after_help = "Examples:\n  maestro playbook            # list the available guides\n  maestro playbook rust       # print the Rust styleguide"
    )]
    Playbook(PlaybookArgs),
    #[command(
        about = "Print loop recipe contracts, route next, or run choose-phase helpers",
        after_help = "Examples:\n  maestro loop                    # list shipped and project custom recipes\n  maestro loop list               # same as above\n  maestro loop show design        # print one lifecycle recipe contract\n  maestro loop show feature-fanout # print one orchestration recipe\n  maestro loop next --json        # recommend the next recipe without writing\n  maestro loop validate design    # validate one structured loop recipe\n  maestro loop work-lease --json  # run the choose-phase helper for one ready card"
    )]
    Loop(LoopArgs),
    #[command(
        about = "Lean reach-ladder tooling: show/set the session strictness mode, emit review/audit guidance, or harvest debt markers",
        after_help = "Examples:\n  maestro lean                 # print the session lean mode\n  maestro lean ultra           # set the session lean mode\n  maestro lean review          # mode-adjusted reach-ladder review guidance\n  maestro lean debt --card     # mint a deduped task card per `// lean:` marker"
    )]
    Lean(LeanArgs),
    #[command(about = "Print the maestro version and binary path")]
    Version,
}

#[derive(Debug, Args)]
#[command(group(
    clap::ArgGroup::new("mode")
        .args(["dry_run", "merge", "force"])
        .multiple(false)
))]
pub struct InitArgs {
    /// Preview the tree and bundled extraction without writing files.
    #[arg(long)]
    pub dry_run: bool,
    /// Keep existing files; create only what is missing.
    #[arg(long)]
    pub merge: bool,
    /// Overwrite existing files, backing them up first.
    #[arg(long)]
    pub force: bool,
    /// Assume yes for non-interactive/scripted runs: with no explicit mode,
    /// behave like `--merge` (keep existing files, create only what is missing,
    /// safe to re-run). Combines with `--merge`/`--force`; an explicit `--force`
    /// still wins.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct AgentArgs {
    /// Agent to target as a positional, e.g. `claude` (defaults to codex).
    #[arg(value_enum, value_name = "AGENT")]
    pub agent_positional: Option<Agent>,
    /// Agent as a flag, e.g. `--agent claude`; cannot be combined with the positional.
    #[arg(
        long = "agent",
        value_enum,
        value_name = "AGENT",
        conflicts_with = "agent_positional"
    )]
    pub agent_flag: Option<Agent>,
}

impl AgentArgs {
    /// Resolve the selected agent from either the positional or `--agent` flag,
    /// defaulting to codex. clap rejects supplying both, so at most one is set.
    pub fn agent(&self) -> Agent {
        self.agent_positional
            .clone()
            .or_else(|| self.agent_flag.clone())
            .unwrap_or(Agent::Codex)
    }
}

#[derive(Debug, Args)]
pub struct UpgradeArgs {
    #[arg(
        long,
        help = "Check for an update without downloading or installing it"
    )]
    pub check: bool,
    #[arg(long, help = "Show extra detail, including the installed binary path")]
    pub verbose: bool,
    #[arg(long, help = "Reinstall even when already on the latest version")]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct SyncArgs {
    /// Preview the resync without writing files.
    #[arg(long)]
    pub dry_run: bool,
    /// Resync the user-level Maestro global skill cache and supported agent skill links.
    #[arg(long = "global-skills")]
    pub global_skills: bool,
}

#[derive(Debug, Args)]
pub struct DesignArgs {
    #[command(subcommand)]
    pub command: DesignCommand,
}

#[derive(Debug, Subcommand)]
pub enum DesignCommand {
    #[command(about = "List shipped DESIGN.md style tokens")]
    List,
    #[command(
        about = "Write repo-root DESIGN.md from a shipped style",
        after_help = "Examples:\n  maestro design init --dry-run\n  maestro design init --style awesome:linear.app\n  maestro design init --force"
    )]
    Init {
        /// Shipped style token. Omit for the neutral non-brand default.
        #[arg(long, value_name = "STYLE")]
        style: Option<String>,
        /// Preview the target, selected style, pin metadata, and overwrite state.
        #[arg(long)]
        dry_run: bool,
        /// Overwrite an existing DESIGN.md after backing it up.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Clone, Debug, ValueEnum)]
pub enum Agent {
    Claude,
    Codex,
    Droid,
}

#[derive(Debug, Args)]
pub struct PlaybookArgs {
    /// Language token to print (e.g. rust, python, html-css); omit for the index.
    #[arg(value_name = "LANGUAGE")]
    pub lang: Option<String>,
}

#[derive(Debug, Args)]
pub struct LoopArgs {
    #[command(subcommand)]
    pub command: Option<LoopCommand>,
}

#[derive(Debug, Subcommand)]
pub enum LoopCommand {
    #[command(about = "List shipped and project custom recipes")]
    List,
    #[command(about = "Recommend the next loop recipe without mutating state")]
    Next(LoopNextArgs),
    #[command(about = "Print one shipped or project custom recipe")]
    Show {
        /// Recipe name (e.g. feature-fanout); run `maestro loop` for the list.
        #[arg(value_name = "NAME")]
        name: String,
    },
    #[command(about = "Validate one structured shipped or project custom loop recipe")]
    Validate {
        /// Recipe name; project custom recipes live under .maestro/loop-recipes/<name>.yml.
        #[arg(value_name = "NAME")]
        name: String,
    },
    #[command(about = "Run the internal Work Lease choose-phase helper and print JSON")]
    WorkLease(Box<WorkLeaseArgs>),
}

#[derive(Debug, Args)]
pub struct LoopNextArgs {
    /// Print machine-readable router JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct WorkLeaseArgs {
    /// Accepted for explicit machine use; output is always JSON.
    #[arg(long)]
    pub json: bool,
    /// Only cards whose stored project scope equals this value.
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
    /// Restrict to cards parented to this feature id (one level).
    #[arg(long, value_name = "FEATURE")]
    pub feature: Option<String>,
    /// Authority record id or prompt/card reference for external ship actions.
    #[arg(long, value_name = "REF")]
    pub authority_ref: Option<String>,
    /// Compact human-readable summary of the granted run-scoped authority.
    #[arg(long, value_name = "SUMMARY")]
    pub authority_summary: Option<String>,
    /// Scope covered by external ship authority, e.g. branch, feature, or repo.
    #[arg(long, value_name = "SCOPE")]
    pub authority_scope: Option<String>,
    /// Target covered by external ship authority, e.g. branch name or release id.
    #[arg(long, value_name = "TARGET")]
    pub authority_target: Option<String>,
    /// External ship action allowed by authority; repeat for each action.
    #[arg(long = "allow-external-action", value_name = "ACTION")]
    pub allow_external_actions: Vec<String>,
    /// Evidence required before using external ship authority; repeatable.
    #[arg(long = "required-evidence", value_name = "EVIDENCE")]
    pub required_evidence: Vec<String>,
    /// Authority expiry timestamp in Maestro event time format.
    #[arg(long, value_name = "TIMESTAMP")]
    pub authority_expires_at: Option<String>,
    /// Hard stop named by the authority; repeatable.
    #[arg(long = "authority-hard-stop", value_name = "STOP")]
    pub authority_hard_stops: Vec<String>,
}

#[derive(Debug, Args)]
pub struct LeanArgs {
    /// Omit to print the session lean mode; lite|full|ultra|off to set it;
    /// review or audit for mode-adjusted guidance; debt to list `// lean:`
    /// markers; help for the modes, verbs, and default-mode env.
    #[arg(value_name = "TARGET")]
    pub target: Option<String>,
    /// With `debt`: mint a deduped task card per marker.
    #[arg(long)]
    pub card: bool,
}

#[derive(Debug, Args)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommand,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    #[arg(long, help = "Print machine-readable status JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct NextArgs {
    #[arg(long, help = "Print machine-readable next-action JSON")]
    pub json: bool,
    #[arg(long, conflicts_with = "loop_mode", help = "Run one auto-safe action")]
    pub run: bool,
    #[arg(
        long = "loop",
        conflicts_with = "run",
        help = "Run auto-safe actions until blocked"
    )]
    pub loop_mode: bool,
    #[arg(long, default_value_t = 10, help = "Maximum actions for --loop")]
    pub max_steps: usize,
}

#[derive(Debug, Args)]
pub struct ActiveArgs {
    #[command(subcommand)]
    pub command: Option<ActiveCommand>,
    /// Include stale sessions (last event beyond the live window), hidden by default.
    #[arg(long)]
    pub all: bool,
    /// Print explicit link/message/conflict suggestions without mutating links.
    #[arg(long)]
    pub connect: bool,
}

#[derive(Debug, Subcommand)]
pub enum ActiveCommand {
    #[command(about = "Release this session's ownership of a card without changing card status")]
    Release(ActiveReleaseArgs),
}

#[derive(Debug, Args)]
pub struct SessionArgs {
    #[command(subcommand)]
    pub command: SessionCommand,
}

#[derive(Debug, Subcommand)]
pub enum SessionCommand {
    #[command(about = "Show a joined readout for one logical session id")]
    Show(SessionShowArgs),
}

#[derive(Debug, Args)]
pub struct SessionShowArgs {
    /// Logical session id / run bucket to inspect.
    #[arg(value_name = "SESSION_ID")]
    pub session_id: String,
    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ActiveReleaseArgs {
    pub card_id: String,
    #[arg(long, help = "Why this session is yielding the card")]
    pub reason: String,
}

#[derive(Debug, Args)]
pub struct WorktreeArgs {
    #[command(subcommand)]
    pub command: WorktreeCommand,
}

#[derive(Debug, Subcommand)]
pub enum WorktreeCommand {
    #[command(about = "Plan a worker lane in the owning card ledger; does not run git")]
    Plan {
        card: String,
        #[arg(long)]
        slug: String,
        #[arg(long)]
        branch: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        base: String,
        #[arg(long = "owner-checkout")]
        owner_checkout: Option<String>,
        #[arg(long = "worker-checkout")]
        worker_checkout: Option<String>,
    },
    #[command(
        about = "Mark a worktree ledger milestone; does not run git",
        group(
            clap::ArgGroup::new("milestone")
                .args(["lane_created", "merged_back", "verified"])
                .required(true)
                .multiple(false)
        )
    )]
    Mark {
        card: String,
        #[arg(long)]
        slug: String,
        #[arg(long = "lane-created")]
        lane_created: bool,
        #[arg(long = "merged-back")]
        merged_back: bool,
        #[arg(long)]
        verified: bool,
        #[arg(long)]
        commit: Option<String>,
    },
    #[command(about = "Record that the agent already cleaned a worker lane; does not run git")]
    CleanupRecord {
        card: String,
        #[arg(long)]
        slug: String,
        #[arg(long = "removed-path")]
        removed_path: String,
        #[arg(long = "deleted-branch")]
        deleted_branch: String,
        #[arg(long)]
        pruned: bool,
        #[arg(long = "recorded-by")]
        recorded_by: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct CardArgs {
    #[command(subcommand)]
    pub command: CardCommand,
}

/// The flat card-store verbs again under `maestro card <verb>`, sharing the
/// flat spellings' arg structs and handlers so output stays byte-identical.
#[derive(Debug, Subcommand)]
pub enum CardCommand {
    #[command(about = "List workable cards with no open blockers")]
    Ready(ReadyArgs),
    #[command(about = "List cards filtered by parent, type, assignee, or coarse status")]
    List(ListArgs),
    #[command(about = "Author dependency edges between cards")]
    Dep(DepArgs),
    #[command(about = "Archive a feature card and its child cards")]
    Archive(CardArchiveArgs),
    #[command(about = "Claim a workable card for this session")]
    Claim(ClaimArgs),
    #[command(about = "Suggest an owner for a workable card (advisory; never blocks a claim)")]
    Assign(AssignArgs),
    #[command(about = "Append a dated note to a card's notes.md")]
    Note(NoteArgs),
    #[command(about = "Create a card of any type")]
    Create(CreateArgs),
    #[command(about = "Prepare a card container into owned tasks")]
    Prepare(CardPrepareArgs),
    #[command(about = "Show a card's header, edges, and body")]
    Show(ShowArgs),
    #[command(about = "Update a card's status, title, description, or claim")]
    Update(UpdateArgs),
    #[command(about = "Close a card: status -> closed")]
    Close(CloseArgs),
    #[command(
        about = "Walk a card's typed edges (parent/blocks/related/supersedes)",
        after_help = "Examples:\n  maestro card graph task-0a1b2c        # connected cards, two hops\n  maestro card graph --dot > cards.dot  # the whole web as Graphviz DOT\n  maestro card graph task-0a1b2c --dot  # one card's connected component"
    )]
    Graph {
        /// Card id to walk from; omit with --dot to export the whole web.
        id: Option<String>,
        /// Emit Graphviz DOT instead of a tree.
        #[arg(long)]
        dot: bool,
    },
}

#[derive(Debug, Args)]
pub struct ReadyArgs {
    /// Print machine-readable ready JSON.
    #[arg(long)]
    pub json: bool,
    /// Only cards whose stored project scope equals this value.
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
    /// Restrict to cards parented to this feature id (one level).
    #[arg(value_name = "FEATURE")]
    pub feature: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Only cards whose parent is this card id.
    #[arg(long, value_name = "PARENT")]
    pub parent: Option<String>,
    /// Only cards of this type (feature, custom, progress, memory, task, bug, chore, idea, decision).
    #[arg(long = "type", value_name = "TYPE")]
    pub card_type: Option<String>,
    /// Only cards whose advisory assignee hint OR actual claim is this agent or
    /// full `<agent>#<session>` token.
    #[arg(long, value_name = "ASSIGNEE")]
    pub assignee: Option<String>,
    /// Only cards in this coarse status (open, in_progress, closed).
    #[arg(long, value_name = "STATUS")]
    pub status: Option<String>,
    /// Only cards whose stored project scope equals this value.
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
    /// Only cards whose title, description, or notes.md/spec.md sidecars
    /// contain this case-insensitive substring.
    #[arg(long, value_name = "TERM")]
    pub grep: Option<String>,
    /// Include archived cards (rows marked archived).
    #[arg(long)]
    pub archived: bool,
    /// Show all cards, including coarse-closed ones the bare list hides.
    #[arg(long)]
    pub all: bool,
    /// Print machine-readable list JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
#[command(group(
    clap::ArgGroup::new("resume_target")
        .args(["task", "feature"])
        .multiple(false)
))]
pub struct ResumeArgs {
    #[arg(long, value_name = "TASK_ID", help = "Resume from this task")]
    pub task: Option<String>,
    #[arg(long, value_name = "FEATURE_ID", help = "Resume from this feature")]
    pub feature: Option<String>,
    #[arg(long, help = "Include fuller source-backed handoff context")]
    pub full: bool,
    #[arg(long, help = "Include handoff context and suggested prompt text")]
    pub handoff: bool,
    #[arg(long, help = "Write the resume packet as an explicit artifact")]
    pub write: bool,
    #[arg(long, help = "Print machine-readable resume JSON")]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    #[command(about = "Add a low-ceremony task ready to start")]
    Add {
        title: String,
        #[arg(long, help = "Attach this simple task to a Chore card")]
        card: Option<String>,
        #[arg(long, help = "Project/service scope stored on the card")]
        project: Option<String>,
        #[arg(long, help = "Print only the new task id on stdout")]
        id_only: bool,
    },
    #[command(about = "Create a task (-> draft)")]
    Create {
        title: String,
        #[arg(long)]
        feature: Option<String>,
        #[arg(
            long,
            conflicts_with = "feature",
            help = "Attach this task to a card container"
        )]
        card: Option<String>,
        #[arg(long)]
        lane: Option<String>,
        #[arg(long)]
        risk: Option<String>,
        #[arg(
            long = "check",
            help = "Acceptance check (repeatable); seeds the task's verify+ contract"
        )]
        check: Vec<String>,
        #[arg(
            long = "covers",
            help = "Feature acceptance id this task covers, e.g. ac-1 (repeatable)"
        )]
        covers: Vec<String>,
        #[arg(long, help = "Project/service scope stored on the card")]
        project: Option<String>,
        #[arg(long, help = "Print only the new card id on stdout")]
        id_only: bool,
    },
    #[command(about = "Author task checks or change its feature link")]
    Set {
        id: String,
        #[arg(
            long = "check",
            help = "Acceptance check (repeatable); replaces the task's current checks"
        )]
        check: Vec<String>,
        #[arg(long, help = "Attach or move the task to this feature id")]
        feature: Option<String>,
        #[arg(
            long = "no-feature",
            conflicts_with = "feature",
            help = "Detach the task from its feature"
        )]
        no_feature: bool,
        #[arg(
            long = "covers",
            help = "Feature acceptance id this task covers, e.g. ac-1 (repeatable)"
        )]
        covers: Vec<String>,
        #[arg(
            long = "verify-command",
            help = "Per-task narrow falsifier; when set, `task verify` runs ONLY this instead of the repo-global stack.verify"
        )]
        verify_command: Option<String>,
        #[arg(
            long = "clear-verify-command",
            conflicts_with = "verify_command",
            help = "Clear the per-task verify command (revert to the repo-global stack.verify)"
        )]
        clear_verify_command: bool,
    },
    #[command(about = "Move a draft into exploring (-> exploring)")]
    Explore { id: String },
    #[command(about = "Lock acceptance and mark the task ready (-> ready)")]
    Accept { id: String },
    #[command(about = "Claim a ready, unblocked task to work on it (-> in_progress)")]
    Claim {
        id: Option<String>,
        #[arg(long, help = "Claim the next safe ready task")]
        next: bool,
    },
    #[command(alias = "begin", about = "Start a ready task (alias for claim)")]
    Start {
        #[arg(value_name = "REF_OR_ID")]
        id: String,
    },
    #[command(about = "Mark a low-ceremony task done when it has no explicit gate")]
    Done {
        #[arg(value_name = "REF_OR_ID")]
        id: String,
        #[arg(long, help = "Completion summary stored in task history")]
        summary: Option<String>,
        #[arg(
            long,
            required = true,
            help = "Observed proof text for the simple completion"
        )]
        proof: Vec<String>,
    },
    #[command(about = "Submit work for verification (-> needs_verification)")]
    Complete {
        id: String,
        #[arg(long)]
        summary: String,
        #[arg(
            long,
            required = true,
            help = "Completion claim (repeatable); hook-backed tool proof uses '<tool> <tool_input_hash>'"
        )]
        claim: Vec<String>,
        #[arg(
            long,
            help = "Observed proof text to record before automatic verification (repeatable)"
        )]
        proof: Vec<String>,
    },
    #[command(about = "Run the evidence gate; on pass marks the task verified")]
    Verify { id: Option<String> },
    #[command(about = "Print the next task action for the current repo")]
    Next {
        #[arg(long, help = "Print machine-readable next-action JSON")]
        json: bool,
    },
    #[command(about = "Append a dated note to a task's notes.md")]
    Note { id: String, text: String },
    #[command(about = "Record progress (summary and/or claims) without changing state")]
    Update {
        id: String,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long, help = "Progress claim (repeatable)")]
        claim: Vec<String>,
    },
    #[command(about = "Add a blocker to a task")]
    Block {
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        by: Option<String>,
    },
    #[command(about = "Resolve a blocker by its blk- id")]
    Unblock {
        id: String,
        #[arg(long)]
        blocker: String,
    },
    #[command(about = "Terminally reject a task (-> rejected)")]
    Reject {
        id: String,
        #[arg(long)]
        reason: String,
    },
    #[command(about = "Terminally abandon a task (-> abandoned)")]
    Abandon {
        id: String,
        #[arg(long)]
        reason: String,
    },
    #[command(about = "Replace a task with another (-> superseded)")]
    Supersede {
        id: String,
        #[arg(long)]
        by: String,
        #[arg(long)]
        reason: String,
    },
    #[command(about = "Show a task's detail: state, claim, blockers")]
    Show {
        #[arg(value_name = "REF_OR_ID")]
        id: Option<String>,
    },
    #[command(about = "List tasks, with optional filters")]
    List {
        #[arg(long)]
        blocked: bool,
        #[arg(long)]
        blocked_by: Option<String>,
        #[arg(long)]
        blocks: Option<String>,
        #[arg(long)]
        feature: Option<String>,
        #[arg(long)]
        ready: bool,
        #[arg(long, help = "Only tasks claimed by this actor")]
        mine: bool,
        #[arg(
            long,
            help = "Include terminal/done tasks (verified, rejected, abandoned, superseded) and archived ones"
        )]
        all: bool,
        #[arg(long, help = "Print machine-readable task list JSON")]
        json: bool,
        #[arg(long, hide = true)]
        watch: bool,
        #[arg(long)]
        interval: Option<u64>,
    },
    #[command(about = "Watch tasks live, refreshing on an interval")]
    Watch {
        id: Option<String>,
        #[arg(long)]
        interval: Option<u64>,
    },
    #[command(about = "Show a task's proof status")]
    Proof {
        task_id: Option<String>,
        #[arg(long = "task-id", value_name = "TASK_ID")]
        task_id_flag: Option<String>,
    },
    #[command(about = "Check the task blocker graph for cycles and dangling refs")]
    Doctor,
    #[command(
        hide = true,
        about = "Archive a done task out of the live scan (-> .maestro/archive/tasks)"
    )]
    Archive {
        id: String,
        #[arg(long, help = "Preview the move without archiving")]
        dry_run: bool,
    },
    #[command(hide = true, about = "Restore an archived task to the live scan")]
    Unarchive { id: String },
}

#[derive(Debug, Args)]
pub struct EventArgs {
    #[command(subcommand)]
    pub command: EventCommand,
}

#[derive(Debug, Subcommand)]
pub enum EventCommand {
    #[command(about = "Record a run event, optionally bound to a task and carrying claims")]
    Create {
        #[arg(long, help = "Bind the event to this task id")]
        task_id: Option<String>,
        #[arg(long, help = "Human-readable event message")]
        message: Option<String>,
        #[arg(long, help = "Raw JSON payload to attach to the event")]
        payload: Option<String>,
        #[arg(long, help = "Completion claim recorded as task proof (repeatable)")]
        claim: Vec<String>,
        #[arg(long, help = "Run label grouping the event")]
        run: Option<String>,
    },
    #[command(about = "Record an explicit human correction/intervention event")]
    Intervention {
        #[arg(long, help = "What the agent got wrong")]
        note: String,
        #[arg(long, help = "Stable topic slug for clustering repeated corrections")]
        topic: Option<String>,
        #[arg(long, help = "Run label grouping the event")]
        run: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct FeatureArgs {
    #[command(subcommand)]
    pub command: FeatureCommand,
}

#[derive(Debug, Args)]
pub struct QaArgs {
    #[command(subcommand)]
    pub command: QaCommand,
}

#[derive(Debug, Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    #[command(about = "Create a Memory card from an explicit source or suggestion id")]
    Create {
        #[arg(long = "from", value_name = "SOURCE_OR_SUGGESTION")]
        from: String,
        #[arg(
            long,
            help = "Lesson summary; required when --from is not a suggestion id"
        )]
        summary: Option<String>,
        #[arg(long, help = "Full lesson text; defaults to the summary")]
        lesson: Option<String>,
        #[arg(long = "signal-type", help = "Signal type for explicit source refs")]
        signal_type: Option<String>,
        #[arg(long = "scope-kind", default_value = "repo")]
        scope_kind: String,
        #[arg(long = "scope-ref")]
        scope_ref: Vec<String>,
        #[arg(long = "target-surface", default_value = "memory_note")]
        target_surface: String,
        #[arg(long, help = "Print only the new Memory card id")]
        id_only: bool,
    },
    #[command(about = "List Memory cards")]
    List {
        #[arg(long, help = "Include non-promoted Memory cards")]
        all: bool,
    },
    #[command(about = "Show a Memory card, candidate metadata, and lesson")]
    Show { id: String },
    #[command(about = "Search approved Memory")]
    Search {
        #[arg(value_name = "QUERY")]
        query: Vec<String>,
    },
    #[command(about = "Plan or apply a gated Memory promotion")]
    Promote {
        #[arg(value_name = "MEMORY_OR_PROMOTION_ID")]
        id: String,
        #[arg(long, conflicts_with = "apply", help = "Create a promotion plan")]
        plan: bool,
        #[arg(
            long,
            conflicts_with = "plan",
            help = "Apply an existing promotion plan"
        )]
        apply: bool,
        #[arg(long = "scorer-receipt", value_name = "REF")]
        scorer_receipt: Option<String>,
        #[arg(long = "review-evidence", value_name = "EVIDENCE")]
        review_evidence: Option<String>,
    },
    #[command(about = "Create a bounded Memory maintenance contract")]
    Maintain(MemoryMaintainArgs),
    #[command(about = "Create a bounded Memory-dream maintenance contract")]
    Dream(MemoryMaintainArgs),
    #[command(about = "Attach scorer contracts to Memory candidates")]
    Scorer(MemoryScorerArgs),
    #[command(about = "List, create, and dismiss visible Memory suggestions")]
    Suggest(MemorySuggestArgs),
}

#[derive(Debug, Args)]
pub struct MemoryMaintainArgs {
    #[arg(long, value_name = "L0|L1|L2|L3")]
    pub level: Option<String>,
    #[arg(long = "scope-kind", default_value = "repo")]
    pub scope_kind: String,
    #[arg(long = "scope-ref")]
    pub scope_ref: Vec<String>,
    #[arg(long = "source-ref")]
    pub source_ref: Vec<String>,
    #[arg(long)]
    pub reason: String,
    #[arg(long = "proof-link")]
    pub proof_link: Vec<String>,
    #[arg(long = "run-link")]
    pub run_link: Vec<String>,
    #[arg(long = "human-approved")]
    pub human_approved: bool,
    #[arg(long)]
    pub tokens: Option<u64>,
    #[arg(long = "wall-minutes")]
    pub wall_minutes: Option<u64>,
    #[arg(long = "max-source-refs")]
    pub max_source_refs: Option<u64>,
    #[arg(long = "max-files")]
    pub max_files: Option<u64>,
    #[arg(long)]
    pub subagents: Option<u64>,
    #[arg(long, help = "Print only the maintenance id")]
    pub id_only: bool,
}

#[derive(Debug, Args)]
pub struct MemoryScorerArgs {
    #[command(subcommand)]
    pub command: MemoryScorerCommand,
}

#[derive(Debug, Subcommand)]
pub enum MemoryScorerCommand {
    #[command(about = "Attach an inline scorer contract file to memory/candidate.yml")]
    Attach {
        id: String,
        #[arg(long = "contract-file", value_name = "PATH")]
        contract_file: PathBuf,
    },
}

#[derive(Debug, Args)]
pub struct MemorySuggestArgs {
    #[command(subcommand)]
    pub command: MemorySuggestCommand,
}

#[derive(Debug, Subcommand)]
pub enum MemorySuggestCommand {
    #[command(about = "List Memory suggestions")]
    List {
        #[arg(long, help = "Include dismissed, created, and expired suggestions")]
        all: bool,
    },
    #[command(about = "Create or upsert a visible Memory suggestion")]
    Create {
        #[arg(
            long = "source-ref",
            help = "Source ref as kind:id or kind:path=<path>"
        )]
        source_ref: Vec<String>,
        #[arg(long = "signal-type")]
        signal_type: String,
        #[arg(long)]
        summary: String,
        #[arg(long = "scope-kind", default_value = "repo")]
        scope_kind: String,
        #[arg(long = "scope-ref")]
        scope_ref: Vec<String>,
        #[arg(long = "target-surface", default_value = "memory_note")]
        target_surface: String,
        #[arg(long = "dedupe-key")]
        dedupe_key: Option<String>,
        #[arg(long = "expires-at")]
        expires_at: Option<String>,
    },
    #[command(about = "Dismiss an open Memory suggestion")]
    Dismiss {
        id: String,
        #[arg(long)]
        reason: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum QaCommand {
    #[command(about = "Write a feature QA baseline from explicit observed behavior")]
    Baseline {
        id: String,
        #[arg(
            long,
            allow_hyphen_values = true,
            help = "Observed current behavior for the baseline scenario"
        )]
        observed: Option<String>,
        #[arg(
            long = "observed-file",
            value_name = "PATH",
            help = "Read observed baseline evidence from a file"
        )]
        observed_file: Option<PathBuf>,
        #[arg(
            long = "observed-stdin",
            help = "Read observed baseline evidence from stdin"
        )]
        observed_stdin: bool,
    },
    #[command(about = "Append counting QA slice evidence for baseline scenarios")]
    Slice {
        id: String,
        #[arg(long = "scenario", help = "Baseline scenario id, e.g. bl-001")]
        scenario: Vec<String>,
        #[arg(long, allow_hyphen_values = true, help = "Observed slice evidence")]
        observed: Option<String>,
        #[arg(
            long = "observed-file",
            value_name = "PATH",
            help = "Read observed slice evidence from a file"
        )]
        observed_file: Option<PathBuf>,
        #[arg(
            long = "observed-stdin",
            help = "Read observed slice evidence from stdin"
        )]
        observed_stdin: bool,
    },
}

#[derive(Debug, Args)]
pub struct ScorerArgs {
    #[command(subcommand)]
    pub command: ScorerCommand,
}

#[derive(Debug, Subcommand)]
pub enum ScorerCommand {
    #[command(about = "Run a typed scorer contract reference")]
    Run { contract_ref: String },
    #[command(about = "Show one scorer receipt by memory-id#receipt-id")]
    Show { receipt_ref: String },
    #[command(about = "List scorer receipts for a Memory card")]
    List {
        #[arg(long = "memory", value_name = "MEMORY_ID")]
        memory: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum FeatureCommand {
    #[command(about = "Propose a new feature (-> proposed)")]
    New {
        title: String,
        #[arg(long, help = "Set the initial description")]
        description: Option<String>,
        #[arg(long = "question", help = "Initial open question (repeatable)")]
        question: Vec<String>,
        #[arg(long, help = "Project/service scope stored on the card")]
        project: Option<String>,
        #[arg(long, help = "Print only the new card id on stdout")]
        id_only: bool,
    },
    #[command(about = "Author a proposed feature's contract (replace or append fields)")]
    Set {
        id: String,
        #[arg(
            long = "acceptance",
            help = "Acceptance criterion (repeatable); replaces the current acceptance list"
        )]
        acceptance: Vec<String>,
        #[arg(
            long = "area",
            help = "Affected area (repeatable); replaces the current areas list"
        )]
        area: Vec<String>,
        #[arg(
            long = "non-goal",
            help = "Non-goal (repeatable); replaces the current non-goals list"
        )]
        non_goal: Vec<String>,
        #[arg(
            long = "question",
            help = "Open question (repeatable; REPLACES the full questions list; repeat to keep existing questions)"
        )]
        question: Vec<String>,
        #[arg(
            long = "clear-questions",
            hide = true,
            help = "Clear all open questions (with --question, clear then set the passed list)"
        )]
        clear_questions: bool,
        #[arg(
            long = "add-acceptance",
            hide = true,
            help = "Acceptance criterion to append while proposed (repeatable)"
        )]
        add_acceptance: Vec<String>,
        #[arg(
            long = "add-area",
            hide = true,
            help = "Affected area to append while proposed (repeatable)"
        )]
        add_area: Vec<String>,
        #[arg(
            long = "add-non-goal",
            hide = true,
            help = "Non-goal to append while proposed (repeatable)"
        )]
        add_non_goal: Vec<String>,
        #[arg(
            long = "add-question",
            hide = true,
            help = "Open question to append while proposed (repeatable)"
        )]
        add_question: Vec<String>,
        #[arg(
            long = "edit-acceptance",
            value_name = "AC_ID",
            hide = true,
            help = "Acceptance id to edit in place, paired by index with --text"
        )]
        edit_acceptance: Vec<String>,
        #[arg(
            long = "text",
            value_name = "TEXT",
            hide = true,
            help = "Replacement acceptance text, paired by index with --edit-acceptance"
        )]
        text: Vec<String>,
        #[arg(long, help = "Replace the description")]
        description: Option<String>,
        #[arg(long, help = "Replace the raw request")]
        request: Option<String>,
        #[arg(
            long = "type",
            help = "Replace the input type (e.g. bug_report, refactor)"
        )]
        input_type: Option<String>,
    },
    #[command(about = "Write or refresh the clean design handoff before accept/prepare")]
    Finalize { id: String },
    #[command(about = "Reopen a DB-backed finalized feature into .maestro/workbench/<id>")]
    Reopen { id: String },
    #[command(about = "Report or apply feature contract reconciliation before finalize")]
    Reconcile {
        id: String,
        #[arg(long, help = "Print full human-readable reconciliation context")]
        full: bool,
        #[arg(long, help = "Emit the full agent-readable reconciliation JSON")]
        json: bool,
        #[arg(
            long = "apply-plan",
            value_name = "PLAN_FILE",
            help = "Apply an explicit full-contract reconcile.yml plan"
        )]
        apply_plan: Option<PathBuf>,
    },
    #[command(about = "Accept a feature into ready, freezing its contract (-> ready; gated)")]
    Accept {
        id: String,
        #[arg(
            long = "qa",
            value_name = "SURFACE",
            help = "QA declaration; pass `none` only for no behavioral surface"
        )]
        qa: Option<String>,
        #[arg(long = "reason", help = "Reason required with `--qa none`")]
        reason: Option<String>,
        #[arg(long, help = "Preview the accept gate without transitioning")]
        dry_run: bool,
    },
    #[command(about = "Prepare an accepted feature into a ready implementation queue")]
    Prepare {
        id: String,
        #[arg(
            long = "from",
            value_name = "PLAN_FILE",
            help = "Read explicit task plan file"
        )]
        from: Option<PathBuf>,
        #[arg(
            long,
            conflicts_with = "from",
            help = "Create or point to the feature prepare draft file"
        )]
        draft: bool,
        #[arg(
            long = "task",
            conflicts_with_all = ["from", "draft"],
            help = "Structured task entry, e.g. T1: Add parser"
        )]
        task: Vec<String>,
        #[arg(
            long = "check",
            requires = "task",
            help = "Task check for --task entries"
        )]
        check: Vec<String>,
        #[arg(
            long = "covers",
            requires = "task",
            help = "Acceptance id covered by --task"
        )]
        covers: Vec<String>,
        #[arg(long = "blocker", requires = "task", help = "Blocker line for --task")]
        blocker: Vec<String>,
        #[arg(long = "after", requires = "task", help = "Dependency ref for --task")]
        after: Vec<String>,
    },
    #[command(about = "Grow a frozen contract additively with an audit reason (ready/in_progress)")]
    Amend {
        id: String,
        #[arg(
            long = "add-acceptance",
            help = "Acceptance criterion to add (repeatable)"
        )]
        add_acceptance: Vec<String>,
        #[arg(long = "add-area", help = "Affected area to add (repeatable)")]
        add_area: Vec<String>,
        #[arg(long = "add-non-goal", help = "Non-goal to add (repeatable)")]
        add_non_goal: Vec<String>,
        #[arg(long = "add-question", help = "Open question to add (repeatable)")]
        add_question: Vec<String>,
        #[arg(long, help = "Why the contract is growing (required, audited)")]
        reason: String,
    },
    #[command(about = "Start work on a ready feature (-> in_progress)")]
    Start { id: String },
    #[command(about = "Sweep or record proof for a feature's acceptance contract")]
    Verify {
        id: String,
        #[arg(
            long,
            value_name = "AC_ID",
            help = "Acceptance id to prove explicitly (repeatable)"
        )]
        prove: Vec<String>,
        #[arg(long, help = "Observed evidence for --prove (repeatable)")]
        evidence: Vec<String>,
        #[arg(
            long,
            value_name = "AC_ID",
            help = "Acceptance id to waive (repeatable)"
        )]
        waive: Vec<String>,
        #[arg(long, help = "Reason required with --waive (repeatable)")]
        reason: Vec<String>,
        #[arg(
            long,
            help = "Record the proof but suppress the auto-close when this verify completes close-readiness (defer to explicit `feature close`)"
        )]
        no_close: bool,
        #[arg(
            long,
            help = "Outcome line for the auto-close (overrides the generated default; ignored unless this verify auto-closes)"
        )]
        outcome: Option<String>,
    },
    #[command(about = "Record or waive feature acceptance proof with explicit input")]
    Proof {
        #[command(subcommand)]
        command: FeatureProofCommand,
    },
    #[command(about = "Append a dated note to a feature's notes.md")]
    Note { id: String, text: String },
    #[command(about = "Close an in-progress feature (-> closed; gated)")]
    Close {
        id: String,
        #[arg(
            long,
            help = "One-line outcome recorded on the feature, shown in `feature list --all`"
        )]
        outcome: Option<String>,
        #[arg(long, help = "Preview the close gate without transitioning")]
        dry_run: bool,
    },
    #[command(
        about = "Cancel a non-terminal feature, abandoning its live child tasks (-> cancelled)"
    )]
    Cancel {
        id: String,
        #[arg(long, help = "Why the feature is being cancelled (required, audited)")]
        reason: String,
        #[arg(long, help = "Preview the cancel and the child tasks it would abandon")]
        dry_run: bool,
    },
    #[command(about = "Show a feature's status, full contract, and task counts")]
    Show { id: String },
    #[command(
        about = "Render a feature's spec-of-record, or fill one section (--section with --append/--replace)"
    )]
    Spec {
        id: String,
        #[arg(long, help = "Spec section to write, e.g. \"Current state\"")]
        section: Option<String>,
        #[arg(long, help = "Append text to the section body", value_name = "TEXT")]
        append: Option<String>,
        #[arg(
            long,
            help = "Replace the section body with the text",
            value_name = "TEXT",
            conflicts_with = "append"
        )]
        replace: Option<String>,
    },
    #[command(about = "List features with their statuses and task counts")]
    List {
        #[arg(
            long,
            help = "Include terminal features (closed, cancelled) and archived ones"
        )]
        all: bool,
    },
    #[command(
        about = "Archive a terminal feature and its terminal child tasks (-> .maestro/archive/features)"
    )]
    Archive {
        #[arg(help = "Feature id to archive (omit when using --closed)")]
        id: Option<String>,
        #[arg(
            long,
            help = "Archive every closed feature (closed or cancelled; mutually exclusive with <id>)"
        )]
        closed: bool,
        #[arg(
            long,
            help = "Preview the feature and child-task moves without archiving"
        )]
        dry_run: bool,
    },
    #[command(about = "Archive a terminal feature after commit-bound QA evidence passes")]
    AutoArchive {
        #[arg(help = "Terminal feature id to archive")]
        id: String,
        #[arg(
            long = "authority-ref",
            help = "Durable user/SPEC/run authority reference"
        )]
        authority_ref: String,
        #[arg(long = "authority-target", help = "Feature id scoped by the authority")]
        authority_target: String,
        #[arg(
            long = "authority-head",
            help = "Full Git HEAD SHA scoped by the authority"
        )]
        authority_head: String,
        #[arg(
            long = "authority-state",
            help = "Current authority state; must be current"
        )]
        authority_state: String,
        #[arg(long = "tested-head", help = "Full Git HEAD SHA that passed QA")]
        tested_head: String,
        #[arg(long = "qa-result", help = "QA verdict; must be pass/passed")]
        qa_result: String,
        #[arg(
            long = "qa-evidence",
            help = "QA evidence item naming command, exit status, timestamp, and tested head (repeatable)"
        )]
        qa_evidence: Vec<String>,
        #[arg(long, help = "Run id that receives the auto_archive event")]
        run: String,
        #[arg(
            long = "multi-agent",
            help = "Disposition of multi-agent/worktree work, e.g. none or merged back into target HEAD"
        )]
        multi_agent: String,
        #[arg(
            long = "canonical-store",
            help = "Canonical .maestro store path that owns the target card"
        )]
        canonical_store: String,
        #[arg(
            long = "worker-source",
            help = "Worker branch/worktree that produced final HEAD, or none"
        )]
        worker_source: String,
        #[arg(
            long = "target-card-hash",
            help = "Optional sha256:<hex> hash of the live target card from preflight"
        )]
        target_card_hash: Option<String>,
        #[arg(
            long,
            help = "Preview the auto-archive without moving cards or writing receipts"
        )]
        dry_run: bool,
    },
    #[command(about = "Restore an archived feature and its archived child tasks")]
    Unarchive { id: String },
}

#[derive(Debug, Subcommand)]
pub enum FeatureProofCommand {
    #[command(about = "Record explicit feature acceptance proof")]
    Add {
        id: String,
        #[arg(long = "ac", help = "Acceptance id to prove")]
        ac: String,
        #[arg(long, help = "Observed evidence")]
        evidence: String,
        #[arg(long, help = "Record proof but suppress auto-close")]
        no_close: bool,
        #[arg(long, help = "Outcome line if this proof auto-closes")]
        outcome: Option<String>,
    },
    #[command(about = "Waive a feature acceptance item with an explicit reason")]
    Waive {
        id: String,
        #[arg(long = "ac", help = "Acceptance id to waive")]
        ac: String,
        #[arg(long, help = "Waiver reason")]
        reason: String,
    },
}

#[derive(Debug, Args)]
pub struct DecisionArgs {
    #[command(subcommand)]
    pub command: DecisionCommand,
}

#[derive(Debug, Subcommand)]
pub enum DecisionCommand {
    #[command(
        about = "Open a structured decision fork (mints a decision card)",
        after_help = "Examples:\n  maestro decision new \"Adopt X for Y\" --feature csv-export\n  maestro decision new \"Adopt X for Y\" --feature csv-export --lock --decision \"X\" --rejected \"Z: slower\"   # pre-decided fork, one call"
    )]
    New {
        title: String,
        #[arg(long, help = "Why this fork exists")]
        context: Option<String>,
        #[arg(long, help = "Owning feature id; omit for a global decision")]
        feature: Option<String>,
        #[arg(
            long,
            help = "Lock in the same call (requires --decision)",
            requires = "decision"
        )]
        lock: bool,
        #[arg(long, help = "Chosen decision text (with --lock)", requires = "lock")]
        decision: Option<String>,
        #[arg(
            long = "rejected",
            help = "Rejected option and reason (repeatable, with --lock)",
            requires = "lock"
        )]
        rejected: Vec<String>,
        #[arg(
            long,
            help = "Preview or concrete example (with --lock)",
            requires = "lock"
        )]
        preview: Option<String>,
        #[arg(
            long = "supersedes",
            help = "Decision id superseded by this lock (repeatable, with --lock)",
            requires = "lock"
        )]
        supersedes: Vec<String>,
        #[arg(long, help = "Project/service scope stored on the card")]
        project: Option<String>,
        #[arg(long, help = "Print only the new card id on stdout")]
        id_only: bool,
    },
    #[command(about = "Lock an open decision with the chosen answer")]
    Lock {
        id: String,
        #[arg(long, help = "Chosen decision text")]
        decision: String,
        #[arg(long = "rejected", help = "Rejected option and reason (repeatable)")]
        rejected: Vec<String>,
        #[arg(long, help = "Preview or concrete example")]
        preview: Option<String>,
        #[arg(
            long = "supersedes",
            help = "Decision id superseded by this lock (repeatable)"
        )]
        supersedes: Vec<String>,
    },
    #[command(
        about = "Replace a locked decision by superseding it",
        after_help = "Example:\n  maestro decision supersede dec-old --decision \"Use the new ruling\" --reason \"Why the old ruling is replaced\" --title \"New ruling\""
    )]
    Supersede {
        old_id: String,
        #[arg(long, help = "New ruling that replaces the old decision")]
        decision: String,
        #[arg(long, help = "Why the old ruling is replaced")]
        reason: String,
        #[arg(long, help = "Replacement decision title (defaults to the old title)")]
        title: Option<String>,
        #[arg(long = "rejected", help = "Rejected option and reason (repeatable)")]
        rejected: Vec<String>,
        #[arg(long, help = "Preview or concrete example")]
        preview: Option<String>,
        #[arg(long, help = "Print only the new card id on stdout")]
        id_only: bool,
    },
    #[command(about = "Show a decision card by id")]
    Show { id: String },
    #[command(about = "List decision cards (recent 20 by activity unless --all)")]
    List {
        /// List all decisions, not just the recent window.
        #[arg(long)]
        all: bool,
        /// Scope to one feature's decisions.
        #[arg(long, value_name = "FEATURE")]
        feature: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct ArchiveArgs {
    #[command(subcommand)]
    pub command: Option<ArchiveCommand>,
    /// Compatibility path: the feature card to archive.
    #[arg(value_name = "FEATURE", conflicts_with = "loose")]
    pub feature: Option<String>,
    /// Compatibility path: sweep terminal parentless cards instead.
    #[arg(long)]
    pub loose: bool,
}

#[derive(Debug, Subcommand)]
pub enum ArchiveCommand {
    #[command(about = "Migrate folder-backed archived cards into cards.sqlite")]
    MigrateDb(ArchiveMigrateDbArgs),
    #[command(about = "Verify the DB-backed archive store")]
    Doctor,
    #[command(about = "Delete legacy archive quarantine folders after doctor passes")]
    Cleanup(ArchiveCleanupArgs),
    #[command(about = "Show DB-backed archive store counts")]
    Stats,
}

#[derive(Debug, Args)]
pub struct ArchiveMigrateDbArgs {
    #[arg(long, help = "Preview migration without importing or moving folders")]
    pub dry_run: bool,
    #[arg(
        long,
        help = "Import folder-backed archives and move them to quarantine"
    )]
    pub apply: bool,
}

#[derive(Debug, Args)]
pub struct ArchiveCleanupArgs {
    #[arg(long, help = "Preview cleanup without deleting quarantine folders")]
    pub dry_run: bool,
    #[arg(long, help = "Delete quarantine folders after archive doctor passes")]
    pub apply: bool,
}

#[derive(Debug, Args)]
pub struct CardArchiveArgs {
    /// The feature card to archive (its `parent=<feature>` children ride along).
    #[arg(
        value_name = "FEATURE",
        required_unless_present = "loose",
        conflicts_with = "loose"
    )]
    pub feature: Option<String>,
    /// Sweep terminal parentless cards instead: closed loose tasks/ideas and
    /// superseded decisions move to the archive; locked decisions stay live.
    #[arg(long)]
    pub loose: bool,
}

#[derive(Debug, Args)]
pub struct ClaimArgs {
    /// The workable card (task/bug/chore) to claim for this session.
    #[arg(value_name = "ID")]
    pub id: String,
}

#[derive(Debug, Args)]
pub struct AssignArgs {
    /// The workable card (task/bug/chore) to suggest an owner for.
    #[arg(value_name = "ID")]
    pub id: String,
    /// Who to suggest the card for; pass `none` to clear the hint. Advisory
    /// only -- it never claims the card or blocks another session's claim.
    #[arg(value_name = "WHO")]
    pub who: Option<String>,
    /// Clear the advisory assignee hint.
    #[arg(long)]
    pub clear: bool,
}

#[derive(Debug, Args)]
pub struct NoteArgs {
    /// The card to append a note to.
    #[arg(value_name = "ID")]
    pub id: String,
    /// The note text; a dated line is appended to the card's notes.md.
    #[arg(value_name = "TEXT")]
    pub text: String,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Card type: feature, custom, progress, memory, task, bug, chore, idea, or decision.
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    pub card_type: String,
    /// One or more card titles; each title mints a card. A single title is the
    /// original one-card behavior; two or more batch-mint that many open cards.
    #[arg(value_name = "TITLE", required = true)]
    pub titles: Vec<String>,
    /// Parent card id; sets each new card's one-level `parent`.
    #[arg(long, value_name = "PARENT")]
    pub parent: Option<String>,
    /// Longer description stored on the card; rejected when batch-minting more
    /// than one title (per-card descriptions belong on `card update`).
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// Present-tense board label shown on the active row in place of the title;
    /// rejected when batch-minting more than one title.
    #[arg(long, value_name = "TEXT")]
    pub active_form: Option<String>,
    /// Project/service scope stored on the card (does not affect readiness).
    #[arg(long, value_name = "PROJECT")]
    pub project: Option<String>,
    /// Required custom kind when `--type custom`; team vocabulary such as ui,
    /// docs, release, ops, security, or migration.
    #[arg(long, value_name = "KIND")]
    pub kind: Option<String>,
    /// Print only the new card ids on stdout, one per line.
    #[arg(long)]
    pub id_only: bool,
}

#[derive(Debug, Args)]
pub struct CardPrepareArgs {
    /// The card container to prepare.
    #[arg(value_name = "ID")]
    pub id: String,
    /// Read explicit task plan file.
    #[arg(long = "from", value_name = "PLAN_FILE")]
    pub from: Option<PathBuf>,
    /// Create or point to the card prepare draft file.
    #[arg(long, conflicts_with = "from")]
    pub draft: bool,
    /// Structured task entry, e.g. T1: Add parser.
    #[arg(long = "task", conflicts_with_all = ["from", "draft"])]
    pub task: Vec<String>,
    /// Task check for --task entries.
    #[arg(long = "check", requires = "task")]
    pub check: Vec<String>,
    /// Acceptance id covered by --task.
    #[arg(long = "covers", requires = "task")]
    pub covers: Vec<String>,
    /// Blocker line for --task.
    #[arg(long = "blocker", requires = "task")]
    pub blocker: Vec<String>,
    /// Dependency ref for --task.
    #[arg(long = "after", requires = "task")]
    pub after: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// The card to show.
    #[arg(value_name = "ID")]
    pub id: String,
    /// Print the card as JSON.
    #[arg(long)]
    pub json: bool,
    /// Print the compact agent-facing card JSON.
    #[arg(long = "compact-json", conflicts_with = "json")]
    pub compact_json: bool,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// The card to update; omit to print usage.
    #[arg(value_name = "ID")]
    pub id: Option<String>,
    /// Set the card's status (free per-type word).
    #[arg(long, value_name = "STATUS")]
    pub status: Option<String>,
    /// Set the card's title.
    #[arg(long, value_name = "TITLE")]
    pub title: Option<String>,
    /// Set the card's description.
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// Set the card's present-tense board label (shown on the active row in
    /// place of the title).
    #[arg(long, value_name = "TEXT")]
    pub active_form: Option<String>,
    /// Claim the card for this session (same seam as `maestro claim`).
    #[arg(long)]
    pub claim: bool,
    /// Print the updated card as compact JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct CloseArgs {
    /// The card to close (status -> closed).
    #[arg(value_name = "ID")]
    pub id: String,
}

#[derive(Debug, Args)]
pub struct DepArgs {
    #[command(subcommand)]
    pub command: DepCommand,
}

#[derive(Debug, Subcommand)]
pub enum DepCommand {
    #[command(
        about = "Add a blocking edge: CHILD waits until PARENT closes",
        after_help = "Examples:\n  maestro dep add task-002 task-001   # task-002 is blocked by task-001"
    )]
    Add {
        /// The dependent card; the edge is stored on it.
        #[arg(value_name = "CHILD")]
        child: String,
        /// The blocker card the dependent waits on.
        #[arg(value_name = "PARENT")]
        parent: String,
    },
    #[command(
        about = "Remove a blocking edge so CHILD no longer waits on PARENT",
        after_help = "Examples:\n  maestro dep remove task-002 task-001   # task-002 no longer blocked by task-001"
    )]
    Remove {
        /// The dependent card the edge is stored on.
        #[arg(value_name = "CHILD")]
        child: String,
        /// The blocker card it waited on.
        #[arg(value_name = "PARENT")]
        parent: String,
    },
}

#[derive(Debug, Args)]
pub struct LinkArgs {
    #[command(subcommand)]
    pub command: LinkCommand,
}

#[derive(Debug, Subcommand)]
pub enum LinkCommand {
    #[command(
        about = "Add a non-blocking related link between two live cards",
        after_help = "Examples:\n  maestro link add task-a task-b   # links task-a and task-b (order does not matter)"
    )]
    Add {
        /// One of the two cards to link (order does not matter).
        #[arg(value_name = "CARD-A")]
        from: String,
        /// The other card to link.
        #[arg(value_name = "CARD-B")]
        to: String,
    },
    #[command(
        about = "Remove a related link between two live cards",
        after_help = "Examples:\n  maestro link remove task-b task-a   # argument order does not matter"
    )]
    Remove {
        /// First live card.
        #[arg(value_name = "FROM")]
        from: String,
        /// Second live card.
        #[arg(value_name = "TO")]
        to: String,
    },
}

#[derive(Debug, Args)]
pub struct MsgArgs {
    #[command(subcommand)]
    pub command: MsgCommand,
}

#[derive(Debug, Subcommand)]
pub enum MsgCommand {
    #[command(
        about = "Send a message to a linked card (sender is your current card)",
        after_help = "Examples:\n  maestro msg send task-b \"ready for review\""
    )]
    Send {
        /// Assert the sender card matches the running session's current card.
        #[arg(long = "from", value_name = "CARD")]
        from: Option<String>,
        /// The linked partner card to message.
        #[arg(value_name = "TO")]
        to: String,
        /// The message text.
        #[arg(value_name = "TEXT")]
        text: String,
    },
    #[command(
        about = "Read unread messages; with no card, aggregate every linked partner",
        after_help = "Examples:\n  maestro msg read\n  maestro msg read task-b"
    )]
    Read {
        /// Scope to one partner card; omit to read every visible channel.
        #[arg(value_name = "CARD")]
        card: Option<String>,
    },
    #[command(
        about = "Channel overview, or one partner's full timeline",
        after_help = "Examples:\n  maestro msg list\n  maestro msg list task-b"
    )]
    List {
        /// Scope to one partner's full timeline; omit for the overview.
        #[arg(value_name = "CARD")]
        card: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct ConflictArgs {
    /// The peer card whose ground you are taking (the card to warn).
    #[arg(value_name = "PEER")]
    pub peer: String,
    /// Why you are taking it -- the advisory the peer sees. Required to assert;
    /// omit with --clear.
    #[arg(value_name = "REASON")]
    pub reason: Option<String>,
    /// Retract the conflict notice you asserted against PEER.
    #[arg(long)]
    pub clear: bool,
}

#[derive(Debug, Args)]
pub struct HarnessArgs {
    #[command(subcommand)]
    pub command: HarnessCommand,
}

#[derive(Debug, Subcommand)]
pub enum HarnessCommand {
    #[command(about = "List proposals (proposed + accepted; --all adds the terminal ledger)")]
    List {
        /// Include measured and dismissed proposals (the terminal ledger).
        #[arg(long)]
        all: bool,
    },
    #[command(about = "Show a proposal's detail and history")]
    Show { id: String },
    #[command(about = "Set harness policy flags")]
    Set {
        #[arg(
            long = "claims-only",
            help = "Accept claims-only task verification when no verify commands are configured"
        )]
        claims_only: bool,
    },
    #[command(about = "File an agent-authored repo audit proposal")]
    Propose {
        #[arg(long, help = "Proposal title")]
        title: String,
        #[arg(
            long,
            required = true,
            help = "Evidence supporting the proposal (repeatable)"
        )]
        evidence: Vec<String>,
        #[arg(long, help = "Stable topic slug for merging repeated audit findings")]
        topic: Option<String>,
    },
    #[command(about = "Accept a proposal and spawn a linked task (-> accepted)")]
    Apply {
        id: String,
        #[arg(
            long = "check",
            help = "Task acceptance check to use instead of the proposal preset (repeatable)"
        )]
        check: Vec<String>,
    },
    #[command(about = "Undo an accepted proposal before its linked task is claimed")]
    Unapply {
        id: String,
        #[arg(long, help = "Why this accepted proposal is being rolled back")]
        reason: Option<String>,
    },
    #[command(about = "Dismiss a noisy proposal and suppress its fingerprint")]
    Dismiss {
        id: String,
        #[arg(long, help = "Why this proposal is noise or not worth acting on")]
        reason: String,
    },
    #[command(about = "Re-run the detector to close or revert a proposal (-> measured)")]
    Measure {
        id: String,
        /// Measure even if the linked task is not verified.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Args)]
pub struct QueryArgs {
    #[command(subcommand)]
    pub command: QueryCommand,
}

#[derive(Debug, Subcommand)]
pub enum QueryCommand {
    #[command(about = "Show the feature x task matrix (FEATURE/TASK/STATE/PROOF/TITLE)")]
    Matrix,
    #[command(about = "Summarize recorded run friction (events, prompts, corrections)")]
    Friction,
    #[command(
        hide = true,
        about = "List decision cards (ID/STATUS/HOME/TITLE; recent 20 unless --all)"
    )]
    Decisions {
        /// List all decisions, not just the recent window.
        #[arg(long)]
        all: bool,
        /// Scope to one feature's decisions.
        #[arg(long, value_name = "FEATURE")]
        feature: Option<String>,
    },
    #[command(about = "List improvement backlog items (ID/TITLE)")]
    Backlog,
    #[command(
        about = "Reassemble the run trace for a window from the durable run log",
        after_help = "Examples:\n  maestro query run                       # last 12h: per-card outcome + crash spans + status\n  maestro query run --since 2026-06-20T00:00:00Z\n  maestro query run --json"
    )]
    Run {
        /// Trace activity at or after this RFC3339 timestamp (default: last 12h).
        #[arg(long, value_name = "TS")]
        since: Option<String>,
        /// Emit the trace and status as one JSON line.
        #[arg(long)]
        json: bool,
    },
    #[command(hide = true, about = "Show a task's proof status")]
    Proof {
        task_id: Option<String>,
        #[arg(long = "task-id", value_name = "TASK_ID")]
        task_id_flag: Option<String>,
    },
    #[command(
        hide = true,
        about = "Walk a card's typed edges (parent/blocks/related/supersedes)",
        after_help = "Examples:\n  maestro card graph task-0a1b2c        # connected cards, two hops\n  maestro card graph --dot > cards.dot  # the whole web as Graphviz DOT\n  maestro card graph task-0a1b2c --dot  # one card's connected component"
    )]
    Graph {
        /// Card id to walk from; omit with --dot to export the whole web.
        id: Option<String>,
        /// Emit Graphviz DOT instead of a tree.
        #[arg(long)]
        dot: bool,
    },
}

#[derive(Debug, Args)]
pub struct IndexArgs {
    #[command(subcommand)]
    pub command: IndexCommand,
}

#[derive(Debug, Subcommand)]
pub enum IndexCommand {
    #[command(
        about = "Rebuild local derived indexes from scratch",
        after_help = "The archive is maestro's memory: list --grep [--archived] searches it,\nand the index keeps that search fast as the store grows. The index is\nlocal derived state (.maestro/index/); reads fall back to a plain scan\nwhenever it is missing or stale, so rebuilding is recovery, not setup."
    )]
    Rebuild {
        /// Rebuild only the Maestro-memory grep shard.
        #[arg(long, conflicts_with_all = ["source", "cards"])]
        memory: bool,
        /// Rebuild only the repo-source grep shard.
        #[arg(long, conflicts_with_all = ["memory", "cards"])]
        source: bool,
        /// Rebuild the legacy card grep text index plus the memory/card shard.
        #[arg(long, conflicts_with_all = ["memory", "source"])]
        cards: bool,
    },
}

#[derive(Debug, Args)]
pub struct GrepArgs {
    /// Emit an additive-only maestro.grep.v1 JSON envelope.
    #[arg(long)]
    pub json: bool,
    /// Query string. Multiple shell words are joined with spaces.
    #[arg(required = true, num_args = 1.., value_name = "QUERY")]
    pub query: Vec<String>,
}

#[derive(Debug, Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,
}

#[derive(Debug, Subcommand)]
pub enum McpCommand {
    #[command(alias = "stdio", about = "Run the MCP server over stdio")]
    Serve,
    #[command(hide = true, about = "Run the MCP server over stdio (same as serve)")]
    Stdin,
    #[command(about = "List the MCP tool names maestro exposes")]
    Tools,
    #[command(
        hide = true,
        about = "List the MCP tool names maestro exposes (same as tools)"
    )]
    List,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct WatchArgs {
    #[command(subcommand)]
    pub command: Option<WatchCommand>,
    /// Focus on a single feature id (overview when omitted)
    pub id: Option<String>,
    /// Re-render interval in seconds (live mode)
    #[arg(long)]
    pub interval: Option<u64>,
}

#[derive(Debug, Subcommand)]
pub enum WatchCommand {
    #[command(about = "Render the live board once and exit")]
    Snapshot {
        /// Focus on a single feature id (overview when omitted)
        id: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct MissionControlArgs {
    /// Output the current Mission Control snapshot as JSON.
    #[arg(long, conflicts_with_all = ["preview", "screen", "render_check", "format", "size"])]
    pub json: bool,
    /// Renderer to use for preview/render-check; no flags default to the restored OpenTUI app.
    #[arg(long, value_enum)]
    pub renderer: Option<MissionControlRenderer>,
    /// Render a read-only preview frame. Omit value for the dashboard.
    #[arg(long, num_args = 0..=1, default_missing_value = "dashboard")]
    pub preview: Option<Option<String>>,
    /// Alias for --preview <screen>, intended for programmatic callers.
    #[arg(long, conflicts_with = "preview")]
    pub screen: Option<String>,
    /// Select a feature/card id for dashboard or cards previews.
    #[arg(long)]
    pub feature: Option<String>,
    /// Render dimensions, e.g. 120x40.
    #[arg(long)]
    pub size: Option<String>,
    /// Output format for preview frames.
    #[arg(long, value_enum)]
    pub format: Option<MissionControlFormat>,
    /// Validate supported preview screens and report JSON.
    #[arg(long, conflicts_with_all = ["json", "preview", "screen", "feature", "format"])]
    pub render_check: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum MissionControlFormat {
    Plain,
    Ansi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum MissionControlRenderer {
    Rust,
    Opentui,
}

#[derive(Debug, Args)]
pub struct HookArgs {
    #[command(subcommand)]
    pub command: HookCommand,
}

#[derive(Debug, Subcommand)]
pub enum HookCommand {
    Record {
        #[arg(long, help = "Event type to record instead of reading JSON from stdin")]
        event: Option<String>,
        #[arg(long, help = "Skill name for skill_activation events")]
        skill: Option<String>,
        #[arg(
            long,
            help = "Logical session id; defaults to session env or cli-YYYY-MM-DD"
        )]
        session: Option<String>,
    },
}

pub fn run(cli: Cli) -> Result<()> {
    // The ambient inbox banner rides on every command (STDERR, best-effort) so a
    // linked peer's message is impossible to miss -- except `hook`/`mcp`, whose
    // high-frequency / protocol-stream output the banner must not pollute, and
    // `version`, which clap treats as a help-class query.
    if !matches!(
        cli.command,
        RootCommand::Hook(_) | RootCommand::Mcp(_) | RootCommand::Version
    ) {
        let _ = msg::inbox_banner();
        let _ = active::overlap_banner();
        let _ = conflict::conflict_banner();
        // `maestro active` prints the gate holder in its own structured view, so
        // the ambient pre-command banner would surface the same busy state twice.
        if !matches!(cli.command, RootCommand::Active(_)) {
            let _ = active::busy_banner();
        }
    }
    match cli.command {
        RootCommand::Init(args) => init::run(args),
        RootCommand::Install(args) => install::run(args),
        RootCommand::Upgrade(args) => update::run(args),
        RootCommand::Sync(args) => sync::run(args),
        RootCommand::Design(args) => design::run(args),
        RootCommand::MigrateV2 => migrate::run(),
        RootCommand::Migrate => migrate::run_card_fold(),
        RootCommand::Uninstall(args) => uninstall::run(args),
        RootCommand::Doctor => doctor::run(),
        RootCommand::ShellInit => shell_init::run(),
        RootCommand::Status(args) => status::run(args),
        RootCommand::Next(args) => status::run_next(args),
        RootCommand::Resume(args) => resume::run(args),
        RootCommand::Task(args) => task::run(args),
        RootCommand::Event(args) => event::run(args),
        RootCommand::Feature(args) => feature::run(args),
        RootCommand::Qa(args) => qa::run(args),
        RootCommand::Worktree(args) => worktree::run(args),
        RootCommand::Grep(args) => grep::run(args),
        RootCommand::Memory(args) => memory::run(args),
        RootCommand::Scorer(args) => scorer::run(args),
        RootCommand::Decision(args) => decision::run(args),
        RootCommand::Card(args) => match args.command {
            CardCommand::Ready(args) => card::ready(args),
            CardCommand::List(args) => card::list(args),
            CardCommand::Dep(args) => card::dep(args),
            CardCommand::Archive(args) => card::archive(args),
            CardCommand::Claim(args) => card::claim(args),
            CardCommand::Assign(args) => card::assign(args),
            CardCommand::Note(args) => card::note(args),
            CardCommand::Create(args) => card::create(args),
            CardCommand::Prepare(args) => card::prepare(args),
            CardCommand::Show(args) => card::show(args),
            CardCommand::Update(args) => card::update(args),
            CardCommand::Close(args) => card::close(args),
            CardCommand::Graph { id, dot } => query::run_graph(id, dot),
        },
        RootCommand::Ready(args) => card::ready(args),
        RootCommand::List(args) => card::list(args),
        RootCommand::Dep(args) => card::dep(args),
        RootCommand::Active(args) => active::run(args),
        RootCommand::Session(args) => session::run(args),
        RootCommand::Link(args) => card::link(args),
        RootCommand::Msg(args) => msg::run(args),
        RootCommand::Conflict(args) => conflict::run(args),
        RootCommand::Archive(args) => archive::run(args),
        RootCommand::Claim(args) => card::claim(args),
        RootCommand::Assign(args) => card::assign(args),
        RootCommand::Note(args) => card::note(args),
        RootCommand::Create(args) => card::create(args),
        RootCommand::Show(args) => card::show(args),
        RootCommand::Update(args) => card::update(args),
        RootCommand::Close(args) => card::close(args),
        RootCommand::Harness(args) => harness::run(args),
        RootCommand::Query(args) => query::run(args),
        RootCommand::Index(args) => index::run(args),
        RootCommand::Mcp(args) => mcp::run(args),
        RootCommand::Hook(args) => hook::run(args),
        RootCommand::MissionControl(args) => mission_control::run(args),
        RootCommand::Watch(args) => watch::run(args),
        RootCommand::Verify { id } => verify::run(id),
        RootCommand::Playbook(args) => playbook::run(args),
        RootCommand::Loop(args) => loop_recipes::run(args),
        RootCommand::Lean(args) => lean::run(args),
        RootCommand::Version => version::run(),
    }
}

pub(super) fn actor() -> String {
    std::env::var("MAESTRO_ACTOR").unwrap_or_else(|_| "maestro".to_string())
}

/// Resolve a card's project at create time (T3): an explicit `--project` always
/// wins and short-circuits inference; otherwise the repo's declared `projects:`
/// scopes drive folder->project auto-infer from the real cwd. Shared by the four
/// explicit create entry points (create / feature new / task create / decision
/// new). `None` means store no project.
pub(super) fn resolve_project(
    explicit: Option<String>,
    paths: &MaestroPaths,
) -> Result<Option<String>> {
    let cwd = env::current_dir()?;
    resolve_project_in(explicit, paths, &cwd)
}

/// Inner seam of [`resolve_project`] with the cwd passed in, so the inference
/// path is testable without mutating the process cwd. `repo_root` from discovery
/// is canonicalized, so `cwd` is canonicalized here to match -- otherwise a
/// `/var` vs `/private/var`-style mismatch on macOS would make `strip_prefix`
/// silently fail and turn inference off.
pub(super) fn resolve_project_in(
    explicit: Option<String>,
    paths: &MaestroPaths,
    cwd: &std::path::Path,
) -> Result<Option<String>> {
    if let Some(project) = explicit {
        return Ok(Some(project));
    }
    // Function-local root import: shadows the cli `harness` submodule so the
    // facade call is a plain root import (the sanctioned form), not a forbidden
    // deep `operations::harness::*` path under the architecture guard.
    use crate::operations::harness;
    let patterns = match harness::load_config(paths) {
        Ok(Some(config)) => config.projects,
        Ok(None) => Vec::new(),
        Err(error) if is_managed_path_symlink_error(&error) => Vec::new(),
        Err(error) => return Err(error),
    };
    if patterns.is_empty() {
        return Ok(None);
    }
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    Ok(crate::foundation::core::paths::infer_project(
        paths.repo_root(),
        &cwd,
        &patterns,
    ))
}

fn is_managed_path_symlink_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<MaestroError>(),
            Some(MaestroError::ManagedPathContainsSymlink { .. })
        )
    })
}

pub(super) fn cli_run_id() -> String {
    crate::foundation::core::session::session_token()
}

/// Best-effort: bind this session to a card it just mutated by recording a
/// `card_touch` run event (D3), so a parallel session's `maestro active` can see
/// the binding without anyone declaring it. Awareness rides on normal work, so a
/// failed append must never abort the verb: the error is swallowed with a
/// warning, mirroring `maestro hook record`'s warn-and-continue.
pub(super) fn emit_card_touch(paths: &MaestroPaths, card_id: &str) {
    let payload = serde_json::json!({
        "event": "card_touch",
        "session_id": cli_run_id(),
        "card_id": card_id,
        "agent": actor(),
    });
    if let Err(error) = record::record_value(paths, &payload) {
        eprintln!("maestro: card_touch run-event note failed: {error:#}");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnershipReleaseStatus {
    Released,
    Done,
}

impl OwnershipReleaseStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Released => "released",
            Self::Done => "done",
        }
    }
}

pub(super) fn emit_ownership_acquire(paths: &MaestroPaths, card_id: &str) {
    let payload = serde_json::json!({
        "event": "ownership_acquire",
        "session_id": cli_run_id(),
        "card_id": card_id,
        "agent": actor(),
    });
    if let Err(error) = record::record_value(paths, &payload) {
        eprintln!("maestro: ownership_acquire run-event note failed: {error:#}");
    }
}

pub(super) fn emit_work_touch(paths: &MaestroPaths, card_id: &str) {
    emit_card_touch(paths, card_id);
    emit_ownership_acquire(paths, card_id);
}

pub(super) fn emit_ownership_release(
    paths: &MaestroPaths,
    card_id: &str,
    status: OwnershipReleaseStatus,
    reason: Option<&str>,
) {
    let mut payload = serde_json::json!({
        "event": "ownership_release",
        "session_id": cli_run_id(),
        "card_id": card_id,
        "agent": actor(),
        "status": status.as_str(),
    });
    if let Some(reason) = reason {
        payload["reason"] = serde_json::json!(reason);
    }
    if let Err(error) = record::record_value(paths, &payload) {
        eprintln!("maestro: ownership_release run-event note failed: {error:#}");
    }
}

/// The card the running session is currently working: the `card_id` of the last
/// `card_touch` in this session's OWN run log (D3). Best-effort and read-only --
/// the `msg` verbs and the inbox banner use it to resolve "my card" without the
/// user naming it. `None` when the session has touched no card (or the log is
/// unreadable); callers treat that as "no current card", never an error.
pub(super) fn current_card(paths: &MaestroPaths) -> Option<String> {
    run::current_bound_card(paths, &cli_run_id()).ok().flatten()
}

/// Every worktree root the repo exposes, each as its own `MaestroPaths`, for the
/// verbs that read across worktrees (`active` liveness union, `msg` read union).
/// `git::worktree_roots` returns a single root for a lone repo, so the common
/// one-worktree case is unchanged and no flag engages the union; an unreadable
/// git topology (not a repo, bare) falls back to the local root alone
/// (`dec-cross-worktree-active-auto-unions-read-51b9`). Visible to the TUI board
/// (`maestro watch`) so its LIVE SESSIONS block unions over the same roots
/// `maestro active` does.
pub(crate) fn worktree_roots(paths: &MaestroPaths) -> Vec<MaestroPaths> {
    match git::worktree_roots(paths.repo_root()) {
        Ok(roots) if !roots.is_empty() => roots.into_iter().map(MaestroPaths::new).collect(),
        _ => vec![MaestroPaths::new(paths.repo_root().to_path_buf())],
    }
}

/// The `<session>` half of a card claim identity (SPEC E6): `MAESTRO_SESSION` if
/// set, then any real per-session id the agent runtime exports (the shared
/// `session_id_from_env` scan, so a claim buckets under the same id as the run
/// log and the active union), else a process-unique token. Never the colliding
/// `cli-DATE` form `cli_run_id` falls back to -- a date is not a session, and
/// would let two runs share one claim. Delegating the scan fixed
/// bug-claim-...-2ce0: the old inline list missed `CODEX_THREAD_ID` and
/// `CLAUDE_CODE_SESSION_ID`, so a real Codex/Claude run fell through to a
/// per-process token that changed every call and self-locked the agent.
pub(super) fn claim_session() -> String {
    resolve_claim_session(
        env::var("MAESTRO_SESSION").ok(),
        crate::foundation::core::session::session_id_from_env(),
        process_unique_token,
    )
}

/// Pure precedence for [`claim_session`]: `MAESTRO_SESSION` override, then the
/// runtime session id, then the process-unique fallback. Split out so the order
/// is testable without mutating the process-global env.
fn resolve_claim_session(
    maestro_session: Option<String>,
    runtime_session_id: Option<String>,
    fallback: impl FnOnce() -> String,
) -> String {
    if let Some(value) = maestro_session {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    runtime_session_id.unwrap_or_else(fallback)
}

/// A token unique to this process invocation; the deliberate non-`cli-DATE`
/// fallback so two concurrent runs never collide on one claim.
fn process_unique_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    format!("s{}-{nanos}", std::process::id())
}

pub(super) fn detected_agent_hint() -> &'static str {
    crate::foundation::core::session::agent_runtime_from_env().unwrap_or("<claude|codex|droid>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::proof::{ProofStaleReason, ProofStatusKind};
    use clap::Parser;

    fn parse_watch(argv: &[&str]) -> WatchArgs {
        match Cli::try_parse_from(argv)
            .unwrap_or_else(|e| panic!("parse {argv:?}: {e}"))
            .command
        {
            RootCommand::Watch(args) => args,
            other => panic!("expected watch, got {other:?}"),
        }
    }

    fn parse_mission_control(argv: &[&str]) -> MissionControlArgs {
        match Cli::try_parse_from(argv)
            .unwrap_or_else(|e| panic!("parse {argv:?}: {e}"))
            .command
        {
            RootCommand::MissionControl(args) => args,
            other => panic!("expected mission-control, got {other:?}"),
        }
    }

    fn parse_active(argv: &[&str]) -> ActiveArgs {
        match Cli::try_parse_from(argv)
            .unwrap_or_else(|e| panic!("parse {argv:?}: {e}"))
            .command
        {
            RootCommand::Active(args) => args,
            other => panic!("expected active, got {other:?}"),
        }
    }

    #[test]
    fn resolve_claim_session_prefers_the_maestro_session_override() {
        let got = resolve_claim_session(
            Some("  override  ".to_string()),
            Some("runtime".to_string()),
            || panic!("override present, must not reach fallback"),
        );
        assert_eq!(got, "override");
    }

    #[test]
    fn resolve_claim_session_uses_the_runtime_id_when_no_override() {
        // bug-claim-...-2ce0: a real runtime session id (CLAUDE_CODE_SESSION_ID /
        // CODEX_THREAD_ID, surfaced by session_id_from_env) must win over the
        // per-process fallback, or the agent self-locks out of its own card.
        let got = resolve_claim_session(None, Some("claude-58fc".to_string()), || {
            panic!("runtime id present, must not reach the per-process fallback")
        });
        assert_eq!(got, "claude-58fc");
    }

    #[test]
    fn resolve_claim_session_falls_back_when_override_blank_and_no_runtime_id() {
        let got = resolve_claim_session(Some("   ".to_string()), None, || "s123-456".to_string());
        assert_eq!(got, "s123-456");
    }

    #[test]
    fn stale_merge_advisory_is_silent_when_shared_branch_has_not_moved() {
        let git = GitReadout {
            branch: Some("slice".to_string()),
            code_other_dirty: 0,
            maestro_dirty: 0,
            divergence: Some(GitDivergenceReadout {
                shared_branch: "main".to_string(),
                ahead: 2,
                behind: 0,
            }),
        };

        assert_eq!(stale_merge_advisory(&git), None);
    }

    #[test]
    fn stale_merge_advisory_points_to_rebase_before_merging() {
        let git = GitReadout {
            branch: Some("slice".to_string()),
            code_other_dirty: 0,
            maestro_dirty: 0,
            divergence: Some(GitDivergenceReadout {
                shared_branch: "main".to_string(),
                ahead: 1,
                behind: 4,
            }),
        };

        assert_eq!(
            stale_merge_advisory(&git).as_deref(),
            Some("[stale] main moved 4 commits since this worktree forked; rebase before merging")
        );
    }

    #[test]
    fn merge_busy_advisory_points_to_manual_fast_forward_wait() {
        assert_eq!(
            merge_busy_advisory("session-a"),
            "[merge-busy] session-a is merging back; wait before rebase+fast-forward"
        );
    }

    #[test]
    fn merge_back_workflow_stays_on_existing_surfaces_not_a_merge_verb() {
        assert!(
            Cli::try_parse_from(["maestro", "merge"]).is_err(),
            "merge-back coordination should not introduce a required maestro merge verb"
        );
        let recipe = crate::domain::loop_recipes::serve("conflict-handoff").expect("recipe ships");
        assert!(recipe.contains("maestro status"));
        assert!(recipe.contains("[stale]"));
        assert!(recipe.contains("[merge-busy]"));
        assert!(recipe.contains("git merge --ff-only <branch>"));
        assert!(recipe.contains("Maestro never does"));
    }

    #[test]
    fn active_release_command_requires_card_and_reason() {
        let args = parse_active(&[
            "maestro",
            "active",
            "release",
            "task-owned",
            "--reason",
            "handoff",
        ]);
        match args.command {
            Some(ActiveCommand::Release(release)) => {
                assert_eq!(release.card_id, "task-owned");
                assert_eq!(release.reason, "handoff");
            }
            other => panic!("expected active release, got {other:?}"),
        }

        assert!(
            Cli::try_parse_from(["maestro", "active", "release", "task-owned"]).is_err(),
            "release must record an explicit reason"
        );
    }

    #[test]
    fn watch_parse_matrix_bare_focus_and_snapshot() {
        let bare = parse_watch(&["maestro", "watch"]);
        assert!(bare.command.is_none() && bare.id.is_none());

        // The fall-through case: a bare positional must land on `id`, not be
        // rejected as an unknown subcommand.
        let focus = parse_watch(&["maestro", "watch", "feat-x"]);
        assert!(focus.command.is_none());
        assert_eq!(focus.id.as_deref(), Some("feat-x"));

        let snap = parse_watch(&["maestro", "watch", "snapshot"]);
        assert!(matches!(
            snap.command,
            Some(WatchCommand::Snapshot { id: None })
        ));

        let snap_focus = parse_watch(&["maestro", "watch", "snapshot", "feat-x"]);
        assert!(
            matches!(snap_focus.command, Some(WatchCommand::Snapshot { id: Some(ref s) }) if s == "feat-x")
        );

        let interval = parse_watch(&["maestro", "watch", "--interval", "5"]);
        assert_eq!(interval.interval, Some(5));
    }

    #[test]
    fn mission_control_parse_matrix() {
        let preview = parse_mission_control(&[
            "maestro",
            "mission-control",
            "--preview",
            "--size",
            "120x40",
            "--format",
            "plain",
        ]);
        assert_eq!(preview.preview, Some(Some("dashboard".to_string())));
        assert_eq!(preview.size.as_deref(), Some("120x40"));
        assert_eq!(preview.format, Some(MissionControlFormat::Plain));
        assert_eq!(preview.renderer, None);

        let cards = parse_mission_control(&[
            "maestro",
            "mission-control",
            "--screen",
            "cards",
            "--feature",
            "feat-x",
        ]);
        assert_eq!(cards.screen.as_deref(), Some("cards"));
        assert_eq!(cards.feature.as_deref(), Some("feat-x"));

        let opentui = parse_mission_control(&[
            "maestro",
            "mission-control",
            "--renderer",
            "opentui",
            "--preview",
            "tasks",
        ]);
        assert_eq!(opentui.renderer, Some(MissionControlRenderer::Opentui));

        let json = parse_mission_control(&["maestro", "mission-control", "--json"]);
        assert!(json.json);

        let render_check = parse_mission_control(&[
            "maestro",
            "mission-control",
            "--render-check",
            "--size",
            "120x40",
        ]);
        assert!(render_check.render_check);
    }

    #[test]
    fn hidden_verbs_and_relocated_views_still_parse() {
        // `task list --watch` is hidden from help (clap arg hide), but the flag
        // still parses and sets watch=true -- the dispatch path is unchanged.
        match Cli::try_parse_from(["maestro", "task", "list", "--watch", "--interval", "2"])
            .expect("task list --watch must still parse")
            .command
        {
            RootCommand::Task(args) => match args.command {
                TaskCommand::List {
                    watch, interval, ..
                } => {
                    assert!(watch, "the hidden --watch flag must still set watch=true");
                    assert_eq!(interval, Some(2));
                }
                other => panic!("expected task list, got {other:?}"),
            },
            other => panic!("expected task, got {other:?}"),
        }

        // The relocated read views parse under their canonical resource.
        assert!(matches!(
            Cli::try_parse_from(["maestro", "task", "proof", "task-x"])
                .expect("task proof must parse")
                .command,
            RootCommand::Task(_)
        ));
        assert!(matches!(
            Cli::try_parse_from(["maestro", "card", "graph", "card-x", "--dot"])
                .expect("card graph must parse")
                .command,
            RootCommand::Card(_)
        ));

        // Every hidden duplicate/synonym still parses, preserving back-compat.
        for argv in [
            ["maestro", "ready"].as_slice(),
            ["maestro", "verify", "task-x"].as_slice(),
            ["maestro", "migrate-v2"].as_slice(),
            ["maestro", "migrate"].as_slice(),
            ["maestro", "query", "proof", "task-x"].as_slice(),
            ["maestro", "query", "graph", "card-x"].as_slice(),
            ["maestro", "query", "decisions"].as_slice(),
            ["maestro", "mcp", "stdin"].as_slice(),
            ["maestro", "mcp", "list"].as_slice(),
        ] {
            assert!(
                Cli::try_parse_from(argv).is_ok(),
                "hidden alias must still parse: {argv:?}"
            );
        }
    }

    fn commit_drift() -> Vec<ProofStaleReason> {
        vec![ProofStaleReason {
            field: "verified_commit",
            // expected = current HEAD (new); actual = stored verified_commit (old).
            expected: "def5678def".to_string(),
            actual: "abc1234abc".to_string(),
        }]
    }

    #[test]
    fn stale_with_commit_drift_names_old_to_new_and_verify_refresh() {
        let line = proof_concern_line_from(
            "task-x",
            &ProofStatusKind::Stale,
            &TaskState::Verified,
            &commit_drift(),
        )
        .expect("stale proof is a concern");
        assert_eq!(
            line,
            "proof: stale (abc1234->def5678); refresh (HEAD moved, likely no code change): maestro task verify task-x"
        );
    }

    #[test]
    fn stale_without_commit_drift_falls_back_to_bare_refresh() {
        let reasons = vec![ProofStaleReason {
            field: "contract_hash",
            expected: "new".to_string(),
            actual: "old".to_string(),
        }];
        let line = proof_concern_line_from(
            "task-x",
            &ProofStatusKind::Stale,
            &TaskState::Verified,
            &reasons,
        )
        .expect("stale proof is a concern");
        assert_eq!(line, "proof: stale; refresh: maestro task verify task-x");
    }

    #[test]
    fn failed_proof_names_fix_then_verify() {
        let line = proof_concern_line_from(
            "task-x",
            &ProofStatusKind::Failed,
            &TaskState::NeedsVerification,
            &[],
        )
        .expect("failed proof is a concern");
        assert_eq!(
            line,
            "proof: failed; fix, then re-verify: maestro task verify task-x"
        );
    }

    #[test]
    fn missing_proof_is_a_concern_only_while_needs_verification() {
        let line = proof_concern_line_from(
            "task-x",
            &ProofStatusKind::Missing,
            &TaskState::NeedsVerification,
            &[],
        )
        .expect("missing proof on a needs_verification task is a concern");
        assert_eq!(
            line,
            "proof: missing; bind proof: maestro task verify task-x"
        );
        for state in [
            TaskState::Draft,
            TaskState::Exploring,
            TaskState::Ready,
            TaskState::InProgress,
            TaskState::Verified,
        ] {
            assert!(
                proof_concern_line_from("task-x", &ProofStatusKind::Missing, &state, &[]).is_none(),
                "missing proof must be silent in {state:?}"
            );
        }
    }

    #[test]
    fn accepted_proof_is_never_a_concern() {
        assert!(
            proof_concern_line_from(
                "task-x",
                &ProofStatusKind::Accepted,
                &TaskState::Verified,
                &[]
            )
            .is_none()
        );
    }
}
