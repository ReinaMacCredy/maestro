use std::env;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::domain::feature::{FeatureStatus, FeatureView};
use crate::foundation::core::paths::MaestroPaths;
use crate::interfaces::hooks::record;

pub mod active;
pub mod card;
pub mod decision;
pub mod doctor;
pub mod event;
pub mod feature;
pub mod harness;
pub mod hook;
pub mod index;
pub mod init;
pub mod install;
pub mod mcp;
pub mod migrate;
pub mod query;
pub mod reference;
pub mod resume;
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

pub(crate) fn recovery_label(hint: Option<&str>) -> String {
    match hint {
        Some(hint) => format!("fix: {hint}"),
        None => "fix: run maestro doctor".to_string(),
    }
}

/// Next-step label for a feature, shared by `status` and `feature list` so the
/// hint never diverges between the two surfaces.
pub(crate) fn feature_next_label(view: &FeatureView) -> &'static str {
    match view.status {
        FeatureStatus::Proposed
            if !view.acceptance.is_empty() && !view.affected_areas.is_empty() =>
        {
            "template: qa_baseline"
        }
        FeatureStatus::Proposed => "template: set_contract",
        FeatureStatus::Ready => "run: prepare_feature",
        FeatureStatus::InProgress
            if view.counts.total > 0 && view.counts.total == view.counts.verified =>
        {
            "template: ship_feature"
        }
        FeatureStatus::InProgress => "run: resolve_tasks",
        FeatureStatus::Shipped | FeatureStatus::Cancelled => "run: archive_feature",
    }
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
    #[command(about = "Install maestro hooks and config for an agent (claude, codex)")]
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
    #[command(about = "Migrate v1 Maestro artifacts to the reduced v2 layout")]
    MigrateV2,
    #[command(
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
    #[command(about = "Create, show, and list decision cards in the card store")]
    Decision(DecisionArgs),
    #[command(
        about = "Card-store verbs under one namespace (same output as the flat spellings)",
        after_help = "Examples:\n  maestro card show task-0a1b2c   # identical to `maestro show task-0a1b2c`\n  maestro card ready"
    )]
    Card(CardArgs),
    #[command(
        about = "List workable cards with no open blockers (card store)",
        after_help = "Examples:\n  maestro ready                # every unblocked task/bug/chore\n  maestro ready --json\n  maestro ready agent-cli-ux   # only those parented to a feature"
    )]
    Ready(ReadyArgs),
    #[command(
        about = "List cards filtered by parent, type, assignee, or coarse status (card store)",
        after_help = "Examples:\n  maestro list --parent agent-cli-ux\n  maestro list --json --type bug --status open\n  maestro list --assignee claude#s1"
    )]
    List(ListArgs),
    #[command(about = "Author dependency edges between cards (card store)")]
    Dep(DepArgs),
    #[command(
        about = "Show what other live sessions are doing (cross-session awareness)",
        after_help = "Examples:\n  maestro active               # live sessions, newest first\n  maestro active --all         # include stale sessions beyond the window"
    )]
    Active(ActiveArgs),
    #[command(
        about = "Author non-blocking related links between cards (card store)",
        after_help = "Examples:\n  maestro link add task-a task-b\n  maestro link remove task-b task-a"
    )]
    Link(LinkArgs),
    #[command(
        about = "Archive a feature card and its child cards (card store)",
        after_help = "Examples:\n  maestro archive csv-export   # archives the feature card + every parent=csv-export card\n  maestro archive --loose      # sweeps closed loose tasks/ideas + superseded decisions"
    )]
    Archive(ArchiveArgs),
    #[command(
        about = "Claim a workable card for this session (card store)",
        after_help = "Examples:\n  maestro claim task-0a1b2c   # take an unclaimed task/bug/chore\n  MAESTRO_SESSION=mine maestro claim task-0a1b2c"
    )]
    Claim(ClaimArgs),
    #[command(
        about = "Append a dated note to a card's notes.md (card store)",
        after_help = "Examples:\n  maestro note task-0a1b2c \"chose option B; A breaks on reparent\""
    )]
    Note(NoteArgs),
    #[command(
        about = "Create a card of any type (card store)",
        after_help = "Examples:\n  maestro create -t task \"Add CSV export\" --parent csv-export\n  maestro create -t bug \"Fix ordering race\"\n  maestro create -t feature \"CSV export\""
    )]
    Create(CreateArgs),
    #[command(about = "Show a card's header, edges, and body (card store)")]
    Show(ShowArgs),
    #[command(
        about = "Update a card's status, title, description, or claim (card store)",
        after_help = "Examples:\n  maestro update task-add-csv-export-0a1b --status needs_verification\n  maestro update task-add-csv-export-0a1b --claim\n  maestro update task-add-csv-export-0a1b --title \"New title\""
    )]
    Update(UpdateArgs),
    #[command(about = "Close a card: status -> closed (card store)")]
    Close(CloseArgs),
    #[command(
        about = "List, show, apply, unapply, dismiss, and measure harness improvement suggestions"
    )]
    Harness(HarnessArgs),
    #[command(about = "Query computed read models (matrix, friction, decisions, proof, backlog)")]
    Query(QueryArgs),
    #[command(about = "Maintain the local text index that accelerates list --grep")]
    Index(IndexArgs),
    #[command(about = "Run or inspect the MCP server (serve, stdin, tools, list)")]
    Mcp(McpArgs),
    #[command(about = "Hook entry points invoked by the agent harness")]
    Hook(HookArgs),
    #[command(about = "Watch tasks and render snapshots on change")]
    Watch(WatchArgs),
    #[command(about = "Verify a task against its recorded proof")]
    Verify { id: Option<String> },
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

