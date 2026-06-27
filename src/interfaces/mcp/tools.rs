use std::collections::BTreeMap;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::domain::{card, feature, task};
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
            "Returns high-level repo state, current task context, and guidance for choosing task versus card tools.",
            json!({"type":"object","properties":{}}),
        ),
        tool(
            "maestro_task_next",
            "Returns the next recommended task action as structured JSON plus raw CLI output.",
            json!({"type":"object","properties":{}}),
        ),
        tool(
            "maestro_task_create",
            "Creates a task through the normal task lifecycle; returns a draft task.",
            json!({"type":"object","properties":{"title":{"type":"string"},"feature_id":{"type":"string"},"card_id":{"type":"string"},"lane":{"type":"string"},"risk":{"type":"string"},"checks":{"type":"array","items":{"type":"string"}},"covers":{"type":"array","items":{"type":"string"}},"project":{"type":"string"},"id_only":{"type":"boolean"}},"required":["title"]}),
        ),
        tool(
            "maestro_task_add",
            "Adds a low-ceremony task ready to start; optional card_id is limited to Chore cards.",
            json!({"type":"object","properties":{"title":{"type":"string"},"card_id":{"type":"string"},"project":{"type":"string"},"id_only":{"type":"boolean"}},"required":["title"]}),
        ),
        tool(
            "maestro_task_explore",
            "Moves a draft task to exploring.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_task_accept",
            "Locks task acceptance and moves the task to ready.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
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
            "maestro_task_start",
            "Starts a ready task; alias for task claim.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_task_done",
            "Marks a low-ceremony standalone or Chore-owned task done when no explicit verification gate exists.",
            json!({"type":"object","properties":{"id":{"type":"string"},"summary":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_task_complete",
            "Completes a task; in_progress to needs_verification; takes summary, one or more claims, and optional proof entries.",
            json!({"type":"object","properties":{"id":{"type":"string"},"summary":{"type":"string"},"claim":{"type":"string"},"claims":{"type":"array","items":{"type":"string"},"minItems":1},"proof":{"type":"array","items":{"type":"string"}}},"required":["id","summary"],"anyOf":[{"required":["claim"]},{"required":["claims"]}]}),
        ),
        tool(
            "maestro_task_update",
            "Records task progress without changing lifecycle state; accepts optional summary and multiple claims.",
            json!({"type":"object","properties":{"id":{"type":"string"},"summary":{"type":"string"},"claim":{"type":"string"},"claims":{"type":"array","items":{"type":"string"}}},"required":["id"]}),
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
            "maestro_qa_baseline",
            "Records explicit observed QA baseline evidence for a feature through the normal QA gate.",
            json!({"type":"object","properties":{"feature_id":{"type":"string"},"observed":{"type":"string"}},"required":["feature_id","observed"]}),
        ),
        tool(
            "maestro_feature_accept",
            "Accepts a feature using an explicit QA mode and returns the lifecycle envelope.",
            json!({"type":"object","properties":{"feature_id":{"type":"string"},"qa":{"oneOf":[{"type":"object","properties":{"mode":{"const":"recorded_baseline"}},"required":["mode"],"additionalProperties":false},{"type":"object","properties":{"mode":{"const":"none"},"reason":{"type":"string"}},"required":["mode","reason"],"additionalProperties":false}]}},"required":["feature_id","qa"]}),
        ),
        tool(
            "maestro_feature_prepare",
            "Prepares an accepted feature into a task queue and returns the lifecycle envelope.",
            json!({"type":"object","properties":{"feature_id":{"type":"string"},"draft":{"type":"boolean"},"tasks":{"type":"array","items":{"type":"object","properties":{"title":{"type":"string"},"checks":{"type":"array","items":{"type":"string"},"minItems":1},"covers":{"type":"array","items":{"type":"string"}},"blockers":{"type":"array","items":{"type":"string"}},"after":{"type":"array","items":{"type":"string"}}},"required":["title"]}}},"required":["feature_id"]}),
        ),
        tool(
            "maestro_feature_verify",
            "Records or sweeps feature acceptance proof without auto-closing; returns close as valid_next when ready.",
            json!({"type":"object","properties":{"feature_id":{"type":"string"},"prove":{"type":"array","items":{"type":"string"}},"evidence":{"type":"array","items":{"type":"string"}},"waive":{"type":"array","items":{"type":"string"}},"reason":{"type":"array","items":{"type":"string"}},"outcome":{"type":"string"}},"required":["feature_id"]}),
        ),
        tool(
            "maestro_qa_slice",
            "Records explicit observed QA slice evidence for a feature through the normal QA gate.",
            json!({"type":"object","properties":{"feature_id":{"type":"string"},"scenarios":{"type":"array","items":{"type":"string"}},"observed":{"type":"string"}},"required":["feature_id","observed"]}),
        ),
        tool(
            "maestro_feature_start",
            "Starts a ready feature; ready to in_progress.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_feature_close",
            "Closes an in_progress feature as a separate lifecycle step and returns the lifecycle envelope.",
            json!({"type":"object","properties":{"feature_id":{"type":"string"},"id":{"type":"string"},"outcome":{"type":"string"},"dry_run":{"type":"boolean"}},"anyOf":[{"required":["feature_id"]},{"required":["id"]}]}),
        ),
        tool(
            "maestro_card_create",
            "Creates a card container or record using guided full-card-store intents: feature, custom, bug, decision, idea, or chore. Atomic work uses maestro_task_add/create.",
            json!({"type":"object","properties":{"intent":{"type":"string","enum":["feature","custom","bug","decision","idea","chore"]},"title":{"type":"string"},"kind":{"type":"string"},"parent":{"type":"string"},"description":{"type":"string"},"problem":{"type":"string"},"active_form":{"type":"string"},"acceptance":{"type":"string"},"project":{"type":"string"}},"required":["intent","title"]}),
        ),
        tool(
            "maestro_card_prepare",
            "Prepares a card container into owned tasks using the same plan shape as feature prepare.",
            json!({"type":"object","properties":{"card_id":{"type":"string"},"draft":{"type":"boolean"},"tasks":{"type":"array","items":{"type":"object","properties":{"title":{"type":"string"},"checks":{"type":"array","items":{"type":"string"},"minItems":1},"covers":{"type":"array","items":{"type":"string"}},"blockers":{"type":"array","items":{"type":"string"}},"after":{"type":"array","items":{"type":"string"}}},"required":["title"]}}},"required":["card_id"]}),
        ),
        tool(
            "maestro_card_list",
            "Lists cards with optional parent, type, assignee, status, project, grep, archived, all, and json filters.",
            json!({"type":"object","properties":{"parent":{"type":"string"},"type":{"type":"string"},"assignee":{"type":"string"},"status":{"type":"string"},"project":{"type":"string"},"grep":{"type":"string"},"archived":{"type":"boolean"},"all":{"type":"boolean"},"json":{"type":"boolean"}}}),
        ),
        tool(
            "maestro_card_show",
            "Shows one card, optionally as JSON.",
            json!({"type":"object","properties":{"id":{"type":"string"},"json":{"type":"boolean"},"compact_json":{"type":"boolean"}},"required":["id"]}),
        ),
        tool(
            "maestro_card_ready",
            "Lists workable cards with no open blockers, optionally scoped to a feature.",
            json!({"type":"object","properties":{"feature":{"type":"string"},"json":{"type":"boolean"},"project":{"type":"string"}}}),
        ),
        tool(
            "maestro_card_claim",
            "Claims a workable card for this session.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_card_update",
            "Updates card lifecycle fields with guided progress fields mapped to the card update CLI.",
            json!({"type":"object","properties":{"id":{"type":"string"},"status":{"type":"string"},"title":{"type":"string"},"description":{"type":"string"},"problem":{"type":"string"},"active_form":{"type":"string"},"progress":{"type":"string"},"claim":{"type":"boolean"},"json":{"type":"boolean"}},"required":["id"]}),
        ),
        tool(
            "maestro_card_close",
            "Closes a legacy task card or a Bug/Chore/Custom container whose owned tasks are verified.",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
        ),
        tool(
            "maestro_card_graph",
            "Renders a card's typed relationships, optionally as DOT.",
            json!({"type":"object","properties":{"id":{"type":"string"},"dot":{"type":"boolean"}},"required":["id"]}),
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
        "maestro_task_next" => task_next(),
        "maestro_task_create" => task_create(arguments),
        "maestro_task_add" => task_add(arguments),
        "maestro_task_explore" => cli(required_args(arguments, &["task", "explore"], &["id"])?),
        "maestro_task_accept" => cli(required_args(arguments, &["task", "accept"], &["id"])?),
        "maestro_task_list" => task_list(paths, arguments),
        "maestro_task_show" => cli(optional_id_args("task", "show", arguments, "id")),
        "maestro_task_claim" => cli(required_args(arguments, &["task", "claim"], &["id"])?),
        "maestro_task_start" => cli(required_args(arguments, &["task", "start"], &["id"])?),
        "maestro_task_done" => task_done(arguments),
        "maestro_task_complete" => task_complete(arguments),
        "maestro_task_update" => task_update(arguments),
        "maestro_task_block" => task_block(arguments),
        "maestro_task_unblock" => task_unblock(arguments),
        "maestro_feature_list" => feature_list(arguments),
        "maestro_feature_show" => cli(required_args(arguments, &["feature", "show"], &["id"])?),
        "maestro_qa_baseline" => qa_baseline(paths, arguments),
        "maestro_feature_accept" => feature_accept(paths, arguments),
        "maestro_feature_prepare" => feature_prepare(paths, arguments),
        "maestro_feature_verify" => feature_verify(paths, arguments),
        "maestro_qa_slice" => qa_slice(paths, arguments),
        "maestro_feature_start" => cli(required_args(arguments, &["feature", "start"], &["id"])?),
        "maestro_feature_close" => feature_close(paths, arguments),
        "maestro_card_create" => card_create(arguments),
        "maestro_card_prepare" => card_prepare(paths, arguments),
        "maestro_card_list" => card_list(arguments),
        "maestro_card_show" => card_show(arguments),
        "maestro_card_ready" => card_ready(arguments),
        "maestro_card_claim" => cli(required_args(arguments, &["card", "claim"], &["id"])?),
        "maestro_card_update" => card_update(arguments),
        "maestro_card_close" => cli(required_args(arguments, &["card", "close"], &["id"])?),
        "maestro_card_graph" => card_graph(arguments),
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
    out.push_str("MCP workflow guidance:\n");
    out.push_str("  Use maestro_task_next and maestro_task_* for the normal task progress loop.\n");
    out.push_str("  Use maestro_card_ready, maestro_card_graph, and maestro_card_* for backlog/card lifecycle work.\n");
    Ok(out)
}

fn task_next() -> Result<String> {
    let raw = cli(vec![
        "task".to_string(),
        "next".to_string(),
        "--json".to_string(),
    ])?;
    let structured = serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}));
    serde_json::to_string_pretty(&json!({
        "structured": structured,
        "raw": raw.trim_end()
    }))
    .context("failed to encode task_next MCP response")
}

