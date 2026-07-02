use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::json;

use crate::domain::decisions::{
    self, DecisionRecord, DecisionRecordKind, DecisionSetArchiveScope, DecisionSetChildSummary,
    DecisionSetPlan, DecisionSetWriteReport,
};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::table;
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{DecisionArgs, DecisionCommand, DecisionSetCommand};
use crate::operations::harness;

/// Execute `maestro decision`.
pub fn run(args: DecisionArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        DecisionCommand::Audit { compressed, json } => audit_decisions(&paths, compressed, json),
        DecisionCommand::Set { command } => run_decision_set(&paths, command),
        DecisionCommand::New {
            title,
            context,
            feature,
            lock,
            decision,
            rejected,
            preview,
            supersedes,
            allow_summary_decision,
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
                        allow_summary_decision,
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
            allow_summary_decision,
        } => lock_decision(
            &paths,
            &id,
            &decision,
            &rejected,
            preview.as_deref(),
            &supersedes,
            allow_summary_decision,
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
        DecisionCommand::Show { id, include_set } => show_decision(&paths, &id, include_set),
        DecisionCommand::List { all, feature } => {
            render_decision_list(decisions::list_tolerant(&paths), all, feature.as_deref())
        }
    }
}

fn run_decision_set(paths: &MaestroPaths, command: DecisionSetCommand) -> Result<()> {
    match command {
        DecisionSetCommand::Draft {
            from,
            from_text,
            output,
            json,
        } => draft_decision_set(from.as_deref(), from_text.as_deref(), output, json),
        DecisionSetCommand::Lock {
            from,
            dry_run,
            json,
            show,
        } => lock_decision_set(paths, &from, dry_run, json, show),
        DecisionSetCommand::Repair {
            id,
            from,
            dry_run,
            json,
        } => repair_decision_set(paths, &id, &from, dry_run, json),
        DecisionSetCommand::Archive {
            id,
            set_only,
            include_children,
        } => archive_decision_set(paths, &id, set_only, include_children),
        DecisionSetCommand::Show { id, json } => show_decision_set(paths, &id, json),
    }
}

fn audit_decisions(paths: &MaestroPaths, compressed: bool, emit_json: bool) -> Result<()> {
    if !compressed {
        bail!("decision audit currently requires --compressed");
    }
    let candidates = decisions::compressed_summary_candidates(paths)?;
    if emit_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "audit": "compressed_decision_summaries",
                "candidates": candidates,
            }))?
        );
        return Ok(());
    }
    if candidates.is_empty() {
        println!("no compressed decision summaries found");
        return Ok(());
    }
    println!("compressed decision summaries:");
    for candidate in candidates {
        println!(
            "- {} [{}] {} (signals: {})",
            candidate.id,
            candidate.status,
            candidate.title,
            candidate.signals.join(", ")
        );
    }
    Ok(())
}

fn draft_decision_set(
    from: Option<&Path>,
    from_text: Option<&str>,
    output: Option<PathBuf>,
    emit_json: bool,
) -> Result<()> {
    let plan = decision_set_plan_from_input(from, from_text)?;
    let rendered = if emit_json {
        serde_json::to_string_pretty(&plan)?
    } else {
        render_decision_set_plan(&plan)
    };
    if let Some(path) = output {
        fs::write(&path, &rendered)
            .with_context(|| format!("failed to write DecisionSet draft {}", path.display()))?;
        println!("wrote {}", path.display());
        return Ok(());
    }
    println!("{rendered}");
    Ok(())
}

