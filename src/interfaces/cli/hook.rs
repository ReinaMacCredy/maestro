use anyhow::Result;
use serde_json::json;

use crate::domain::run::{self, RecordOutcome};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{HookArgs, HookCommand};
use crate::interfaces::hooks::record;

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
    let outcome = match event {
        Some(event) => {
            let session_id = session.unwrap_or_else(super::cli_run_id);
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
        None => record::record_stdin(paths)?,
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
