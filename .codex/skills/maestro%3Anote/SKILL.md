---
name: maestro:note
description: "Capture decisions, constraints, and context to persistent notepad. Priority notes are injected into every session and implementation run."
argument-hint: "<content> [--priority|--manual|--show|--prune|--clear <section>]"
---

# Note -- Persistent Working Memory

Notes are the cross-cutting memory layer. They persist across sessions, survive context resets, and -- for Priority Context -- are automatically injected into every worker prompt.

**Core principle:** If a future session needs this to avoid a mistake, capture it. If not, skip it.

## Mental Model

Two-tier system:

- **Priority Context** = pinned message in a channel. Every agent sees it, every time. Hard constraints that, if violated, produce wrong code. Workers cannot ask clarifying questions -- they need these upfront.
- **Working Memory** = searchable thread history. Insights, learnings, and soft observations accumulated during implementation. Available on demand, pruned periodically.
- **Manual** = user sticky notes. Never auto-touched, never pruned. The user manages these directly.

## When to Capture

**Always:**
- Decisions that constrain future work (API choice, library version, architecture direction)
- Constraints discovered mid-implementation (rate limits, API quirks, platform bugs)
- User decisions on blockers (the "why" behind the choice, so no one re-asks)
- Cross-track dependencies ("track X depends on auth refactor from track Y")

**Never:**
- Status updates ("started task 3") -- use `task-update`
- Things already in spec or plan -- don't duplicate
- Temporary debugging notes ("tried X, didn't work") -- ephemeral, discard
- Obvious codebase facts -- use `context-write` for codebase knowledge

**Decision heuristic:** "Will a future session need this to avoid a mistake?" Yes = capture. No = skip.

## Priority Context vs Working Memory

| Attribute | Priority Context | Working Memory |
|-----------|-----------------|----------------|
| Injection | Every session + every worker prompt | On-demand (show/prune) |
| Audience | All agents, including workers who cannot ask | Orchestrator only |
| Content | Hard constraints, blocking decisions | Insights, learnings, soft preferences |
| Lifespan | Until feature ships or constraint lifts | Until pruned |
| Volume | **3-7 bullets max** (more = noise) | Unlimited (prune periodically) |
| Format | Imperative constraint | Dated insight with track context |

**Promoting to Priority:** If a Working Memory note keeps causing mistakes across sessions, promote it to Priority Context. If a Priority note no longer applies, demote or delete it.

## Good Notes vs Bad Notes

<Good>
```
--priority "All new endpoints MUST use v2 response envelope ({ data, meta, errors })"
```
Specific, actionable, a worker can follow this without asking.
</Good>

<Bad>
```
--priority "Remember to follow the API conventions"
```
Which conventions? This tells a worker nothing. They will guess wrong.
</Bad>

<Good>
```
"[2026-03-15] [payments:task-1] Stripe rate limit is 100/sec in test mode; batch ops must throttle with 10ms delay"
```
Dated, attributed, quantified, includes the mitigation.
</Good>

<Bad>
```
"Stripe has rate limits"
```
Every API has rate limits. No date, no track, no number, no action.
</Bad>

See `reference/note-patterns.md` for more examples across decisions, constraints, discoveries, and dependencies.

## Arguments

`$ARGUMENTS`

| Input | Target Section |
|-------|---------------|
| `<content>` (no flag) | Working Memory |
| `--priority <content>` | Priority Context |
| `--manual <content>` | Manual |
| `--show` | Display full notepad (read-only) |
| `--prune` | Remove stale Working Memory entries |
| `--clear <section>` | Clear a section (`priority`, `working`, `all`) |

## Execution

### Step 1: Ensure Notepad Exists

If `.maestro/notepad.md` does not exist, create it with section headers:

```markdown
# Notepad
## Priority Context

## Working Memory

## Manual
```

### Step 2: Parse and Execute

**Add** (default, `--priority`, `--manual`):
1. Read `.maestro/notepad.md`
2. Find the target section header
3. Append `- <content>` after the header, before the next `##`
4. Write the updated file
5. Display the updated section to confirm

**Show** (`--show`):
1. Read and display `.maestro/notepad.md`
2. If missing: "No notepad found. Use `/maestro:note <content>` to start."

**Prune** (`--prune`):
1. Review each bullet in `## Working Memory`
2. Remove items that are stale, resolved, or superseded
3. Keep `## Priority Context` and `## Manual` intact
4. Show what was removed and what was kept
5. If uncertain about an item, ask the user

**Clear** (`--clear <section>`):
1. Parse section: `priority`, `working`, or `all`
2. If clearing Priority Context or all: confirm with user first
3. Remove all bullets from specified section(s), keep headers intact

## Note Lifecycle

```
Create --> Review (session start) --> Update (constraint changes) --> Archive/Delete (resolved)
```

| Trigger | Action |
|---------|--------|
| New decision or constraint discovered | Create note (choose tier) |
| Session start | Review Priority Context -- still accurate? |
| Constraint changed | Update the note in place (don't append a duplicate) |
| Feature shipped | Prune Working Memory, demote/delete Priority notes |
| Constraint lifted | Delete the Priority note |
| Reasoning might matter later | Archive to `context-write` before deleting |

## Anti-Patterns

| Anti-Pattern | Why It Hurts | Fix |
|-------------|-------------|-----|
| Notes too vague ("the API thing") | Future sessions cannot act on it | Include specifics: which API, what behavior, what constraint |
| Notes too long (full paragraphs) | Agents skim or skip long bullets | One line per note. Details go in context-write |
| Duplicating spec/plan content | Two sources of truth that drift apart | Reference the spec; don't copy it |
| Priority Context bloat (>7 items) | Agents stop reading; noise drowns signal | Prune aggressively. Only hard constraints belong here |
| Never pruning Working Memory | Graveyard of stale insights misleads agents | Prune after each feature completion |
| Using notes for codebase knowledge | Notes are ephemeral memory, not documentation | Use `context-write` for durable codebase knowledge |
| Status updates as notes | Notes are for decisions and constraints, not progress | Use `task-update` for status |

## Section Contracts

| Section | Written By | Read By | Persistence |
|---------|-----------|---------|-------------|
| Priority Context | User via `--priority` | `maestro:implement` Step 3.8, team-mode worker prompts | Until manually cleared |
| Working Memory | Default `/maestro:note`, auto-capture during implementation | Sessions, prune | Pruned periodically |
| Manual | User via `--manual` | Sessions | Until manually cleared |

## Relationship to Other Commands

- `/maestro:setup` -- Scaffold project context (run first)
- `/maestro:new-track` -- Create a feature/bug track with spec and plan
- `/maestro:implement` -- Execute implementation (reads Priority Context at Step 3.8)
- `/maestro:review` -- Verify implementation correctness
- `/maestro:status` -- Check progress across all tracks
- `/maestro:note` -- **You are here.** Capture decisions and context to persistent notepad

Priority Context is automatically loaded by `/maestro:implement` at execution start and injected into worker prompts in team mode. Working Memory accumulates insights from both manual notes and auto-capture during implementation. Use `--prune` after each feature completion to keep Working Memory relevant.