#[derive(Clone, Debug, ValueEnum)]
pub enum Agent {
    Claude,
    Codex,
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
pub struct ActiveArgs {
    /// Include stale sessions (last event beyond the live window), hidden by default.
    #[arg(long)]
    pub all: bool,
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
    Archive(ArchiveArgs),
    #[command(about = "Claim a workable card for this session")]
    Claim(ClaimArgs),
    #[command(about = "Append a dated note to a card's notes.md")]
    Note(NoteArgs),
    #[command(about = "Create a card of any type")]
    Create(CreateArgs),
    #[command(about = "Show a card's header, edges, and body")]
    Show(ShowArgs),
    #[command(about = "Update a card's status, title, description, or claim")]
    Update(UpdateArgs),
    #[command(about = "Close a card: status -> closed")]
    Close(CloseArgs),
}

#[derive(Debug, Args)]
pub struct ReadyArgs {
    /// Print machine-readable ready JSON.
    #[arg(long)]
    pub json: bool,
    /// Restrict to cards parented to this feature id (one level).
    #[arg(value_name = "FEATURE")]
    pub feature: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Only cards whose parent is this card id.
    #[arg(long, value_name = "PARENT")]
    pub parent: Option<String>,
    /// Only cards of this type (feature, task, bug, chore, idea, decision).
    #[arg(long = "type", value_name = "TYPE")]
    pub card_type: Option<String>,
    /// Only cards claimed by this agent or full `<agent>#<session>` token.
    #[arg(long, value_name = "ASSIGNEE")]
    pub assignee: Option<String>,
    /// Only cards in this coarse status (open, in_progress, closed).
    #[arg(long, value_name = "STATUS")]
    pub status: Option<String>,
    /// Only cards whose title, description, or notes.md/spec.md sidecars
    /// contain this case-insensitive substring.
    #[arg(long, value_name = "TERM")]
    pub grep: Option<String>,
    /// Include archived cards (rows marked archived).
    #[arg(long)]
    pub archived: bool,
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
    #[command(about = "Create a task (-> draft)")]
    Create {
        title: String,
        #[arg(long)]
        feature: Option<String>,
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
    Show { id: Option<String> },
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
        #[arg(
            long,
            help = "Include terminal/done tasks (verified, rejected, abandoned, superseded) and archived ones"
        )]
        all: bool,
        #[arg(long)]
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
    #[command(about = "Check the task blocker graph for cycles and dangling refs")]
    Doctor,
    #[command(about = "Archive a done task out of the live scan (-> .maestro/archive/tasks)")]
    Archive {
        id: String,
        #[arg(long, help = "Preview the move without archiving")]
        dry_run: bool,
    },
    #[command(about = "Restore an archived task to the live scan")]
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

#[derive(Debug, Subcommand)]
pub enum FeatureCommand {
    #[command(about = "Propose a new feature (-> proposed)")]
    New {
        title: String,
        #[arg(long, help = "Set the initial description")]
        description: Option<String>,
        #[arg(long = "question", help = "Initial open question (repeatable)")]
        question: Vec<String>,
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
            help = "Clear all open questions (with --question, clear then set the passed list)"
        )]
        clear_questions: bool,
        #[arg(
            long = "add-acceptance",
            help = "Acceptance criterion to append while proposed (repeatable)"
        )]
        add_acceptance: Vec<String>,
        #[arg(
            long = "add-area",
            help = "Affected area to append while proposed (repeatable)"
        )]
        add_area: Vec<String>,
        #[arg(
            long = "add-non-goal",
            help = "Non-goal to append while proposed (repeatable)"
        )]
        add_non_goal: Vec<String>,
        #[arg(
            long = "add-question",
            help = "Open question to append while proposed (repeatable)"
        )]
        add_question: Vec<String>,
        #[arg(
            long = "edit-acceptance",
            value_name = "AC_ID",
            help = "Acceptance id to edit in place, paired by index with --text"
        )]
        edit_acceptance: Vec<String>,
        #[arg(
            long = "text",
            value_name = "TEXT",
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
    },
    #[command(about = "Append a dated note to a feature's notes.md")]
    Note { id: String, text: String },
    #[command(about = "Ship an in-progress feature (-> shipped; gated)")]
    Ship {
        id: String,
        #[arg(
            long,
            help = "One-line outcome recorded on the feature, shown in `feature list --all`"
        )]
        outcome: Option<String>,
        #[arg(long, help = "Preview the ship gate without transitioning")]
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
            help = "Include terminal features (shipped, cancelled) and archived ones"
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
            help = "Archive every closed feature (shipped or cancelled; mutually exclusive with <id>)"
        )]
        closed: bool,
        #[arg(
            long,
            help = "Preview the feature and child-task moves without archiving"
        )]
        dry_run: bool,
    },
    #[command(about = "Restore an archived feature and its archived child tasks")]
    Unarchive { id: String },
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
    #[command(about = "Show a decision card by id")]
    Show { id: String },
    #[command(about = "List decision cards")]
    List,
}

