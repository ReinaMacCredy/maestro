use anyhow::Result;
use serde_json::json;

use crate::domain::run::RecordOutcome;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{HookArgs, HookCommand};
use crate::interfaces::hooks::record;

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
    } = outcome
    {
        let skill_detail = skill_for_ack
            .filter(|_| event_type == "skill_activation")
            .map(|skill| format!(" ({skill})"))
            .unwrap_or_default();
        println!("recorded {event_type}{skill_detail} -> runs/{run_dir}");
    }
    Ok(())
}