fn lock_decision_set(
    paths: &MaestroPaths,
    from: &Path,
    dry_run: bool,
    emit_json: bool,
    show: bool,
) -> Result<()> {
    let plan = decision_set_plan_from_file(from)?;
    if dry_run {
        if emit_json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "dry_run": true,
                    "plan": plan,
                    "set_id": plan.set_id,
                    "children": plan.children,
                    "warnings": plan.warnings,
                }))?
            );
            return Ok(());
        }
        println!("dry-run {}", plan.set_id);
        println!("children: {}", plan.children.len());
        for child in &plan.children {
            println!("  {}. {} -> {}", child.order, child.key, child.title);
        }
        return Ok(());
    }

    let report = decisions::write_plan_records(paths, &plan, &utc_now_timestamp())?;
    if emit_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&DecisionSetLockJson::from_report(&report))?
        );
        return Ok(());
    }

    println!("locked {}", report.set_record.id);
    println!("children: {}", report.child_records.len());
    println!("show: maestro decision set show {}", report.set_record.id);
    if show {
        print!("{}", decisions::render_record(&report.set_record));
    }
    Ok(())
}

fn repair_decision_set(
    paths: &MaestroPaths,
    id: &str,
    from: &Path,
    dry_run: bool,
    emit_json: bool,
) -> Result<()> {
    let candidate = decisions::compressed_summary_candidate(paths, id)?;
    let plan = decision_set_plan_from_file(from)?;
    if dry_run {
        if emit_json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "dry_run": true,
                    "set_id": plan.set_id,
                    "supersedes": candidate.id,
                    "signals": candidate.signals,
                    "children": plan.children,
                    "warnings": plan.warnings,
                }))?
            );
            return Ok(());
        }
        println!("dry-run repair {}", candidate.id);
        println!("replacement: {}", plan.set_id);
        println!("children: {}", plan.children.len());
        for child in &plan.children {
            println!("  {}. {} -> {}", child.order, child.key, child.title);
        }
        return Ok(());
    }

    let report =
        decisions::repair_compressed_summary(paths, &candidate.id, &plan, &utc_now_timestamp())?;
    if emit_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&DecisionSetRepairJson::from_report(
                &candidate.id,
                &report,
            ))?
        );
        return Ok(());
    }
    println!("repaired {}", candidate.id);
    println!("locked {}", report.set_record.id);
    println!("children: {}", report.child_records.len());
    Ok(())
}

fn show_decision_set(paths: &MaestroPaths, id: &str, emit_json: bool) -> Result<()> {
    let record = match decisions::show(paths, id)? {
        decisions::DecisionContent::Structured { record, .. } => record,
        decisions::DecisionContent::Legacy { .. } => {
            bail!("{id} is a legacy decision, not a DecisionSet")
        }
    };
    if record.kind != DecisionRecordKind::DecisionSet {
        bail!("{id} is a {}, not a DecisionSet", record.kind.as_str());
    }

    if emit_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&DecisionSetShowJson::from_record(paths, &record)?)?
        );
        return Ok(());
    }
    print!("{}", render_decision_set_record(paths, &record)?);
    Ok(())
}

fn archive_decision_set(
    paths: &MaestroPaths,
    id: &str,
    set_only: bool,
    include_children: bool,
) -> Result<()> {
    let scope = match (set_only, include_children) {
        (true, false) => DecisionSetArchiveScope::SetOnly,
        (false, true) => DecisionSetArchiveScope::IncludeChildren,
        _ => bail!("choose exactly one: --set-only OR --include-children"),
    };
    let report = decisions::archive_set(paths, id, scope)?;
    println!("archived {}", report.set_id);
    println!("cards: {}", report.archived.len());
    for id in report.archived {
        println!("  {id}");
    }
    Ok(())
}

fn decision_set_plan_from_input(
    from: Option<&Path>,
    from_text: Option<&str>,
) -> Result<DecisionSetPlan> {
    match (from, from_text) {
        (Some(path), None) => decision_set_plan_from_file(path),
        (None, Some(text)) => decisions::draft_from_text(text),
        (None, None) => bail!("decision set draft needs --from <path> or --from-text <text>"),
        (Some(_), Some(_)) => bail!("use either --from or --from-text, not both"),
    }
}

fn decision_set_plan_from_file(path: &Path) -> Result<DecisionSetPlan> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read DecisionSet input {}", path.display()))?;
    decisions::plan_from_yaml(&extract_fenced_yaml(&raw))
}

