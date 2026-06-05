use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

pub mod decision;
pub mod doctor;
pub mod event;
pub mod feature;
pub mod harness;
pub mod hook;
pub mod init;
pub mod install;
pub mod mcp;
pub mod query;
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
        after_help = "Examples:\n  maestro update               # upgrade to the latest release and refresh resources\n  maestro update --check       # report whether an update is available, install nothing\n  maestro update --force       # reinstall the latest even when already up to date"
    )]
    Update(UpdateArgs),
    #[command(
        about = "Resync bundled resources to this binary's versions (offline)",
        after_help = "Examples:\n  maestro sync                 # resync repo bundled resources to this binary, preserving edits\n  maestro sync --global-skills # resync user-level Maestro skill cache and links\n  maestro sync --dry-run       # preview the resync, write nothing"
    )]
    Sync(SyncArgs),
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
    #[command(about = "Manage tasks: create, claim, complete, verify, and query")]
    Task(TaskArgs),
    #[command(about = "Record run events from the agent harness")]
    Event(EventArgs),
    #[command(about = "Manage features: the product contract and its lifecycle")]
    Feature(FeatureArgs),
    #[command(about = "Create, show, and list decision records in .maestro/decisions/")]
    Decision(DecisionArgs),
    #[command(about = "List, show, apply, dismiss, and measure harness improvement suggestions")]
    Harness(HarnessArgs),
    #[command(about = "Query computed read models (matrix, friction, decisions, proof, backlog)")]
    Query(QueryArgs),
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
pub struct UpdateArgs {
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
    },
    #[command(about = "Move a draft into exploring (-> exploring)")]
    Explore { id: String },
    #[command(about = "Lock acceptance and mark the task ready (-> ready)")]
    Accept { id: String },
    #[command(about = "Claim a ready, unblocked task to work on it (-> in_progress)")]
    Claim { id: String },
    #[command(about = "Submit work for verification (-> needs_verification)")]
    Complete {
        id: String,
        #[arg(long)]
        summary: String,
        #[arg(
            long,
            help = "Completion claim; hook-backed tool proof uses '<tool> <tool_input_hash>'"
        )]
        claim: String,
        #[arg(
            long,
            help = "Observed proof text to record before automatic verification"
        )]
        proof: Option<String>,
    },
    #[command(about = "Run the evidence gate; on pass marks the task verified")]
    Verify { id: Option<String> },
    #[command(about = "Print the next task action for the current repo")]
    Next {
        #[arg(long, help = "Print machine-readable next-action JSON")]
        json: bool,
    },
    #[command(about = "Record progress (summary and/or claims) without changing state")]
    Update {
        id: String,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
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
        #[arg(long, default_value = "manual", help = "Run label grouping the event")]
        run: String,
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
    New { title: String },
    #[command(about = "Author a proposed feature's contract (replace-per-field; proposed only)")]
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
            help = "Open question (repeatable); replaces the current questions list"
        )]
        question: Vec<String>,
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
        #[arg(long, help = "Preview the accept gate without transitioning")]
        dry_run: bool,
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
        #[arg(help = "Feature id to archive (omit when using --shipped)")]
        id: Option<String>,
        #[arg(
            long,
            help = "Archive every shipped feature (mutually exclusive with <id>)"
        )]
        shipped: bool,
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
    #[command(about = "Create a decision record (-> decision-NN)")]
    New { title: String },
    #[command(about = "Show a decision record by id (decision-NN)")]
    Show { id: String },
    #[command(about = "List decision records")]
    List,
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
    #[command(about = "Accept a proposal and spawn a linked task (-> accepted)")]
    Apply { id: String },
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
    #[command(about = "List decision records (ID/FILE/TITLE)")]
    Decisions,
    #[command(about = "List improvement backlog items (ID/TITLE)")]
    Backlog,
    #[command(about = "Show a task's proof status")]
    Proof {
        task_id: Option<String>,
        #[arg(long = "task-id", value_name = "TASK_ID")]
        task_id_flag: Option<String>,
    },
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
    Record,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        RootCommand::Init(args) => init::run(args),
        RootCommand::Install(args) => install::run(args),
        RootCommand::Update(args) => update::run(args),
        RootCommand::Sync(args) => sync::run(args),
        RootCommand::Uninstall(args) => uninstall::run(args),
        RootCommand::Doctor => doctor::run(),
        RootCommand::ShellInit => shell_init::run(),
        RootCommand::Status(args) => status::run(args),
        RootCommand::Task(args) => task::run(args),
        RootCommand::Event(args) => event::run(args),
        RootCommand::Feature(args) => feature::run(args),
        RootCommand::Decision(args) => decision::run(args),
        RootCommand::Harness(args) => harness::run(args),
        RootCommand::Query(args) => query::run(args),
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
