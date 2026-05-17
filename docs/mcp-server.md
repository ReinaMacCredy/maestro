# MCP Server

Maestro ships a Model Context Protocol (MCP) server that exposes its core verbs to MCP-aware agent runtimes (Claude Code, Codex, and any other client that speaks the MCP stdio transport). This lets agents call `maestro_task_create`, `maestro_evidence_record`, `maestro_verdict_request`, and so on as structured tools instead of shelling out to the CLI and parsing text.

The server is the same maestro binary, run with `maestro mcp serve`. Agents launch it; you do not start it manually.

## Tools

20 tools across 8 surfaces. Each is a 1:1 wrapper around a maestro use case; semantics match the CLI verbs documented in the bundled skills.

### Task

| Tool | Behavior |
|------|----------|
| `maestro_task_list` | Paginated list. Filters: `mission_id`, `state`, plus `limit`/`offset`. |
| `maestro_task_get` | Fetch one task by id. Returns `code: TASK_NOT_FOUND` if missing. |
| `maestro_task_from_spec` | Create a `draft` task from a product-spec markdown path. |
| `maestro_task_claim` | Flip `draft → claimed`. Session id is auto-detected from `MAESTRO_SESSION_ID`, `CLAUDECODE_SESSION_ID`, `CODEX_THREAD_ID`, falling back to `<user>@<host>`. |
| `maestro_task_block` | State-transition verb: moves the task to `blocked` with a required reason. |
| `maestro_task_ship` | Manual `ready → shipped`; optional `pr_url`. |

`task verify` is intentionally CLI-only — the interactive grill protocol and exit-code routing cannot be faithfully proxied over MCP.

### Evidence

| Tool | Behavior |
|------|----------|
| `maestro_evidence_record` | Record either a command result (`command` + `exitCode`) or a manual note (`note`). Exactly one of those two forms — passing both, neither, or `command` without `exitCode` is rejected at the schema layer. Optional `witnessLevel`. |
| `maestro_evidence_list` | Paginated list, optional `kind` and `witnessLevel` filters. |

### Contract

| Tool | Behavior |
|------|----------|
| `maestro_contract_show` | Current contract by default; pass `version` for a historical version. |
| `maestro_contract_amend` | Add or remove paths from `filesExpected`. Records a versioned amendment plus a `contract-amendment` evidence row. |

### Verdict

| Tool | Behavior |
|------|----------|
| `maestro_verdict_show` | Latest verdict for a task; optional `id` for a specific verdict. |
| `maestro_verdict_request` | Run the verifier and return a fresh verdict. |

### Policy

| Tool | Behavior |
|------|----------|
| `maestro_policy_check` | Compute effective risk class, autopilot rules, and sensitive-path matches against the task's current diff. |

### Handoff

| Tool | Behavior |
|------|----------|
| `maestro_handoff_list` | List handoff envelopes under `.maestro/handoffs/`. Filters: `task_id`, `trigger_verb`, `picked_up` (bool), plus `limit`/`offset`. |
| `maestro_handoff_show` | Fetch one envelope by id; includes the pickup sidecar if present. `HANDOFF_NOT_FOUND` if missing, `HANDOFF_MALFORMED` if the JSON is corrupt. |
| `maestro_handoff_emit` | Write a new envelope for a task. Bookkeeping only — does not change task state. |
| `maestro_handoff_pickup` | Exclusive-create a pickup sidecar at `.maestro/handoffs/<id>.picked_up.json` so concurrent agents do not duplicate. Does not claim the task. Codes: `HANDOFF_NOT_FOUND`, `HANDOFF_ALREADY_PICKED_UP`, `HANDOFF_PICKUP_FAILED`, `HANDOFF_MALFORMED`. |

### Principle

| Tool | Behavior |
|------|----------|
| `maestro_principle_promote` | Materialize `docs/principles/<slug>.md` from a `lint-violation` evidence row. |

### Setup

| Tool | Behavior |
|------|----------|
| `maestro_setup_check` | Read-only audit of the v2 directory tree, principles pack, and `.maestro/config.yaml`. |

The merged `setup` verb (idempotent state machine, hard-deletes v1, migrates `.maestro/plans/` → `.maestro/missions/`) is CLI-only — destructive actions stay off the MCP surface.

Grill-driven verbs (`spec new`, `mission from-spec`, `mission decompose`) are CLI-only — the interactive grill protocol cannot be sustained over MCP.