#[derive(Debug, Args)]
pub struct ArchiveArgs {
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
    /// Card type: feature, task, bug, chore, idea, or decision.
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    pub card_type: String,
    /// Card title.
    #[arg(value_name = "TITLE")]
    pub title: String,
    /// Parent card id; sets the new card's one-level `parent`.
    #[arg(long, value_name = "PARENT")]
    pub parent: Option<String>,
    /// Longer description stored on the card.
    #[arg(long, value_name = "TEXT")]
    pub description: Option<String>,
    /// Print only the new card id on stdout.
    #[arg(long)]
    pub id_only: bool,
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
        after_help = "Examples:\n  maestro link add task-a task-b   # task-a is related to task-b"
    )]
    Add {
        /// First live card; the edge is stored here on first add.
        #[arg(value_name = "FROM")]
        from: String,
        /// Second live card.
        #[arg(value_name = "TO")]
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
    #[command(about = "List decision cards (ID/STATUS/HOME/TITLE)")]
    Decisions,
    #[command(about = "List improvement backlog items (ID/TITLE)")]
    Backlog,
    #[command(about = "Show a task's proof status")]
    Proof {
        task_id: Option<String>,
        #[arg(long = "task-id", value_name = "TASK_ID")]
        task_id_flag: Option<String>,
    },
    #[command(
        about = "Walk a card's typed edges (parent/blocks/related/supersedes)",
        after_help = "Examples:\n  maestro query graph task-0a1b2c        # connected cards, two hops\n  maestro query graph --dot > cards.dot  # the whole web as Graphviz DOT\n  maestro query graph task-0a1b2c --dot  # one card's connected component"
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
        about = "Rebuild the text index over live + archived cards from scratch",
        after_help = "The archive is maestro's memory: list --grep [--archived] searches it,\nand the index keeps that search fast as the store grows. The index is\nlocal derived state (.maestro/index/); reads fall back to a plain scan\nwhenever it is missing or stale, so rebuilding is recovery, not setup."
    )]
    Rebuild,
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
    #[command(about = "Run the MCP server over stdio (same as serve)")]
    Stdin,
    #[command(about = "List the MCP tool names maestro exposes")]
    Tools,
    #[command(about = "List the MCP tool names maestro exposes (same as tools)")]
    List,
}

#[derive(Debug, Args)]
pub struct WatchArgs {
    #[command(subcommand)]
    pub command: WatchCommand,
}