fn task_create(arguments: &Value) -> Result<String> {
    let mut args = vec![
        "task".to_string(),
        "create".to_string(),
        required_string(arguments, "title")?,
    ];
    push_optional_flag(arguments, &mut args, "feature_id", "--feature");
    push_optional_flag(arguments, &mut args, "card_id", "--card");
    push_optional_flag(arguments, &mut args, "lane", "--lane");
    push_optional_flag(arguments, &mut args, "risk", "--risk");
    push_repeated_flag(arguments, &mut args, "checks", "--check")?;
    push_repeated_flag(arguments, &mut args, "covers", "--covers")?;
    push_optional_flag(arguments, &mut args, "project", "--project");
    if bool_arg(arguments, "id_only") {
        args.push("--id-only".to_string());
    }
    cli(args)
}

fn task_add(arguments: &Value) -> Result<String> {
    let mut args = vec![
        "task".to_string(),
        "add".to_string(),
        required_string(arguments, "title")?,
    ];
    push_optional_flag(arguments, &mut args, "card_id", "--card");
    push_optional_flag(arguments, &mut args, "project", "--project");
    if bool_arg(arguments, "id_only") {
        args.push("--id-only".to_string());
    }
    cli(args)
}

fn task_done(arguments: &Value) -> Result<String> {
    let mut args = vec![
        "task".to_string(),
        "done".to_string(),
        required_string(arguments, "id")?,
    ];
    push_optional_flag(arguments, &mut args, "summary", "--summary");
    cli(args)
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
    list_with_all("feature", arguments)
}

