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
pub mod uninstall;
pub mod update;
pub mod verify;
pub mod watch;

#[derive(Debug, Parser)]
#[command(
    name = "maestro",
    version = env!("MAESTRO_BUILD_VERSION"),
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
    Create {
        title: String,
        #[arg(long)]
        feature: Option<String>,
        #[arg(long)]
        lane: Option<String>,
        #[arg(long)]
        risk: Option<String>,
    },
    Explore {
        id: String,
    },
    Accept {
        id: String,
    },
    Claim {
        id: String,
    },
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
    Verify {
        id: Option<String>,
    },
    Update {
        id: String,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        claim: Vec<String>,
    },
    Block {
        id: String,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        by: Option<String>,
    },
    Unblock {
        id: String,
        #[arg(long)]
        blocker: String,
    },
    Reject {
        id: String,
        #[arg(long)]
        reason: String,
    },
    Abandon {
        id: String,
        #[arg(long)]
        reason: String,
    },
    Supersede {
        id: String,
        #[arg(long)]
        by: String,
        #[arg(long)]
        reason: String,
    },
    Show {
        id: Option<String>,
    },
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
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        interval: Option<u64>,
    },
    Watch {
        id: Option<String>,
        #[arg(long)]
        interval: Option<u64>,
    },
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
        #[arg(long, alias = "kind", default_value = "task_proof")]
        event: String,
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
    New { title: String },
    Show { id: String },
    List,
    Edit { id: String },
    Ship { id: String },
    Cancel { id: String },
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
        RootCommand::Verify { id } => task::run(TaskArgs {
            command: TaskCommand::Verify { id },
        }),
        RootCommand::Identity => {
            println!("maestro {}", env!("CARGO_PKG_VERSION"));
            println!("binary: {}", std::env::args().next().unwrap_or_default());
            Ok(())
        }
    }
}
