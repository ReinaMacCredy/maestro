use anyhow::{Context, Result, bail};
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
                eprintln!("maestro hook record warning: {error:#}");
            }
            Ok(())
        }
    }
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
            if let Err(error) = ensure_auto_progress_for_hook(paths, &payload) {
                eprintln!(
                    "maestro hook record warning: automatic progress start failed: {error:#}"
                );
            }
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
    if current_task_is_set() || !is_write_like_pre_tool_use(payload) {
        return Ok(());
    }
    let Some(session_id) = record::payload_session_id(payload) else {
        return Ok(());
    };
    let (agent, actor) = auto_progress_actor(payload, &session_id);
    let title = auto_progress_title(payload, &session_id);
    if let Some(progress_card) = active_progress_card_for_actor(paths, &actor)? {
        emit_card_touch_for_session(paths, &progress_card, &session_id, &agent);
        return Ok(());
    }
    bail!(
        "Progress setup required before write-like work; run: maestro task setup --task {:?} --start",
        title
    )
}

fn active_progress_card_for_actor(paths: &MaestroPaths, actor: &str) -> Result<Option<String>> {
    for entry in task::load_progress_task_entries(paths)? {
        if entry.task.state != task::TaskState::InProgress {
            continue;
        }
        if entry.task.claimed_by.as_deref() != Some(actor) {
            continue;
        }
        let progress_card = entry
            .task_dir
            .file_name()
            .and_then(|name| name.to_str())
            .context("progress task directory is missing card id")?
            .to_string();
        return Ok(Some(progress_card));
    }
    Ok(None)
}

fn current_task_is_set() -> bool {
    std::env::var("MAESTRO_CURRENT_TASK")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
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