fn decision_list(arguments: &Value) -> Result<String> {
    list_with_all("decision", arguments)
}

fn list_with_all(noun: &str, arguments: &Value) -> Result<String> {
    let mut argv = vec![noun.to_string(), "list".to_string()];
    if bool_arg(arguments, "all") {
        argv.push("--all".to_string());
    }
    cli(argv)
}

fn qa_baseline(paths: &MaestroPaths, arguments: &Value) -> Result<String> {
    let feature_id = feature_id_arg(arguments)?;
    lifecycle_cli(
        paths,
        "maestro_qa_baseline",
        "feature",
        &feature_id,
        vec![
            "qa".to_string(),
            "baseline".to_string(),
            feature_id.clone(),
            "--observed".to_string(),
            required_non_empty_string(arguments, "observed")?,
        ],
    )
}

fn feature_accept(paths: &MaestroPaths, arguments: &Value) -> Result<String> {
    let feature_id = feature_id_arg(arguments)?;
    let mut args = vec![
        "feature".to_string(),
        "accept".to_string(),
        feature_id.clone(),
    ];
    let qa = arguments
        .get("qa")
        .with_context(|| "missing required argument: qa")?;
    let mode = qa
        .get("mode")
        .and_then(Value::as_str)
        .with_context(|| "missing required argument: qa.mode")?;
    match mode {
        "recorded_baseline" => {
            if qa.get("reason").is_some() {
                bail!("qa.reason is only valid when qa.mode is none");
            }
        }
        "none" => {
            args.push("--qa".to_string());
            args.push("none".to_string());
            args.push("--reason".to_string());
            args.push(
                qa.get("reason")
                    .and_then(Value::as_str)
                    .with_context(|| "missing required argument: qa.reason")
                    .and_then(|reason| non_empty_value("qa.reason", reason))?,
            );
        }
        other => bail!("unsupported qa.mode: {other}"),
    }
    lifecycle_cli(
        paths,
        "maestro_feature_accept",
        "feature",
        &feature_id,
        args,
    )
}

