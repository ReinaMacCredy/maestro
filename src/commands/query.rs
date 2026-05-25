use anyhow::Result;

use crate::commands::{QueryArgs, QueryCommand};
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::verification::proof_status::{proof_status, render_proof_status};

/// Execute `maestro query`.
pub fn run(args: QueryArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        QueryCommand::Proof { task_id } => {
            let status = proof_status(&paths, &task_id)?;
            print!("{}", render_proof_status(&status));
            Ok(())
        }
        QueryCommand::Matrix
        | QueryCommand::Friction
        | QueryCommand::Decisions
        | QueryCommand::Backlog => {
            println!("query is not implemented in this phase slice");
            Ok(())
        }
    }
}
