# Worker Type Matching: Feature to Executor

This is the third step of mission planning. You have a decomposed plan with milestones and features from Step 2. You need to assign each feature to a worker type that can actually execute it.

Workers are the external processes that implement features. Maestro is the conductor — it holds the score, drafts the next handoff, and validates results, but it does not write code itself. The four worker types below cover every real execution pattern.

## The four worker types

**`codex-cli`** — mechanical implementation against a spec. Cheap, fast, runs in a fresh context with no carryover. No judgment, no taste, no pushback. Best when the feature reads like a to-do list: the decision work is already done and someone just needs to type it in. If a feature would benefit from "what if we did X instead," `codex-cli` is the wrong choice — it will do exactly what the prompt says, even when the prompt is wrong.

**`claude-code`** — ambiguous or heavy-lift work that needs judgment. Expensive, slower, brings taste and pushback. Best when the feature has multiple valid approaches, when requirements are loose, when the change touches architecture, or when a reviewer has to think like a senior engineer. Also the right choice when the work is small but the blast radius is large (security, auth, migration paths).

**`subagent`** — native Claude Code Task tool. Shares the parent context, so it can see what the parent has already read without re-exploring. Fast and cheap for research and exploration. Best for "survey the codebase for X" or "find every caller of Y" — things where the answer is discoverable from existing files and the parent wants a summary back, not a patch. Not appropriate for multi-hour implementation work; subagents lose coherence as their scope grows.

**`human`** — ideation, approval, trust calls, final taste judgments. The only worker type that can say "this is ugly, do it differently" without a formal rubric. Best for brainstorm opening, final go/no-go decisions, and any milestone where the question is "is this what I actually want?" not "does this match the spec?"

## Decision table

| Feature characteristic | Worker type |
|---|---|
| Mechanical code following a spec | `codex-cli` |
| Bug fix with clear reproduction and fix direction | `codex-cli` |
| Exploratory research across the codebase | `subagent` |
| Enumerating callers, references, or patterns | `subagent` |
| Architectural decision with multiple valid options | `claude-code` |
| New feature with ambiguous requirements | `claude-code` |
| Refactor that touches cross-cutting concerns | `claude-code` |
| Code review of a generator's output | `claude-code` (different instance than generator) |
| Final go/no-go decision on a milestone | `human` |
| Brainstorming a raw idea into a plan | `human` + `claude-code` together |

## The "different instance" rule

If a milestone profile is `code-review` or `plan-review`, the reviewer must be a different instance than whatever produced the artifact being reviewed. A fresh `claude-code` session reviewing another `claude-code` session's code is fine. The same session reviewing its own work is not.

Self-review is pathologically lenient. The Anthropic harness-engineering article calls this out directly: models are strongly biased toward approving their own output because their working memory still holds the justifications for every decision they made. A fresh reviewer does not carry those justifications, and will notice problems the generator rationalized away. Always spawn a new instance for review milestones.

## Worked examples

`codex-cli` — matches:
- "Add a `--format json` flag to `maestro mission list` that emits the existing data through `JSON.stringify`." Clear spec, clear location, zero decisions left.
- "Rename `WorkerKind` to `WorkerType` across `src/` and `tests/`." Pure mechanical rewrite.

`codex-cli` — anti-example:
- "Design a provider-selection policy for the handoff launcher." Requires taste and judgment. Hand to `claude-code`.

`subagent` — matches:
- "List every file that imports `buildHandoffPrompt()` and group by directory." Discoverable, summary-shaped output.
- "Find all places where launch records are filtered by mission id and note which ones also read principle outcomes." Pure exploration.

`subagent` — anti-example:
- "Implement a new supervisor replacement across 12 files." Too large, too long-running. Hand to `claude-code`.

`claude-code` — matches:
- "Refactor `buildHandoffPrompt()` so mission and repository contexts share less duplicated logic." Architectural, multiple valid shapes.
- "Review the `codex-cli`-generated launch-store refactor from the previous milestone." Fresh instance, applies judgment.

`claude-code` — anti-example:
- "Add a trailing newline to `CHANGELOG.md`." Massive overkill. Hand to `codex-cli`.

`human` — matches:
- "Approve the final mission plan before handoff." Trust call.
- "Decide whether the auth rewrite ships in this mission or a follow-up." Scope and taste.

`human` — anti-example:
- "Write the actual middleware code." Humans are not the cheapest implementer. Hand to `codex-cli` or `claude-code` depending on ambiguity.
