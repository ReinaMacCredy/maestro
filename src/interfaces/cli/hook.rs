use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::domain::run::{self, RecordOutcome};
use crate::domain::task;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::session::agent_runtime_from_env;
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{HookArgs, HookCommand};
use crate::interfaces::hooks::record;
use crate::operations::harness;

/// Event kinds that fire ~once per meaningful action, so the echo can afford a
/// verbose multi-line block (D8). Everything else -- the per-tool firehose --
/// stays a single terse line so a full hook install never floods the console.
const VERBOSE_EVENTS: [&str; 3] = ["skill_activation", "SessionStart", "card_touch"];

pub fn run(args: HookArgs) -> Result<()> {
    match args.command {
        HookCommand::Record {
            event,
            skill,
            session,
        } => {
            let result = discover_repo_root()
                .map(MaestroPaths::new)
                .and_then(|paths| record_hook(&paths, event, skill, session));
            if let Err(error) = result {
                if error.is::<ProgressSetupBlock>() {
                    return Err(error);
                }
                eprintln!("maestro hook record warning: {error:#}");
            }
            Ok(())
        }
    }
}

#[derive(Debug)]
struct ProgressSetupBlock(String);

impl std::fmt::Display for ProgressSetupBlock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ProgressSetupBlock {}

fn progress_setup_block(message: String) -> anyhow::Error {
    ProgressSetupBlock(message).into()
}

fn record_hook(
    paths: &MaestroPaths,
    event: Option<String>,
    skill: Option<String>,
    session: Option<String>,
) -> Result<()> {
    let skill_for_ack = skill.clone();
    let stdin_payload = record::optional_stdin_payload()?;
    let outcome = match event {
        Some(event) => {
            let session_id = session
                .or_else(|| stdin_payload.as_ref().and_then(record::payload_session_id))
                .unwrap_or_else(super::cli_run_id);
            let mut payload = json!({
                "event": event,
                "session_id": session_id,
                "agent": super::actor(),
            });
            if let Some(skill) = skill {
                payload["skill_name"] = json!(skill);
            }
            record::record_value(paths, &payload)?
        }
        None => {
            let Some(payload) = stdin_payload else {
                return Ok(());
            };
            ensure_auto_progress_for_hook(paths, &payload)?;
            record::record_value(paths, &payload)?
        }
    };
    if let RecordOutcome::Recorded {
        event_type,
        run_dir,
        session_id,
    } = outcome
    {
        if VERBOSE_EVENTS.contains(&event_type.as_str()) {
            print_verbose_block(
                paths,
                &event_type,
                skill_for_ack,
                session_id.as_deref(),
                &run_dir,
            );
        } else {
            println!("recorded {event_type} -> runs/{run_dir}");
        }
    }
    Ok(())
}

fn ensure_auto_progress_for_hook(paths: &MaestroPaths, payload: &Value) -> Result<()> {
    if !is_write_like_pre_tool_use(payload) {
        return Ok(());
    }
    if let Some(current_task_id) = current_task_id() {
        return ensure_current_progress_allows_write(paths, &current_task_id);
    }
    let Some(session_id) = record::payload_session_id(payload) else {
        return Ok(());
    };
    let (agent, actor) = auto_progress_actor(payload, &session_id);
    let title = auto_progress_title(payload, &session_id);
    if let Some(progress) = active_progress_for_actor(paths, &actor)? {
        ensure_progress_allows_write(&progress)?;
        emit_card_touch_for_session(paths, &progress.card_id, &session_id, &agent);
        return Ok(());
    }
    Err(progress_setup_block(format!(
        "blocked: Progress setup required before write-like work\nreason: no active Progress checklist is claimed by {actor}\nfix: maestro task setup --task \"Map current behavior\" --task \"Implement scoped fix\" --task \"Verify\" --start\noverride: maestro task setup --task {:?} --start --atomic --reason \"<why one row is enough>\"",
        title
    )))
}

#[derive(Clone, Debug)]
struct ActiveProgress {
    card_id: String,
    task: task::TaskRecord,
    total_tasks: usize,
}

#[derive(Clone, Debug)]
struct ProgressRow {
    card_id: String,
    task: task::TaskRecord,
}

fn ensure_current_progress_allows_write(paths: &MaestroPaths, current_task_id: &str) -> Result<()> {
    if let Some(progress) = progress_for_task(paths, current_task_id)? {
        ensure_progress_allows_write(&progress)?;
    }
    Ok(())
}

fn active_progress_for_actor(paths: &MaestroPaths, actor: &str) -> Result<Option<ActiveProgress>> {
    let rows = progress_rows(paths)?;
    Ok(active_progress_from_rows(&rows, |task| {
        task.state == task::TaskState::InProgress && task.claimed_by.as_deref() == Some(actor)
    }))
}

fn progress_for_task(paths: &MaestroPaths, task_id: &str) -> Result<Option<ActiveProgress>> {
    let rows = progress_rows(paths)?;
    Ok(active_progress_from_rows(&rows, |task| task.id == task_id))
}

