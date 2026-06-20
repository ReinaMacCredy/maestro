use std::collections::BTreeMap;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::domain::task;
use crate::foundation::core::paths::MaestroPaths;
use crate::operations::harness;

/// MCP tool metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

/// Return all V1 Maestro MCP tool definitions.
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        tool(
            "maestro_status",
            "Returns high-level repo state: counts of tasks by state, current claimed_by per agent, current MAESTRO_CURRENT_TASK (if set).",
            json!({"type":"object","properties":{}}),
        ),
        tool(
            "maestro_task_list",
            "Lists tasks (terminal/done tasks hidden unless all=true). Filters: ready, blocked, blocked_by, blocks, feature_id, claimed_by, all.",
            json!({"type":"object","properties":{"ready":{"type":"boolean"},"blocked":{"type":"boolean"},"blocked_by":{"type":"string"},"blocks":{"type":"string"},"feature_id":{"type":"string"},"claimed_by":{"type":"string"},"all":{"type":"boolean"}}}),
        ),
        tool(
            "maestro_task_show",
            "Returns full task detail for one task id (or current).",
            json!({"type":"object","properties":{"id":{"type":"string"}}}),
        ),
        tool(
            "maestro_task_claim",
            "Claims a task; sets claimed_by; auto-progresses ready to in_progress.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_task_complete",
            "Completes a task; in_progress to needs_verification; takes summary and one completion claim.",
            json!({"type":"object","properties":{"id":{"type":"string"},"summary":{"type":"string"},"claim":{"type":"string"},"claims":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":1}},"required":["id","summary"]}),
        ),
        tool(
            "maestro_task_block",
            "Adds a blocker to a task; takes reason and optional blocked_ref.",
            json!({"type":"object","properties":{"id":{"type":"string"},"reason":{"type":"string"},"blocked_ref":{"type":"string"}},"required":["id","reason"]}),
        ),
        tool(
            "maestro_task_unblock",
            "Resolves a blocker on a task.",
            json!({"type":"object","properties":{"id":{"type":"string"},"blocker":{"type":"string"}},"required":["id","blocker"]}),
        ),
        tool(
            "maestro_feature_list",
            "Lists features with their computed task counts and statuses (terminal features hidden unless all=true).",
            json!({"type":"object","properties":{"all":{"type":"boolean"}}}),
        ),
        tool(
            "maestro_feature_show",
            "Returns rich feature view computed at read time.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_feature_start",
            "Starts a ready feature; ready to in_progress.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_feature_close",
            "Closes an in_progress feature; in_progress to closed; enforces the close gate.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_decision_list",
            "Lists decision cards (recent 20 by activity hidden behind all=true, mirroring maestro_task_list).",
            json!({"type":"object","properties":{"all":{"type":"boolean"}}}),
        ),
        tool(
            "maestro_decision_new",
            "Creates a new decision file with the given title; opens template content.",
            json!({"type":"object","properties":{"title":{"type":"string"}},"required":["title"]}),
        ),
        tool(
            "maestro_verify",
            "Runs maestro task verify on a task; returns the verification result.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_query_matrix",
            "Returns the computed behavior to proof matrix.",
            json!({"type":"object","properties":{}}),
        ),
        tool(
            "maestro_sync",
            "Resyncs bundled resources (skills, hook script, harness) to this binary's shipped versions. Offline and edit-preserving; set dry_run=true to preview without writing.",
            json!({"type":"object","properties":{"dry_run":{"type":"boolean"}}}),
        ),
    ]
}

/// Execute a V1 Maestro MCP tool.
pub fn call_tool(paths: &MaestroPaths, name: &str, arguments: &Value) -> Result<String> {
    match name {
        "maestro_status" => status(paths),
        "maestro_task_list" => task_list(paths, arguments),
        "maestro_task_show" => cli(optional_id_args("task", "show", arguments, "id")),
        "maestro_task_claim" => cli(required_args(arguments, &["task", "claim"], &["id"])?),
        "maestro_task_complete" => task_complete(arguments),
        "maestro_task_block" => task_block(arguments),
        "maestro_task_unblock" => task_unblock(arguments),
        "maestro_feature_list" => feature_list(arguments),
        "maestro_feature_show" => cli(required_args(arguments, &["feature", "show"], &["id"])?),
        "maestro_feature_start" => cli(required_args(arguments, &["feature", "start"], &["id"])?),
        "maestro_feature_close" => cli(required_args(arguments, &["feature", "close"], &["id"])?),
        "maestro_decision_list" => decision_list(arguments),
        "maestro_decision_new" => cli(required_args(arguments, &["decision", "new"], &["title"])?),
        "maestro_verify" => cli(required_args(arguments, &["task", "verify"], &["id"])?),
        "maestro_query_matrix" => cli(vec!["query".to_string(), "matrix".to_string()]),
        "maestro_sync" => sync_tool(arguments),
        _ => bail!("unknown MCP tool: {name}"),
    }
}

fn tool(name: &'static str, description: &'static str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name,
        description,
        input_schema,
    }
}

