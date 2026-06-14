//! Feature ship coordinator: the full `stack.verify` suite backstop at the ship gate.
//!
//! decision-002 pairs the per-task narrow falsifier at task-verify with a full
//! repo-global `stack.verify` run at `feature ship`. The suite is a ship ACTION,
//! not an evidence gap: it must run only on a REAL ship (after the evidence gaps
//! pass), never on `--dry-run`, `maestro status`, or the verify handoff. The
//! feature domain owns the evidence gate; this operation layers the suite run on
//! top so the feature aggregate never reaches into proof's command runner.

use anyhow::{Result, bail};

use crate::domain::{feature, proof};
use crate::foundation::core::paths::MaestroPaths;

/// Coordinate `feature ship`: evidence gate -> full suite (real ship only) -> transition.
///
/// `--dry-run` previews the evidence gate and states that the suite WOULD run,
/// without executing it. On a real ship, once the evidence gaps pass, the full
/// `stack.verify` suite runs from the repo root; a failing command blocks the
/// ship and the feature stays `in_progress`.
pub(crate) fn ship(
    paths: &MaestroPaths,
    id: &str,
    outcome: Option<String>,
    dry_run: bool,
) -> Result<feature::TransitionReport> {
    if dry_run {
        // Pure preview: the domain gate decides ship-ability; the suite is not run.
        return feature::ship(paths, id, outcome, true);
    }

    // Run the full suite only once the evidence gate is clear, so a feature with
    // unresolved gaps fails fast on the cheaper check rather than after a slow suite.
    let gaps = feature::ship_gaps(paths, id)?;
    if gaps.is_empty() {
        let suite = proof::run_stack_verify(paths)?;
        let failed = suite.failed();
        if !failed.is_empty() {
            let lines = failed
                .iter()
                .map(|command| format!("{} (exit {})", command.cmd, command.exit_code))
                .collect::<Vec<_>>()
                .join("\n  ");
            bail!(
                "cannot ship {id}: full verify suite failed (stack: {})\n  {lines}\n  fix: make the suite green, then re-ship\n  retry: maestro feature ship {id} --outcome \"<outcome>\"",
                suite.stack_kind
            );
        }
    }

    // Real transition: the domain re-checks the evidence gate (it bails on gaps),
    // so a feature that did not clear above never reaches the suite or the flip.
    feature::ship(paths, id, outcome, false)
}
