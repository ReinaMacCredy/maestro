# Token Budget Doctrine

Maestro is consumed by long-running agents whose context windows are
finite. Every list-shape response we emit competes for room with the
agent's own work. This document codifies how we keep our footprint small.

## Principles

1. **Lean by default; `--full` / `view: "full"` opts into verbose.**
   List endpoints emit summary-grade items unless the caller explicitly
   asks for the detail shape.

2. **Project at the use-case layer.** CLI and MCP share the same summary
   helpers in `src/shared/lib/projection.ts`. Fix the shape once; both
   surfaces benefit.

3. **Detail calls (`get` / `show`) stay full.** A reader asking for one
   item by id wants the whole row. Only collection endpoints get summary
   projection.

4. **Doctrine error shape.** MCP errors are flat:
   `{ code, message, hints?, arg? }`. No nested SDK envelopes, no
   pretty-printed Zod issue dumps, no duplicated `structuredContent` for
   error rows. The lean shape lets agents pattern-match on `code` and
   `arg` rather than parse free-text messages.

5. **Progressive disclosure for skills.** A `SKILL.md` is the trigger
   body; deep content lives in `reference/*.md` files one level deep.
   Agents load reference files only when the trigger points to them.

## Per-surface conventions

### CLI (`--json`)

| Verb | Default shape | Recover full shape |
|------|---------------|--------------------|
| `task list --json` | Summary, default `--limit 20` | `--full --all` |
| `task status --json` | Digest (no `tasksById`) | `--full` |
| `task ready --json` | Summary | (no full mode; already lean) |
| `task stuck --json` | Summary | `--full` |
| `mission list --json` | Summary, default `--limit 20` | `--full --all` |
| `evidence list --json` | Summary, default `--limit 20` | `--full --all` |
| `handoff list --json` | Summary, default `--limit 20` | `--full --all` |
| `skills list --json` | Summary (no `body`) | `--full` |

Human-readable CLI output is already concise and unchanged.

### MCP

`maestro_task_list`, `maestro_evidence_list`, `maestro_handoff_list` accept:

- `view?: "summary" | "full"` — defaults to `"summary"`.
- `limit?: number` — defaults to 20.
- The `paginate()` envelope still surfaces `total` and `hasMore` so an
  agent that needs more knows to ask.

Tool registrations no longer carry `outputSchema`. The text content in
`content[0].text` is the authoritative payload.

Error rows use the doctrine shape:

```json
{ "code": "INVALID_ARG", "message": "Required", "arg": "taskId" }
```

The `code` field is the routing key. Add a `hint` (singular) or `hints[]`
(plural) only when the caller cannot recover from `code` + `arg` alone.

### Skills

A bundled `SKILL.md` is a trigger body ≤ 300 lines. Topics > 100 lines
move to `reference/<topic>.md`. Reference files stay one level deep —
they do not link onward to further reference files (Claude may partial-
read a chained file and miss the tail).

Files > 100 lines start with a `## Contents` table of contents so
partial reads still see the full scope.

`skills list` exposes summary records; `skills inspect <name>` reads
the `body`.

## Opt-in / opt-out flags

| Flag | Where | Effect |
|------|-------|--------|
| `--full` | CLI list verbs | Recover the verbose pre-doctrine shape |
| `--all` | CLI list verbs | Drop the default `--limit 20` cap |
| `--limit <n>` | CLI list verbs | Override default cap |
| `view: "full"` | MCP list tools | Same as CLI `--full` |
| `limit` | MCP list tools | Override default cap (20) |

## Regression guard

```bash
maestro inspect token-budget --json
```

Measures bytes and estimated tokens for each agent-facing list verb in
both default and `--full` modes. Run before/after a list-shape change
and after any projection helper edit; surprise regressions show up
immediately.

The estimator is a heuristic (Anthropic publishes ~4 chars/token; JSON
tokenizes at ~3.5). Use it for comparing the same shape before and
after a change, not for absolute cost projections.

## Further reading

The doctrine is grounded in external guidance from 2025–2026:

- Anthropic — *Writing tools for agents*: pagination, range selection,
  filtering, sensible default parameter values; small default page
  sizes; `response_format: "detailed" | "concise"` (matches our `view`
  enum); prefer semantic identifiers (`name`, `slug`) over UUIDs.
- Anthropic — *Skill authoring best practices*: `SKILL.md` body
  ≤ 500 lines; references one level deep from `SKILL.md`; table of
  contents for reference files > 100 lines; description ≤ 1024 chars,
  third person, includes both *what it does* and *when to use it*.
- Anthropic — *Effective context engineering*: "context rot" — accuracy
  degrades as context grows; minimal-by-default beats opt-in-compact.
- Anthropic — *Code execution with MCP*: `defer_loading: true` and
  filesystem-based progressive tool loading as future direction for
  even-larger MCP surfaces.

When in doubt, optimize for "the smallest payload that still answers
the agent's likely follow-up question." The agent can always ask for
more; it cannot un-spend context on noise we sent eagerly.