fn status(paths: &MaestroPaths) -> Result<String> {
    let tasks = task::load_task_entries(&paths.tasks_dir())?;
    let total = tasks.len();
    let mut verified = 0_usize;
    let mut needs_verification = 0_usize;
    let mut in_progress = 0_usize;
    let mut claimed = BTreeMap::<String, Vec<String>>::new();
    for entry in tasks {
        match &entry.task.state {
            task::TaskState::Verified => verified += 1,
            task::TaskState::NeedsVerification => needs_verification += 1,
            task::TaskState::InProgress => in_progress += 1,
            _ => {}
        }
        if let Some(agent) = entry.task.claimed_by {
            claimed.entry(agent).or_default().push(entry.task.id);
        }
    }

    let mut out = format!(
        "Tasks: {total} ({verified} verified, {needs_verification} needs_verification, {in_progress} in_progress)\n"
    );
    match std::env::var("MAESTRO_CURRENT_TASK") {
        Ok(task_id) => out.push_str(&format!("Current task: {task_id}\n")),
        Err(_) => out.push_str("Current task: <none>\n"),
    }
    out.push_str("Claimed:\n");
    if claimed.is_empty() {
        out.push_str("  <none>\n");
    } else {
        for (agent, tasks) in claimed {
            out.push_str(&format!("  {agent}: {}\n", tasks.join(", ")));
        }
    }
    let friction = harness::over_threshold_items(paths)?;
    if !friction.is_empty() {
        out.push_str("Harness friction:\n");
        for item in friction {
            out.push_str(&format!(
                  "  ! {} {}x/{}s: {} (apply: maestro harness apply {}; dismiss: maestro harness dismiss {} --reason \"<why>\")\n",
                  item.id, item.occurrences, item.sessions, item.title, item.id, item.id
              ));
        }
    }
    Ok(out)
}

fn task_list(paths: &MaestroPaths, arguments: &Value) -> Result<String> {
    let all = bool_arg(arguments, "all");
    let mut tasks = task::load_task_records(&paths.tasks_dir())?;
    let mut archived_ids = std::collections::BTreeSet::new();
    if all {
        let archived: Vec<_> = task::load_archived_task_entries(paths)?
            .into_iter()
            .map(|entry| entry.task)
            .collect();
        archived_ids.extend(archived.iter().map(|t| t.id.clone()));
        tasks.extend(archived);
    }
    let filter = |include_terminal| task::TaskFilter {
        ready: bool_arg(arguments, "ready"),
        blocked: bool_arg(arguments, "blocked"),
        blocked_by: string_arg(arguments, "blocked_by"),
        blocks: string_arg(arguments, "blocks"),
        feature_id: string_arg(arguments, "feature_id"),
        claimed_by: string_arg(arguments, "claimed_by"),
        include_terminal,
    };
    let shown = task::filter_tasks(tasks.clone(), &filter(all));
    let missing_verify_contract_ids = task::missing_verify_contract_ids(paths, &shown)?;
    let mut out = task::render_task_list_with_missing_checks(
        &shown,
        &archived_ids,
        &missing_verify_contract_ids,
    );
    if !all {
        let hidden = task::filter_tasks(tasks, &filter(true)).len() - shown.len();
        if hidden > 0 {
            out.push_str(&format!(
                "# {hidden} terminal task(s) hidden; set all=true to include\n"
            ));
        }
    }
    Ok(out)
}

