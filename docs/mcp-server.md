# MCP Server

Maestro ships a Model Context Protocol (MCP) server that exposes its core verbs to MCP-aware agent runtimes (Claude Code, Codex, and any other client that speaks the MCP stdio transport). This lets agents call `maestro_task_create`, `maestro_evidence_record`, `maestro_verdict_request`, and so on as structured tools instead of shelling out to the CLI and parsing text.

The server is the same maestro binary, run with `maestro mcp serve`. Agents launch it; you do not start it manually.

## Tools

14 tools across 5 surfaces. Each is a 1:1 wrapper around an existing maestro use case; semantics match the CLI verbs documented in the bundled skills.

### Task

| Tool | Behavior |
|------|----------|
| `maestro_task_list` | Paginated list. Filters: `missionId`, `status`, `type`, `priority`, `label`, `parentId`, `assignee`, plus `limit`/`offset`. |
| `maestro_task_get` | Fetch one task by id. Returns `code: TASK_NOT_FOUND` if missing. |
| `maestro_task_create` | Create a top-level task. Slug derived from title. |
| `maestro_task_claim` | Claim a task for the current MCP session. Session id is auto-detected from `MAESTRO_SESSION_ID`, `CLAUDECODE_SESSION_ID`, `CODEX_THREAD_ID`, falling back to `<user>@<host>`. |
| `maestro_task_complete` | Mark completed. Optional `summary` stored on the receipt. |
| `maestro_task_block` | Add bidirectional blocker edges. Detects cycles. |
| `maestro_task_unblock` | Remove blocker edges. |

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

## Result shape

Successful calls return a `tools/call` result whose first content block is JSON text and whose `structuredContent` contains the same payload as a JS object.

```json
{
  "content": [{ "type": "text", "text": "{ \"task\": { ... } }" }],
  "structuredContent": { "task": { ... } }
}
```

Failures set `isError: true` and the payload is `{ code, message, hints }`. Codes are stable and safe to branch on. The full set surfaced today:

| Code | Source |
|------|--------|
| `TASK_NOT_FOUND` | task lookup miss; thrown explicitly by the task-error factory |
| `CONTRACT_NOT_FOUND` | contract show/amend on a task with no contract |
| `CONTRACT_VERSION_NOT_FOUND` | `version` argument doesn't match any stored version |
| `VERDICT_NOT_FOUND` | verdict show on a task with no computed verdict |
| `ALREADY_COMPLETED` | mutating a task that has already been completed |
| `OWNERSHIP_CONFLICT` | claim/mutate of a task owned by a different session |
| `CYCLE_DETECTED` | task block would create a cycle in the blocker graph |
| `SELF_BLOCK` | task tries to block itself |
| `NO_SCOPE_CHANGES` | contract amend whose add/remove paths produce no diff |
| `VALIDATION_ERROR` | other validation failures (e.g. empty addPaths and removePaths) |

The MCP layer prefers explicit codes attached at the throw site (via `MaestroError.code`) and falls back to message-pattern matching only for legacy throw sites. Adding a new error code in the domain is opt-in and doesn't risk silently shadowing an existing one.

## Pagination

`maestro_task_list` and `maestro_evidence_list` accept `limit` (default 20, max 100) and `offset` (default 0). Responses include a `pagination: { total, limit, offset, hasMore }` block.

## Strict input validation

Every tool's input schema is `strict`: unknown fields cause the call to fail rather than being silently dropped. A typo such as `missionID` (correct: `missionId`) on a tool that does not declare that field will return a tool error instead of succeeding with the typo'd field ignored. Match the field names documented above exactly.

## Output schemas

Every tool also declares an `outputSchema` that mirrors the success-path `structuredContent`. Clients can use this to type-check returned data without reading docs. Inner objects (Task, Evidence, Verdict, Contract) are open (`passthrough`) so that future field additions don't break older clients. Top-level wrappers (`{ task }`, `{ items, pagination }`, etc.) are closed.

Output validation is skipped when `isError: true`, so error payloads (`{ code, message, hints }`) never have to satisfy the success schema.

## Project root resolution

The server walks up from its working directory looking for a `.maestro/` directory. To override, set `MAESTRO_PROJECT_ROOT` before launch. The server fails fast with `Not in a maestro project` if no `.maestro/` ancestor exists.

## Running standalone

```bash
maestro mcp serve            # stdio transport, default
maestro mcp check            # introspect installed binary + agent runtime configs
maestro mcp check --json
```

`mcp serve` reads JSON-RPC over stdin and writes responses to stdout; logs go to stderr. There is no HTTP/SSE transport at this time.

## See also

- `docs/mcp-setup.md` — wiring the server into Claude Code and Codex.
- `docs/mcp-evaluation.xml` — 10 question/answer pairs that exercise every tool surface against the checked-in fixture at `tests/e2e/mcp-eval-fixture/`.
- `skills/bundled/maestro-task/SKILL.md` — task-side MCP tool table and CLI fallback note.
- `skills/bundled/maestro-verify/SKILL.md` — verification-side MCP tool table.
