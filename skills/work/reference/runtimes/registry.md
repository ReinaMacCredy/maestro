# Runtime Registry

This document defines how the orchestrator detects which runtime environment it is executing in, selects the appropriate adapter, and logs that selection.

Adapters map abstract capabilities (defined in `reference/core/capabilities.md`) to concrete tools available in the host CLI. Each adapter lives in `reference/runtimes/`.

---

## Detection Algorithm

At session start the orchestrator probes the tool inventory in priority order. The first adapter whose detection signature matches is selected.

```
1. Probe available tools
2. Walk priority list from highest to lowest
3. First match → select adapter → log selection
4. No match → fall back to generic-chat adapter
```

If the user supplies `--runtime=<name>`, skip detection and load that adapter directly.

---

## Detection Signatures

Each runtime is identified by a minimal set of tool probes. A probe passes if the named capability is accessible in the current session.

### Claude Code Agent Teams

**Tier**: Full orchestration (Tier 1)

Detection probes (all must pass):
- A tool that creates named coordination groups is present
- A tool that creates tracked tasks on a shared board is present
- A tool that sends directed messages between agents is present

**Adapter file**: `reference/runtimes/claude-teams.md`

---

### Codex (OpenAI)

**Tier**: Full orchestration (Tier 1)

Detection probes (all must pass):
- A tool that spawns a subprocess or child agent by name is present
- A tool that sends text input to a running subprocess is present
- A tool that blocks until a subprocess exits is present

**Adapter file**: `reference/runtimes/codex-spawn.md`

---

### Amp

**Tier**: Partial orchestration (Tier 2)

Detection probes (all must pass):
- A tool that creates tasks with a handoff or continuation pattern is present
- Direct inter-agent messaging is absent or unreliable

**Adapter file**: `reference/runtimes/amp-task-handoff.md`

---

### Generic Chat

**Tier**: Serial execution (Tier 3)

Detection condition: No spawning tool and no task-board tool detected. Only basic filesystem and shell capabilities are present.

**Adapter file**: `reference/runtimes/generic-chat.md`

---

## Priority Order

When multiple probes could match (e.g., an environment that exposes both Codex and basic tools), the orchestrator applies this priority:

1. Claude Code Agent Teams
2. Codex
3. Amp
4. Generic Chat (fallback — always matches)

---

## Manual Override

Pass `--runtime=<name>` when invoking `/work` to bypass detection:

```
/work my-plan --runtime=claude-teams
/work my-plan --runtime=codex-spawn
/work my-plan --runtime=amp-task-handoff
/work my-plan --runtime=generic-chat
```

Valid names correspond to the adapter file names in `reference/runtimes/` without the `.md` extension.

---

## Logging Format

After selection, the orchestrator logs one line before beginning task creation:

```
[runtime] detected: <adapter-name> (tier <N>) — <reason>
```

Examples:

```
[runtime] detected: claude-teams (tier 1) — agent coordination + task board + messaging probes passed
[runtime] detected: generic-chat (tier 3) — no spawning or task board tools found; using serial fallback
[runtime] override: codex-spawn (tier 1) — --runtime flag set by user
```

---

## Adding a New Runtime

1. Create `reference/runtimes/<name>.md` using `reference/runtimes/_template.md`
2. Define detection probes in abstract terms (capability names, not tool names)
3. Add an entry in this file under Detection Signatures
4. Insert it into the Priority Order list at the appropriate tier