#[derive(Debug, Subcommand)]
pub enum WatchCommand {
    Snapshot,
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
    match cli.command {
        RootCommand::Init(args) => init::run(args),
        RootCommand::Install(args) => install::run(args),
        RootCommand::Upgrade(args) => update::run(args),
        RootCommand::Sync(args) => sync::run(args),
        RootCommand::MigrateV2 => migrate::run(),
        RootCommand::Migrate => migrate::run_card_fold(),
        RootCommand::Uninstall(args) => uninstall::run(args),
        RootCommand::Doctor => doctor::run(),
        RootCommand::ShellInit => shell_init::run(),
        RootCommand::Status(args) => status::run(args),
        RootCommand::Resume(args) => resume::run(args),
        RootCommand::Task(args) => task::run(args),
        RootCommand::Event(args) => event::run(args),
        RootCommand::Feature(args) => feature::run(args),
        RootCommand::Decision(args) => decision::run(args),
        RootCommand::Card(args) => match args.command {
            CardCommand::Ready(args) => card::ready(args),
            CardCommand::List(args) => card::list(args),
            CardCommand::Dep(args) => card::dep(args),
            CardCommand::Archive(args) => card::archive(args),
            CardCommand::Claim(args) => card::claim(args),
            CardCommand::Note(args) => card::note(args),
            CardCommand::Create(args) => card::create(args),
            CardCommand::Show(args) => card::show(args),
            CardCommand::Update(args) => card::update(args),
            CardCommand::Close(args) => card::close(args),
        },
        RootCommand::Ready(args) => card::ready(args),
        RootCommand::List(args) => card::list(args),
        RootCommand::Dep(args) => card::dep(args),
        RootCommand::Active(args) => active::run(args),
        RootCommand::Link(args) => card::link(args),
        RootCommand::Archive(args) => card::archive(args),
        RootCommand::Claim(args) => card::claim(args),
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
        RootCommand::Watch(args) => watch::run(args),
        RootCommand::Verify { id } => verify::run(id),
        RootCommand::Version => version::run(),
    }
}

pub(super) fn actor() -> String {
    std::env::var("MAESTRO_ACTOR").unwrap_or_else(|_| "maestro".to_string())
}

pub(super) fn cli_run_id() -> String {
    for key in [
        "MAESTRO_SESSION_ID",
        "MAESTRO_RUN_ID",
        "CODEX_SESSION_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDECODE_SESSION_ID",
        // Claude Code's real per-session id; without it every CLI-path event in a
        // Claude session collapses into one cli-<date> bucket (D9).
        "CLAUDE_CODE_SESSION_ID",
    ] {
        if let Ok(value) = env::var(key)
            && !value.trim().is_empty()
        {
            // Trimmed: the raw value becomes a claim/assignee token, and
            // stray whitespace would break later equality lookups.
            return value.trim().to_string();
        }
    }
    let date = crate::foundation::core::time::utc_now_timestamp()
        .split_once('T')
        .map(|(date, _)| date.to_string())
        .unwrap_or_else(|| "1970-01-01".to_string());
    format!("cli-{date}")
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

/// The `<session>` half of a card claim identity (SPEC E6): `MAESTRO_SESSION` if
/// set, then any real per-session id the agent runtime exports, else a
/// process-unique token. Never the colliding `cli-DATE` form `cli_run_id` falls
/// back to -- a date is not a session, and would let two runs share one claim.
pub(super) fn claim_session() -> String {
    for key in [
        "MAESTRO_SESSION",
        "MAESTRO_SESSION_ID",
        "MAESTRO_RUN_ID",
        "CODEX_SESSION_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDECODE_SESSION_ID",
    ] {
        if let Ok(value) = env::var(key)
            && !value.trim().is_empty()
        {
            // Trimmed: the raw value becomes a claim/assignee token, and
            // stray whitespace would break later equality lookups.
            return value.trim().to_string();
        }
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    format!("s{}-{nanos}", std::process::id())
}

pub(super) fn detected_agent_hint() -> &'static str {
    if let Ok(agent) = env::var("MAESTRO_AGENT") {
        if agent.eq_ignore_ascii_case("claude") {
            return "claude";
        }
        if agent.eq_ignore_ascii_case("codex") {
            return "codex";
        }
    }
    if env::var_os("CLAUDECODE")
        .or_else(|| env::var_os("CLAUDE_CODE"))
        .is_some()
    {
        return "claude";
    }
    if env::var_os("CODEX_CLI")
        .or_else(|| env::var_os("CODEX_SANDBOX"))
        .is_some()
    {
        return "codex";
    }
    "<claude|codex>"
}
