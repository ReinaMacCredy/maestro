# Token Budget Compression — Research-Grounded Plan

**Status:** approved-to-execute
**Branch:** feat/harness-pivot
**Probe:** `./dist/maestro inspect token-budget`
**Doctrine:** `docs/token-budget.md`

## Baseline (2026-05-13, dist/maestro 0.78.0-g19d6d75)

```
verb           mode     bytes    tokens
-------------  -------  -------  -------
skills list    default   121976    34844
skills list    full     1040242   296303
task list      default     5192     1484
task list      full       84187    24054
task status    default    30141     8612
task status    full      124096    35456
task ready     default    13079     3737
task stuck     default     3854     1102
task stuck     full        6704     1916
mission list   default     3816     1091
mission list   full       23011     6575
evidence list  default     2159      617
evidence list  full        3284      939
handoff list   default        2        1
handoff list   full           2        1

totals (default): 180219 bytes, ~51488 tokens
totals (--full):  1281526 bytes, ~365244 tokens
```

**Top 3 default offenders:**
1. `skills list` — 34,844 tokens (68% of default total)
2. `task status` — 8,612 tokens (17%)
3. `task ready` — 3,737 tokens (7%)

These three are 92% of the default agent-facing payload. Everything else is rounding noise.

## External grounding (verified May 2026)

- **Anthropic, "Writing tools for agents"** — concise vs detailed response shapes differ ~3× in tokens; pagination with sensible defaults; clarity over comprehensiveness; "actionable improvements rather than opaque error codes."
- **Anthropic, "Code execution with MCP"** — progressive tool loading (filesystem + `search_tools` with detail level) cut a real workload from 150k → 2k tokens (98.7%). Distant future for maestro; the near-term insight is *detail-level parameter* on list endpoints.
- **Anthropic, "Effective context engineering"** — context rot: transformer accuracy degrades as the window fills; smallest set of high-signal tokens wins. Reinforces minimal-by-default.
- **Maestro doctrine** (`docs/token-budget.md`) — already codifies: summary by default, `--full` opt-in, flat error shape, paginate with `--limit 20` cap.

The doctrine is sound. The gap is that `skills list` was never made to conform: it has no `--limit`, includes oversized `description` strings, and emits absolute filesystem `path` per record.

## Strategy

**Frame:** the probe is the scoreboard. Every change-set must move the probe and not regress others. No vibes; no batched surfaces; one logical change per commit.

**Ordering:** biggest verb first (skills list), then task status, then task ready. After three surfaces, re-evaluate against the <2% stop rule.

**Out of scope for this plan (parked):**
- MCP tool description / Zod `.describe()` compression — not measured by the probe; defer until probe gains an MCP surface, or address only if probe shows skills+task work is exhausted.
- AGENTS.md / CLAUDE.md trimming — already lean; only revisit if probe shows it.

## Surface 1 — `skills list` (target ≥ 70% reduction)

**Owner:** `src/features/skills/commands/skills.command.ts` (inline `summarizeSkill`, `SkillSummary`).

**Findings (from code exploration):**
- ~101 unique skills after dedup, no `--limit`, all dumped every call.
- Per-record fields: `name`, `description`, `scope`, `source`, `path`.
- `description` totals ~27 KB across records; 65/101 are >200 chars (e.g. `blueprint` 834, `swiftui-expert-skill` 756).
- `path` totals ~5.2 KB and is an absolute home-dir path with no agent-usable signal beyond what `scope`+`source` already convey.

**Changes:**

`skills list` is **discovery-shaped**: agents enumerate the full catalog to decide which skill to trigger. Pagination breaks that, so no `--limit` here (unlike tasks/evidence/missions). Compress within the per-record shape instead.

1. **Tighten `SkillSummary` projection — drop `path`, truncate `description` to first sentence (≤200 chars).** Single commit. Both cuts target the same struct; `--full` recovers the pre-doctrine shape; `skills inspect <name>` returns full description+body. Expected: ~14 KB / ~4 K tokens, mostly from description.

**Validation per commit:** `bun run build && ./dist/maestro --version && bun test` plus `./dist/maestro inspect token-budget`, diff vs baseline. Commit body records before→after for the targeted row.

## Surface 2 — `task status` (target ≥ 50% reduction)

**Investigate next session:** find the use-case behind `task status --json`. Read shape before editing.

**Hypotheses from sampled output:**
- Per-track records duplicate `slug` (also present as `identifier` at parent level and inside `task.slug` and `task.id`).
- Empty `steps: []` is emitted on every track.
- `assignee` UUIDs are paid per active track.
- Default may include `pending`/`blocked` tracks that an agent rarely needs at the digest layer.

**Candidate changes (decide after reading code):**
- Default to active+ready only; pending/blocked behind `--full`.
- Drop the duplicated `slug` and `identifier` at track level (one is enough).
- Omit empty `steps`.
- Replace `assignee` UUID with a short label or omit by default.

## Surface 3 — `task ready` (target ≥ 40% reduction)

**Investigate next session:** find the use-case behind `task ready --json`.

**Hypotheses from sampled output:**
- Long `description` strings paid in full.
- `hints[]` carries long `reason` prose per hint; multiple hints per task.

**Candidate changes:**
- Truncate `description` to ~200 chars at default; full text via `task get <id>` or `--full`.
- Cap `hints` to top 1–2; truncate `reason` to ~80 chars.
- Drop `matchedKeywords` from default; recover via `--full`.

## Execution loop (per surface)

1. `TaskCreate` for the surface; mark `in_progress`.
2. Read the file. Spot duplicated/oversized prose.
3. Refactor: extract shared descriptor where applicable; shorten descriptions to one sentence; add limit/cap if missing.
4. `bun run build && ./dist/maestro --version && bun test`.
5. Re-run probe; diff against baseline; reject the change if the targeted row didn't drop or any other row regressed >1%.
6. Conventional commit; bump `src/shared/version.ts` + `package.json` (behavior change).
7. Record evidence with before→after delta in commit body.
8. Mark task `completed`. Next surface.

## Stop conditions

- 3 consecutive surfaces yield <2% improvement on their targeted row → stop, report total.
- Probe regresses anywhere not deliberately accepted → revert that commit, name the reason, pick a different angle.
- Queue empty with no obvious next surface → stop, summarize cumulative reduction.

## Definition of done (per surface)

- Targeted row in `inspect token-budget` drops by the stated target.
- No other row regresses >1%.
- `bun test` + `bun run build` pass.
- Commit body cites the before→after numbers.
- Version bumped.

## Out-of-band considerations

- The `[!] Skill name '...' does not match directory ...` warnings hitting stderr during `skills list` are not in the probe's bytes (it measures stdout only), but they are noisy for humans. Out of scope for this plan; flag for later.
- Generated embeds in `src/infra/domain/*.ts` are off-limits (regenerate via `bun run sync:bundled-skills` if a bundled-skill text changes).
- Skill drafts in `skills/bundled/` need user approval. The plan only touches code, not skill bodies.

## Sources

- Anthropic, "Writing effective tools for AI agents" — https://www.anthropic.com/engineering/writing-tools-for-agents
- Anthropic, "Code execution with MCP" — https://www.anthropic.com/engineering/code-execution-with-mcp
- Anthropic, "Effective context engineering for AI agents" — https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- Maestro doctrine — `docs/token-budget.md`
