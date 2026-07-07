use anyhow::Result;

use crate::domain::extraction;
use crate::domain::harness;
use crate::domain::install::{self, InstallMirrorAction};
use crate::domain::skills;
use crate::foundation::core::paths::{MaestroPaths, announce_repo_root, discover_repo_root};
use crate::interfaces::cli::{Agent, InstallArgs};

/// Execute `maestro install --agent`.
pub fn run(args: InstallArgs) -> Result<()> {
    let agent = install::InstallAgent::from(args.agent());
    let repo_root = discover_repo_root()?;
    announce_repo_root(&repo_root);
    let paths = MaestroPaths::new(repo_root);
    harness::ensure_harness_protocol_exists(&paths)?;
    extraction::ensure_hook_script_exists(&paths)?;
    if args.dry_run {
        let preview = install::preview_install_agent(&paths, agent)?;
        print_install_preview(&preview);
        match skills::prepare_global_skills() {
            Ok(prepared) => print!("{}", skills::render_global_skills_dry_run(&prepared)),
            Err(error) => {
                println!("warning: global skill dry-run failed: {error:#}");
                println!("repair, then rerun `maestro sync --global-skills --dry-run`");
            }
        }
        return Ok(());
    }
    install::install_agent(&paths, agent)?;
    // The mirror writes above print their diffs; close with a uniform success
    // line plus the per-agent next step so both agents end the same way (T6.4).
    println!("installed maestro {} integration", agent.key());
    // A failed global sync must not fail the repo install that already landed;
    // warn and name the repair instead.
    match skills::sync_global_skills() {
        Ok(outcome) => print!("{}", skills::render_global_skills_outcome(&outcome)),
        Err(error) => {
            println!("warning: global skill sync failed: {error:#}");
            println!("repair, then rerun `maestro sync --global-skills`");
        }
    }
    match agent {
        install::InstallAgent::Claude => {
            println!("Claude hooks are active automatically; no approval step needed.");
        }
        install::InstallAgent::Codex => {
            println!("Run /hooks in Codex to approve the maestro hook.");
        }
        install::InstallAgent::Droid => {
            println!("Droid hooks were written to .factory/hooks.json.");
        }
    }
    let readout = {
        use crate::operations::harness;
        harness::complete_readout(&paths)?
    };
    println!("{}", readout.hook_trace_summary_line());
    println!("{}", readout.runtime_summary_line());

    Ok(())
}

fn print_install_preview(preview: &install::InstallPreview) {
    println!("install dry-run: {}", preview.agent.key());
    println!(
        "safety: writes=false backup_if_changed=true managed-block refresh=true shim refresh=true stale-resource detection=maestro sync --dry-run resource guards=tests/resources_version_guard.rs"
    );
    for mirror in &preview.mirrors {
        let action = match mirror.action {
            InstallMirrorAction::Current => "current",
            InstallMirrorAction::Create => "create",
            InstallMirrorAction::Refresh => "refresh",
        };
        let backup = if mirror.backup_if_changed {
            "backup"
        } else {
            "no-backup"
        };
        let managed = if mirror.managed_block_refresh {
            "managed-block"
        } else {
            "plain"
        };
        let shim = if mirror.shim_refresh {
            "shim refresh"
        } else {
            "resource"
        };
        println!(
            "- {} action={} kind={:?} {} {} {}",
            mirror.relative_path, action, mirror.kind, backup, managed, shim
        );
    }
}

impl From<Agent> for install::InstallAgent {
    fn from(agent: Agent) -> Self {
        match agent {
            Agent::Claude => install::InstallAgent::Claude,
            Agent::Codex => install::InstallAgent::Codex,
            Agent::Droid => install::InstallAgent::Droid,
        }
    }
}