fn feature_prepare(paths: &MaestroPaths, arguments: &Value) -> Result<String> {
    let feature_id = feature_id_arg(arguments)?;
    let mut args = vec![
        "feature".to_string(),
        "prepare".to_string(),
        feature_id.clone(),
    ];
    if bool_arg(arguments, "draft") {
        args.push("--draft".to_string());
    } else if let Some(tasks) = arguments.get("tasks") {
        for (index, task) in tasks
            .as_array()
            .with_context(|| "tasks must be an array")?
            .iter()
            .enumerate()
        {
            args.push("--task".to_string());
            let title = task
                .get("title")
                .and_then(Value::as_str)
                .with_context(|| format!("tasks[{index}].title must be a string"))?;
            if title.contains(':') {
                args.push(title.to_string());
            } else {
                args.push(format!("T{}: {title}", index + 1));
            }
            push_repeated_flag(task, &mut args, "checks", "--check")?;
            push_repeated_flag(task, &mut args, "covers", "--covers")?;
            push_repeated_flag(task, &mut args, "blockers", "--blocker")?;
            push_repeated_flag(task, &mut args, "after", "--after")?;
        }
    } else {
        args.push("--draft".to_string());
    }
    lifecycle_cli(
        paths,
        "maestro_feature_prepare",
        "feature",
        &feature_id,
        args,
    )
}

