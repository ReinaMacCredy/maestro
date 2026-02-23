# Capability Model

This document defines the abstract capabilities required by the `/work` orchestration workflow. Each capability represents an operation the orchestrator needs to perform. Runtime adapters map these abstract capabilities to concrete tools available in the host CLI.

No CLI-specific tool names appear in this document. References to concrete implementations belong in `reference/runtimes/`.

---

## Capability Categories

### Agent Coordination

#### `agent.spawn(role, prompt, model?)`

Launch a worker agent with a designated role and task prompt.

- **Required**: Yes
- **Inputs**: `role` (string) — the agent's identity label; `prompt` (string) — the task instructions; `model` (string, optional) — preferred model tier
- **Output**: an agent handle or identifier for subsequent messaging
- **Fallback**: If unavailable, the orchestrator executes the worker's task directly in serial mode (see Capability Tiers)

#### `agent.message(recipient, content)`

Send a directed message to a specific running worker.

- **Required**: Yes
- **Inputs**: `recipient` (string) — agent identifier or role label; `content` (string) — message body
- **Output**: delivery confirmation or void
- **Fallback**: If unavailable, write the message content to a shared task description so the worker reads it on next poll

#### `agent.wait(agent_id?)`

Pause until a worker completes or signals readiness.

- **Required**: No
- **Inputs**: `agent_id` (string, optional) — wait for a specific worker; omit to wait for any
- **Output**: completion signal or agent status
- **Fallback**: Poll the task board (`task.list()`) on a timed interval until the target task reaches a terminal state

#### `agent.close(agent_id)`

Terminate a worker that is no longer needed.

- **Required**: No
- **Inputs**: `agent_id` (string) — the worker to close
- **Output**: void
- **Fallback**: Workers self-terminate when they find no remaining tasks assigned to them; no explicit close is needed

---

### Team Lifecycle

#### `team.create(name, description)`

Initialize a named coordination context for a group of workers.

- **Required**: No
- **Inputs**: `name` (string) — team identifier; `description` (string) — purpose or scope
- **Output**: team handle
- **Fallback**: Skip; workers coordinate implicitly through the shared task board without a named team context

#### `team.delete()`

Tear down the coordination context and release associated resources.

- **Required**: No
- **Inputs**: none
- **Output**: void
- **Fallback**: No-op; resources are released when the session ends

---

### Task Board

The task board is the shared state through which orchestrators and workers coordinate. All task.board capabilities are required.

#### `task.create(subject, description, activeForm?)`

Create a tracked unit of work on the shared task board.

- **Required**: Yes
- **Inputs**: `subject` (string) — short title in imperative form; `description` (string) — full requirements and acceptance criteria; `activeForm` (string, optional) — present-continuous label shown while the task runs
- **Output**: task identifier
- **Fallback**: none — the task board is a required capability tier

#### `task.list()`

Retrieve all tasks with their current status, owner, and blocking relationships.

- **Required**: Yes
- **Inputs**: none
- **Output**: list of task summaries including `id`, `subject`, `status`, `owner`, `blockedBy`
- **Fallback**: none

#### `task.get(id)`

Fetch the full detail record for a single task.

- **Required**: Yes
- **Inputs**: `id` (string) — task identifier
- **Output**: full task record including `description`, `status`, `blocks`, `blockedBy`
- **Fallback**: none

#### `task.update(id, fields)`

Modify a task's status, owner, description, or dependency links.

- **Required**: Yes
- **Inputs**: `id` (string) — task identifier; `fields` (object) — any subset of `{ status, owner, subject, description, activeForm, addBlocks, addBlockedBy }`
- **Output**: updated task record or void
- **Fallback**: none

---

### User Interaction

#### `prompt.structured(question, options, multiSelect?)`

Present a question to the user with a structured UI — checkboxes, radio buttons, or a selection list.

- **Required**: No
- **Inputs**: `question` (string) — the prompt text; `options` (string[]) — selectable choices; `multiSelect` (boolean, optional) — allow multiple selections
- **Output**: selected option(s) as string or string[]
- **Fallback**: Fall through to `prompt.chat` — ask the same question as plain text and parse the response

#### `prompt.chat(message)`

Send a plain-text message or question to the user and wait for a response.

