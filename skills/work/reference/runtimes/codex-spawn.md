# Runtime Adapter: Codex CLI

**Tier**: Partial orchestration (Tier 2)
**Prerequisite**: Codex CLI installed and authenticated.

This adapter maps every abstract capability from `reference/core/capabilities.md` to concrete Codex CLI tools. Codex supports agent spawning and shell execution but has no native shared task board. Workers coordinate through filesystem-based task state — the orchestrator writes tasks to a JSON file and workers read and update it directly.

---

## Capability Mapping

| Capability | Codex Tool / Mechanism | Notes |
|---|---|---|
| `agent.spawn(role, prompt, model?)` | `spawn_agent` | Pass `role` and `prompt` as agent instructions. Use `model` param if supported; otherwise omit and accept Codex default. |
| `agent.message(recipient, content)` | `send_input` to agent handle | Send a message string to a running agent by its handle. If the agent has exited, re-spawn and replay context. |
| `agent.wait(agent_id?)` | `wait` | Block until the target agent emits a completion signal or exits. Omit `agent_id` to wait for any. |
| `agent.close(agent_id)` | `close_agent` | Terminate the agent by handle. Call after the agent signals task completion. |
| `team.create(name, description)` | Not available — no-op | Codex has no named team context. Skip; workers coordinate via the filesystem task file. |
| `team.delete()` | Not available — no-op | No team context to tear down. Clean up the task file instead when the plan is archived. |
| `task.create(subject, description, activeForm?)` | Filesystem task file (`exec_command` write) | Append a task record to `.maestro/tasks.json`. Fields: `id`, `subject`, `description`, `activeForm`, `status: "pending"`, `owner: null`, `blockedBy: []`. |
| `task.list()` | Filesystem task file (`exec_command` read) | Read `.maestro/tasks.json` and return all records. Workers filter for `status: "pending"` and unblocked entries. |
| `task.get(id)` | Filesystem task file (`exec_command` read + filter) | Read `.maestro/tasks.json`, filter by `id`, return the full record. |
| `task.update(id, fields)` | Filesystem task file (`exec_command` read-modify-write) | Read `.maestro/tasks.json`, patch the target record, write back atomically. |
| `prompt.structured(question, options, multiSelect?)` | `request_user_input` + text parsing | Present options as a numbered list in the prompt text. Parse the user's numeric reply to resolve the selection. |
| `prompt.chat(message)` | `request_user_input` | Send message as prompt text; return the user's reply string directly. |
| `fs.read(path)` | `exec_command` (`cat <path>`) | Read file contents via shell. |
| `fs.write(path, content)` | `exec_command` (`tee <path>` or equivalent) | Write or overwrite file via shell. Prefer atomic write (write to temp file, then `mv`). |
| `fs.search(pattern)` | `exec_command` (`find` or `ls` with glob) | Glob file search via shell. |
| `fs.grep(pattern, path?)` | `exec_command` (`grep -rn`) | Content search via shell. Scope with `path` if provided. |
| `exec.command(cmd)` | `exec_command` | Direct passthrough. Returns stdout, stderr, and exit code. |

---

## Task Board Simulation

Codex has no native shared task board. The orchestrator simulates one using a JSON file:

**File**: `.maestro/tasks.json`

**Schema**:
```json
[
  {
    "id": "1",
    "subject": "Implement login endpoint",
    "description": "Full requirements and acceptance criteria here",
    "activeForm": "Implementing login endpoint",
    "status": "pending",
    "owner": null,
    "blockedBy": [],
    "blocks": []
  }
]
```

**Orchestrator responsibilities**:
1. Create `.maestro/tasks.json` before spawning workers.
2. Write all tasks upfront with correct `blockedBy` wiring.
3. After each `agent.wait` completes, read the file and verify status was updated by the worker.
4. When a task is verified complete, update any tasks that had it in `blockedBy` to unblock them.

**Worker responsibilities**:
1. Read `.maestro/tasks.json` to find unblocked, unowned, pending tasks.
2. Claim a task by writing `owner: <role>` and `status: "in_progress"`.
3. On completion, write `status: "completed"`.
4. Workers must use file locking or retry-on-conflict to avoid race conditions on the shared file.

**Write safety pattern** (for workers via `exec_command`):
```bash
# Atomic update via temp file
cp .maestro/tasks.json .maestro/tasks.json.tmp
# ... patch tasks.json.tmp ...
mv .maestro/tasks.json.tmp .maestro/tasks.json
```

---

## Codex-Specific Patterns

### Spawning Workers

Each `spawn_agent` call returns an agent handle. Store handles in a local map keyed by role so `send_input` and `close_agent` calls can reference them:

```
handles = {}
handles["kraken-1"] = spawn_agent(role="kraken", prompt="...")
handles["spark-1"]  = spawn_agent(role="spark",  prompt="...")
```

### Worker Prompts

Worker prompts must include the task file path, since workers have no native task board access:

```
You are a worker agent. Your task board is at .maestro/tasks.json.
Claim and complete one pending task at a time. Update status in the file directly.
...
```

### Inter-Worker Communication

Codex has no direct agent-to-agent messaging. Workers communicate through task descriptions:
- The orchestrator updates a task's `description` field to pass new context.
- Workers re-read their task record to pick up updated instructions.

### Waiting for Completion

After spawning workers, the orchestrator calls `wait` on each handle in order:

```
result-1 = wait(handles["kraken-1"])
result-2 = wait(handles["spark-1"])
```

If a worker exits with an error, re-read the task file to determine which tasks remain incomplete before deciding whether to re-spawn or fail.

### User Interaction

`request_user_input` is Codex's single interaction primitive. Use it for both `prompt.chat` and `prompt.structured`:

- **Chat**: Pass the message directly as the prompt string.
- **Structured**: Format options as a numbered list in the prompt string. Parse the user's reply by matching the number or the option label text.

Example structured prompt text:
```
Execute this plan? Enter the number of your choice:
1. Yes, execute — Proceed with team creation
2. Cancel — Stop without executing
```

---

## Limitations

| Capability | Limitation | Mitigation |
|---|---|---|
| `team.create` / `team.delete` | Not available | No-op; skip these calls |
| `agent.message` | No direct inter-agent channel | Write context to task descriptions; workers poll |
| `task.*` (native) | No native task board | Filesystem JSON file at `.maestro/tasks.json` |
| `prompt.structured` | No clickable UI | Numbered-list text prompt with text parsing |
| Concurrent writes to task file | Race condition risk | Atomic `mv` pattern; workers retry on conflict |
| `fs.read` / `fs.write` via shell | Slower than native file tools | Acceptable for coordination files; avoid for large payloads |

---

## Tier Capabilities Available

| Capability | Available |
|---|---|
| `agent.spawn` | yes |
| `agent.message` | partial (via task description updates) |
| `agent.wait` | yes |
| `agent.close` | yes |
| `team.create` | no |
| `team.delete` | no |
| `task.create` | yes (filesystem simulation) |
| `task.list` | yes (filesystem simulation) |
| `task.get` | yes (filesystem simulation) |
| `task.update` | yes (filesystem simulation) |
| `prompt.structured` | partial (text parsing) |
| `prompt.chat` | yes |
| `fs.read` | yes |
| `fs.write` | yes |
| `fs.search` | yes |
| `fs.grep` | yes |
| `exec.command` | yes |