fn card_prepare(paths: &MaestroPaths, arguments: &Value) -> Result<String> {
    let card_id = required_string(arguments, "card_id")?;
    let mut args = vec!["card".to_string(), "prepare".to_string(), card_id.clone()];
    if bool_arg(arguments, "draft") {
        args.push("--draft".to_string());
    } else if let Some(tasks) = arguments.get("tasks") {
        for (index, task) in tasks
            .as_array()
            .with_context(|| "tasks must be an array")?
            .iter()
            .enumerate()
        {
            args.push("--task".to_string());
            let title = task
                .get("title")
                .and_then(Value::as_str)
                .with_context(|| format!("tasks[{index}].title must be a string"))?;
            if title.contains(':') {
                args.push(title.to_string());
            } else {
                args.push(format!("T{}: {title}", index + 1));
            }
            push_repeated_flag(task, &mut args, "checks", "--check")?;
            push_repeated_flag(task, &mut args, "covers", "--covers")?;
            push_repeated_flag(task, &mut args, "blockers", "--blocker")?;
            push_repeated_flag(task, &mut args, "after", "--after")?;
        }
    } else {
        args.push("--draft".to_string());
    }
    lifecycle_cli(paths, "maestro_card_prepare", "card", &card_id, args)
}

fn feature_verify(paths: &MaestroPaths, arguments: &Value) -> Result<String> {
    let feature_id = feature_id_arg(arguments)?;
    let mut args = vec![
        "feature".to_string(),
        "verify".to_string(),
        feature_id.clone(),
        "--no-close".to_string(),
    ];
    push_repeated_flag(arguments, &mut args, "prove", "--prove")?;
    push_repeated_flag(arguments, &mut args, "evidence", "--evidence")?;
    push_repeated_flag(arguments, &mut args, "waive", "--waive")?;
    push_repeated_flag(arguments, &mut args, "reason", "--reason")?;
    push_optional_flag(arguments, &mut args, "outcome", "--outcome");
    lifecycle_cli(
        paths,
        "maestro_feature_verify",
        "feature",
        &feature_id,
        args,
    )
}

fn qa_slice(paths: &MaestroPaths, arguments: &Value) -> Result<String> {
    let feature_id = feature_id_arg(arguments)?;
    let mut args = vec!["qa".to_string(), "slice".to_string(), feature_id.clone()];
    push_repeated_flag(arguments, &mut args, "scenarios", "--scenario")?;
    args.push("--observed".to_string());
    args.push(required_non_empty_string(arguments, "observed")?);
    lifecycle_cli(paths, "maestro_qa_slice", "feature", &feature_id, args)
}

fn feature_close(paths: &MaestroPaths, arguments: &Value) -> Result<String> {
    let feature_id = feature_id_arg(arguments)?;
    let mut args = vec![
        "feature".to_string(),
        "close".to_string(),
        feature_id.clone(),
    ];
    push_optional_flag(arguments, &mut args, "outcome", "--outcome");
    push_bool_flag(arguments, &mut args, "dry_run", "--dry-run");
    lifecycle_cli(paths, "maestro_feature_close", "feature", &feature_id, args)
}

fn task_complete(arguments: &Value) -> Result<String> {
    let mut args = vec![
        "task".to_string(),
        "complete".to_string(),
        required_string(arguments, "id")?,
        "--summary".to_string(),
        required_string(arguments, "summary")?,
    ];
    let claims = claims_arg(arguments)?;
    for claim in claims {
        args.push("--claim".to_string());
        args.push(claim);
    }
    push_repeated_flag(arguments, &mut args, "proof", "--proof")?;
    cli(args)
}