fn sync_tool(arguments: &Value) -> Result<String> {
    let mut argv = vec!["sync".to_string()];
    if bool_arg(arguments, "dry_run") {
        argv.push("--dry-run".to_string());
    }
    cli(argv)
}

fn feature_list(arguments: &Value) -> Result<String> {
    let mut argv = vec!["feature".to_string(), "list".to_string()];
    if bool_arg(arguments, "all") {
        argv.push("--all".to_string());
    }
    cli(argv)
}

fn decision_list(arguments: &Value) -> Result<String> {
    let mut argv = vec!["decision".to_string(), "list".to_string()];
    if bool_arg(arguments, "all") {
        argv.push("--all".to_string());
    }
    cli(argv)
}

fn task_complete(arguments: &Value) -> Result<String> {
    let id = required_string(arguments, "id")?;
    let summary = required_string(arguments, "summary")?;
    let claim = claim_arg(arguments)?;
    cli(vec![
        "task".to_string(),
        "complete".to_string(),
        id,
        "--summary".to_string(),
        summary,
        "--claim".to_string(),
        claim,
    ])
}

fn task_block(arguments: &Value) -> Result<String> {
    let mut args = vec![
        "task".to_string(),
        "block".to_string(),
        required_string(arguments, "id")?,
        "--reason".to_string(),
        required_string(arguments, "reason")?,
    ];
    if let Some(blocked_ref) = string_arg(arguments, "blocked_ref") {
        args.push("--by".to_string());
        args.push(blocked_ref);
    }
    cli(args)
}

fn task_unblock(arguments: &Value) -> Result<String> {
    cli(vec![
        "task".to_string(),
        "unblock".to_string(),
        required_string(arguments, "id")?,
        "--blocker".to_string(),
        required_string(arguments, "blocker")?,
    ])
}

fn cli(args: Vec<String>) -> Result<String> {
    let output = Command::new(std::env::current_exe().context("failed to find current binary")?)
        .args(&args)
        .output()
        .with_context(|| format!("failed to run maestro {}", args.join(" ")))?;
    if output.status.success() {
        return String::from_utf8(output.stdout).context("maestro stdout was not UTF-8");
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!("maestro {} failed: {}", args.join(" "), stderr.trim());
}

fn optional_id_args(first: &str, second: &str, arguments: &Value, field: &str) -> Vec<String> {
    let mut args = vec![first.to_string(), second.to_string()];
    if let Some(value) = string_arg(arguments, field) {
        args.push(value);
    }
    args
}

fn required_args(arguments: &Value, prefix: &[&str], fields: &[&str]) -> Result<Vec<String>> {
    let mut args = prefix
        .iter()
        .map(|entry| entry.to_string())
        .collect::<Vec<_>>();
    for field in fields {
        args.push(required_string(arguments, field)?);
    }
    Ok(args)
}

fn required_string(arguments: &Value, field: &str) -> Result<String> {
    string_arg(arguments, field).with_context(|| format!("missing required argument: {field}"))
}

fn string_arg(arguments: &Value, field: &str) -> Option<String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn claim_arg(arguments: &Value) -> Result<String> {
    if let Some(claim) = string_arg(arguments, "claim") {
        return Ok(claim);
    }
    let Some(claims) = arguments.get("claims").and_then(Value::as_array) else {
        bail!("missing required argument: claim");
    };
    if claims.len() != 1 {
        bail!("claims must contain exactly one claim");
    }
    claims[0]
        .as_str()
        .map(str::to_string)
        .context("claims[0] must be a string")
}

fn bool_arg(arguments: &Value, field: &str) -> bool {
    arguments
        .get(field)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
