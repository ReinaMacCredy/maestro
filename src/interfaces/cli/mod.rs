use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

pub mod decision;
pub mod doctor;
pub mod event;
pub mod feature;
pub mod hook;
pub mod improve;
pub mod init;
pub mod install;
pub mod mcp;
pub mod metrics;
pub mod migrate;
pub mod query;
pub mod shell_init;
pub mod task;
mod task_id;
pub mod uninstall;
pub mod update;
pub mod verify;
pub mod watch;

#[derive(Debug, Parser)]
#[command(
    name = "maestro",
    version = env!("CARGO_PKG_VERSION"),
    about = "Local-first agent harness CLI",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: RootCommand,
}

#[derive(Debug, Subcommand)]
pub enum RootCommand {
    Init(InitArgs),
    Install(AgentArgs),
    Update(UpdateArgs),
    Uninstall(AgentArgs),
    Doctor,
    ShellInit,
    Task(TaskArgs),
    Event(EventArgs),
    Feature(FeatureArgs),
    Decision(DecisionArgs),
    Improve(ImproveArgs),
    Query(QueryArgs),
    Metrics(MetricsArgs),
    Mcp(McpArgs),
    Hook(HookArgs),
    Migrate(MigrateArgs),
    Watch(WatchArgs),
    Verify { id: Option<String> },
    Identity,
}

#[derive(Debug, Args)]
#[command(group(
    clap::ArgGroup::new("mode")
        .args(["dry_run", "merge", "force"])
        .multiple(false)
))]
pub struct InitArgs {
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub merge: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct AgentArgs {
    #[arg(long, value_enum, default_value = "codex")]
    pub agent: Agent,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub verbose: bool,
    #[arg(long)]
    pub force: bool,
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
    Explore {
        id: String,
    },
    #[command(about = "Lock acceptance and mark the task ready (-> ready)")]
    Accept {
        id: String,
    },
    #[command(about = "Claim a ready, unblocked task to work on it (-> in_progress)")]
    Claim {
        id: String,
    },
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
    },
    #[command(about = "Run the evidence gate; on pass marks the task verified")]
    Verify {
        id: Option<String>,
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
    Show {
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
        #[arg(
            long,
            help = "Include terminal/done tasks (verified, rejected, abandoned, superseded)"
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
}

#[derive(Debug, Args)]
pub struct EventArgs {
    #[command(subcommand)]
    pub command: EventCommand,
}

#[derive(Debug, Subcommand)]
pub enum EventCommand {
    Create {
        #[arg(long)]
        task_id: Option<String>,
        #[arg(long)]
        message: Option<String>,
        #[arg(long)]
        payload: Option<String>,
        #[arg(long)]
        claim: Vec<String>,
        #[arg(long, default_value = "manual")]
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
        #[arg(long = "type", help = "Replace the input type (e.g. bug_report, refactor)")]
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
        #[arg(long = "add-acceptance", help = "Acceptance criterion to add (repeatable)")]
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
    },
    #[command(about = "Show a feature's status, full contract, and task counts")]
    Show { id: String },
    #[command(about = "List features with their statuses and task counts")]
    List {
        #[arg(long, help = "Include terminal features (shipped, cancelled)")]
        all: bool,
    },
}

#[derive(Debug, Args)]
pub struct DecisionArgs {
    #[command(subcommand)]
    pub command: DecisionCommand,
}

#[derive(Debug, Subcommand)]
pub enum DecisionCommand {
    New { title: String },
    Show { id: String },
    List,
}

#[derive(Debug, Args)]
pub struct ImproveArgs {
    #[command(subcommand)]
    pub command: ImproveCommand,
}

#[derive(Debug, Subcommand)]
pub enum ImproveCommand {
    List,
    Show { id: String },
    Apply { id: String },
}

#[derive(Debug, Args)]
pub struct QueryArgs {
    #[command(subcommand)]
    pub command: QueryCommand,
}

#[derive(Debug, Subcommand)]
pub enum QueryCommand {
    Matrix,
    Friction,
    Decisions,
    Backlog,
    Proof {
        task_id: Option<String>,
        #[arg(long = "task-id")]
        task_id_flag: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct MetricsArgs {
    #[command(subcommand)]
    pub command: MetricsCommand,
}

#[derive(Debug, Subcommand)]
pub enum MetricsCommand {
    Summary,
}

#[derive(Debug, Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommand,
}

#[derive(Debug, Subcommand)]
pub enum McpCommand {
    #[command(alias = "stdio")]
    Serve,
    Stdin,
    Tools,
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

#[derive(Debug, Args)]
pub struct MigrateArgs {
    #[arg(long)]
    pub check: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub project: Option<PathBuf>,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        RootCommand::Init(args) => init::run(args),
        RootCommand::Install(args) => install::run(args),
        RootCommand::Update(args) => update::run(args),
        RootCommand::Uninstall(args) => uninstall::run(args),
        RootCommand::Doctor => doctor::run(),
        RootCommand::ShellInit => shell_init::run(),
        RootCommand::Task(args) => task::run(args),
        RootCommand::Event(args) => event::run(args),
        RootCommand::Feature(args) => feature::run(args),
        RootCommand::Decision(args) => decision::run(args),
        RootCommand::Improve(args) => improve::run(args),
        RootCommand::Query(args) => query::run(args),
        RootCommand::Metrics(args) => metrics::run(args),
        RootCommand::Mcp(args) => mcp::run(args),
        RootCommand::Hook(args) => hook::run(args),
        RootCommand::Migrate(args) => migrate::run(args),
        RootCommand::Watch(args) => watch::run(args),
        RootCommand::Verify { id } => verify::run(id),
        RootCommand::Identity => {
            println!("maestro {}", env!("CARGO_PKG_VERSION"));
            println!("binary: {}", std::env::args().next().unwrap_or_default());
            Ok(())
        }
    }
}

pub(super) fn actor() -> String {
    std::env::var("MAESTRO_ACTOR").unwrap_or_else(|_| "maestro".to_string())
}