fn progress_rows(paths: &MaestroPaths) -> Result<Vec<ProgressRow>> {
    let mut rows = Vec::new();
    for entry in task::load_progress_task_entries(paths)? {
        let card_id = entry
            .task_dir
            .file_name()
            .and_then(|name| name.to_str())
            .context("progress task directory is missing card id")?
            .to_string();
        rows.push(ProgressRow {
            card_id,
            task: entry.task,
        });
    }
    Ok(rows)
}

fn active_progress_from_rows(
    rows: &[ProgressRow],
    predicate: impl Fn(&task::TaskRecord) -> bool,
) -> Option<ActiveProgress> {
    let row = rows.iter().find(|row| predicate(&row.task))?;
    let total_tasks = rows
        .iter()
        .filter(|candidate| candidate.card_id == row.card_id)
        .count();
    Some(ActiveProgress {
        card_id: row.card_id.clone(),
        task: row.task.clone(),
        total_tasks,
    })
}

fn ensure_progress_allows_write(progress: &ActiveProgress) -> Result<()> {
    if progress.total_tasks >= 2 {
        return Ok(());
    }
    if progress.task.atomic
        && progress
            .task
            .atomic_reason
            .as_deref()
            .is_some_and(|reason| !reason.trim().is_empty())
    {
        return Ok(());
    }
    Err(progress_setup_block(format!(
        "blocked: Progress setup needs a visible checklist before write-like work\ntask: {}\nreason: one active Progress task is not marked atomic\nfix: maestro task setup --task \"Map current behavior\" --task \"Implement scoped fix\" --task \"Verify\"\noverride: create the task with --atomic --reason when one row is truly enough",
        progress.task.id
    )))
}

fn current_task_id() -> Option<String> {
    std::env::var("MAESTRO_CURRENT_TASK")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_write_like_pre_tool_use(payload: &Value) -> bool {
    if payload_event_type(payload) != Some("PreToolUse") {
        return false;
    }
    let Some(tool_name) = string_field(payload, "tool_name") else {
        return false;
    };
    let tool_name = tool_name.to_ascii_lowercase();
    matches!(
        tool_name.as_str(),
        "edit" | "multiedit" | "write" | "notebookedit" | "apply_patch" | "functions.apply_patch"
    ) || tool_name.ends_with("::apply_patch")
        || tool_name.ends_with(".apply_patch")
}

fn payload_event_type(payload: &Value) -> Option<&str> {
    ["event_type", "hook_event_name", "kind", "event", "type"]
        .into_iter()
        .find_map(|field| payload.get(field).and_then(Value::as_str))
}

fn auto_progress_actor(payload: &Value, session_id: &str) -> (String, String) {
    let agent = string_field(payload, "agent")
        .or_else(|| agent_runtime_from_env().map(str::to_string))
        .unwrap_or_else(|| "maestro".to_string());
    let actor = format!("{agent}#{session_id}");
    (agent, actor)
}

fn auto_progress_title(payload: &Value, session_id: &str) -> String {
    if let Some(file_path) = payload
        .get("tool_input")
        .and_then(|input| input.get("file_path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("Work on {file_path}");
    }
    let short_session: String = session_id.chars().take(8).collect();
    format!("Implementation work for session {short_session}")
}

fn string_field(payload: &Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn emit_card_touch_for_session(paths: &MaestroPaths, card_id: &str, session_id: &str, agent: &str) {
    let payload = json!({
        "event": "card_touch",
        "session_id": session_id,
        "card_id": card_id,
        "agent": agent,
    });
    if let Err(error) = record::record_value(paths, &payload) {
        eprintln!("maestro hook record warning: auto-progress card_touch failed: {error:#}");
    }
}

/// The D8 verbose block for a low-frequency event: kind, skill (when a
/// skill_activation), session, bound card, run dir, and a `maestro active` tip.
/// The bound card is the session's latest `card_touch` (read back through the
/// awareness view), so activating a skill shows which card the session is on.
fn print_verbose_block(
    paths: &MaestroPaths,
    event_type: &str,
    skill: Option<String>,
    session_id: Option<&str>,
    run_dir: &str,
) {
    println!("recorded: {event_type}");
    if event_type == "skill_activation"
        && let Some(skill) = skill
    {
        println!("  skill:   {skill}");
    }
    println!("  session: {}", session_id.unwrap_or("unattributed"));
    if let Some(card) = session_id.and_then(|session| bound_card(paths, session)) {
        println!("  card:    {card}");
    }
    println!("  -> runs/{run_dir}");
    println!("  tip:     maestro active  (see other live sessions)");
    if let Ok(readout) = harness::complete_readout(paths) {
        println!("  {}", readout.hook_trace_summary_line());
    }
}

/// The session's currently bound card -- its latest `card_touch` -- read through
/// the same liveness view `maestro active` renders, so the echo and the verb
/// agree. Best-effort: a read failure just drops the card line.
fn bound_card(paths: &MaestroPaths, session_id: &str) -> Option<String> {
    run::active_sessions(paths, &utc_now_timestamp())
        .ok()?
        .into_iter()
        .find(|row| row.session_id == session_id)
        .and_then(|row| row.bound_card)
}
