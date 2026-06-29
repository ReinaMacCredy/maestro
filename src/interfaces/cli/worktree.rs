use anyhow::{Result, bail};

use crate::domain::feature::{
    self, WorktreeCleanupReceipt, WorktreeIntent, WorktreeMilestoneKind, WorktreeRecordReport,
};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{WorktreeArgs, WorktreeCommand};

pub fn run(args: WorktreeArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        WorktreeCommand::Plan {
            card,
            slug,
            branch,
            path,
            base,
            owner_checkout,
            worker_checkout,
        } => {
            let report = feature::plan_lane(
                &paths,
                &card,
                WorktreeIntent {
                    slug,
                    branch,
                    path,
                    base,
                    owner_checkout,
                    expected_worker_checkout: worker_checkout,
                },
                &utc_now_timestamp(),
            )?;
            print_report("planned", &report);
        }
        WorktreeCommand::Mark {
            card,
            slug,
            lane_created,
            merged_back,
            verified,
            commit,
        } => {
            let milestone = mark_milestone(lane_created, merged_back, verified, commit)?;
            let report = feature::mark_lane(&paths, &card, &slug, milestone, &utc_now_timestamp())?;
            print_report("marked", &report);
        }
        WorktreeCommand::CleanupRecord {
            card,
            slug,
            removed_path,
            deleted_branch,
            pruned,
            recorded_by,
        } => {
            let report = feature::record_cleanup(
                &paths,
                &card,
                &slug,
                WorktreeCleanupReceipt {
                    removed_path,
                    deleted_branch,
                    pruned_stale_metadata: pruned,
                    recorded_by: recorded_by.unwrap_or_else(super::actor),
                    recorded_at: utc_now_timestamp(),
                },
            )?;
            print_report("recorded cleanup", &report);
        }
    }

    println!("boundary: maestro recorded ledger facts only; run git commands separately");
    Ok(())
}

fn mark_milestone(
    lane_created: bool,
    merged_back: bool,
    verified: bool,
    commit: Option<String>,
) -> Result<WorktreeMilestoneKind> {
    if lane_created {
        return Ok(WorktreeMilestoneKind::LaneCreated);
    }
    if merged_back {
        return Ok(WorktreeMilestoneKind::MergedBack {
            commit: required_commit(commit, "--merged-back")?,
        });
    }
    if verified {
        return Ok(WorktreeMilestoneKind::Verified {
            commit: required_commit(commit, "--verified")?,
        });
    }
    bail!("choose one milestone: --lane-created, --merged-back, or --verified")
}

fn required_commit(commit: Option<String>, flag: &str) -> Result<String> {
    let Some(commit) = commit else {
        bail!("{flag} requires --commit <commit>");
    };
    if commit.trim().is_empty() {
        bail!("--commit must not be empty");
    }
    Ok(commit)
}

fn print_report(action: &str, report: &WorktreeRecordReport) {
    println!(
        "{action} worktree lane {} for {}",
        report.slug, report.feature_id
    );
    println!("state: {}", report.state.as_str());
}
