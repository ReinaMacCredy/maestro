use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::domain::card;
use crate::domain::decisions;
use crate::domain::feature::{
    self, ContractAdditions, ContractChangeCounts, ContractEdits, FeatureStatus,
};
use crate::domain::run;
use crate::domain::task;
use crate::foundation::core::git;
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::EVENT_SCHEMA_VERSION;
use crate::foundation::core::session::agent_runtime_from_env;
use crate::foundation::core::table;
use crate::foundation::core::time::{render_timestamp, timestamp_nanos, utc_now_timestamp};
use crate::interfaces::cli::{
    FeatureArgs, FeatureCommand, FeatureProofCommand, feature_next_label, recovery_label,
};
use crate::operations::{feature_close, feature_prepare};

/// Execute `maestro feature`.
pub fn run(args: FeatureArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        FeatureCommand::New {
            title,
            description,
            question,
            project,
            id_only,
        } => new_feature(&paths, &title, description, question, project, id_only),
        FeatureCommand::Set {
            id,
            acceptance,
            area,
            non_goal,
            question,
            clear_questions,
            add_acceptance,
            add_area,
            add_non_goal,
            add_question,
            edit_acceptance,
            text,
            description,
            request,
            input_type,
        } => set_feature(
            &paths,
            &id,
            ContractEdits {
                acceptance: opt_list(acceptance),
                affected_areas: opt_list(area),
                non_goals: opt_list(non_goal),
                open_questions: if !question.is_empty() {
                    opt_list(question)
                } else if clear_questions {
                    Some(Vec::new())
                } else {
                    None
                },
                description,
                raw_request: request,
                input_type,
                add_acceptance,
                add_affected_areas: add_area,
                add_non_goals: add_non_goal,
                add_open_questions: add_question,
                edit_acceptance: paired_acceptance_edits(edit_acceptance, text)?,
            },
        ),
        FeatureCommand::Finalize { id } => finalize_feature(&paths, &id),
        FeatureCommand::Accept {
            id,
            qa,
            reason,
            dry_run,
        } => {
            let report = accept_feature(&paths, &id, qa, reason, dry_run)?;
            if !dry_run && report.changed {
                super::emit_work_touch(&paths, &report.id);
            }
            print_note(report.note)?;
            if report.changed && report.status == FeatureStatus::Ready {
                println!("next: maestro feature prepare {} --draft", report.id);
            }
            if !dry_run {
                let _ = super::active::worktree_advisory(&paths);
            }
            Ok(())
        }
        FeatureCommand::Prepare {
            id,
            from,
            draft,
            task,
            check,
            covers,
            blocker,
            after,
        } => {
            prepare_feature(
                &paths,
                &id,
                from.as_deref(),
                draft,
                InlinePrepareArgs {
                    tasks: task,
                    checks: check,
                    covers,
                    blockers: blocker,
                    after,
                },
            )?;
            let _ = super::active::worktree_advisory(&paths);
            Ok(())
        }
        FeatureCommand::Amend {
            id,
            add_acceptance,
            add_area,
            add_non_goal,
            add_question,
            reason,
        } => amend_feature(
            &paths,
            &id,
            ContractAdditions {
                acceptance: add_acceptance,
                affected_areas: add_area,
                non_goals: add_non_goal,
                open_questions: add_question,
            },
            &reason,
        ),
        FeatureCommand::Start { id } => {
            let report = feature::start(&paths, &id)?;
            super::emit_work_touch(&paths, &id);
            print_note(report.note)?;
            print_uncovered_acceptance_warning(&paths, &id, CoverageFix::Locked)
        }
        FeatureCommand::Verify {
            id,
            prove,
            evidence,
            waive,
            reason,
            no_close,
            outcome,
        } => verify_feature(
            &paths, &id, prove, evidence, waive, reason, no_close, outcome,
        ),
        FeatureCommand::Proof { command } => match command {
            FeatureProofCommand::Add {
                id,
                ac,
                evidence,
                no_close,
                outcome,
            } => verify_feature(
                &paths,
                &id,
                vec![ac],
                vec![evidence],
                Vec::new(),
                Vec::new(),
                no_close,
                outcome,
            ),
            FeatureProofCommand::Waive { id, ac, reason } => verify_feature(
                &paths,
                &id,
                Vec::new(),
                Vec::new(),
                vec![ac],
                vec![reason],
                true,
                None,
            ),
        },
        FeatureCommand::Note { id, text } => {
            let report = feature::note(&paths, &id, &text)?;
            super::emit_work_touch(&paths, &report.id);
            if report.created {
                println!("noted {} (notes.md created)", report.id);
            } else {
                println!("noted {}", report.id);
            }
            println!("  {}", report.line);
            Ok(())
        }
        FeatureCommand::Close {
            id,
            outcome,
            dry_run,
        } => close_feature(&paths, &id, outcome, dry_run),
        FeatureCommand::Cancel {
            id,
            reason,
            dry_run,
        } => cancel_feature(&paths, &id, &reason, dry_run),
        FeatureCommand::Show { id } => show_feature(&paths, &id),
        FeatureCommand::Spec {
            id,
            section,
            append,
            replace,
        } => feature_spec(&paths, &id, section, append, replace),
        FeatureCommand::List { all } => list_features(&paths, all),
        FeatureCommand::Archive {
            id,
            closed,
            dry_run,
        } => archive_features(&paths, id, closed, dry_run),
        FeatureCommand::AutoArchive {
            id,
            authority_ref,
            authority_target,
            authority_head,
            authority_state,
            tested_head,
            qa_result,
            qa_evidence,
            run,
            multi_agent,
            canonical_store,
            worker_source,
            target_card_hash,
            dry_run,
        } => auto_archive_feature(
            &paths,
            AutoArchiveArgs {
                id,
                authority_ref,
                authority_target,
                authority_head,
                authority_state,
                tested_head,
                qa_result,
                qa_evidence,
                run_id: run,
                multi_agent,
                canonical_store,
                worker_source,
                target_card_hash,
                dry_run,
            },
        ),
        FeatureCommand::Unarchive { id } => match feature::unarchive_feature(&paths, &id) {
            Ok(note) => {
                print_feature_unarchive_note(&id, &note);
                Ok(())
            }
            Err(error) => bail!(
                "{}",
                feature_unarchive_error_message(&id, &error.to_string())
            ),
        },
    }
}

fn accept_feature(
    paths: &MaestroPaths,
    id: &str,
    qa: Option<String>,
    reason: Option<String>,
    dry_run: bool,
) -> Result<feature::TransitionReport> {
    match (qa.as_deref(), reason.as_deref()) {
        (None, None) => feature::accept(paths, id, dry_run),
        (Some("none"), Some(reason)) if reason.trim().is_empty() => {
            bail!("--reason must not be empty with --qa none")
        }
        (Some("none"), Some(reason)) => feature::accept_with_qa_none(paths, id, reason, dry_run),
        (Some("none"), None) => bail!("--reason is required with --qa none"),
        (Some(other), _) => bail!("unsupported --qa value `{other}`; only `--qa none` is accepted"),
        (None, Some(_)) => bail!("--reason requires --qa none"),
    }
}

fn finalize_feature(paths: &MaestroPaths, id: &str) -> Result<()> {
    let report = feature::finalize(paths, id)?;
    super::emit_card_touch(paths, id);
    println!("finalized {}", report.id);
    println!("handoff: {}", report.path.display());
    println!("source_sha256: {}", report.fingerprint);
    if !report.next_commands.is_empty() {
        println!("next:");
        for command in report.next_commands {
            println!("  {command}");
        }
    }
    Ok(())
}

fn prepare_feature(
    paths: &MaestroPaths,
    id: &str,
    plan_file: Option<&std::path::Path>,
    draft: bool,
    inline: InlinePrepareArgs,
) -> Result<()> {
    let has_inline_tasks = !inline.tasks.is_empty();
    match (plan_file, draft, has_inline_tasks) {
        (Some(_), true, _) => bail!("use either --from <plan-file> or --draft, not both"),
        (Some(_), _, true) => bail!("use either --from <plan-file> or --task, not both"),
        (_, true, true) => bail!("use either --draft or --task, not both"),
        (None, false, false) => bail!(
            "feature prepare requires --from <plan-file>, --draft, or --task\n  maestro feature prepare {id} --draft\n  maestro feature prepare {id} --from <plan-file>\n  maestro feature prepare {id} --task \"T1: <title>\" --check \"<observable result>\""
        ),
        (None, true, false) => {
            let report = feature_prepare::write_draft(paths, id)?;
            if report.written {
                println!("wrote {}", report.path.display());
            } else {
                println!("draft exists: {}", report.path.display());
            }
            print_uncovered_acceptance_warning(paths, id, CoverageFix::Plan)?;
            println!("review and run:");
            println!(
                "  maestro feature prepare {id} --from {}",
                report.path.display()
            );
            Ok(())
        }
        (Some(plan_file), false, false) => {
            let actor = super::actor();
            let report = feature_prepare::prepare_from_file(paths, id, plan_file, &actor)?;
            super::emit_work_touch(paths, id);
            print_prepare_report(&report);
            print_uncovered_acceptance_warning(paths, id, CoverageFix::Locked)?;
            Ok(())
        }
        (None, false, true) => {
            if inline.checks.iter().any(|check| check.trim().is_empty()) {
                bail!("--check must not be empty");
            }
            if inline.checks.is_empty() {
                bail!("--task requires at least one --check");
            }
            let plan = inline_prepare_plan(
                &inline.tasks,
                &inline.checks,
                &inline.covers,
                &inline.blockers,
                &inline.after,
            )?;
            let path = feature::feature_sidecar_dir(paths, id).join("prepare-inline.md");
            write_string_atomic(&path, &plan)?;
            let actor = super::actor();
            let report = feature_prepare::prepare_from_file(paths, id, &path, &actor)?;
            super::emit_work_touch(paths, id);
            print_prepare_report(&report);
            print_uncovered_acceptance_warning(paths, id, CoverageFix::Locked)?;
            Ok(())
        }
    }
}