- **Required**: Yes
- **Inputs**: `message` (string) — the message or question
- **Output**: user's reply as string
- **Fallback**: none — user interaction is a required capability

---

### Filesystem

#### `fs.read(path)`

Read the contents of a file.

- **Required**: Yes
- **Inputs**: `path` (string) — absolute or project-relative file path
- **Output**: file contents as string
- **Fallback**: none

#### `fs.write(path, content)`

Write content to a file, creating or overwriting as needed.

- **Required**: Yes
- **Inputs**: `path` (string) — target file path; `content` (string) — file contents
- **Output**: void or success confirmation
- **Fallback**: none

#### `fs.search(pattern)`

Find files matching a glob pattern.

- **Required**: Yes
- **Inputs**: `pattern` (string) — glob expression
- **Output**: list of matching file paths
- **Fallback**: none

#### `fs.grep(pattern, path?)`

Search file contents for a regex or literal pattern.

- **Required**: Yes
- **Inputs**: `pattern` (string) — search expression; `path` (string, optional) — scope to a file or directory
- **Output**: list of matches with file path and line context
- **Fallback**: none

---

### Execution

#### `exec.command(cmd)`

Run a shell command and capture its output.

- **Required**: Yes
- **Inputs**: `cmd` (string) — the shell command to run
- **Output**: stdout, stderr, and exit code
- **Fallback**: none

---

## Capability Tiers

Runtimes vary in which capabilities they support. The orchestration workflow adapts its strategy based on the tier the host environment falls into.

### Tier 1 — Full Orchestration

All capabilities are available. The orchestrator launches parallel workers, coordinates via the task board and messaging, and manages team lifecycle.

**Characteristics:**
- `agent.spawn` available
- Task board fully supported
- `agent.message` available for direct worker communication
- Optional capabilities (`agent.wait`, `agent.close`, `team.create`, `prompt.structured`) may also be present

**Examples of runtime environments in this tier:** those that support spawning subagents with shared task visibility and inter-agent messaging.

---

### Tier 2 — Partial Orchestration

Worker spawning and the task board are available, but direct messaging between agents is limited or absent.

**Characteristics:**
- `agent.spawn` available
- Task board available
- `agent.message` absent or unreliable — coordination happens through task description updates
- Workers poll `task.list()` for their assignments

**Adaptation:** The orchestrator writes instructions into task descriptions rather than sending direct messages. Workers read their task to receive updated context.

**Examples of runtime environments in this tier:** those that support parallel task execution but route communication through a shared board rather than direct channels.

---

### Tier 3 — Serial Execution

No worker spawning available. The orchestrator executes all tasks itself, in sequence.

**Characteristics:**
- `agent.spawn` absent
- Task board may be present (used for progress tracking) or simulated via filesystem notes
- All work is done inline by the orchestrator

**Adaptation:** The orchestrator steps through each task in dependency order, completing work directly rather than delegating. User interaction capabilities remain available for approvals and confirmations.

**Examples of runtime environments in this tier:** single-agent chat environments without subagent support.

---

## Capability Summary Table

| Capability | Required | Tier 1 | Tier 2 | Tier 3 |
|---|---|---|---|---|
| `agent.spawn` | No* | yes | yes | no |
| `agent.message` | Yes** | yes | partial | no |
| `agent.wait` | No | yes | no | no |
| `agent.close` | No | yes | no | no |
| `team.create` | No | yes | varies | no |
| `team.delete` | No | yes | varies | no |
| `task.create` | Yes | yes | yes | yes |
| `task.list` | Yes | yes | yes | yes |
| `task.get` | Yes | yes | yes | yes |
| `task.update` | Yes | yes | yes | yes |
| `prompt.structured` | No | yes | varies | varies |
| `prompt.chat` | Yes | yes | yes | yes |
| `fs.read` | Yes | yes | yes | yes |
| `fs.write` | Yes | yes | yes | yes |
| `fs.search` | Yes | yes | yes | yes |
| `fs.grep` | Yes | yes | yes | yes |
| `exec.command` | Yes | yes | yes | yes |

\* `agent.spawn` is required for parallel orchestration but optional overall — Tier 3 runtimes operate without it.
\*\* `agent.message` degrades gracefully to task-description updates when direct messaging is unavailable.
