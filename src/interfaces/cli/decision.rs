use anyhow::{Result, bail};

use crate::domain::decisions;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::table;
use crate::interfaces::cli::{DecisionArgs, DecisionCommand};

/// Execute `maestro decision`.
pub fn run(args: DecisionArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        DecisionCommand::New {
            title,
            context,
            feature,
            lock,
            decision,
            rejected,
            preview,
            supersedes,
            project,
            id_only,
        } => {
            if lock {
                let decision = decision.expect("clap invariant: --lock requires --decision");
                new_locked_decision(
                    &paths,
                    &title,
                    context.as_deref(),
                    feature.as_deref(),
                    decisions::LockInputs {
                        decision: &decision,
                        rejected: &rejected,
                        preview: preview.as_deref(),
                        supersedes: &supersedes,
                    },
                    project,
                    id_only,
                )
            } else {
                new_decision(
                    &paths,
                    &title,
                    context.as_deref(),
                    feature.as_deref(),
                    project,
                    id_only,
                )
            }
        }
        DecisionCommand::Lock {
            id,
            decision,
            rejected,
            preview,
            supersedes,
        } => lock_decision(
            &paths,
            &id,
            &decision,
            &rejected,
            preview.as_deref(),
            &supersedes,
        ),
        DecisionCommand::Supersede {
            old_id,
            decision,
            reason,
            title,
            rejected,
            preview,
            id_only,
        } => supersede_decision(
            &paths,
            SupersedeRequest {
                old_id: &old_id,
                decision: &decision,
                reason: &reason,
                title: title.as_deref(),
                rejected: &rejected,
                preview: preview.as_deref(),
                id_only,
            },
        ),
        DecisionCommand::Show { id } => show_decision(&paths, &id),
        DecisionCommand::List { all, feature } => {
            render_decision_list(decisions::list_tolerant(&paths), all, feature.as_deref())
        }
    }
}

fn new_decision(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
    project: Option<String>,
    id_only: bool,
) -> Result<()> {
    if title.trim().is_empty() {
        bail!("decision title cannot be empty; e.g. `maestro decision new \"Adopt X for Y\"`");
    }
    let project = super::resolve_project(project, paths)?;
    let report = decisions::create_open(paths, title, context, feature, project)?;
    emit_feature_touch(paths, &report.record);
    if id_only {
        println!("{}", report.record.id);
        return Ok(());
    }
    println!("opened {} (status: open)", report.record.id);
    if let Some(feature_id) = &report.record.feature {
        println!("feature: {feature_id}");
    }
    println!(
        "next: maestro decision lock {} --decision \"<chosen>\"",
        report.record.id
    );
    Ok(())
}

/// One-shot open+lock for a pre-decided fork. Unlike the standalone lock,
/// `--rejected` stays optional: a fork the user already settled often has no
/// enumerated alternatives worth recording.
fn new_locked_decision(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
    inputs: decisions::LockInputs<'_>,
    project: Option<String>,
    id_only: bool,
) -> Result<()> {
    if title.trim().is_empty() {
        bail!("decision title cannot be empty; e.g. `maestro decision new \"Adopt X for Y\"`");
    }
    let project = super::resolve_project(project, paths)?;
    let report = decisions::create_locked(paths, title, context, feature, inputs, project)?;
    emit_feature_touch(paths, &report.record);
    if id_only {
        println!("{}", report.record.id);
        return Ok(());
    }
    print_lock_report(&report);
    Ok(())
}

fn lock_decision(
    paths: &MaestroPaths,
    id: &str,
    decision: &str,
    rejected: &[String],
    preview: Option<&str>,
    supersedes: &[String],
) -> Result<()> {
    if rejected.is_empty() {
        bail!("decision lock requires at least one --rejected \"<option: why>\"");
    }
    let report = decisions::lock(paths, id, decision, rejected, preview, supersedes)?;
    emit_feature_touch(paths, &report.record);
    print_lock_report(&report);
    Ok(())
}

struct SupersedeRequest<'a> {
    old_id: &'a str,
    decision: &'a str,
    reason: &'a str,
    title: Option<&'a str>,
    rejected: &'a [String],
    preview: Option<&'a str>,
    id_only: bool,
}

