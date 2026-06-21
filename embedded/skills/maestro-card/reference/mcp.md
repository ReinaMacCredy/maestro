# Maestro MCP Reference

Use these tools only when the host exposes native Maestro MCP tools. Do not
invent tool names or fields. If a needed verb is missing here, use the CLI
reference instead.

MCP is the agent ergonomic contract. The CLI remains the compatibility and
human-facing contract, and many MCP tools intentionally call the same lifecycle
logic as the CLI.

## Orientation

- `maestro_status({})`
- `maestro_task_next({})`
- `maestro_query_matrix({})`
- `maestro_sync({"dry_run": true|false})`

## Task Tools

- `maestro_task_create({"title": string, "feature_id"?: string, "lane"?: string, "risk"?: string, "checks"?: string[], "covers"?: string[], "project"?: string, "id_only"?: boolean})`
- `maestro_task_explore({"id": string})`
- `maestro_task_accept({"id": string})`
- `maestro_task_list({"ready"?: boolean, "blocked"?: boolean, "blocked_by"?: string, "blocks"?: string, "feature_id"?: string, "claimed_by"?: string, "all"?: boolean})`
- `maestro_task_show({"id"?: string})`
- `maestro_task_claim({"id": string})`
- `maestro_task_update({"id": string, "summary"?: string, "claim"?: string, "claims"?: string[]})`
- `maestro_task_complete({"id": string, "summary": string, "claim"?: string, "claims"?: string[], "proof"?: string[]})`
- `maestro_task_block({"id": string, "reason": string, "blocked_ref"?: string})`
- `maestro_task_unblock({"id": string, "blocker": string})`
- `maestro_verify({"id": string})`

## Feature And QA Tools

- `maestro_feature_list({"all"?: boolean})`
- `maestro_feature_show({"id": string})`
- `maestro_qa_baseline({"feature_id": string, "observed": string})`
- `maestro_feature_accept({"feature_id": string, "qa": {"mode": "recorded_baseline"}})`
- `maestro_feature_accept({"feature_id": string, "qa": {"mode": "none", "reason": string}})`
- `maestro_feature_prepare({"feature_id": string, "draft"?: boolean, "tasks"?: [{"title": string, "checks"?: string[], "covers"?: string[], "blockers"?: string[], "after"?: string[]}]})`
- `maestro_feature_verify({"feature_id": string, "prove"?: string[], "evidence"?: string[], "waive"?: string[], "reason"?: string[], "outcome"?: string})`
- `maestro_qa_slice({"feature_id": string, "scenarios"?: string[], "observed": string})`
- `maestro_feature_start({"id": string})`
- `maestro_feature_close({"feature_id": string, "outcome"?: string, "dry_run"?: boolean})`
- `maestro_feature_close({"id": string, "outcome"?: string, "dry_run"?: boolean})`

## Card Tools

- `maestro_card_create({"intent": "feature"|"task"|"bug"|"decision"|"followup"|"chore", "title": string, "parent"?: string, "description"?: string, "problem"?: string, "active_form"?: string, "acceptance"?: string, "project"?: string})`
- `maestro_card_list({"parent"?: string, "type"?: string, "assignee"?: string, "status"?: string, "project"?: string, "grep"?: string, "archived"?: boolean, "all"?: boolean, "json"?: boolean})`
- `maestro_card_show({"id": string, "json"?: boolean, "compact_json"?: boolean})`
- `maestro_card_ready({"feature"?: string, "json"?: boolean, "project"?: string})`
- `maestro_card_claim({"id": string})`
- `maestro_card_update({"id": string, "status"?: string, "title"?: string, "description"?: string, "problem"?: string, "active_form"?: string, "progress"?: string, "claim"?: boolean, "json"?: boolean})`
- `maestro_card_close({"id": string})`
- `maestro_card_graph({"id": string, "dot"?: boolean})`

## Decision Tools

- `maestro_decision_list({"all"?: boolean})`
- `maestro_decision_new({"title": string})`

Design-time decision authoring still needs CLI verbs for fields not exposed by
MCP, such as `decision lock` and rich `feature spec` authoring.