fn task_update(arguments: &Value) -> Result<String> {
    let mut args = vec![
        "task".to_string(),
        "update".to_string(),
        required_string(arguments, "id")?,
    ];
    push_optional_flag(arguments, &mut args, "summary", "--summary");
    for claim in optional_claims_arg(arguments)? {
        args.push("--claim".to_string());
        args.push(claim);
    }
    cli(args)
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

fn card_create(arguments: &Value) -> Result<String> {
    let mut args = vec![
        "card".to_string(),
        "create".to_string(),
        required_string(arguments, "title")?,
        "--type".to_string(),
        required_string(arguments, "intent")?,
    ];
    push_optional_flag(arguments, &mut args, "parent", "--parent");
    push_optional_flag(arguments, &mut args, "kind", "--kind");
    push_guided_text_flag(
        arguments,
        &mut args,
        &["description", "problem"],
        "--description",
    );
    push_guided_text_flag(
        arguments,
        &mut args,
        &["active_form", "acceptance"],
        "--active-form",
    );
    push_optional_flag(arguments, &mut args, "project", "--project");
    cli(args)
}

fn card_list(arguments: &Value) -> Result<String> {
    let mut args = vec!["card".to_string(), "list".to_string()];
    push_optional_flag(arguments, &mut args, "parent", "--parent");
    push_optional_flag(arguments, &mut args, "type", "--type");
    push_optional_flag(arguments, &mut args, "assignee", "--assignee");
    push_optional_flag(arguments, &mut args, "status", "--status");
    push_optional_flag(arguments, &mut args, "project", "--project");
    push_optional_flag(arguments, &mut args, "grep", "--grep");
    push_bool_flag(arguments, &mut args, "archived", "--archived");
    push_bool_flag(arguments, &mut args, "all", "--all");
    push_bool_flag(arguments, &mut args, "json", "--json");
    cli(args)
}

fn card_show(arguments: &Value) -> Result<String> {
    let mut args = vec![
        "card".to_string(),
        "show".to_string(),
        required_string(arguments, "id")?,
    ];
    push_bool_flag(arguments, &mut args, "json", "--json");
    push_bool_flag(arguments, &mut args, "compact_json", "--compact-json");
    cli(args)
}

fn card_ready(arguments: &Value) -> Result<String> {
    let mut args = vec!["card".to_string(), "ready".to_string()];
    if let Some(feature) = string_arg(arguments, "feature") {
        args.push(feature);
    }
    push_bool_flag(arguments, &mut args, "json", "--json");
    push_optional_flag(arguments, &mut args, "project", "--project");
    cli(args)
}

fn card_update(arguments: &Value) -> Result<String> {
    let mut args = vec!["card".to_string(), "update".to_string()];
    args.push(required_string(arguments, "id")?);
    push_optional_flag(arguments, &mut args, "status", "--status");
    push_optional_flag(arguments, &mut args, "title", "--title");
    push_guided_text_flag(
        arguments,
        &mut args,
        &["description", "problem"],
        "--description",
    );
    push_guided_text_flag(
        arguments,
        &mut args,
        &["active_form", "progress"],
        "--active-form",
    );
    push_bool_flag(arguments, &mut args, "claim", "--claim");
    push_bool_flag(arguments, &mut args, "json", "--json");
    cli(args)
}

fn card_graph(arguments: &Value) -> Result<String> {
    let mut args = vec![
        "card".to_string(),
        "graph".to_string(),
        required_string(arguments, "id")?,
    ];
    push_bool_flag(arguments, &mut args, "dot", "--dot");
    cli(args)
}

fn cli(args: Vec<String>) -> Result<String> {
    let output = run_cli(&args)?;
    if output.success {
        return Ok(output.stdout);
    }
    bail!(
        "maestro {} failed: {}",
        args.join(" "),
        output.stderr.trim()
    );
}

struct CliRun {
    success: bool,
    stdout: String,
    stderr: String,
}

fn run_cli(args: &[String]) -> Result<CliRun> {
    let output = Command::new(std::env::current_exe().context("failed to find current binary")?)
        .args(args)
        .output()
        .with_context(|| format!("failed to run maestro {}", args.join(" ")))?;
    Ok(CliRun {
        success: output.status.success(),
        stdout: String::from_utf8(output.stdout).context("maestro stdout was not UTF-8")?,
        stderr: String::from_utf8(output.stderr).context("maestro stderr was not UTF-8")?,
    })
}

fn lifecycle_cli(
    paths: &MaestroPaths,
    tool_name: &str,
    target_type: &str,
    target_id: &str,
    args: Vec<String>,
) -> Result<String> {
    let state_before = lifecycle_status_label(paths, target_type, target_id);
    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let output = run_cli(&args)?;
    let state_after = lifecycle_status_label(paths, target_type, target_id);
    let (ok, changed, blocked, reason_code, message, raw) = if output.success {
        (
            true,
            !dry_run,
            false,
            Value::Null,
            Value::String("ok".to_string()),
            output.stdout.trim_end().to_string(),
        )
    } else {
        let raw = output.stderr.trim().to_string();
        (
            false,
            false,
            true,
            Value::String(lifecycle_reason_code(&raw).to_string()),
            Value::String(lifecycle_message(&raw)),
            raw,
        )
    };
    serde_json::to_string_pretty(&json!({
        "ok": ok,
        "changed": changed,
        "tool": tool_name,
        "target": {"type": target_type, "id": target_id},
        "state_before": state_before,
        "state_after": state_after,
        "blocked": blocked,
        "reason_code": reason_code,
        "message": message,
        "prerequisites": lifecycle_prerequisites(tool_name, target_type, &state_before, blocked),
        "valid_next": lifecycle_valid_next(tool_name, target_type, target_id, &state_before, blocked),
        "raw": raw,
    }))
    .context("failed to encode lifecycle MCP response")
}

fn lifecycle_status_label(paths: &MaestroPaths, target_type: &str, target_id: &str) -> String {
    match target_type {
        "feature" => feature::status(paths, target_id)
            .map(|status| status.as_str().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        "card" => card::store::resolve(paths, target_id)
            .ok()
            .flatten()
            .map(|resolved| resolved.card.status)
            .unwrap_or_else(|| "unknown".to_string()),
        _ => "unknown".to_string(),
    }
}

fn lifecycle_reason_code(raw: &str) -> &'static str {
    let lowered = raw.to_ascii_lowercase();
    if lowered.contains("not found") {
        "not_found"
    } else if lowered.contains("qa") || lowered.contains("baseline") {
        "qa_gate_blocked"
    } else if lowered.contains("state") || lowered.contains("ready") || lowered.contains("proposed")
    {
        "invalid_feature_state"
    } else {
        "lifecycle_blocked"
    }
}

fn lifecycle_message(raw: &str) -> String {
    raw.lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("lifecycle command was blocked")
        .to_string()
}

fn lifecycle_prerequisites(
    tool_name: &str,
    target_type: &str,
    state_before: &str,
    blocked: bool,
) -> Vec<String> {
    if !blocked {
        return Vec::new();
    }
    match (tool_name, target_type, state_before) {
        ("maestro_feature_prepare", "feature", "proposed") => {
            vec!["feature must be accepted before prepare".to_string()]
        }
        ("maestro_feature_close", "feature", "in_progress") => {
            vec!["feature close gate must be satisfied".to_string()]
        }
        _ => Vec::new(),
    }
}

fn lifecycle_valid_next(
    tool_name: &str,
    target_type: &str,
    target_id: &str,
    state_before: &str,
    blocked: bool,
) -> Value {
    if target_type != "feature" {
        return json!([]);
    }
    if !blocked {
        return match tool_name {
            "maestro_qa_baseline" => json!([
                {"tool":"maestro_feature_accept","arguments":{"feature_id":target_id,"qa":{"mode":"recorded_baseline"}}}
            ]),
            "maestro_feature_accept" => json!([
                {"tool":"maestro_feature_prepare","arguments":{"feature_id":target_id,"draft":true}}
            ]),
            "maestro_feature_verify" | "maestro_qa_slice" => json!([
                {"tool":"maestro_feature_close","arguments":{"feature_id":target_id,"outcome":"<outcome>"}}
            ]),
            _ => json!([]),
        };
    }
    match (tool_name, state_before) {
        ("maestro_feature_prepare", "proposed") => json!([
            {"tool":"maestro_qa_baseline","arguments":{"feature_id":target_id,"observed":"<observed baseline>"}}
        ]),
        ("maestro_feature_accept", "proposed") => json!([
            {"tool":"maestro_qa_baseline","arguments":{"feature_id":target_id,"observed":"<observed baseline>"}}
        ]),
        _ => json!([]),
    }
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

fn required_non_empty_string(arguments: &Value, field: &str) -> Result<String> {
    let value = required_string(arguments, field)?;
    non_empty_value(field, &value)
}

fn non_empty_value(field: &str, value: &str) -> Result<String> {
    if value.trim().is_empty() {
        bail!("{field} must not be empty");
    }
    Ok(value.to_string())
}

fn feature_id_arg(arguments: &Value) -> Result<String> {
    string_arg(arguments, "feature_id")
        .or_else(|| string_arg(arguments, "id"))
        .with_context(|| "missing required argument: feature_id")
}

fn string_arg(arguments: &Value, field: &str) -> Option<String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn claims_arg(arguments: &Value) -> Result<Vec<String>> {
    let claims = optional_claims_arg(arguments)?;
    if claims.is_empty() {
        bail!("missing required argument: claim");
    }
    Ok(claims)
}

fn optional_claims_arg(arguments: &Value) -> Result<Vec<String>> {
    let mut claims = Vec::new();
    if let Some(claim) = string_arg(arguments, "claim") {
        claims.push(claim);
    }
    if arguments.get("claims").is_some() {
        claims.extend(array_strings_arg(arguments, "claims")?);
    }
    Ok(claims)
}

fn bool_arg(arguments: &Value, field: &str) -> bool {
    arguments
        .get(field)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn push_optional_flag(arguments: &Value, args: &mut Vec<String>, field: &str, flag: &str) {
    if let Some(value) = string_arg(arguments, field) {
        args.push(flag.to_string());
        args.push(value);
    }
}

fn push_guided_text_flag(arguments: &Value, args: &mut Vec<String>, fields: &[&str], flag: &str) {
    for field in fields {
        if let Some(value) = string_arg(arguments, field) {
            args.push(flag.to_string());
            args.push(value);
            break;
        }
    }
}

fn push_bool_flag(arguments: &Value, args: &mut Vec<String>, field: &str, flag: &str) {
    if bool_arg(arguments, field) {
        args.push(flag.to_string());
    }
}

fn push_repeated_flag(
    arguments: &Value,
    args: &mut Vec<String>,
    field: &str,
    flag: &str,
) -> Result<()> {
    let Some(value) = arguments.get(field) else {
        return Ok(());
    };
    let values = value
        .as_array()
        .with_context(|| format!("{field} must be an array of strings"))?;
    for (index, value) in values.iter().enumerate() {
        let value = value
            .as_str()
            .with_context(|| format!("{field}[{index}] must be a string"))?;
        args.push(flag.to_string());
        args.push(value.to_string());
    }
    Ok(())
}

fn array_strings_arg(arguments: &Value, field: &str) -> Result<Vec<String>> {
    let Some(value) = arguments.get(field) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .with_context(|| format!("{field} must be an array of strings"))?;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .map(str::to_string)
                .with_context(|| format!("{field}[{index}] must be a string"))
        })
        .collect()
}