fn extract_fenced_yaml(raw: &str) -> String {
    let mut in_yaml = false;
    let mut start = 0;
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if !in_yaml && (trimmed == "```yaml" || trimmed == "```yml") {
            in_yaml = true;
            start = index + 1;
            continue;
        }
        if in_yaml && trimmed == "```" {
            return raw
                .lines()
                .skip(start)
                .take(index - start)
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
    raw.to_string()
}

fn render_decision_set_plan(plan: &DecisionSetPlan) -> String {
    let mut output = String::new();
    output.push_str(&format!("set: {}\n", plan.set_id));
    output.push_str(&format!("title: {}\n", plan.title));
    output.push_str(&format!("children: {}\n", plan.children.len()));
    for child in &plan.children {
        output.push_str(&format!(
            "  {}. {} -> {}\n",
            child.order, child.key, child.title
        ));
    }
    for warning in &plan.warnings {
        output.push_str(&format!("warning: {}: {}\n", warning.code, warning.message));
    }
    output
}

fn render_decision_set_record(paths: &MaestroPaths, record: &DecisionRecord) -> Result<String> {
    let mut output = decisions::render_record(record);
    if record.decision_set_children.is_empty() {
        return Ok(output);
    }

    let rows = record
        .decision_set_children
        .iter()
        .map(|child| {
            let live = child
                .child_decision_id
                .as_deref()
                .map(|id| child_live(paths, id))
                .transpose()?
                .unwrap_or(false);
            Ok(vec![
                child.order.to_string(),
                if live { "live" } else { "missing" }.to_string(),
                child
                    .child_decision_id
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                child.title.clone(),
            ])
        })
        .collect::<Result<Vec<_>>>()?;
    output.push_str("child_status:\n");
    output.push_str(&table::render_table(
        &["ORDER", "LIVE", "SNAPSHOT", "TITLE"],
        &rows,
    ));
    Ok(output)
}

fn child_live(paths: &MaestroPaths, id: &str) -> Result<bool> {
    decisions::decision_exists(paths, id)
}

#[derive(Serialize)]
struct DecisionSetLockJson<'a> {
    dry_run: bool,
    id: &'a str,
    kind: &'static str,
    children: Vec<DecisionSetChildJson<'a>>,
}

#[derive(Serialize)]
struct DecisionSetRepairJson<'a> {
    dry_run: bool,
    id: &'a str,
    kind: &'static str,
    supersedes: &'a str,
    children: Vec<DecisionSetChildJson<'a>>,
}

impl<'a> DecisionSetRepairJson<'a> {
    fn from_report(supersedes: &'a str, report: &'a DecisionSetWriteReport) -> Self {
        Self {
            dry_run: false,
            id: &report.set_record.id,
            kind: report.set_record.kind.as_str(),
            supersedes,
            children: report
                .set_record
                .decision_set_children
                .iter()
                .map(DecisionSetChildJson::from_summary)
                .collect(),
        }
    }
}