struct InlinePrepareArgs {
    tasks: Vec<String>,
    checks: Vec<String>,
    covers: Vec<String>,
    blockers: Vec<String>,
    after: Vec<String>,
}

fn inline_prepare_plan(
    tasks: &[String],
    checks: &[String],
    covers: &[String],
    blockers: &[String],
    after: &[String],
) -> Result<String> {
    let mut plan = String::new();
    for task in tasks {
        let task = task.trim();
        if task.is_empty() {
            bail!("--task must not be empty");
        }
        plan.push_str("## Task ");
        plan.push_str(task);
        plan.push('\n');
        if !covers.is_empty() {
            plan.push_str("covers: ");
            plan.push_str(&covers.join(", "));
            plan.push('\n');
        }
        for check in checks {
            plan.push_str("check: ");
            plan.push_str(check);
            plan.push('\n');
        }
        for blocker in blockers {
            plan.push_str("blocker: ");
            plan.push_str(blocker);
            plan.push('\n');
        }
        if !after.is_empty() {
            plan.push_str("after: ");
            plan.push_str(&after.join(", "));
            plan.push('\n');
        }
        plan.push('\n');
    }
    Ok(plan)
}

fn print_prepare_report(report: &feature_prepare::PrepareReport) {
    println!("prepared {} task(s)", report.task_count);
    if report.started {
        println!("started {} -> in_progress", report.feature_id);
    } else if report.remained_ready {
        println!("feature remains ready");
    }
    println!("prepared:");
    for task in &report.prepared {
        let state = if task.blocked {
            "ready / blocked"
        } else {
            "ready"
        };
        println!("  {} {:<15} {}", task.id, state, task.title);
    }
    if !report.blockers.is_empty() {
        println!("blockers:");
        for blocker in &report.blockers {
            println!(
                "  {} {} {}",
                blocker.task_id, blocker.blocker_id, blocker.reason
            );
        }
    }
    if report.ready_count > 0 {
        println!("next: maestro task claim --next");
    } else {
        println!(
            "next: maestro task list --feature {} --blocked",
            report.feature_id
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_feature(
    paths: &MaestroPaths,
    id: &str,
    prove: Vec<String>,
    evidence: Vec<String>,
    waive: Vec<String>,
    reason: Vec<String>,
    no_close: bool,
    outcome: Option<String>,
) -> Result<()> {
    if prove.len() != evidence.len() {
        bail!("each --prove needs its --evidence");
    }
    if waive.len() != reason.len() {
        bail!("each --waive needs its --reason");
    }
    let mut updates = prove
        .into_iter()
        .zip(evidence)
        .map(|(ac_id, evidence)| feature::FeatureProofUpdate::Explicit { ac_id, evidence })
        .collect::<Vec<_>>();
    updates.extend(
        waive
            .into_iter()
            .zip(reason)
            .map(|(ac_id, reason)| feature::FeatureProofUpdate::Waive { ac_id, reason }),
    );
    let report = feature::verify_feature(paths, id, updates)?;
    if let Some(recorded) = report.recorded {
        super::emit_work_touch(paths, &report.feature_id);
        println!("recorded {recorded}");
        return after_prove_autoclose(paths, &report.feature_id, no_close, outcome);
    }
    let Some(sweep) = report.sweep else {
        return Ok(());
    };
    println!(
        "checking contract ({} acceptance items):",
        sweep.items.len()
    );
    if !sweep.invalidated_by.is_empty() {
        println!("re-derived after: {}", sweep.invalidated_by.join("; "));
    }
    for (index, item) in sweep.items.iter().enumerate() {
        println!(
            "  [{}/{}] \"{}\"   {}",
            index + 1,
            sweep.items.len(),
            item.text,
            proof_label(&item.proof)
        );
    }
    let unresolved = sweep
        .items
        .iter()
        .filter(|item| matches!(item.proof, feature::AcceptanceProof::Missing))
        .collect::<Vec<_>>();
    if unresolved.is_empty() {
        println!("ok: every acceptance item has evidence");
        print_green_sweep_next(paths, &report.feature_id)?;
    } else {
        println!(
            "blocked: {} acceptance item(s) have no fresh evidence: {}",
            unresolved.len(),
            unresolved
                .iter()
                .map(|item| item.ac_id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "fix: add task covers, record proof with `maestro feature verify {} --prove <ac-id> --evidence \"<observed>\"`, or waive with `--waive <ac-id> --reason \"<why>\"`",
            report.feature_id
        );
    }
    Ok(())
}

/// After `feature verify --prove` records evidence, re-sweep the contract and,
/// per the auto-close decisions, fire the close gate when this proof completes
/// close-readiness:
/// - dec-fully-automatic-trigger: the verify that empties `close_gaps` runs the
///   full gate (evidence + suite + terminal close) in the same call.
/// - dec-add-a-no-ship-suppressor: `--no-close` records the proof and defers.
/// - dec-keep-write-once-outcome: a generated AC-proof summary is the default
///   outcome; `--outcome` on the triggering verify overrides it.
/// - dec-foreknowledge-nudge: when exactly one acceptance item is left, warn
///   that the next `--prove` will auto-close unless `--no-close`.
/// - dec-auto-fire-gate-suite-failure: a gate/suite failure bails out of
///   `feature_close::close`, so the just-recorded proof stays, the feature stays
///   `in_progress`, and this command exits non-zero (retry via `feature close`).
fn after_prove_autoclose(
    paths: &MaestroPaths,
    id: &str,
    no_close: bool,
    outcome: Option<String>,
) -> Result<()> {
    // Refresh the acceptance sweep so close-readiness reflects the proof just
    // recorded: the close gate's acceptance check requires a fresh contract sweep,
    // which only a bare `feature verify` produces.
    let sweep = feature::verify_feature(paths, id, Vec::new())?.sweep;
    let unresolved = sweep
        .as_ref()
        .map(|report| {
            report
                .items
                .iter()
                .filter(|item| matches!(item.proof, feature::AcceptanceProof::Missing))
                .count()
        })
        .unwrap_or(0);

    if no_close {
        println!(
            "--no-close: proof recorded; auto-close deferred. close when ready: maestro feature close {id} --outcome \"<outcome>\""
        );
        return Ok(());
    }

    let gaps = feature::close_gaps(paths, id)?;
    if gaps.is_empty() {
        let outcome = Some(outcome.unwrap_or_else(|| default_close_outcome(sweep.as_ref())));
        println!("close-ready: auto-closing (full verify suite + close)");
        let report = feature_close::close(paths, id, outcome, false)?;
        if report.changed && report.status == FeatureStatus::Closed {
            super::emit_ownership_release(
                paths,
                &report.id,
                super::OwnershipReleaseStatus::Done,
                Some("feature close"),
            );
        }
        println!("{}", report.note);
        print_close_receipt(paths, &report)?;
        return Ok(());
    }

    if unresolved == 1 {
        eprintln!(
            "note: 1 acceptance item left; the next `maestro feature verify {id} --prove` will auto-close (full verify suite + close) unless you pass --no-close"
        );
    }
    println!("not yet closable:");
    println!("  {}", gaps.join("\n  "));
    println!("next: maestro feature verify {id} --prove <ac-id> --evidence \"<observed>\"");
    Ok(())
}

/// The write-once outcome recorded on an auto-close when the agent passed no
/// `--outcome`: a terse AC-proof summary derived from the just-run sweep.
fn default_close_outcome(sweep: Option<&feature::AcceptanceSweepReport>) -> String {
    match sweep {
        Some(report) if !report.items.is_empty() => {
            let ids = report
                .items
                .iter()
                .map(|item| item.ac_id.as_str())
                .collect::<Vec<_>>();
            format!("{} acceptance proven: {}", ids.len(), ids.join(", "))
        }
        _ => "acceptance proven".to_string(),
    }
}

/// Print the post-close receipt, shared by explicit `feature close` and the
/// auto-close triggered from `feature verify --prove`.
fn print_close_receipt(paths: &MaestroPaths, report: &feature::TransitionReport) -> Result<()> {
    println!("close receipt:");
    println!("  feature: {}", report.id);
    println!("  status: closed");
    println!("  full verify suite passed");
    if let Ok(view) = feature::show(paths, &report.id)
        && let Some(reason) = view.qa_none_reason.as_deref()
    {
        println!("  qa: none ({reason})");
    }
    let claims_only = claims_only_verified_count(paths, &report.id)?;
    if claims_only > 0 {
        println!("  verification: {claims_only} claims-only task(s)");
    }
    println!("inspect: maestro feature show {}", report.id);
    println!("next: maestro card archive {}", report.id);
    println!("retro: anything to make a permanent rule?");
    println!("  record it: maestro harness propose --title \"<rule>\" --evidence \"<why>\"");
    Ok(())
}

fn print_green_sweep_next(paths: &MaestroPaths, feature_id: &str) -> Result<()> {
    // Only the lifecycle status drives the next-step hint; avoid show's task,
    // coverage, and note joins (the InProgress arm re-loads via close_gaps anyway).
    match feature::status(paths, feature_id)? {
        FeatureStatus::Proposed => {}
        FeatureStatus::Ready => println!("next: maestro feature start {feature_id}"),
        FeatureStatus::InProgress => {
            let gaps = feature::close_gaps(paths, feature_id)?;
            if gaps.is_empty() {
                println!("next: maestro feature close {feature_id} --outcome \"<outcome>\"");
            } else {
                println!("not yet closable:");
                println!("  {}", gaps.join("\n  "));
            }
        }
        FeatureStatus::Closed | FeatureStatus::Cancelled => {}
    }
    Ok(())
}

/// Dispatch `feature archive`: exactly one of a single id or `--closed`.
fn archive_features(
    paths: &MaestroPaths,
    id: Option<String>,
    closed: bool,
    dry_run: bool,
) -> Result<()> {
    match (id, closed) {
        (Some(id), false) => match feature::archive_feature(paths, &id, dry_run) {
            Ok(report) => {
                print_feature_archive_note(&id, &report, dry_run);
                Ok(())
            }
            Err(error) => bail!("{}", feature_archive_error_message(&id, &error.to_string())),
        },
        (None, true) => archive_closed(paths, dry_run),
        (Some(_), true) => bail!(
            "provide a feature id or --closed, not both\n  maestro feature archive <id>\n  maestro feature archive --closed"
        ),
        (None, false) => bail!(
            "provide a feature id or --closed\n  maestro feature archive <id>\n  maestro feature archive --closed"
        ),
    }
}

#[derive(Debug)]
struct AutoArchiveArgs {
    id: String,
    authority_ref: String,
    authority_target: String,
    authority_head: String,
    authority_state: String,
    tested_head: String,
    qa_result: String,
    qa_evidence: Vec<String>,
    run_id: String,
    multi_agent: String,
    canonical_store: String,
    worker_source: String,
    target_card_hash: Option<String>,
    dry_run: bool,
}

fn auto_archive_feature(paths: &MaestroPaths, args: AutoArchiveArgs) -> Result<()> {
    let id = required_cli_value("feature id", &args.id)?;
    let authority_ref = required_cli_value("--authority-ref", &args.authority_ref)?;
    let authority_target = required_cli_value("--authority-target", &args.authority_target)?;
    let authority_head = required_cli_value("--authority-head", &args.authority_head)?;
    let authority_state = required_cli_value("--authority-state", &args.authority_state)?;
    let tested_head = required_cli_value("--tested-head", &args.tested_head)?;
    let qa_result = required_cli_value("--qa-result", &args.qa_result)?;
    let run_id = required_cli_value("--run", &args.run_id)?;
    let multi_agent = required_cli_value("--multi-agent", &args.multi_agent)?;
    let canonical_store = required_cli_value("--canonical-store", &args.canonical_store)?;
    let worker_source = required_cli_value("--worker-source", &args.worker_source)?;
    let target_card_hash = args
        .target_card_hash
        .as_deref()
        .map(|hash| required_cli_value("--target-card-hash", hash))
        .transpose()?;
    let qa_evidence = required_cli_values("--qa-evidence", args.qa_evidence)?;

    let current_store_path = canonical_path(&paths.maestro_dir());
    let canonical_store_path = canonical_path(Path::new(&canonical_store));
    let invoking_checkout_path = canonical_path(paths.repo_root());
    let current_store = current_store_path.display().to_string();
    let canonical_store = canonical_store_path.display().to_string();
    let invoking_checkout = invoking_checkout_path.display().to_string();
    if current_store_path != canonical_store_path {
        bail!(
            "cannot auto-archive {id} — current store `{}` is not canonical store `{}`; run from the owning/orchestrator checkout that owns the live target card",
            current_store,
            canonical_store
        );
    }
    let target_card_path = paths.cards_dir().join(&id).join("card.yaml");
    if !target_card_path.is_file() {
        bail!(
            "cannot auto-archive {id} — target feature is missing from current store `{}`; run from the owning/orchestrator checkout that owns the live target card",
            current_store
        );
    }
    if let Some(expected_hash) = target_card_hash.as_deref() {
        let target_card_bytes = fs::read(&target_card_path)?;
        let actual_hash = sha256_prefixed(&target_card_bytes);
        if expected_hash != actual_hash {
            bail!(
                "cannot auto-archive {id} — target card changed since preflight (expected {expected_hash}, found {actual_hash}); re-run the command"
            );
        }
    }

    if authority_target != id {
        bail!(
            "cannot auto-archive {id} — authority target `{authority_target}` does not match feature id `{id}`; retry with a current target-scoped authority"
        );
    }
    if authority_state != "current" {
        bail!(
            "cannot auto-archive {id} — authority `{authority_ref}` is `{authority_state}`, not current; retry with current target-scoped bounded ship authority"
        );
    }
    if !qa_passed(&qa_result) {
        bail!("cannot auto-archive {id} — --qa-result must be pass/passed, got `{qa_result}`");
    }
    if qa_evidence.is_empty() {
        bail!("cannot auto-archive {id} — at least one --qa-evidence item is required");
    }

    let snapshot = git::snapshot(paths.repo_root())?;
    let Some(current_head) = snapshot.head.as_deref() else {
        bail!("cannot auto-archive {id} — git HEAD is unborn; commit the delivered work first");
    };
    if current_head != tested_head {
        bail!(
            "cannot auto-archive {id} — tested head {tested_head} does not match current HEAD {current_head}"
        );
    }
    if authority_head != current_head {
        bail!(
            "cannot auto-archive {id} — authority head {authority_head} does not match current HEAD {current_head}"
        );
    }
    let relevant_dirty = relevant_dirty_paths(&snapshot.dirty_paths, &id, &run_id, &qa_evidence);
    if !relevant_dirty.is_empty() {
        bail!(
            "cannot auto-archive {id} — relevant dirty path(s) at {current_head}: {}; commit or clean before archive",
            relevant_dirty.join(", ")
        );
    }
    let unresolved_conflicts = unresolved_conflicts_for_target(paths, &id)?;
    if !unresolved_conflicts.is_empty() {
        bail!(
            "cannot auto-archive {id} — unresolved Maestro conflict(s): {}; clear conflicts before archive",
            unresolved_conflicts.join(", ")
        );
    }
    let open_work = blocking_work_items(paths, &id)?;
    if !open_work.is_empty() {
        bail!(
            "cannot auto-archive {id} — live or claimed descendant/linked work item(s): {}; verify/archive or release them first",
            open_work.join(", ")
        );
    }

    let before_state = feature::status(paths, &id)
        .map(|status| status.as_str().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());

    if args.dry_run {
        let report = match feature::archive_feature(paths, &id, true) {
            Ok(report) => report,
            Err(error) => bail!("{}", feature_archive_error_message(&id, &error.to_string())),
        };
        println!("dry-run: auto-archive preflight passed for {id}");
        println!("  authority: {authority_ref}");
        println!("  canonical store: {current_store}");
        println!("  invoking checkout: {invoking_checkout}");
        println!("  worker source: {worker_source}");
        println!("  tested head: {current_head}");
        println!("  qa: {qa_result} ({} evidence item(s))", qa_evidence.len());
        println!("  multi-agent/worktree: {multi_agent}");
        print_feature_archive_note(&id, &report, true);
        println!("writes: none");
        return Ok(());
    }

    let report = match feature::archive_feature(paths, &id, false) {
        Ok(report) => report,
        Err(error) => bail!("{}", feature_archive_error_message(&id, &error.to_string())),
    };
    let post_archive_check = post_archive_check(paths, &id)?;
    let archive_path = repo_relative_path(paths, &paths.archive_cards_dir().join(&id));
    let restore_command = format!("maestro feature unarchive {id}");
    let event_path = format!(".maestro/runs/{}/events.jsonl", run::run_dir_name(&run_id));
    let event_id = format!(
        "auto_archive:{id}:{}:{}",
        short_commit(current_head),
        utc_now_timestamp()
    );
    let command = auto_archive_command(AutoArchiveCommandParts {
        id: &id,
        authority_ref: &authority_ref,
        authority_target: &authority_target,
        authority_head: &authority_head,
        tested_head: current_head,
        qa_result: &qa_result,
        run_id: &run_id,
        multi_agent: &multi_agent,
        canonical_store: &current_store,
        worker_source: &worker_source,
    });
    let mut event = auto_archive_event(AutoArchiveEventParts {
        event_id: &event_id,
        id: &id,
        authority_ref: &authority_ref,
        canonical_store_path: &current_store,
        invoking_checkout_path: &invoking_checkout,
        worker_source: &worker_source,
        current_head,
        qa_result: &qa_result,
        qa_evidence: &qa_evidence,
        run_id: &run_id,
        multi_agent: &multi_agent,
        before_state: &before_state,
        command: &command,
        report: &report,
        archive_path: &archive_path,
        restore_command: &restore_command,
        post_archive_check: &post_archive_check,
        snapshot_maestro_dirty: snapshot.maestro_dirty,
        snapshot_code_other_dirty: snapshot.code_other_dirty,
        merge_back_disposition: &multi_agent,
    });
    run::insert_agent_runtime(&mut event, agent_runtime_from_env());
    let event_hash = sha256_prefixed(&serde_json::to_vec(&event)?);
    event
        .as_object_mut()
        .expect("invariant: auto_archive event is an object")
        .insert("event_hash".to_string(), json!(event_hash.clone()));

    if let Err(error) = run::append_manual_event(paths, &run_id, &event) {
        bail!(
            "partial auto-archive for {id}: feature archived, but auto_archive run event failed: {error:#}\n  recovery: record the archive event manually in {event_path}, then append the archive index receipt"
        );
    }

    let receipt = feature::AutoArchiveReceipt {
        feature_id: id.clone(),
        canonical_store_path: current_store.clone(),
        invoking_checkout_path: invoking_checkout.clone(),
        worker_source: worker_source.clone(),
        final_target_head: current_head.to_string(),
        tested_head: current_head.to_string(),
        authority_ref: authority_ref.clone(),
        merge_back_disposition: multi_agent.clone(),
        qa_result: qa_result.clone(),
        run_id: run_id.clone(),
        event_id: event_id.clone(),
        event_hash: event_hash.clone(),
        event_path: event_path.clone(),
        archive_path: archive_path.clone(),
        restore_command: restore_command.clone(),
    };
    let index_line = match feature::append_auto_archive_receipt(paths, &receipt) {
        Ok(line) => line,
        Err(error) => bail!(
            "partial auto-archive for {id}: feature archived and run event was written, but archive index receipt failed: {error:#}\n  recovery: append a receipt to .maestro/archive/cards/INDEX.md for event {event_id} ({event_hash})"
        ),
    };

    println!("auto-archived {id}");
    println!("  authority: {authority_ref}");
    println!("  canonical store: {current_store}");
    println!("  invoking checkout: {invoking_checkout}");
    println!("  worker source: {worker_source}");
    println!("  tested head: {current_head}");
    println!("  qa: {qa_result} ({} evidence item(s))", qa_evidence.len());
    println!("  multi-agent/worktree: {multi_agent}");
    println!("  run event: {event_path} ({event_hash})");
    println!("  archive: {archive_path}");
    println!("  restore: {restore_command}");
    println!("  index: {}", index_line.trim());
    Ok(())
}

struct AutoArchiveEventParts<'a> {
    event_id: &'a str,
    id: &'a str,
    authority_ref: &'a str,
    canonical_store_path: &'a str,
    invoking_checkout_path: &'a str,
    worker_source: &'a str,
    current_head: &'a str,
    qa_result: &'a str,
    qa_evidence: &'a [String],
    run_id: &'a str,
    multi_agent: &'a str,
    before_state: &'a str,
    command: &'a str,
    report: &'a feature::FeatureArchiveReport,
    archive_path: &'a str,
    restore_command: &'a str,
    post_archive_check: &'a str,
    snapshot_maestro_dirty: usize,
    snapshot_code_other_dirty: usize,
    merge_back_disposition: &'a str,
}

fn auto_archive_event(parts: AutoArchiveEventParts<'_>) -> Value {
    json!({
        "schema_version": EVENT_SCHEMA_VERSION,
        "ts": utc_now_timestamp(),
        "event_type": "auto_archive",
        "event_id": parts.event_id,
        "session_id": parts.run_id,
        "feature_id": parts.id,
        "action": "auto_archive",
        "target_kind": "feature",
        "target_id": parts.id,
        "authority_ref": parts.authority_ref,
        "canonical_store_path": parts.canonical_store_path,
        "invoking_checkout_path": parts.invoking_checkout_path,
        "worker_source": parts.worker_source,
        "final_target_head": parts.current_head,
        "tested_head": parts.current_head,
        "commit": parts.current_head,
        "qa_result": parts.qa_result,
        "qa_evidence": parts.qa_evidence,
        "multi_agent_disposition": parts.multi_agent,
        "merge_back_disposition": parts.merge_back_disposition,
        "preflight_result": {
            "git_head": parts.current_head,
            "git_dirty": false,
            "maestro_dirty": parts.snapshot_maestro_dirty,
            "code_other_dirty": parts.snapshot_code_other_dirty,
            "archive_preflight": "passed"
        },
        "archive_receipt": {
            "note": parts.report.note,
            "child_tasks": parts.report.child_tasks,
            "archive_path": parts.archive_path,
            "restore_command": parts.restore_command,
            "archive_index": ".maestro/archive/cards/INDEX.md"
        },
        "post_archive_check": parts.post_archive_check,
        "archive_path": parts.archive_path,
        "restore_command": parts.restore_command,
        "before_state": parts.before_state,
        "command": parts.command,
        "result": "archived",
        "after_state": "archived"
    })
}

fn post_archive_check(paths: &MaestroPaths, id: &str) -> Result<String> {
    feature::show_archived(paths, id)?;
    if feature::ensure_exists(paths, id).is_ok() {
        bail!("post-archive check failed for {id}: live feature still exists");
    }
    Ok("archived-visible; live feature absent".to_string())
}

fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn relevant_dirty_paths(
    dirty_paths: &[PathBuf],
    id: &str,
    run_id: &str,
    qa_evidence: &[String],
) -> Vec<String> {
    let evidence_paths = qa_evidence_paths(qa_evidence);
    dirty_paths
        .iter()
        .filter(|path| path_is_auto_archive_relevant(path, id, run_id, &evidence_paths))
        .map(|path| path.display().to_string())
        .collect()
}

fn qa_evidence_paths(qa_evidence: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for item in qa_evidence {
        for token in item.split_whitespace() {
            let Some(value) = token
                .strip_prefix("path=")
                .or_else(|| token.strip_prefix("paths="))
            else {
                continue;
            };
            for path in value
                .split(',')
                .map(str::trim)
                .filter(|path| !path.is_empty())
            {
                paths.push(PathBuf::from(path));
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn path_is_auto_archive_relevant(
    path: &Path,
    id: &str,
    run_id: &str,
    evidence_paths: &[PathBuf],
) -> bool {
    path.starts_with(Path::new(".maestro").join("cards").join(id))
        || path == Path::new(".maestro/archive/cards/INDEX.md")
        || path.starts_with(Path::new(".maestro").join("archive").join("cards").join(id))
        || path.starts_with(
            Path::new(".maestro")
                .join("runs")
                .join(run::run_dir_name(run_id)),
        )
        || evidence_paths
            .iter()
            .any(|evidence_path| path == evidence_path || path.starts_with(evidence_path))
}

fn blocking_work_items(paths: &MaestroPaths, id: &str) -> Result<Vec<String>> {
    let mut blockers = Vec::new();
    for card in card::query::scan(paths)? {
        if !card.card_type.workable() {
            continue;
        }
        let owned_by_target =
            card.parent.as_deref() == Some(id) || card.deps.iter().any(|dep| dep.target == id);
        if !owned_by_target {
            continue;
        }
        let coarse = card::query::coarse_of(&card.status);
        if coarse != Some(card::query::Coarse::Closed) || card.claimed_by.is_some() {
            let claimed = card
                .claimed_by
                .as_deref()
                .map(|owner| format!(", claimed_by={owner}"))
                .unwrap_or_default();
            blockers.push(format!("{} status={}{}", card.id, card.status, claimed));
        }
    }
    blockers.sort();
    Ok(blockers)
}

fn unresolved_conflicts_for_target(paths: &MaestroPaths, id: &str) -> Result<Vec<String>> {
    let path = paths.maestro_dir().join("conflicts.jsonl");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut latest: BTreeMap<(String, String), (String, String, String)> = BTreeMap::new();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let action = value.get("action").and_then(Value::as_str).unwrap_or("");
        let session = value
            .get("asserter_session")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let peer = value
            .get("peer_card")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let asserter = value
            .get("asserter_card")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let reason = value
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        latest.insert((session, peer), (action.to_string(), asserter, reason));
    }
    let mut conflicts = Vec::new();
    for ((session, peer), (action, asserter, reason)) in latest {
        if action == "assert" && (peer == id || asserter == id) {
            conflicts.push(format!("{session}->{peer}: {reason}"));
        }
    }
    conflicts.sort();
    Ok(conflicts)
}

fn required_cli_value(name: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{name} must not be empty");
    }
    Ok(trimmed.to_string())
}

fn required_cli_values(name: &str, values: Vec<String>) -> Result<Vec<String>> {
    values
        .into_iter()
        .map(|value| required_cli_value(name, &value))
        .collect()
}

fn qa_passed(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "pass" | "passed"
    )
}

struct AutoArchiveCommandParts<'a> {
    id: &'a str,
    authority_ref: &'a str,
    authority_target: &'a str,
    authority_head: &'a str,
    tested_head: &'a str,
    qa_result: &'a str,
    run_id: &'a str,
    multi_agent: &'a str,
    canonical_store: &'a str,
    worker_source: &'a str,
}

fn auto_archive_command(parts: AutoArchiveCommandParts<'_>) -> String {
    format!(
        "maestro feature auto-archive {} --authority-ref {} --authority-target {} --authority-head {} --authority-state current --tested-head {} --qa-result {} --run {} --multi-agent {} --canonical-store {} --worker-source {}",
        parts.id,
        parts.authority_ref,
        parts.authority_target,
        parts.authority_head,
        parts.tested_head,
        parts.qa_result,
        parts.run_id,
        parts.multi_agent,
        parts.canonical_store,
        parts.worker_source,
    )
}

fn repo_relative_path(paths: &MaestroPaths, path: &Path) -> String {
    path.strip_prefix(paths.repo_root())
        .ok()
        .unwrap_or(path)
        .display()
        .to_string()
}

fn short_commit(commit: &str) -> String {
    commit.chars().take(12).collect()
}

/// Bulk-archive every closed (terminal) feature (§5 L3). Collect-and-continue:
/// one feature's failure never aborts the sweep; the summary exits non-zero iff
/// any failed, so a re-run safely retries (archived features no-op, failures
/// retry).
fn archive_closed(paths: &MaestroPaths, dry_run: bool) -> Result<()> {
    let closed: Vec<String> = feature::list(paths)?
        .into_iter()
        .filter(|view| view.status.is_terminal())
        .map(|view| view.id)
        .collect();

    if closed.is_empty() {
        println!("no closed features to archive");
        return Ok(());
    }

    let mut failures = Vec::new();
    let mut archived = 0usize;
    let mut child_tasks = 0usize;
    for id in &closed {
        match feature::archive_feature(paths, id, dry_run) {
            Ok(report) => {
                archived += 1;
                child_tasks += report.child_tasks;
            }
            Err(err) => failures.push(format!("{id}: {err:#}")),
        }
    }

    if dry_run {
        println!("dry-run: would archive closed features");
    } else {
        println!("archived closed features");
    }
    println!("archive summary:");
    let feature_verb = if dry_run { "would archive" } else { "archived" };
    let task_verb = if dry_run { "would archive" } else { "archived" };
    println!("  features: {archived} {feature_verb}");
    println!("  child tasks: {child_tasks} {task_verb}");
    // A terminal feature has no live children by construction, so nothing is
    // ever skipped; the line stays for receipt-shape stability.
    println!("  skipped: 0");
    println!("  failed: {}", failures.len());

    if !failures.is_empty() {
        println!("failed:");
        for failure in &failures {
            println!("  - {failure}");
        }
        println!("next:");
        println!("  retry: maestro feature archive --closed");
        bail!(
            "{} closed feature(s) failed to archive (re-run to retry):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
    if dry_run {
        println!("writes: none");
        println!("run: maestro feature archive --closed");
    } else {
        println!("next: maestro status");
    }
    Ok(())
}

fn new_feature(
    paths: &MaestroPaths,
    title: &str,
    description: Option<String>,
    questions: Vec<String>,
    project: Option<String>,
    id_only: bool,
) -> Result<()> {
    let project = super::resolve_project(project, paths)?;
    let id = feature::create(paths, title, project)?;
    let initialized = description.is_some() || !questions.is_empty();
    if initialized {
        feature::set(
            paths,
            &id,
            ContractEdits {
                description,
                open_questions: opt_list(questions),
                ..Default::default()
            },
        )?;
    }
    super::emit_work_touch(paths, &id);
    if id_only {
        println!("{id}");
        return Ok(());
    }
    println!("created feature {id} (proposed)");
    println!("spec: .maestro/cards/{id}/spec.md");
    println!("fill: maestro feature spec {id} --section \"Current state\" --append \"<text>\"");
    println!("decisions: maestro decision new \"<title>\" --feature {id}");
    if initialized {
        println!("initialized contract fields");
    }
    Ok(())
}

fn set_feature(paths: &MaestroPaths, id: &str, edits: ContractEdits) -> Result<()> {
    if edits.is_empty() {
        bail!(
            "no fields to set\n  maestro feature set {id} --acceptance \"<criterion>\" --area \"<surface>\"\n  flags: --acceptance --area --non-goal --question --description --request --type"
        );
    }
    let report = feature::set_with_report(paths, id, edits)?;
    super::emit_work_touch(paths, id);
    print_set_report(id, &report);
    println!("next: maestro feature finalize {id}");
    if !report.view.open_questions.is_empty() {
        println!(
            "fork hint: open real forks with `maestro decision new \"<title>\" --feature {id} --context \"<why>\"`; keep --question for loose questions"
        );
    }
    Ok(())
}

fn paired_acceptance_edits(
    edit_acceptance: Vec<String>,
    text: Vec<String>,
) -> Result<Vec<feature::AcceptanceTextEdit>> {
    if edit_acceptance.len() != text.len() {
        bail!(
            "{} --edit-acceptance but {} --text: each --edit-acceptance needs its --text",
            edit_acceptance.len(),
            text.len()
        );
    }
    Ok(edit_acceptance
        .into_iter()
        .zip(text)
        .map(|(id, text)| feature::AcceptanceTextEdit { id, text })
        .collect())
}

fn print_set_report(id: &str, report: &feature::SetReport) {
    println!("set {id}");
    for line in change_lines("replaced", &report.replaced, &report.view) {
        println!("  {line}");
    }
    for line in change_lines("added", &report.added, &report.view) {
        println!("  {line}");
    }
    if report.edited_acceptance > 0 {
        println!("  acceptance edited ({})", report.edited_acceptance);
    }
    if report.replaced.is_empty() && report.added.is_empty() && report.edited_acceptance == 0 {
        println!("  no list values changed; scalar fields may have been refreshed");
    }
    println!(
        "  totals: acceptance={}, areas={}, non_goals={}, questions={}",
        report.view.acceptance.len(),
        report.view.affected_areas.len(),
        report.view.non_goals.len(),
        report.view.open_questions.len()
    );
}

fn change_lines(
    mode: &str,
    counts: &ContractChangeCounts,
    view: &feature::FeatureView,
) -> Vec<String> {
    let mut lines = Vec::new();
    push_count_line(
        &mut lines,
        mode,
        "acceptance",
        counts.acceptance,
        view.acceptance.len(),
    );
    push_count_line(
        &mut lines,
        mode,
        "areas",
        counts.affected_areas,
        view.affected_areas.len(),
    );
    push_count_line(
        &mut lines,
        mode,
        "non_goals",
        counts.non_goals,
        view.non_goals.len(),
    );
    push_count_line(
        &mut lines,
        mode,
        "questions",
        counts.open_questions,
        view.open_questions.len(),
    );
    if counts.description > 0 {
        lines.push("description replaced".to_string());
    }
    if counts.raw_request > 0 {
        lines.push("raw_request replaced".to_string());
    }
    if counts.input_type > 0 {
        lines.push("input_type replaced".to_string());
    }
    lines
}

fn push_count_line(lines: &mut Vec<String>, mode: &str, label: &str, changed: usize, total: usize) {
    if changed == 0 {
        return;
    }
    if mode == "added" {
        lines.push(format!("+{changed} {label} ({total} total)"));
    } else {
        lines.push(format!(
            "{label} replaced ({total}); other fields untouched"
        ));
    }
}

fn amend_feature(
    paths: &MaestroPaths,
    id: &str,
    additions: ContractAdditions,
    reason: &str,
) -> Result<()> {
    if reason.trim().is_empty() {
        bail!("`--reason` must not be empty; record why the contract is growing (it is audited)");
    }
    if additions.is_empty() {
        bail!(
            "no values to amend\n  maestro feature amend {id} --add-acceptance \"<criterion>\" --reason \"<why>\"\n  add-flags: --add-acceptance --add-area --add-non-goal --add-question"
        );
    }
    let note = feature::amend(paths, id, additions, reason)?.note;
    super::emit_work_touch(paths, id);
    print_note(note)
}

fn cancel_feature(paths: &MaestroPaths, id: &str, reason: &str, dry_run: bool) -> Result<()> {
    if reason.trim().is_empty() {
        bail!(
            "blocked: feature cancel needs an audited reason\nreason: --reason is empty\nrun: maestro feature cancel {id} --reason \"<why this feature is being cancelled>\""
        );
    }
    let report = match feature::cancel(paths, id, reason, dry_run) {
        Ok(report) => report,
        Err(error) => bail!(
            "{}",
            feature_cancel_error_message(id, reason, &error.to_string())
        ),
    };
    println!("{}", report.note);
    println!("cancel receipt:");
    println!("  feature: {}", report.id);
    println!("  abandoned_tasks: {}", report.abandoned.len());
    if dry_run {
        println!("writes: none");
        println!("retry: maestro feature cancel {id} --reason \"<reason>\"");
    } else if report.changed {
        println!("inspect: maestro feature show {}", report.id);
        println!("next: maestro card archive {}", report.id);
    } else {
        println!("inspect: maestro feature show {}", report.id);
        println!("next: maestro status");
    }
    Ok(())
}

fn close_feature(
    paths: &MaestroPaths,
    id: &str,
    outcome: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let report = feature_close::close(paths, id, outcome, dry_run)?;
    println!("{}", report.note);
    if dry_run {
        println!("close preview:");
        println!("  feature: {}", report.id);
        // dec-ac-7-final: a non-blocking reminder that verified children carry
        // proof from older commits. It never feeds the close gate, so it cannot
        // turn a passing preview into a blocked one.
        let drifted = feature::verified_child_commit_drift(paths, &report.id)?;
        if !drifted.is_empty() {
            println!(
                "  note: {} child task(s) verified at older commits (HEAD moved); re-verify if their code changed: {} (advisory; does not block close)",
                drifted.len(),
                drifted.join(", ")
            );
        }
        println!("  target: closed");
        println!("  full verify suite would run before closing");
        println!("writes: none");
        println!(
            "retry: maestro feature close {} --outcome \"<outcome>\"",
            report.id
        );
    } else if report.changed && report.status == FeatureStatus::Closed {
        super::emit_ownership_release(
            paths,
            &report.id,
            super::OwnershipReleaseStatus::Done,
            Some("feature close"),
        );
        print_close_receipt(paths, &report)?;
    } else {
        println!("inspect: maestro feature show {}", report.id);
        println!("next: maestro status");
    }
    Ok(())
}

fn print_feature_archive_note(id: &str, report: &feature::FeatureArchiveReport, dry_run: bool) {
    println!("{}", report.note);
    if dry_run {
        println!("archive receipt preview:");
        println!("  feature: {id}");
        println!("  child tasks: {} would archive", report.child_tasks);
        println!("  skipped: 0");
        println!("writes: none");
        println!("run: maestro feature archive {id}");
    } else if report.note.starts_with("already archived") {
        println!("inspect: maestro feature show {id}");
        println!("next: maestro status");
    } else {
        println!("archive receipt:");
        println!("  feature: {id}");
        println!("  child tasks: {} archived", report.child_tasks);
        println!("  skipped: 0");
        println!("inspect: maestro feature show {id}");
        println!("next: maestro status");
        println!("restore: maestro feature unarchive {id}");
    }
}

fn print_feature_unarchive_note(id: &str, note: &str) {
    println!("{note}");
    let child_tasks = count_before_marker(note, " child task(s)").unwrap_or(0);
    if note.starts_with("already live") {
        println!("inspect: maestro feature show {id}");
        println!("next: maestro status");
    } else {
        println!("restore receipt:");
        println!("  feature: {id}");
        println!("  child tasks: {child_tasks} restored");
        println!("inspect: maestro feature show {id}");
        println!("next: maestro status");
        println!("optional: maestro feature archive {id}");
    }
}

fn feature_archive_error_message(id: &str, error: &str) -> String {
    if error.contains("not terminal") {
        return format!(
            "cannot archive {id}:\n  not terminal\nnext:\n  close: maestro feature close {id} --outcome \"<outcome>\"\n  or cancel: maestro feature cancel {id} --reason \"<reason>\""
        );
    }
    if error.contains("live child task") {
        return format!(
            "cannot archive {id}:\n  live child tasks\nnext:\n  inspect: maestro feature show {id}\n  retry: maestro feature archive {id}"
        );
    }
    if error.contains("feature not found") {
        return format!(
            "cannot archive {id}:\n  feature not found\nnext:\n  list features: maestro feature list --all"
        );
    }
    if error.contains("archived copy already exists") {
        return format!(
            "cannot archive {id}:\n  archived copy already exists\ninspect:\n  live: maestro feature show {id}\n  archived: .maestro/archive/cards/{id}\nnext:\n  resolve the duplicate archive, then retry: maestro feature archive {id}"
        );
    }
    error.to_string()
}

fn feature_unarchive_error_message(id: &str, error: &str) -> String {
    if error.contains("archived feature not found") {
        return format!(
            "cannot unarchive {id}:\n  archived feature not found\nnext:\n  list archived features: maestro feature list --all"
        );
    }
    if error.contains("live feature already occupies") {
        return format!(
            "cannot unarchive {id}:\n  live feature already exists\ninspect:\n  live: maestro feature show {id}\n  archived: .maestro/archive/cards/{id}\nnext:\n  resolve the live feature conflict, then retry: maestro feature unarchive {id}"
        );
    }
    if error.contains("a live copy of") {
        let detail = error.split(" — ").nth(1).unwrap_or(error);
        return format!(
            "cannot unarchive {id}:\n  {detail}\ninspect:\n  live: maestro feature show {id}\n  archived: .maestro/archive/cards/{id}\nnext:\n  resolve the live copy conflict, then retry: maestro feature unarchive {id}"
        );
    }
    error.to_string()
}

fn feature_cancel_error_message(id: &str, reason: &str, error: &str) -> String {
    if error.contains("closed features are terminal") || error.contains("terminal") {
        return format!(
            "blocked: cannot cancel {id}\nreason: closed features are terminal\ninspect: maestro feature show {id}\nnext: maestro feature archive {id}"
        );
    }
    if error.contains("failed to abandon child task") {
        return format!(
            "blocked: cancel cascade failed\nfeature: {id}\nreason: {error}\ninspect: maestro feature show {id}\nretry: maestro feature cancel {id} --reason \"{reason}\""
        );
    }
    error.to_string()
}

fn count_before_marker(note: &str, marker: &str) -> Option<usize> {
    let prefix = note.split(marker).next()?;
    prefix.split_whitespace().last()?.parse().ok()
}

fn show_feature(paths: &MaestroPaths, id: &str) -> Result<()> {
    // L6b: reads cross the boundary — fall through to the archive so a
    // historical reference to an archived feature still renders.
    let (view, archived) = match feature::show(paths, id) {
        Ok(view) => (view, false),
        Err(live_err) => (
            feature::show_archived(paths, id).map_err(|_| live_err)?,
            true,
        ),
    };

    println!("id: {}", view.id);
    println!("title: {}", view.title);
    println!("status: {}", feature::status_label(&view.status));
    if archived {
        println!("archived: true");
    }
    // An archived view counts only the archive tree; an L6c-skipped child stays live,
    // so disclose the live referrers it omits rather than reporting a misleading total.
    let live_unarchived = if archived {
        feature::query::count_tasks_for_feature(&paths.tasks_dir(), id)?.total
    } else {
        0
    };
    if live_unarchived > 0 {
        println!(
            "tasks_total: {} ({live_unarchived} live task(s) not archived)",
            view.counts.total
        );
    } else {
        println!("tasks_total: {}", view.counts.total);
    }
    println!("tasks_verified: {}", view.counts.verified);
    println!("created_at: {}", render_timestamp(&view.created_at));
    println!("updated_at: {}", render_timestamp(&view.updated_at));
    if let Some(description) = view.description.as_deref() {
        println!("description: {description}");
    }
    if let Some(request) = view.raw_request.as_deref() {
        println!("raw_request: {request}");
    }
    if let Some(input_type) = view.input_type.as_deref() {
        println!("input_type: {input_type}");
    }
    if let Some(outcome) = view.outcome.as_deref() {
        println!("outcome: {outcome}");
    }
    if let Some(cancel_reason) = view.cancel_reason.as_deref() {
        println!("cancel_reason: {cancel_reason}");
    }
    if let Some(reason) = view.qa_none_reason.as_deref() {
        println!("qa: none ({reason})");
    }
    print_decision_summary(paths, &view.id)?;
    print_acceptance(
        paths,
        &view.id,
        &view.acceptance,
        view.acceptance_coverage.as_deref(),
        archived,
    )?;
    print_list("affected_areas", &view.affected_areas);
    print_list("non_goals", &view.non_goals);
    print_list("open_questions", &view.open_questions);
    if let Some(notes) = view.notes.as_deref() {
        println!("notes:");
        for line in notes.lines() {
            println!("  {line}");
        }
    }

    Ok(())
}

fn print_decision_summary(paths: &MaestroPaths, id: &str) -> Result<()> {
    let records = decisions::decisions_for_feature(paths, id)?;
    let open = records
        .iter()
        .filter(|record| record.status == decisions::schema::DecisionStatus::Open)
        .count();
    let locked = records
        .iter()
        .filter(|record| record.status == decisions::schema::DecisionStatus::Locked)
        .count();
    let superseded = records
        .iter()
        .filter(|record| record.status == decisions::schema::DecisionStatus::Superseded)
        .count();
    println!(
        "decisions: {} (open: {open}, locked: {locked}, superseded: {superseded})",
        records.len()
    );
    Ok(())
}

fn feature_spec(
    paths: &MaestroPaths,
    id: &str,
    section: Option<String>,
    append: Option<String>,
    replace: Option<String>,
) -> Result<()> {
    match (section, append, replace) {
        (None, None, None) => show_feature_spec(paths, id),
        (Some(section), Some(text), None) => write_feature_spec(paths, id, &section, &text, false),
        (Some(section), None, Some(text)) => write_feature_spec(paths, id, &section, &text, true),
        (Some(section), None, None) => bail!(
            "--section needs the text to write\n  append: maestro feature spec {id} --section \"{section}\" --append \"<text>\"\n  replace: maestro feature spec {id} --section \"{section}\" --replace \"<text>\""
        ),
        (None, _, _) => bail!(
            "--append/--replace need --section\n  maestro feature spec {id} --section \"<name>\" --append \"<text>\""
        ),
        (Some(_), Some(_), Some(_)) => unreachable!("clap rejects --append with --replace"),
    }
}

fn write_feature_spec(
    paths: &MaestroPaths,
    id: &str,
    section: &str,
    text: &str,
    replace: bool,
) -> Result<()> {
    let report = feature::write_spec_section(paths, id, section, text, replace)?;
    super::emit_work_touch(paths, id);
    let verb = if replace { "replaced" } else { "appended to" };
    let created = if report.created_section {
        " (new section)"
    } else {
        ""
    };
    println!("{verb} section \"{}\"{created}", section.trim());
    // The section body runs to the next heading, so headings inside the
    // written text become section boundaries a later --section edit stops at.
    if text
        .lines()
        .any(|line| line.starts_with("## ") || line.starts_with("# "))
    {
        println!(
            "note: the text contains markdown headings, which start new sections; a later --section \"{}\" edit stops at the first one",
            section.trim()
        );
    }
    println!("spec: .maestro/cards/{id}/spec.md");
    println!("inspect: maestro feature spec {id}");
    Ok(())
}

fn show_feature_spec(paths: &MaestroPaths, id: &str) -> Result<()> {
    // L6b: reads cross the boundary -- mirror `show_feature`'s archive
    // fallthrough so a historical spec still renders. Only when neither tree
    // resolves does the unreadable-card recovery view take over, carrying the
    // live error.
    let (view, archived) = match feature::show(paths, id) {
        Ok(view) => (view, false),
        Err(live_err) => match feature::show_archived(paths, id) {
            Ok(view) => (view, true),
            Err(_) => return show_unreadable_feature_spec(paths, id, live_err),
        },
    };
    println!("status: {}", feature::status_label(&view.status));
    println!("feature: {}", view.id);
    if archived {
        println!("archived: true");
    }
    println!();
    let sidecar_dir = if archived {
        paths.archive_cards_dir().join(&view.id)
    } else {
        feature::feature_sidecar_dir(paths, &view.id)
    };
    let spec_path = sidecar_dir.join("spec.md");
    match std::fs::read_to_string(&spec_path) {
        Ok(spec) => print!("{}", spec.trim_end()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("# {}", view.title);
            println!();
            println!("(no spec.md found)");
        }
        Err(error) => bail!("failed to read {}: {error}", spec_path.display()),
    }
    println!();
    println!();
    println!("## Contract");
    if let Some(description) = view.description.as_deref() {
        println!("description: {description}");
    }
    print_plain_list("acceptance", &view.acceptance);
    print_plain_list("affected_areas", &view.affected_areas);
    print_plain_list("non_goals", &view.non_goals);
    print_plain_list("open_questions", &view.open_questions);

    let records = decisions::decisions_for_feature(paths, &view.id)?;
    println!();
    println!("## Decisions");
    let open = records
        .iter()
        .filter(|record| record.status == decisions::schema::DecisionStatus::Open)
        .collect::<Vec<_>>();
    if !open.is_empty() {
        println!("Open forks:");
        for record in &open {
            println!("- {}: {}", record.id, record.title);
            if let Some(context) = record.context.as_deref() {
                println!("  context: {context}");
            }
        }
    }
    let closed = records
        .iter()
        .filter(|record| record.status != decisions::schema::DecisionStatus::Open)
        .collect::<Vec<_>>();
    if closed.is_empty() && open.is_empty() {
        println!("- none");
    } else {
        for record in closed {
            println!(
                "- {} [{}]: {}",
                record.id,
                record.status.as_str(),
                record.title
            );
            if let Some(decision) = record.decision.as_deref() {
                println!("  decision: {decision}");
            }
            if let Some(preview) = record.preview.as_deref() {
                println!("  preview:");
                for line in preview.lines() {
                    println!("    {line}");
                }
            }
        }
    }

    if let Some(notes) = view.notes.as_deref() {
        println!();
        println!("## Recent notes");
        for line in notes
            .lines()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            println!("{line}");
        }
    }
    Ok(())
}

fn show_unreadable_feature_spec(
    paths: &MaestroPaths,
    id: &str,
    error: anyhow::Error,
) -> Result<()> {
    let path = card::store::card_path(paths, id);
    println!("status: unreadable");
    println!("feature: {id}");
    println!("path: {}", path.display());
    println!("error: {error:#}");
    println!();
    println!("## Raw card.yaml");
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            println!("```yaml");
            print!("{}", contents.trim_end());
            println!();
            println!("```");
        }
        Err(read_error) => println!("unavailable: {read_error}"),
    }
    println!();
    println!("## Decisions");
    match decisions::decisions_for_feature(paths, id) {
        Ok(records) if records.is_empty() => println!("- none"),
        Ok(records) => {
            for record in records {
                println!(
                    "- {} [{}]: {}",
                    record.id,
                    record.status.as_str(),
                    record.title
                );
            }
        }
        Err(error) => println!("- unreadable decisions.yaml: {error:#}"),
    }
    Ok(())
}

fn print_plain_list(label: &str, values: &[String]) {
    println!("{label}:");
    if values.is_empty() {
        println!("- none");
        return;
    }
    for value in values {
        println!("- {value}");
    }
}

fn print_acceptance(
    paths: &MaestroPaths,
    id: &str,
    fallback: &[String],
    coverage: Option<&[feature::AcceptanceCoverage]>,
    archived: bool,
) -> Result<()> {
    let loaded_coverage;
    let coverage = if let Some(coverage) = coverage {
        coverage
    } else {
        loaded_coverage = if archived {
            feature::acceptance_coverage_archived(paths, id)?
        } else {
            feature::acceptance_coverage(paths, id)?
        };
        &loaded_coverage
    };
    println!("acceptance:");
    if coverage.is_empty() {
        if fallback.is_empty() {
            println!("- none");
        }
        for (index, item) in fallback.iter().enumerate() {
            println!("- [{}] {}", feature::acceptance_id(index), item);
        }
        return Ok(());
    }
    for item in coverage {
        println!("- [{}] {}", item.ac_id, item.text);
        if !item.tasks.is_empty() {
            println!("  covers: {}", item.tasks.join(", "));
        }
    }
    Ok(())
}

/// What can still close an uncovered acceptance item at this point: prepared
/// tasks are accepted on creation, so `task set --covers` never works right
/// after prepare/start; the fix is the plan file or new work, never a locked task.
enum CoverageFix {
    /// Tasks come from the plan file; coverage is authored as `covers:` lines.
    Plan,
    /// Existing tasks are acceptance-locked; cover with new work or evidence.
    Locked,
}

fn print_uncovered_acceptance_warning(
    paths: &MaestroPaths,
    id: &str,
    fix: CoverageFix,
) -> Result<()> {
    let uncovered = feature::uncovered_acceptance(paths, id)?;
    if uncovered.is_empty() {
        return Ok(());
    }
    println!(
        "warning: {} acceptance item(s) have no covering task: {}",
        uncovered.len(),
        uncovered.join(", ")
    );
    match fix {
        CoverageFix::Plan => {
            println!(
                "fix: add `covers: <ac-id>` to task lines in the plan before `prepare --from`"
            );
        }
        CoverageFix::Locked => {
            println!("fix: maestro task create \"<title>\" --feature {id} --covers <ac-id>");
            println!(
                "     or prove directly: maestro feature verify {id} --prove <ac-id> --evidence \"<proof>\""
            );
        }
    }
    Ok(())
}

fn proof_label(proof: &feature::AcceptanceProof) -> String {
    match proof {
        feature::AcceptanceProof::Task(tasks) => format!("proof: {} OK", tasks.join(", ")),
        feature::AcceptanceProof::Qa(items) => format!("proof: {} OK", items.join(", ")),
        feature::AcceptanceProof::Explicit(evidence) => format!("proof: {evidence} OK"),
        feature::AcceptanceProof::Waived(reason) => format!("WAIVED: {reason}"),
        feature::AcceptanceProof::Missing => "NO FRESH EVIDENCE".to_string(),
    }
}

fn list_features(paths: &MaestroPaths, all: bool) -> Result<()> {
    let mut views = Vec::new();
    let mut unreadable = Vec::new();
    for entry in feature::list_tolerant(paths) {
        match entry {
            feature::FeatureRosterEntry::Loaded(view) => views.push(*view),
            feature::FeatureRosterEntry::Unreadable {
                id, error, hint, ..
            } => unreadable.push((id, error, hint)),
        }
    }
    let now_nanos = timestamp_nanos(&utc_now_timestamp()).unwrap_or(0);
    let terminal_hidden = views
        .iter()
        .filter(|view| view.status.is_terminal())
        .count();
    // Stale proposed features collapse out of the table on both default and
    // --all; --all re-surfaces them in a dedicated build/retire guidance block.
    let stale: Vec<feature::FeatureView> = views
        .iter()
        .filter(|view| feature::is_stale_proposed(&view.status, &view.updated_at, now_nanos))
        .cloned()
        .collect();
    let shown: Vec<_> = if all {
        // L6b: --all also reads the archive sibling tree.
        let mut all_views: Vec<_> = views
            .into_iter()
            .filter(|view| !feature::is_stale_proposed(&view.status, &view.updated_at, now_nanos))
            .collect();
        all_views.extend(feature::list_archived(paths)?);
        all_views
    } else {
        views
            .into_iter()
            .filter(|view| !view.status.is_terminal())
            .filter(|view| !feature::is_stale_proposed(&view.status, &view.updated_at, now_nanos))
            .collect()
    };

    if shown.is_empty() && unreadable.is_empty() && (!all || stale.is_empty()) {
        println!("no features found");
    } else {
        let mut rows: Vec<Vec<String>> = shown
            .iter()
            .map(|view| {
                let title = match view.outcome.as_deref() {
                    Some(outcome) => format!("{} -- {outcome}", view.title),
                    None => view.title.clone(),
                };
                vec![
                    view.id.clone(),
                    feature::status_label(&view.status).to_string(),
                    feature_next_label(view).to_string(),
                    view.counts.total.to_string(),
                    view.counts.verified.to_string(),
                    title,
                ]
            })
            .collect();
        for (id, error, hint) in &unreadable {
            rows.push(vec![
                id.clone(),
                "unreadable".to_string(),
                recovery_label(hint.as_deref()).to_string(),
                "0".to_string(),
                "0".to_string(),
                error.clone(),
            ]);
        }
        print!(
            "{}",
            table::render_table(
                &["ID", "STATE", "NEXT", "TASKS", "VERIFIED", "TITLE"],
                &rows
            )
        );
        println!("inspect any: maestro feature show <id>");
    }

    if all {
        let stale_refs: Vec<&feature::FeatureView> = stale.iter().collect();
        print!("{}", stale_reveal_block(&stale_refs, now_nanos));
    } else {
        if terminal_hidden > 0 {
            println!("# {terminal_hidden} terminal feature(s) hidden; use --all to include");
        }
        if !stale.is_empty() {
            println!(
                "# {} proposed stale feature(s) hidden; use --all to review",
                stale.len()
            );
            println!("{}", feature::RETIRE_REMINDER);
        }
    }

    Ok(())
}

/// The `feature list --all` reveal for stale proposed features: each one with
/// its age marker and the existing build / retire verbs surfaced as guidance
/// text (no new command). Returns the empty string when nothing is stale.
fn stale_reveal_block(stale: &[&feature::FeatureView], now_nanos: i128) -> String {
    if stale.is_empty() {
        return String::new();
    }
    let mut out = String::from("STALE PROPOSED (review or retire):\n");
    for view in stale {
        let age = feature::age_days(&view.updated_at, now_nanos).unwrap_or(0);
        out.push_str(&format!("  {}\n", view.id));
        out.push_str(&format!("    proposed [stale {age}d]\n"));
        out.push_str(&format!("    build:  {}\n", stale_build_hint(view)));
        out.push_str(&format!(
            "    retire: maestro feature cancel {0} --reason \"...\"  then  maestro feature archive {0}\n",
            view.id
        ));
    }
    out.push_str(feature::RETIRE_REMINDER);
    out.push('\n');
    out
}

/// The happy-path build command for a stale proposed feature: keep authoring
/// the contract while it is incomplete, otherwise finalize it. Mirrors the
/// proposed branch of `feature_next_label`, rendered as a runnable command.
fn stale_build_hint(view: &feature::FeatureView) -> String {
    if !view.acceptance.is_empty() && !view.affected_areas.is_empty() {
        format!("maestro feature finalize {}", view.id)
    } else {
        format!("maestro feature set {}", view.id)
    }
}

fn print_note(note: String) -> Result<()> {
    println!("{note}");
    Ok(())
}

fn claims_only_verified_count(paths: &MaestroPaths, feature_id: &str) -> Result<usize> {
    Ok(task::load_task_records(&paths.tasks_dir())?
        .into_iter()
        .filter(|task| {
            task.feature_id.as_deref() == Some(feature_id)
                && task.state == task::TaskState::Verified
                && task.verification.claims_only
        })
        .count())
}

fn print_list(label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    println!("{label}:");
    for item in items {
        println!("  - {item}");
    }
}

fn opt_list(values: Vec<String>) -> Option<Vec<String>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proposed_view(id: &str, updated_at: &str, contract_complete: bool) -> feature::FeatureView {
        feature::FeatureView {
            id: id.to_string(),
            title: format!("{id} title"),
            status: FeatureStatus::Proposed,
            counts: feature::query::FeatureTaskCounts::default(),
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
            description: None,
            raw_request: None,
            input_type: None,
            acceptance: if contract_complete {
                vec!["ac-1 do X".to_string()]
            } else {
                Vec::new()
            },
            acceptance_coverage: None,
            affected_areas: if contract_complete {
                vec!["src/x.rs".to_string()]
            } else {
                Vec::new()
            },
            non_goals: Vec::new(),
            open_questions: Vec::new(),
            outcome: None,
            cancel_reason: None,
            qa_none_reason: None,
            notes: None,
            project: None,
        }
    }

    fn now() -> i128 {
        timestamp_nanos("2026-06-21T00:00:00.000Z").expect("fixed now parses")
    }

    #[test]
    fn reveal_block_shows_age_marker_build_and_retire_for_each_stale() {
        let v1 = proposed_view("incomplete-feat", "2026-06-01T00:00:00.000Z", false); // 20d
        let v2 = proposed_view("ready-feat", "2026-05-22T00:00:00.000Z", true); // 30d
        let block = stale_reveal_block(&[&v1, &v2], now());
        assert!(block.contains("STALE PROPOSED (review or retire):"));
        assert!(block.contains("incomplete-feat"));
        assert!(block.contains("proposed [stale 20d]"));
        assert!(block.contains("build:  maestro feature set incomplete-feat"));
        assert!(block.contains("ready-feat"));
        assert!(block.contains("proposed [stale 30d]"));
        assert!(block.contains("build:  maestro feature finalize ready-feat"));
        assert!(block.contains(
            "retire: maestro feature cancel incomplete-feat --reason \"...\"  then  maestro feature archive incomplete-feat"
        ));
        // The reminder is a single constant line, printed once at the end.
        assert_eq!(block.matches(feature::RETIRE_REMINDER).count(), 1);
    }

    #[test]
    fn reveal_block_is_empty_without_stale() {
        assert!(stale_reveal_block(&[], now()).is_empty());
    }

    #[test]
    fn retire_reminder_prints_once_regardless_of_stale_count() {
        // ac-4: one const reminder for the whole block, byte-identical whether
        // one feature is stale or many -- it never moves into the per-card loop.
        let many: Vec<feature::FeatureView> = (0..5)
            .map(|i| proposed_view(&format!("f{i}"), "2026-06-01T00:00:00.000Z", false))
            .collect();
        let many_refs: Vec<&feature::FeatureView> = many.iter().collect();
        let block_one = stale_reveal_block(&[&many[0]], now());
        let block_many = stale_reveal_block(&many_refs, now());
        assert_eq!(block_one.matches(feature::RETIRE_REMINDER).count(), 1);
        assert_eq!(block_many.matches(feature::RETIRE_REMINDER).count(), 1);
        assert!(block_one.trim_end().ends_with(feature::RETIRE_REMINDER));
        assert!(block_many.trim_end().ends_with(feature::RETIRE_REMINDER));
    }

    #[test]
    fn build_hint_is_set_until_contract_complete_then_finalize() {
        let incomplete = proposed_view("f", "2026-06-01T00:00:00.000Z", false);
        let complete = proposed_view("f", "2026-06-01T00:00:00.000Z", true);
        assert_eq!(stale_build_hint(&incomplete), "maestro feature set f");
        assert_eq!(stale_build_hint(&complete), "maestro feature finalize f");
    }
}