fn supersede_decision(paths: &MaestroPaths, request: SupersedeRequest<'_>) -> Result<()> {
    let report = decisions::supersede(
        paths,
        request.old_id,
        decisions::SupersedeInputs {
            title: request.title,
            decision: request.decision,
            reason: request.reason,
            rejected: request.rejected,
            preview: request.preview,
        },
    )?;
    emit_feature_touch(paths, &report.record);
    if request.id_only {
        println!("{}", report.record.id);
        return Ok(());
    }
    print_lock_report(&report);
    if let Some(feature_id) = &report.record.feature {
        println!("next: maestro feature finalize {feature_id}");
    }
    Ok(())
}

/// Bind the session to the decision's parent feature (D3 preview: a decision
/// verb touches the feature the design work belongs to, not the decision card).
/// A global decision has no feature, so nothing is bound.
fn emit_feature_touch(paths: &MaestroPaths, record: &decisions::schema::DecisionRecord) {
    if let Some(feature_id) = record.feature.as_deref() {
        super::emit_work_touch(paths, feature_id);
    }
}

fn print_lock_report(report: &decisions::DecisionLockReport) {
    println!("locked {}", report.record.id);
    for superseded in &report.record.supersedes {
        println!("  supersedes {superseded}");
    }
    if let Some(line) = &report.note_line {
        println!("note:");
        println!("  {line}");
    }
}

fn show_decision(paths: &MaestroPaths, id: &str) -> Result<()> {
    match decisions::show(paths, id)? {
        decisions::DecisionContent::Structured { record, path, .. } => {
            println!("store: {}", path.display());
            print!("{}", decisions::query::render_record(&record));
        }
        decisions::DecisionContent::Legacy { contents, path, .. } => {
            println!("legacy: {}", path.display());
            print!("{contents}");
        }
    }
    Ok(())
}

/// How many decisions the bare `decision list` / `query decisions` shows before
/// `--all` is needed: design history grows without bound, but an agent orienting
/// only needs the recent forks, so the default bounds output to this window.
const RECENT_DECISIONS: usize = 20;

/// Shared renderer for `decision list` and `query decisions` (ac-4): scope to one
/// feature when asked, window to the most recent decisions by activity unless
/// `--all`, and render the ID/STATUS/HOME/TITLE table. Both call sites pass their
/// already-scanned entries (tolerant vs strict scan), so the windowing stays
/// identical across the two verbs.
pub(crate) fn render_decision_list(
    mut entries: Vec<decisions::DecisionListEntry>,
    all: bool,
    feature: Option<&str>,
) -> Result<()> {
    if let Some(feature_id) = feature {
        entries.retain(|entry| {
            matches!(&entry.source, decisions::DecisionSource::Feature { feature_id: id } if id == feature_id)
        });
        if entries.is_empty() {
            println!("no decisions for feature {feature_id}");
            return Ok(());
        }
    } else if entries.is_empty() {
        println!("no decisions found");
        return Ok(());
    }

    // Most-recent-first by activity (locked_at else created_at). Ties and legacy
    // rows (empty activity) fall back to a stable id order so output is deterministic.
    entries.sort_by(|left, right| {
        right
            .activity()
            .cmp(left.activity())
            .then_with(|| left.id.cmp(&right.id))
    });

    let total = entries.len();
    if !all && total > RECENT_DECISIONS {
        entries.truncate(RECENT_DECISIONS);
        println!(
            "{} of {total} recent (--all for full; --feature <id> to scope)",
            entries.len()
        );
    }

    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|entry| {
            vec![
                entry.id.clone(),
                entry.status.clone(),
                home(&entry.source),
                entry.title.clone(),
            ]
        })
        .collect();
    print!(
        "{}",
        table::render_table(&["ID", "STATUS", "HOME", "TITLE"], &rows)
    );

    Ok(())
}

fn home(source: &decisions::DecisionSource) -> String {
    match source {
        decisions::DecisionSource::Global => "global".to_string(),
        decisions::DecisionSource::Feature { feature_id } => format!("feature:{feature_id}"),
        decisions::DecisionSource::Legacy => "legacy-md".to_string(),
    }
}