impl<'a> DecisionSetLockJson<'a> {
    fn from_report(report: &'a DecisionSetWriteReport) -> Self {
        Self {
            dry_run: false,
            id: &report.set_record.id,
            kind: report.set_record.kind.as_str(),
            children: report
                .set_record
                .decision_set_children
                .iter()
                .map(DecisionSetChildJson::from_summary)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct DecisionSetShowJson<'a> {
    id: &'a str,
    title: &'a str,
    kind: &'static str,
    input_hash: Option<&'a str>,
    children: Vec<DecisionSetChildJson<'a>>,
}

impl<'a> DecisionSetShowJson<'a> {
    fn from_record(paths: &MaestroPaths, record: &'a DecisionRecord) -> Result<Self> {
        Ok(Self {
            id: &record.id,
            title: &record.title,
            kind: record.kind.as_str(),
            input_hash: record.input_hash.as_deref(),
            children: record
                .decision_set_children
                .iter()
                .map(|child| DecisionSetChildJson::from_summary_with_live(paths, child))
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

#[derive(Serialize)]
struct DecisionSetChildJson<'a> {
    key: &'a str,
    title: &'a str,
    order: u32,
    preview: Option<&'a str>,
    child_decision_id: Option<&'a str>,
    locked_at: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    live: Option<bool>,
}

impl<'a> DecisionSetChildJson<'a> {
    fn from_summary(summary: &'a DecisionSetChildSummary) -> Self {
        Self {
            key: &summary.key,
            title: &summary.title,
            order: summary.order,
            preview: summary.preview.as_deref(),
            child_decision_id: summary.child_decision_id.as_deref(),
            locked_at: summary.locked_at.as_deref(),
            live: None,
        }
    }

    fn from_summary_with_live(
        paths: &MaestroPaths,
        summary: &'a DecisionSetChildSummary,
    ) -> Result<Self> {
        let live = summary
            .child_decision_id
            .as_deref()
            .map(|id| child_live(paths, id))
            .transpose()?;
        Ok(Self {
            live,
            ..Self::from_summary(summary)
        })
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
    println!("{}", harness::security_decision_gate_line());
    println!("{}", harness::guardrail_decision_line());
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
    allow_summary_decision: bool,
) -> Result<()> {
    if rejected.is_empty() {
        bail!("decision lock requires at least one --rejected \"<option: why>\"");
    }
    let report = decisions::lock(
        paths,
        id,
        decision,
        rejected,
        preview,
        supersedes,
        allow_summary_decision,
    )?;
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
fn emit_feature_touch(paths: &MaestroPaths, record: &DecisionRecord) {
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
    println!("{}", harness::security_decision_gate_line());
    println!("{}", harness::guardrail_decision_line());
}

fn show_decision(paths: &MaestroPaths, id: &str, include_set: bool) -> Result<()> {
    match decisions::show(paths, id)? {
        decisions::DecisionContent::Structured { record, path, .. } => {
            println!("store: {}", path.display());
            print!("{}", decisions::render_record(&record));
            if include_set {
                print_decision_set_pointer(paths, &record)?;
            }
            println!("{}", harness::security_decision_gate_line());
            println!("{}", harness::guardrail_decision_line());
        }
        decisions::DecisionContent::Legacy { contents, path, .. } => {
            println!("legacy: {}", path.display());
            print!("{contents}");
            println!("{}", harness::security_decision_gate_line());
            println!("{}", harness::guardrail_decision_line());
        }
    }
    Ok(())
}

fn print_decision_set_pointer(paths: &MaestroPaths, record: &DecisionRecord) -> Result<()> {
    let Some(set_id) = record.decision_set_id.as_deref() else {
        return Ok(());
    };
    let decisions::DecisionContent::Structured { record: set, .. } =
        decisions::show(paths, set_id)?
    else {
        return Ok(());
    };
    if set.kind != DecisionRecordKind::DecisionSet {
        return Ok(());
    }
    println!("decision_set:");
    println!("  id: {}", set.id);
    println!("  title: {}", set.title);
    println!("  children: {}", set.decision_set_children.len());
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
                decision_list_title(entry),
            ]
        })
        .collect();
    print!(
        "{}",
        table::render_table(&["ID", "STATUS", "HOME", "TITLE"], &rows)
    );

    Ok(())
}

fn decision_list_title(entry: &decisions::DecisionListEntry) -> String {
    if entry.kind == "decision_set" {
        format!("[set] {}", entry.title)
    } else if let Some(set_id) = entry.decision_set_id.as_deref() {
        format!("[child:{set_id}] {}", entry.title)
    } else {
        entry.title.clone()
    }
}

fn home(source: &decisions::DecisionSource) -> String {
    match source {
        decisions::DecisionSource::Global => "global".to_string(),
        decisions::DecisionSource::Feature { feature_id } => format!("feature:{feature_id}"),
        decisions::DecisionSource::Legacy => "legacy-md".to_string(),
    }
}
