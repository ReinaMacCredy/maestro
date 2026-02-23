# Runtime Adapter: Amp (Task + Handoff)

**Tier**: Partial orchestration (Tier 2)
**Prerequisite**: None — no feature flags required. Available in standard Amp sessions.

This adapter maps every abstract capability from `reference/core/capabilities.md` to concrete Amp tools. Use this adapter when running inside an Amp session.

---

## Capability Mapping

| Capability | Amp Tool | Notes |
|---|---|---|
| `agent.spawn(role, prompt, model?)` | `Task` tool | Launches an isolated subagent with the given prompt. The subagent runs in a separate context; it does not share the parent's conversation state. Pass role identity and instructions in the prompt. |
| `agent.message(recipient, content)` | Not directly available — use task description | Amp does not support direct inter-agent messaging. Write updated context into the target task's description via `task.update`; the worker reads it on next `task.list()` poll. |
| `agent.wait(agent_id?)` | `Task` tool (blocking) | `Task` in Amp is synchronous by default — the caller blocks until the subagent completes. No explicit wait primitive is needed. |
| `agent.close(agent_id)` | Not available | Subagents self-terminate when their `Task` call returns. No explicit close is needed or possible. |
| `team.create(name, description)` | Not available | Amp has no named team context. Skip; workers coordinate through the shared task board. |
| `team.delete()` | Not available | No-op. Resources release when the session ends. |
| `task.create(subject, description, activeForm?)` | `TaskCreate` | Maps directly: `subject`, `description`, `activeForm` parameters match. |
| `task.list()` | `TaskList` | Returns all tasks with `id`, `subject`, `status`, `owner`, `blockedBy`. |
| `task.get(id)` | `TaskGet` | Returns full task record including `description`, `blocks`, `blockedBy`. |
| `task.update(id, fields)` | `TaskUpdate` | Pass `taskId` plus any subset of `{ status, owner, subject, description, activeForm, addBlocks, addBlockedBy }`. |
| `prompt.structured(question, options, multiSelect?)` | Not available | Fall through to `prompt.chat` — ask the same question as plain text and parse the response manually. |
| `prompt.chat(message)` | Plain chat output | Write the message or question as assistant output; the user responds in the next turn. |
| `fs.read(path)` | `Read` tool | Reads file at absolute or project-relative path. |
| `fs.write(path, content)` | `create_file` | Creates or overwrites file. |
| `fs.search(pattern)` | `glob` tool | Accepts standard glob patterns. |
| `fs.grep(pattern, path?)` | `Grep` tool | Searches file contents; `path` scopes the search. |
| `exec.command(cmd)` | `Bash` tool | Runs shell command; returns stdout, stderr, and exit code. |

---

## Amp-Specific Patterns

### Task vs Handoff

Amp provides two delegation mechanisms. Understanding when to use each is critical:

#### `Task` — Isolated Subagent

Use `Task` when the work is self-contained and does not need to share conversation state with the parent:

- The subagent starts with a clean context (only what you pass in the prompt)
- Results are returned when the subagent finishes
- The parent blocks until the subagent completes (synchronous)
- Good for: file edits, code generation, targeted research

#### `handoff` — Context-Preserving Delegation

Use `handoff` when the next step needs full access to the current conversation history:

- The handoff agent receives the complete conversation context
- The original agent terminates; the handoff agent continues the session
- Control does not return to the original agent
- Good for: handing off a long session to a specialist, escalating complexity

**Rule of thumb:** Use `Task` for parallel or isolated work. Use `handoff` for sequential continuation where context matters.

### Parallel Fan-Out

Amp's `Task` tool is synchronous, which means true parallel fan-out (multiple workers running simultaneously) is not natively available. The orchestration strategy in Amp is:

1. **Sequential task execution**: The orchestrator works through tasks in dependency order, spawning one `Task` at a time
2. **Logical parallelism via task board**: Create all tasks upfront with dependency wiring; execute them in order as dependencies resolve
3. **No worker self-claim loop**: Because `Task` is synchronous and isolated, workers do not poll `task.list()` independently — the orchestrator drives task assignment explicitly

This places Amp firmly in **Tier 2** (partial orchestration): spawning is available, but the coordination pattern differs from Agent Teams.

### Agent Messaging Workaround

Since `agent.message` is unavailable in Amp, pass updated context to workers through task descriptions:

```
# Instead of:
agent.message("worker", "Use the new API endpoint at /v2/auth")

# Do this:
task.update(taskId, {
  description: "<original description>\n\n[UPDATE]: Use the new API endpoint at /v2/auth"
})
```

Workers read their full task description at the start of each `Task` call, so updates are visible before work begins.

### Filesystem Operations

Amp uses `create_file` (not `Write`) for file creation. For editing existing files, use `edit_file`. The `fs.write` abstraction maps to:

- New file: `create_file`
- Existing file: `edit_file` for targeted changes, or `create_file` to overwrite entirely

---

## Limitations vs Tier 1

| Feature | Claude Code Agent Teams | Amp |
|---|---|---|
| True parallel workers | Yes | No (sequential `Task` calls) |
| Direct agent messaging | Yes (`SendMessage`) | No (use task descriptions) |
| Named team context | Yes (`TeamCreate`/`TeamDelete`) | No |
| Worker self-claim loop | Yes | No (orchestrator assigns explicitly) |
| Structured user prompts | Yes (`AskUserQuestion`) | No (plain chat only) |
| Non-blocking agent spawn | Yes | No (synchronous) |

---

## Tier Capabilities Available

| Capability | Available |
|---|---|
| `agent.spawn` | yes (via `Task`, synchronous) |
| `agent.message` | no — use task description updates |
| `agent.wait` | yes (implicit — `Task` blocks) |
| `agent.close` | no — subagents self-terminate |
| `team.create` | no |
| `team.delete` | no |
| `task.create` | yes |
| `task.list` | yes |
| `task.get` | yes |
| `task.update` | yes |
| `prompt.structured` | no — fall through to `prompt.chat` |
| `prompt.chat` | yes (plain chat output) |
| `fs.read` | yes |
| `fs.write` | yes (via `create_file` / `edit_file`) |
| `fs.search` | yes |
| `fs.grep` | yes |
| `exec.command` | yes |
