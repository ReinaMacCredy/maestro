# Runtime Adapter: [Name]

**Tier**: [Full orchestration (Tier 1) | Partial orchestration (Tier 2) | Serial execution (Tier 3)]
**Target CLIs**: [List the CLIs or environments this adapter targets]
**Prerequisite**: [Any setup required, or "None"]

Brief description of what makes this runtime distinct and when to use it.

---

## Detection Signature

The registry (`reference/runtimes/registry.md`) uses these probes to auto-detect this runtime.

Probes (describe in abstract capability terms — do not use CLI-specific tool names here):

- [ ] [Probe 1 description — e.g., "A tool that creates named coordination groups is present"]
- [ ] [Probe 2 description]
- [ ] [Probe 3 description]

Detection condition: [all probes must pass | any probe must pass | describe condition]

---

## Capability Mapping

Map every capability from `reference/core/capabilities.md` to a concrete tool or fallback.

| Capability | Available | Concrete Tool or Fallback |
|---|---|---|
| `agent.spawn(role, prompt, model?)` | [yes / no] | [tool name or fallback description] |
| `agent.message(recipient, content)` | [yes / no] | [tool name or fallback description] |
| `agent.wait(agent_id?)` | [yes / no] | [tool name or fallback description] |
| `agent.close(agent_id)` | [yes / no] | [tool name or fallback description] |
| `team.create(name, description)` | [yes / no] | [tool name or "skipped"] |
| `team.delete()` | [yes / no] | [tool name or "skipped"] |
| `task.create(subject, description, activeForm?)` | [yes / no] | [tool name or fallback description] |
| `task.list()` | [yes / no] | [tool name or fallback description] |
| `task.get(id)` | [yes / no] | [tool name or fallback description] |
| `task.update(id, fields)` | [yes / no] | [tool name or fallback description] |
| `prompt.structured(question, options, multiSelect?)` | [yes / no] | [tool name or "falls through to prompt.chat"] |
| `prompt.chat(message)` | [yes / no] | [tool name] |
| `fs.read(path)` | [yes / no] | [tool name] |
| `fs.write(path, content)` | [yes / no] | [tool name] |
| `fs.search(pattern)` | [yes / no] | [tool name] |
| `fs.grep(pattern, path?)` | [yes / no] | [tool name] |
| `exec.command(cmd)` | [yes / no] | [tool name] |

---

## Setup Prerequisites

[List any configuration steps required before this adapter works. Delete this section if none.]

---

## Runtime-Specific Features

[Document any capabilities or behaviors unique to this runtime that are not part of the abstract capability model. Examples: isolation modes, model selection hints, session recovery patterns. Delete this section if none.]

---

## Common Errors and Fixes

[Table of known failure modes and their resolutions. Delete this section if unknown.]

| Symptom | Likely cause | Fix |
|---|---|---|
| [symptom] | [cause] | [fix] |

---

## Tier Capabilities Available

Checklist summary for quick reference.

| Capability | Available |
|---|---|
| `agent.spawn` | [yes / no] |
| `agent.message` | [yes / no] |
| `agent.wait` | [yes / no] |
| `agent.close` | [yes / no] |
| `team.create` | [yes / no] |
| `team.delete` | [yes / no] |
| `task.create` | [yes / no] |
| `task.list` | [yes / no] |
| `task.get` | [yes / no] |
| `task.update` | [yes / no] |
| `prompt.structured` | [yes / no] |
| `prompt.chat` | [yes / no] |
| `fs.read` | [yes / no] |
| `fs.write` | [yes / no] |
| `fs.search` | [yes / no] |
| `fs.grep` | [yes / no] |
| `exec.command` | [yes / no] |
