# Runtime Adapter: Claude Code Agent Teams

**Tier**: Full orchestration (Tier 1)
**Prerequisite**: `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` must be set in `~/.claude/settings.json` under `env`.

This adapter maps every abstract capability from `reference/core/capabilities.md` to concrete Claude Code tools. Use this adapter when Agent Teams are available.

---

## Capability Mapping

| Capability | Claude Code Tool | Notes |
|---|---|---|
| `agent.spawn(role, prompt, model?)` | `Task` tool with `subagent_type` | Set `subagent_type` to the agent role name (e.g., `kraken`, `spark`). Pass `model` via prompt instructions or omit to use default. |
| `agent.message(recipient, content)` | `SendMessage` | Set `type: "message"`, `recipient` to agent name, `content` to message body. |
| `agent.wait(agent_id?)` | Implicit — agent reports via `SendMessage` | Workers send completion messages; orchestrator processes them as they arrive. No explicit wait primitive needed. |
| `agent.close(agent_id)` | `SendMessage` with `type: "shutdown_request"` | Send a shutdown request; worker approves via `shutdown_response`. |
| `team.create(name, description)` | `TeamCreate` | Required before spawning workers. Provide `team_name` and `description`. |
| `team.delete()` | `TeamDelete` | Call after all workers have shut down and the plan is archived. |
| `task.create(subject, description, activeForm?)` | `TaskCreate` | Maps directly: `subject`, `description`, `activeForm` parameters match. |
| `task.list()` | `TaskList` | Returns all tasks with `id`, `subject`, `status`, `owner`, `blockedBy`. |
| `task.get(id)` | `TaskGet` | Returns full task record including `description`, `blocks`, `blockedBy`. |
| `task.update(id, fields)` | `TaskUpdate` | Pass `taskId` plus any subset of `{ status, owner, subject, description, activeForm, addBlocks, addBlockedBy }`. |
| `prompt.structured(question, options, multiSelect?)` | `AskUserQuestion` with options array | Pass options as the `options` parameter. `multiSelect` maps to checkbox mode. |
| `prompt.chat(message)` | `AskUserQuestion` (no options) | Plain text ask; returns user's response string. |
| `fs.read(path)` | `Read` tool | Reads file at absolute or project-relative path. |
| `fs.write(path, content)` | `Write` tool | Creates or overwrites file. |
| `fs.search(pattern)` | `Glob` tool | Accepts standard glob patterns. |
| `fs.grep(pattern, path?)` | `Grep` tool | Searches file contents; `path` scopes the search. |
| `exec.command(cmd)` | `Bash` tool | Runs shell command; returns stdout, stderr, exit code. |

---

## Setup Prerequisites

1. Set the feature flag in `~/.claude/settings.json`:
   ```json
   {
     "env": {
       "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
     }
   }
   ```
2. Verify Agent Teams are active with `/setup-check`.
3. The orchestrator must call `team.create` before spawning any workers. Workers that receive tasks before a team context exists will fail silently.

---

## Claude-Specific Features

### Subagent Types

The `agent.spawn` implementation uses `subagent_type` to select a named agent definition from `.claude/agents/`. This controls the agent's identity, system prompt, and model tier:

| Role label | Subagent type | Default model |
|---|---|---|
| `kraken` | `kraken` | Sonnet |
| `spark` | `spark` | Sonnet |
| `build-fixer` | `build-fixer` | Sonnet |
| `critic` | `critic` | Sonnet |
| `oracle` | `oracle` | Sonnet |
| `explore` | `explore` | Haiku |

### Model Selection

To request a specific model tier, include a `model:` hint in the spawn prompt. Workers respect this hint if their agent definition allows model override.

### Worktree Isolation

For plans that require filesystem isolation between workers, pass `isolation: worktree` in the spawn call. Each worker receives its own git worktree. Refer to `reference/worktree-isolation.md` for the full protocol.

### Task Dependency Wiring

Wire task dependencies at creation time using `addBlocks` and `addBlockedBy`. Workers using `task.list()` see only unblocked tasks, which prevents them from starting out-of-order work automatically.

---

## Common Errors and Fixes

| Symptom | Likely cause | Fix |
|---|---|---|
| Workers do not appear after spawn | Agent Teams flag not set | Add `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` to `~/.claude/settings.json` and restart |
| Worker receives task but does nothing | Worker's task has unresolved `blockedBy` | Ensure all blocking tasks are marked `completed` before the dependent task is claimed |
| `team.create` fails silently | Team name conflicts with an existing session | Call `team.delete` first, then recreate |
| `agent.message` not delivered | Recipient agent has already exited | Check worker status via `task.list()` before sending; re-spawn if needed |
| Orchestrator edits a file directly | `orchestrator-guard.sh` hook fires | Delegate the edit to a `spark` or `kraken` worker; orchestrator must not write files |
| Plan file modified by worker | `plan-protection.sh` hook fires | Workers are blocked from `.maestro/plans/`; orchestrator manages plan files directly |

---

## Tier Capabilities Available

| Capability | Available |
|---|---|
| `agent.spawn` | yes |
| `agent.message` | yes |
| `agent.wait` | yes (via message receipts) |
| `agent.close` | yes |
| `team.create` | yes |
| `team.delete` | yes |
| `task.create` | yes |
| `task.list` | yes |
| `task.get` | yes |
| `task.update` | yes |
| `prompt.structured` | yes |
| `prompt.chat` | yes |
| `fs.read` | yes |
| `fs.write` | yes |
| `fs.search` | yes |
| `fs.grep` | yes |
| `exec.command` | yes |
