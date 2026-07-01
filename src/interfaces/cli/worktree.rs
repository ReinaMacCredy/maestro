use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::domain::feature::{
    self, WorktreeCleanupReceipt, WorktreeIntent, WorktreeMilestoneKind, WorktreeRecordReport,
    WorktreeSynthesisHandoff, WorktreeSynthesisState,
};
use crate::foundation::core::git;
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
        WorktreeCommand::Cleanup { card, slug, apply } => {
            cleanup_lane(&paths, &card, &slug, apply)?;
        }
        WorktreeCommand::Handoff {
            card,
            slug,
            created_by_session,
            head,
            target,
            blocker,
            verified_check,
        } => {
            let report = feature::record_synthesis_handoff(
                &paths,
                &card,
                &slug,
                WorktreeSynthesisHandoff {
                    state: WorktreeSynthesisState::NeedsSynthesis,
                    created_by_session,
                    merge_owner: None,
                    next_owner_rule: "next root/main session may claim".to_string(),
                    verified: verified_check,
                    blocker,
                    head,
                    target,
                    recorded_at: utc_now_timestamp(),
                    claimed_at: None,
                },
            )?;
            print_report("recorded synthesis handoff", &report);
            println!("merge_owner: unassigned");
            println!("next: maestro synthesize claim {} --slug {}", card, slug);
        }
    }

    println!("boundary: maestro recorded ledger facts only; run git commands separately");
    Ok(())
}

struct CleanupPlan {
    state: String,
    remove_command: String,
    record_command: String,
    blockers: Vec<String>,
}

fn cleanup_lane(paths: &MaestroPaths, card: &str, slug: &str, apply: bool) -> Result<()> {
    let lane = feature::lane_statuses(paths, card)?
        .into_iter()
        .find(|lane| lane.slug == slug)
        .with_context(|| format!("feature {card} has no worktree lane {slug}"))?;
    let plan = cleanup_plan(paths, &lane)?;
    if !apply {
        println!("cleanup dry-run for {slug}");
        println!("state: {}", plan.state);
        if plan.blockers.is_empty() {
            println!("apply: ready");
        } else {
            println!("apply: blocked");
            for blocker in &plan.blockers {
                println!("blocker: {blocker}");
            }
        }
        println!("run: {}", plan.remove_command);
        println!("record: {}", plan.record_command);
        return Ok(());
    }

    if !plan.blockers.is_empty() {
        bail!("cleanup blocked: {}", plan.blockers.join("; "));
    }

    if lane.evidence.path_exists {
        run_git(paths, &["worktree", "remove", &lane.intent.path])?;
    }
    if lane.evidence.branch_exists {
        run_git(paths, &["branch", "-d", &lane.intent.branch])?;
    }
    run_git(paths, &["worktree", "prune"])?;

    let report = feature::record_cleanup(
        paths,
        card,
        slug,
        WorktreeCleanupReceipt {
            removed_path: lane.intent.path,
            deleted_branch: lane.intent.branch,
            pruned_stale_metadata: true,
            recorded_by: super::actor(),
            recorded_at: utc_now_timestamp(),
        },
    )?;
    print_report("applied cleanup", &report);
    Ok(())
}

fn cleanup_plan(paths: &MaestroPaths, lane: &feature::WorktreeLaneStatus) -> Result<CleanupPlan> {
    let mut blockers = Vec::new();
    if lane.state != feature::WorktreeComputedState::CleanupDue {
        blockers.push(format!("state {} is not cleanup_due", lane.state.as_str()));
    }
    if !lane.evidence.path_exists {
        blockers.push("worker path is missing".to_string());
    }
    if !lane.evidence.branch_exists {
        blockers.push("worker branch is missing".to_string());
    }
    if !lane.evidence.worker_clean_or_absent {
        blockers.push("worker worktree is dirty".to_string());
    }
    if lane.evidence.active_owner {
        blockers.push("active owner still holds the feature or child task".to_string());
    }
    if lane.evidence.open_conflict {
        blockers
            .push("open Maestro conflict still references the feature or child task".to_string());
    }

    let current_head = git::head(paths.repo_root())?;
    if let (Some(current), Some(verified)) = (
        current_head.as_deref(),
        lane.milestones.verified_commit.as_deref(),
    ) && current != verified
    {
        blockers.push(format!(
            "verified commit {verified} differs from current HEAD {current}"
        ));
    }
    if let (Some(current), Some(merged)) = (
        current_head.as_deref(),
        lane.milestones.merged_back_commit.as_deref(),
    ) && current != merged
    {
        blockers.push(format!(
            "merged commit {merged} differs from current HEAD {current}"
        ));
    }

    Ok(CleanupPlan {
        state: lane.state.as_str().to_string(),
        remove_command: format!(
            "git worktree remove {} && git branch -d {} && git worktree prune",
            super::shell_word(&lane.intent.path),
            super::shell_word(&lane.intent.branch)
        ),
        record_command: format!(
            "maestro worktree cleanup-record {} --slug {} --removed-path {} --deleted-branch {} --pruned --recorded-by <agent>",
            super::shell_word(&lane.feature_id),
            super::shell_word(&lane.slug),
            super::shell_word(&lane.intent.path),
            super::shell_word(&lane.intent.branch)
        ),
        blockers,
    })
}

fn run_git(paths: &MaestroPaths, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(paths.repo_root())
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }
    bail!(
        "git {} failed\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
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
