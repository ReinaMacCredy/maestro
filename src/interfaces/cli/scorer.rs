use anyhow::Result;

use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::operations::harness;
use crate::operations::memory::{list_scorer_receipts, run_scorer, show_scorer_receipt};

use super::{ScorerArgs, ScorerCommand};

pub fn run(args: ScorerArgs) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    match args.command {
        ScorerCommand::Run { contract_ref } => {
            let outcome = run_scorer(&paths, &contract_ref, &utc_now_timestamp())?;
            println!(
                "scorer receipt {} {} memory={} type={} path={}",
                outcome.receipt.id,
                outcome.receipt.status.as_str(),
                outcome.receipt.memory_id,
                outcome.receipt.scorer_type.as_str(),
                outcome.path.display()
            );
            println!("{}", harness::guardrail_scorer_line());
            Ok(())
        }
        ScorerCommand::Show { receipt_ref } => {
            let receipt = show_scorer_receipt(&paths, &receipt_ref)?;
            println!("{}", serde_json::to_string_pretty(&receipt)?);
            println!("{}", harness::guardrail_scorer_line());
            Ok(())
        }
        ScorerCommand::List { memory } => {
            let receipts = list_scorer_receipts(&paths, &memory)?;
            if receipts.is_empty() {
                println!("no scorer receipts for {memory}");
                println!("{}", harness::guardrail_scorer_line());
                return Ok(());
            }
            for (receipt, path) in receipts {
                println!(
                    "{} {} memory={} type={} path={}",
                    receipt.id,
                    receipt.status.as_str(),
                    receipt.memory_id,
                    receipt.scorer_type.as_str(),
                    path.display()
                );
            }
            println!("{}", harness::guardrail_scorer_line());
            Ok(())
        }
    }
}