## Result shape

Successful calls return a `tools/call` result whose first content block is JSON text and whose `structuredContent` contains the same payload as a JS object.

```json
{
  "content": [{ "type": "text", "text": "{ \"task\": { ... } }" }],
  "structuredContent": { "task": { ... } }
}
```

Failures set `isError: true` and the payload is `{ code, message, hints }`. Codes are stable and safe to branch on. The set surfaced today:

| Code | Source |
|------|--------|
| `TASK_NOT_FOUND` | task lookup miss; thrown explicitly by the task-error factory |
| `TASK_CLAIM_FAILED` / `TASK_BLOCK_FAILED` / `TASK_SHIP_FAILED` / `TASK_CREATE_FAILED` | wrapping fallback when the underlying use case throws |
| `CONTRACT_NOT_FOUND` | contract show/amend on a task with no contract |
| `CONTRACT_VERSION_NOT_FOUND` | `version` argument doesn't match any stored version |
| `VERDICT_NOT_FOUND` | verdict show on a task with no computed verdict |
| `ALREADY_COMPLETED` | mutating a task that has already been shipped |
| `NO_SCOPE_CHANGES` | contract amend whose add/remove paths produce no diff |
| `VALIDATION_ERROR` | other validation failures (e.g. empty addPaths and removePaths) |
| `HANDOFF_NOT_FOUND` / `HANDOFF_ALREADY_PICKED_UP` / `HANDOFF_PICKUP_FAILED` / `HANDOFF_MALFORMED` | handoff lookup, pickup, or sidecar parse failures |
| `SETUP_CHECK_FAILED` / `SETUP_MIGRATE_FAILED` | wrapping fallback for setup verbs |

The MCP layer prefers explicit codes attached at the throw site (via `MaestroError.code`) and falls back to message-pattern matching only for legacy throw sites. Adding a new error code in the domain is opt-in and doesn't risk silently shadowing an existing one.

## Pagination

`maestro_task_list` and `maestro_evidence_list` accept `limit` (default 20, max 100) and `offset` (default 0). Responses include a `pagination: { total, limit, offset, hasMore }` block.

## Strict input validation

Every tool's input schema is `strict`: unknown fields cause the call to fail rather than being silently dropped. A typo such as `planID` (correct: `plan_id`) on a tool that does not declare that field will return a tool error instead of succeeding with the typo'd field ignored. Match the field names documented above exactly.

## Output schemas

Every tool also declares an `outputSchema` that mirrors the success-path `structuredContent`. Clients can use this to type-check returned data without reading docs. Inner objects (Task, Evidence, Verdict, Contract) are open (`passthrough`) so that future field additions don't break older clients. Top-level wrappers (`{ task }`, `{ items, pagination }`, etc.) are closed.

Output validation is skipped when `isError: true`, so error payloads (`{ code, message, hints }`) never have to satisfy the success schema.

## Project root resolution

The server walks up from its working directory looking for a `.maestro/` directory. To override, set `MAESTRO_PROJECT_ROOT` before launch. The server fails fast with `Not in a maestro project` if no `.maestro/` ancestor exists.

## Running standalone

```bash
maestro mcp serve                                  # stdio transport, default
maestro mcp serve --project-root /abs/path         # override project root detection
maestro mcp check                                  # introspect installed binary + agent runtime configs
maestro mcp check --json
```

`mcp serve` reads JSON-RPC over stdin and writes responses to stdout; diagnostic output and protocol errors go to stderr so the stdout channel stays reserved for protocol traffic. The `--transport` flag accepts `stdio` (the only supported value today). HTTP and SSE transports are not implemented.

`--project-root` and `MAESTRO_PROJECT_ROOT` serve the same purpose; the flag wins when both are set.

`mcp check` exits `1` when the installed binary is missing, `0` otherwise. It reports each runtime as `[ok]` (configured and current), `[stale]` (configured but pointing at a different binary path), or `not configured`.

## See also

- `docs/mcp-setup.md` — wiring the server into Claude Code and Codex.
- `docs/mcp-evaluation.xml` — 10 question/answer pairs that exercise every tool surface against the checked-in fixture at `tests/e2e/mcp-eval-fixture/`.
- `skills/bundled/maestro-task/SKILL.md` — task-side MCP tool table and CLI fallback note.
- `skills/bundled/maestro-verify/SKILL.md` — verification-side MCP tool table.
