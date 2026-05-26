use std::collections::BTreeMap;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::core::paths::MaestroPaths;
use crate::metrics::summary::{render_summary, summarize};
use crate::task::blockers::has_unresolved_blockers;
use crate::task::display::render_task_list;
use crate::task::doctor::load_task_records;
use crate::task::lookup::load_task_with_snapshot;
use crate::task::template::TaskState;

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
        tool("maestro_status", "Returns high-level repo state: counts of tasks by state, current claimed_by per agent, current MAESTRO_CURRENT_TASK (if set).", json!({"type":"object","properties":{}})),
        tool("maestro_task_list", "Lists tasks. Filters: ready, blocked, blocked_by, blocks, feature_id, claimed_by.", json!({"type":"object","properties":{"ready":{"type":"boolean"},"blocked":{"type":"boolean"},"blocked_by":{"type":"string"},"blocks":{"type":"string"},"feature_id":{"type":"string"},"claimed_by":{"type":"string"}}})),
        tool("maestro_task_show", "Returns full task detail for one task id (or current).", json!({"type":"object","properties":{"id":{"type":"string"}}})),
        tool("maestro_task_claim", "Claims a task; sets claimed_by; auto-progresses ready to in_progress.", json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]})),
        tool("maestro_task_complete", "Completes a task; in_progress to needs_verification; takes summary and one completion claim.", json!({"type":"object","properties":{"id":{"type":"string"},"summary":{"type":"string"},"claim":{"type":"string"},"claims":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":1}},"required":["id","summary"]})),
        tool("maestro_task_block", "Adds a blocker to a task; takes reason and optional blocked_ref.", json!({"type":"object","properties":{"id":{"type":"string"},"reason":{"type":"string"},"blocked_ref":{"type":"string"}},"required":["id","reason"]})),
        tool("maestro_task_unblock", "Resolves a blocker on a task.", json!({"type":"object","properties":{"id":{"type":"string"},"blocker":{"type":"string"}},"required":["id","blocker"]})),
        tool("maestro_feature_list", "Lists features with their computed task counts and statuses.", json!({"type":"object","properties":{}})),
        tool("maestro_feature_show", "Returns rich feature view computed at read time.", json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]})),
        tool("maestro_decision_list", "Lists all decisions in .maestro/decisions/.", json!({"type":"object","properties":{}})),
        tool("maestro_decision_new", "Creates a new decision file with the given title; opens template content.", json!({"type":"object","properties":{"title":{"type":"string"}},"required":["title"]})),
        tool("maestro_verify", "Runs maestro task verify on a task; returns the verification result.", json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]})),
        tool("maestro_query_matrix", "Returns the computed behavior to proof matrix.", json!({"type":"object","properties":{}})),
        tool("maestro_metrics_summary", "Returns the computed metrics summary.", json!({"type":"object","properties":{}})),
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
        "maestro_feature_list" => cli(vec!["feature".to_string(), "list".to_string()]),
        "maestro_feature_show" => cli(required_args(arguments, &["feature", "show"], &["id"])?),
        "maestro_decision_list" => cli(vec!["decision".to_string(), "list".to_string()]),
        "maestro_decision_new" => cli(required_args(arguments, &["decision", "new"], &["title"])?),
        "maestro_verify" => cli(required_args(arguments, &["task", "verify"], &["id"])?),
        "maestro_query_matrix" => cli(vec!["query".to_string(), "matrix".to_string()]),
        "maestro_metrics_summary" => Ok(render_summary(&summarize(paths)?)),
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
    let summary = summarize(paths)?;
    let mut claimed = BTreeMap::<String, Vec<String>>::new();
    for task in load_task_records(&paths.tasks_dir())? {
        if let Some(agent) = task.claimed_by {
            claimed.entry(agent).or_default().push(task.id);
        }
    }

    let mut out = render_summary(&summary);
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
    Ok(out)
}

fn task_list(paths: &MaestroPaths, arguments: &Value) -> Result<String> {
    let ready = bool_arg(arguments, "ready");
    let blocked = bool_arg(arguments, "blocked");
    let blocked_by = string_arg(arguments, "blocked_by");
    let blocks = string_arg(arguments, "blocks");
    let feature_id = string_arg(arguments, "feature_id");
    let claimed_by = string_arg(arguments, "claimed_by");
    let mut tasks = load_task_records(&paths.tasks_dir())?;

    if ready {
        tasks.retain(|task| task.state == TaskState::Ready && !has_unresolved_blockers(task));
    }
    if blocked {
        tasks.retain(has_unresolved_blockers);
    }
    if let Some(feature_id) = feature_id.as_deref() {
        tasks.retain(|task| task.feature_id.as_deref() == Some(feature_id));
    }
    if let Some(claimed_by) = claimed_by.as_deref() {
        tasks.retain(|task| task.claimed_by.as_deref() == Some(claimed_by));
    }
    if let Some(blocked_by) = blocked_by.as_deref() {
        tasks.retain(|task| {
            task.blockers.iter().any(|blocker| {
                blocker.resolved_at.is_none()
                    && blocker
                        .blocked_ref
                        .as_ref()
                        .map(|blocked_ref| blocked_ref.id.as_str() == blocked_by)
                        .unwrap_or(false)
            })
        });
    }
    if let Some(blocks) = blocks.as_deref() {
        let (task, _, _) = load_task_with_snapshot(&paths.tasks_dir(), blocks)?;
        let blocking = task
            .blockers
            .iter()
            .filter(|blocker| blocker.resolved_at.is_none())
            .filter_map(|blocker| blocker.blocked_ref.as_ref())
            .map(|blocked_ref| blocked_ref.id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        tasks.retain(|task| blocking.contains(&task.id));
    }

    tasks.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(render_task_list(&tasks))
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
