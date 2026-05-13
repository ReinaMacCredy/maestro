# Changelog

## 0.80.13 - prod polish: handoff-bare default, task-create --title alias

Closes the two remaining LOW friction items surfaced across Round 5/6
UAT so the agent loop is "production polish" clean. Both fixes are
ergonomic — no semantic change to existing usage, just one new tolerant
default and one alias.

### Fixed

- **`maestro handoff` (no args) now lists existing packets instead of
  erroring.** Bare invocation with no positional, no `--prompt-file`,
  and no launch flags (`--agent`, `--task-id`, `--model`, `--worktree`,
  `--base`, `--wait`) is treated as a discovery query: agents
  exploring the verb get the same output as `maestro handoff list`
  rather than a "Task description required" error. Any explicit launch
  signal still routes to the launch path, so a mis-typed task arg
  can't silently become a list. `--json` returns the lean 20-item
  summary used elsewhere.

- **`maestro task create --title <title>` accepts the title via flag.**
  Agents that pattern-match on `--title` (a common convention from
  other task systems) no longer have to special-case `task create` as
  positional-only. The positional form still works; passing both is
  rejected with a "pass it just once" hint, matching the
  `task introspect` / `task verify` / `task proof` `--task` vs
  positional pattern.

## 0.80.12 - UAT round-6 mop-up: task introspect same-id edge case

Round-6 UAT (greenfield + brownfield, real MCP server, v0.80.11 binary)
declared the agent loop "good enough" — both agents returned `YES`.
This release closes the single MED edge case greenfield surfaced.

### Fixed

- **`task introspect --task X X` (same id in both slots) now errors
  cleanly.** Previously the duplication guard only fired when the
  positional and `--task` values differed, so `--task tsk-abc tsk-abc`
  silently succeeded and hid the fact that the caller was uncertain
  which form the verb took. Now both same-value and differing-value
  cases reject with the same "pass it just once" hint.

## 0.80.11 - UAT round-5: MC task-awareness ACTUALLY fires + setup, introspect, gc, verdict_request hardening

Round-5 UAT (greenfield + brownfield, real MCP server connections) found
one HIGH regression and four MED/LOW friction items.

### Fixed

- **Mission Control task-aware headline now fires for `--preview` and
  `--preview <screen>` (HIGH regression).** The v0.80.10 projection fix
  branched on `taskBoard.totalCount`, but the home/dashboard preview
  paths set `includeTaskBoard: false` — so the board never loaded and
  the headline silently stayed at "No missions yet" even with tasks
  present. `buildMissionControlSnapshotDemand` now requests the task
  board for every preview screen (the home projection consumes it
  regardless of which screen is being rendered).
- **`maestro task introspect` accepts `--task <id>` (MED).** Mirrors
  `task verify` and `task proof`, which were already on the flag form.
  Agents extrapolating from those verbs no longer trip on
  `error: unknown option '--task'`. Positional `<id-or-slug>` still
  works; passing both errors out explicitly with a typed MaestroError.
- **`maestro gc` (bare) exits 0 with help text (LOW).** Was exiting 1
  while printing help, inconsistent with other group commands. Now
  matches `--help` semantics.
- **`maestro setup --check` warns when `.maestro/` is missing (MED).**
  Previously emitted "Setup audit — OK" on a completely un-initialized
  repo, which read as success to first-time greenfield agents. New
  `project-not-initialized` finding directs the user to run
  `maestro init` first.
- **`maestro_verdict_request` returns `NO_COMMITS` on an empty repo
  (MED).** The verifier's `git rev-parse HEAD^{tree}` call surfaced the
  raw `fatal: ambiguous argument 'HEAD^{tree}'` git error to the MCP
  caller — unactionable for agents. Now translates to a typed
  `NO_COMMITS` failure with a hint to `git commit` first.

### Build correctness

- **Verified `dist/maestro` matches HEAD.** Round-5 surfaced that
  `dist/maestro` was built 14s before the v0.80.10 commit landed, so
  the test fleet was unknowingly exercising v0.80.9. The HIGH "R14
  regression" above existed in v0.80.10 too, but went undetected
  because the binary under test predated the projection.ts fix. Pre-
  test step: `bun run build && ./dist/maestro --version` to confirm git
  sha against HEAD.

## 0.80.10 - UAT round-4 brownfield: receipt invariant, mission-control task awareness, state/get aliases

Round-4 brownfield UAT reported one HIGH-severity issue plus two
medium-impact items affecting first-time agent ergonomics. All fixed.

### Fixed

- **`maestro_task_complete` now rejects calls without `summary` or
  `reason` (HIGH).** The MCP handler accepted `{id}` with no receipt
  text, silently producing context-free completions that violated the
  `maestro-task` skill's hard rule #2 ("Every completion carries
  `--reason`"). Now returns `INVALID_ARG { arg: "summary" }` with hints
  pointing at the rule. Mirrors the CLI invariant. Tool description and
  schema describe the constraint up front.
- **Mission Control is no longer task-blind on a task-only project.**
  When `missions=0` and the task store has at least one task, the home
  headline reports `N tasks in this project` (not "No missions yet"),
  the summary directs the user to `maestro task status`, and the action
  list includes "See task queue" instead of presenting "Initialize this
  project" / "Run maestro doctor" as the only paths forward.
- **`maestro state` (bare) now defaults to a 24-hour summary.** Was
  exiting 1 with subcommand help on stderr — confusing for first-timers
  who expected a state summary. `state since <iso>` still works; bare
  `state` now executes `state since now-24h` and exits 0.
- **`maestro task get` is an alias for `task show`.** Agents reading the
  MCP `maestro_task_get` tool name no longer trip on the CLI naming
  mismatch.

## 0.80.9 - UAT round-4 fixes: MC doctor stale hint, note positional, audit noise, advisory polish

Round-4 UAT (real MCP, greenfield) reported zero HIGH-severity issues and
declared the agent loop ready. Six MED/LOW friction items addressed here:

### Fixed

- **MC Activity panel no longer emits `next: Run maestro doctor` on a clean
  repo (F1).** The home-mode action list correctly omits the doctor action
  when checks all pass; the Activity panel renderer was hard-coding the
  string as a fallback. Now the `next:` line is skipped entirely when there
  is no recommended action.
- **`maestro note "text"` accepts a positional argument (F2).** The natural
  first invocation no longer fails with `too many arguments`. Both
  `maestro note "..."` and `maestro note --content "..."` work; passing both
  errors out explicitly.
- **`maestro_handoff_pickup.actorAgent` schema description names the
  Claude-Code convention (F3).** Adds "Claude Code agents pass `claude`
  (not `claude-code`)" so agents that read schema descriptions know which
  enum value to use.
- **`maestro session` no-args error mentions subcommands (F4).** First-time
  agents who run `maestro session` outside an agent env now see a hint
  pointing at `maestro session start <taskId>` / `exit <taskId>` and
  `--help`.
- **`maestro setup --check` no longer warns about
  `docs/harness-positioning.md` / `docs/schedule-recipes.md` on fresh user
  projects (F5).** Those docs are maestro-repo-internal; the audit check
  leaked maestro-self-audit findings into user-facing output. Removed.
- **`maestro task proof` surfaces an advisory when no contract or spec
  exists (F6).** Empty proof output used to mean "no criteria found" with
  no guidance on how to add criteria. JSON output gains a `warning:
  "no-criteria-source"` field plus a `hint` pointing at
  `task contract new <id> --from default`; text output prints both lines.

## 0.80.8 - UAT round-3 mop-up: contract-lock hint upgrade, task-ready hint signal cleanup

Last two carry-overs from round-3 UAT, addressed in one shot:

### Changed

- **Contract-lock failure hints now name the template path and CLI shortcut.**
  The MaestroError raised when `maestro task contract lock` fails on a
  draft was telling agents what was missing without telling them where to
  find the schema. The error now points at
  `.maestro/tasks/contract-templates/default.md` and surfaces the
  `--from default` shortcut, in addition to the `contract show` repro
  command.
- **`task ready` hints filter out generic task-verb noise.** The keyword
  extractor's stop-list now drops `fix`, `add`, `remove`, `bump`,
  `refactor`, `task`, `bug`, `feature`, and other meta-nouns that
  appeared in nearly every task title and so dominated the candidate
  index. Hints surfaced by `task ready` (and `findSimilarTasks`) now key
  off specific domain nouns — `argon2`, `jwt`, `middleware`, etc. —
  instead of producing matches purely on shared verbs.

## 0.80.7 - UAT round-3 followups: MCP tool table coverage, doctor-suggestion gating

Two more findings from round-3 UAT:

### Changed

- **`skills/bundled/maestro-task/SKILL.md`** documents all 18 MCP tools.
  Was missing the verdict, policy, and handoff families (~7 tools).
  Added a "When" column and explicit notes on the `id` vs `taskId`
  parameter convention and the `summary`/`reason` alias on
  `maestro_task_complete`.
- **Mission Control** no longer suggests `maestro doctor` on a healthy
  repo. The action is now gated on at least one failing doctor check —
  showing it unconditionally misled first-time users into running
  diagnostics on a clean project.

## 0.80.6 - UAT round-3 fixes (evidence_record schema, task verify exit, lean skills list, CLI/MCP parity)

Round-3 UAT (greenfield + brownfield, real MCP server connection) found
two HIGH-severity issues blocking first-time MCP agent loops, plus four
moderate friction items. All addressed in this release.

### Fixed

- **`maestro_evidence_record` MCP schema is no longer empty (HIGH).**
  `EvidenceRecordInput` used `z.object().refine().refine()`, which wraps
  the schema in `ZodEffects`. The SDK could not introspect `.shape`, so
  `tools/list` surfaced `{"properties": {}}` — agents had zero guidance
  on what to pass. Split into `EvidenceRecordShape` (raw record, used
  for tool registration) and `EvidenceRecordInput` (full schema with
  refines, applied at runtime). Cross-field validation still rejects
  bad inputs with `INVALID_ARG` carrying the offending `arg`.
- **`task verify` exits 0 on the no-contract skip path (HIGH).** Was
  exiting 2 (intended as "warn"), which aborted any agent shell running
  under `set -e` or composing with `task verify && next-step`. The
  advisory JSON / text message still calls out the skip; exit code now
  matches the semantics (verifier didn't run, but that's not a failure).

### Changed

- **`skills list` JSON default drops `description` (MED, 57 % cut).**
  130 skills × ~200-char descriptions = 25 KB of context-tax on every
  startup-time `skills list`. Default now returns `{name, scope,
  source}` only. `--full` recovers `description`, `path`, `root`,
  `metadata`. Bodies remain exclusive to `skills inspect <name>`.
  Probe diff: `skills list default` shrank from 30 889 B / 8 816 tok
  to 13 200 B / 3 772 tok.
- **`maestro_task_complete` accepts `reason` as an alias for `summary`.**
  CLI uses `--reason`; the MCP schema only accepted `summary`. Agents
  reading CLI docs and then calling the MCP tool tripped on the rename.
- **Session-fallback hint suppressed when stderr is piped or
  `MAESTRO_QUIET=1`.** The `[info] no agent session detected` message
  was firing on every mutating command for piped/scripted runs (the
  agent-loop dominant case). Now gated on `process.stderr.isTTY`.

### Doc

- `docs/token-budget.md` clarifies that `inspect token-budget` has no
  `--full` flag — the probe always exercises both modes automatically.

## 0.80.5 - omit dead fields from MCP responses (research-grounded null/empty pruning)

Token-budget pass two — apply the TOON-style "omit null and empty fields
entirely" pattern that Anthropic context-engineering and MCP community
guidance both call out as the highest-ROI cut on agent-facing responses.

### Changed

- **`maestro_policy_check`** omits `matchedRiskPolicyRow` entirely when no
  policy row matched. Previously emitted `"matchedRiskPolicyRow": null`,
  ~28 bytes of zero-information overhead on every successful call.
- **`maestro_contract_amend`** omits `skippedAddPaths` when the array is
  empty. Most amendments don't skip paths; emitting `"skippedAddPaths":[]`
  was pure noise.

Both are non-breaking: agents that already truthy-checked these fields
(`if (response.matchedRiskPolicyRow)`) work unchanged.

## 0.80.4 - skills list --full drops body (876 KB savings)

Doctrine-aligned regression: `skills list --full` was emitting every
SKILL.md body inline. The doctrine has always said "`skills list` exposes
summary records; `skills inspect <name>` reads the `body`", so the
verbose list path was silently violating it.

### Fixed

- **`skills list --full --json`** drops the `body` field. The new
  `--full` projection still includes `description`, `path`, `root`,
  `metadata`, `source`, `scope` — the bits the summary projection
  drops — but bodies are exclusive to `skills inspect <name>`.
- **`maestro inspect token-budget`** measures the difference:
  `skills list --full` shrank from 988 KB / 281 K tokens to 83 KB /
  24 K tokens — a 92 % cut on the worst-offending list verb.

### Added

- `SkillDetail` projection type (`Omit<SkillRecord, "body">`) and
  `detailSkill()` helper alongside the existing `summarizeSkill()`.

## 0.80.3 - token-budget doctrine: MCP success responses minified, no duplicate structuredContent

Token optimization sweep grounded in the doctrine doc and Anthropic guidance
on writing tools for agents.

### Changed

- **MCP success responses** drop the duplicate `structuredContent` field.
  Per the existing doctrine ("text content in `content[0].text` is the
  authoritative payload"), the typed copy was pure duplication. Removing it
  cuts response size for every successful MCP tool call.
- **MCP success responses** are minified (no `null, 2` pretty-print).
  Indentation is pure whitespace overhead for agent consumers.

### Removed

- `src/features/mcp/server/schemas/outputs.ts` — dead `outputSchema` Zod
  definitions that were never wired in.

### Doc

- `docs/token-budget.md` MCP section updated to reflect minified text and
  no-`structuredContent` convention.

## 0.80.2 - UAT round-2 follow-ups: token-budget probe binary, slug-aware claim/unclaim

Round-2 UAT against v0.80.1 confirmed all five round-1 fixes held, then
surfaced two new issues:

### Fixed

- **`maestro inspect token-budget`** now resolves its self-binary via
  `process.execPath` first, then falls back to `process.argv[0]` and only
  finally to `dist/maestro`. Previously the probe would silently fail (every
  row marked `[err]`, 0 bytes) when run from the installed binary without
  `MAESTRO_BIN` set, defeating the purpose of the regression guard.
- **`maestro task claim`** and **`maestro task unclaim`** now accept slugs
  in addition to task ids (parity with `task update`, `task show`). Previously
  these verbs would reject any slug with "task not found," forcing users to
  call `task list --json` to harvest the numeric id first.

## 0.80.1 - UAT-driven friction fixes: lean skills list, future-verb parity, install-hooks UX

Real-scenario greenfield + brownfield UAT (sub-agent first-time users) flagged
five concrete pain points on top of v0.80.0. All addressed in this release.

### Changed

- **`maestro skills list` is quiet by default.** Informational warnings
  (shadowed-skill notices, name/directory mismatches) no longer print to
  stderr unless `--verbose` is passed. Default text output is the clean skill
  table only; only `error`-level diagnostics surface without `--verbose`.
- **`maestro skills list --full` is now actually verbose.** Text output adds
  the first-sentence description and source label per skill (previously it
  was identical to default but paid ~293k tokens in JSON). JSON `--full`
  retains the full `SKILL.md` body.
- **`maestro setup --check`** no longer flags the `maestro qa`/`qa install`/
  `qa check`/`qa modalities` verbs from the bundled `maestro-qa` skill as
  binary drift. The skill declares them as `parity-skip-verbs` in its
  frontmatter; `checkSkillBinaryParity` honors that list.
- **`maestro setup --install-hooks`** now prints actionable guidance when no
  host runtime is detected instead of a single-line no-op message.
- **`maestro task update`** accepts `--receipt <text>` as an alias for
  `--summary <text>`, matching the conceptual model used in `task show`.
- **`maestro task create`** prints a one-line hint to run `maestro init`
  when the project has no `.maestro/config.yaml`.

### Internal

- New frontmatter field `parity-skip-verbs` for bundled SKILL.md files.
  Forward-looking documentation of future CLI verbs no longer trips drift
  checks.

## 0.80.0 - harness pivot: trust substrate, token-budget doctrine, lean MCP

Three PRs land in this release: #56 (CodeQL alerts), #57 (handoff MCP
tools), and #59 (the full harness pivot — phases 1–5, DI, setup, token-budget
doctrine, plus runtime observability).

### Added

- **`maestro task observe`**: DevObservabilityPort verb for surfacing task
  observation events to agents.
- **`maestro inspect token-budget`**: probe-based measurement of CLI/MCP
  output costs across the agent-facing verb catalog. Subprocess concurrency
  is bounded to 4 workers.
- **MCP `maestro_handoff_*` tools**: agents can `list`, `show`,
  `open_for_task`, and `pickup` handoff packets without shelling out.
- **Setup hardening**: `maestro setup --check`, `--self-test`, and
  `--install-hooks` flows. Hook installation writes a memo to
  `.<runtime>/maestro-hooks.md` (Maestro-owned markdown) instead of
  appending non-JSON content into the host runtime's `settings.json`.
- **Edge-case agent failure modes**: 5 distinct failure paths surfaced in
  the regression corpus under `tests/e2e/edge-cases/`.

### Changed

- **Token-budget doctrine**: agent-facing list verbs are lean by default;
  `--full` / `view: "full"` recovers the verbose shape. Touches `skills list`,
  `task list`, `task status`, `task ready`, `task stuck`, `mission list`,
  `evidence list`, `handoff list`. `inspect token-budget` baseline drops
  51,488 → 35,469 tokens (-31.1%) without losing any data — full payload
  remains available behind the opt-in flag.
- **Flat MCP error shape**: `{ code, message, hints?, arg? }` replaces the
  SDK's nested `InvalidParams` text. The interceptor reaches through the
  SDK's private `_requestHandlers` map and degrades to no-op if a future SDK
  version reshapes it.
- **Full DI pass**: every command surface in the codebase now follows the
  `*CommandDeps` factory pattern; the global composition root replaces the
  prior singleton with `createServices(...)`.
- **README** leads with "long-running agent harness"; "conductor" stays as
  the daily-use framing.

### Fixed

- **CodeQL alerts on `main`** (PR #56): glob-match length + wildcard caps;
  `escapeMarkdownBoundaries` covers the `--!>` HTML-comment terminator
  variant; insecure-temp-file paths use `mkdtemp`; redundant type guards
  dropped where types already exclude `null`/`undefined`; misleading
  indentation tidied; dead helpers and imports removed.
- **PR #59 review feedback** (gemini-code-assist): setup hook memo no
  longer corrupts host JSON config; MCP interceptor guards private SDK
  access defensively; architecture-lint `stripComments` now masks block
  and inline `//` (preserving offsets), and `extractFunctionBodies` counts
  braces over the comment-masked text; token-budget probes bounded to 4
  concurrent subprocesses.

### Internal

- **Type strictness**: union-type switches exhaustively handle all cases;
  every async function in `src/` declares an explicit `Promise<...>` return
  type (enforced by a new test).
- **Architecture lints** now mask block and inline `//` comments before
  matching, eliminating a class of false-positive violations.
- **`stripComments` rewritten** from `out += c` to pre-sized `string[]` +
  `join("")` to remove an O(n²) hot path in the lint runner.
- **Local `readFileSafe` helpers consolidated** onto the shared `readText`
  in `@/shared/lib/fs`.
- Net diff vs. v0.75.0: 358 files, +19,552 / -2,203 (mostly the pivot PR).

## 0.72.42 - parallelize verdict and spec store reads (same pattern, same win)

`FsVerdictStoreAdapter` and `FsSpecStoreAdapter` had the same
sequential per-file read pattern that `FsEvidenceStoreAdapter` did
before 0.72.41: one `for` loop awaiting each `readJson` in turn.
`verdict request` and `verdict show` exercise the verdict store on
every call, so the per-file linear walk added measurable wall-clock
once a task accumulated more than a handful of past verdicts.

### Fix

- **Parallelize per-file reads inside `FsVerdictStoreAdapter.readTaskVerdicts`**
  via `Promise.all` over candidate filenames after the directory scan.
- **Parallelize per-task reads inside `findByTreeSha`** so the
  CI-PR-by-tree-SHA lookup walks all task directories concurrently.
- **Parallelize per-spec reads inside `FsSpecStoreAdapter.list`** the
  same way.

### Measured impact

`verdict request --json` (warm cache): 0.13-0.17s → 0.09-0.10s.

## 0.72.41 - parallelize evidence file reads to keep listings flat as rows grow

`FsEvidenceStoreAdapter` stores one JSON file per row under
`.maestro/evidence/<task-id>/`, and `readTaskDir` was awaiting each
read sequentially. With dozens of rows on a single task, the linear
walk added up: at 30+ rows the per-call wall-clock started to scale
with row count even though the reads are I/O-independent.

A render-check screen-parallelism prototype was tested at the same
time and reverted — concurrent OpenTUI/React mounts contend on the
single JS thread and the whole thing got slower (1.0s → 1.5s).

### Fix

- **Parallelize per-file reads inside `readTaskDir`.** Reads now run
  via `Promise.all` over candidate filenames after the directory scan.
- **Parallelize per-task reads inside `list()`.** When no `task_id`
  filter is given, all tasks' rows are now fetched concurrently.

### Measured impact

| Variant | Before | After |
|---------|--------|-------|
| `evidence list` (1 row) | 0.13s | 0.13s |
| `evidence list` (33 rows) | n/a | 0.12-0.17s |
| `evidence list` (133 rows) | n/a | 0.14-0.20s |

Cold-start dominates short lists; parallel reads keep the per-row
cost flat instead of growing linearly.

## 0.72.40 - lazy-load OpenTUI inside mission-control, cut --json mode 86%

`maestro mission-control --json` is the agent-facing snapshot path —
agents read it to inspect mission state programmatically. It paid the
full OpenTUI + React import cost (~250-400ms) even though it never
renders anything; `renderDashboard`, `renderPreviewFrame`, and
`runRenderCheck` were eagerly imported at the top of
`src/infra/commands/mission-control.command.ts`.

### Fix

- **Defer OpenTUI imports to inside the rendering branches.**
  `mission-control --json` now skips the import entirely;
  `--render-check`, `--preview`, and the interactive dashboard
  dynamic-import on demand. Same import is shared across same-process
  invocations of multiple render branches (Bun caches modules).

### Measured impact

| Variant | Before | After |
|---------|--------|-------|
| `mission-control --json` | 1.04s | 0.15s |
| `mission-control --render-check` | 1.12s | 1.17s |
| `mission-control --preview --format plain` | 0.96s | 1.07s |

Render-check and preview retain their full cost because they actually
need the renderer; the small uptick is import-deferral overhead, well
under noise.

## 0.72.39 - cache HEAD sha + pending-loosenings per CLI run, halve verdict-request time

`verdict request` was spawning `git rev-parse HEAD` 4 times and the
pending-loosenings `git log` 3 times in a single invocation. Each
spawn costs 30-50ms, so ~300ms of pure subprocess overhead per call.
Tracing showed:

1. `getEffectiveRiskPolicy` / `getEffectiveAutopilotPolicy` /
   `getEffectiveReleasePolicy` each called `detectPendingLoosenings`
   independently — same git operations 3x.
2. `resolveHeadSha` and `resolveDefaultBase` were called from
   independent code paths (request-verdict.usecase.ts,
   task-verify.command.ts, policy-check.command.ts, merge-auto.command.ts)
   without any shared memoization.

### Fix

- **Memoize `detectPendingLooseningsImpl` per services-instance.**
  `effective-policy.usecase.ts` now caches the promise and shares it
  across all three `getEffective*Policy` calls. services is built once
  per CLI invocation, so this can't return stale data mid-flight.
- **Cache `resolveHeadSha` and `resolveDefaultBase` per (process, cwd).**
  `git-base.ts` now keeps a Map<cwd, Promise> for each. Tests that
  `process.chdir()` between cases get fresh entries automatically.

### Measured impact (relative to v0.72.38)

| Verb | v0.72.38 | v0.72.39 |
|------|----------|----------|
| `verdict request` | 0.37s | 0.18s |
| `task verify` | 0.36s | 0.15s |
| `policy check` | 0.36s | 0.16s |

Trace shows git invocations during `verdict request` dropped from
11 to 6.

## 0.72.38 - lazy-load mission-control, cut cold-start ~85% for non-TUI verbs

Cold-start profiling showed mission-control alone owned 252ms of the 522ms
total module-import cost (it pulls `@opentui/core`, `@opentui/react`,
`react`). Every other verb — `task verify`, `verdict request`,
`task ready`, `evidence record`, `--version`, `--help` — paid that cost
even though they never touch the TUI runtime. For autonomous agents
that fire 6+ verbs per recovery cycle, this added ~5 seconds of pure
overhead per loop.

### Fix

- **mission-control's registration deferred** until argv actually targets
  it. `src/index.ts` now scans argv before `program.parseAsync` and only
  dynamic-imports `@/infra/commands/mission-control.command.js` when the
  verb is `mission-control` or when help text is being rendered. Every
  other path skips the OpenTUI/React import graph.

### Measured impact

| Path | Before | After |
|------|--------|-------|
| `maestro --version` | 0.90s | 0.12s |
| `maestro task ready` | 0.98s | 0.20s |
| `maestro task proof` | 0.89s | 0.12s |
| `maestro task verify` | 1.03s | 0.36s |
| `maestro verdict request` | 1.05s | 0.37s |
| 6-verb agent chain | 5.92s | 0.89s |

`task verify` / `verdict request` retain their git-subprocess floor
(~0.3s); the lazy-load only addresses module-import cost.

`mission-control --render-check`, `mission-control --help`, and
`mission-control --preview` all behave identically — they pay the
253ms import once, as before.

## 0.72.37 - close R35 substrate bug: witness-level gate counted infrastructure rows

R35 sub-agent found a HIGH-severity bug: when effectiveRiskClass was
`high`, `verdict request` returned `HUMAN` with reason
`evidence-witness-level-insufficient` even when every contract
`doneWhen` criterion already had a passing `witnessed-by-maestro`
evidence row covering it. The cause: the witness-level filter in
`compute-risk.ts` examined every evidence row, including infrastructure
rows recorded at `agent-claimed-locally` by design — `plan-check`,
`review-ack`, `verifier` (unlinked trust-verifier findings),
`contract-amendment`, `verdict-override`, `cross-task-conflict`,
`runtime-signal`, `deploy-readiness`, `rollback-exercised`. Agents who
correctly ran the maestro-verify ritual still got blocked because
`task verify` itself wrote 2-4 verifier rows below the threshold.

### Fix

- **Witness-level gate now restricted to criterion-linked evidence
  kinds.** Added `isCriterionLinkedEvidence` predicate: a row counts
  toward the gate only if `kind` is one of `command`, `manual-note`,
  `ai-review`, `threat-model` AND its payload carries a non-empty
  `criterion_id`. Every other row (infra/audit/diagnostic) is excluded.
  The companion ProofMap path is unchanged — coverage is still proven
  the same way; only the witness-strength gate stops mistaking infra
  rows for criterion evidence.

## 0.72.36 - close R34 substrate bugs: L1/L2 amend confusion + version drift between package.json and version.ts

R34 sub-agent found two substrate bugs that wasted autonomous-recovery
budget:

1. **MEDIUM:** `maestro task contract amend <ref> --add-path <path>`
   produced an unhelpful `unknown option '--add-path'` error. Agents
   following the documented `maestro-verify` ritual confused the L1
   editor-based `task contract amend <ref>` (full-YAML replace via
   `--from`/`--editor`) with the L2 path-scoped `contract amend --task
   <id>` (incremental `--add-path`/`--remove-path`). The error message
   gave no clue that a different verb existed.
2. **LOW:** `package.json` and `src/shared/version.ts` could drift out
   of sync. If someone hand-edited `package.json` instead of running
   `bun scripts/bump.ts`, the build would compile a binary that
   reported the stale version embedded in `version.ts`. The R33 release
   shipped with `package.json=0.72.35` but `version.ts=0.72.33`.

### Fixes

- **L1 `task contract amend` now redirects to L2 when path-scoped flags
  are used.** Detects `--task`, `--add-path`, or `--remove-path` and
  throws a `MaestroError` with the exact `maestro contract amend
  --task <id> --add-path <path>` invocation pre-filled with the user's
  values. The verb signature changed from `amend <ref>` to `amend
  [ref]` so misuse no longer trips commander's argument validation
  before the redirect can fire; manual `--reason` and `<ref>`
  validation now lives in the action handler with redirect-friendly
  hints.
- **`scripts/build.ts` now refuses to build when `package.json` and
  `src/shared/version.ts` disagree.** Reads both files before invoking
  Bun's compiler and exits 1 with a redirect to `bun scripts/bump.ts
  patch` if the versions don't match. Eliminates the class of release
  binaries that report a stale version.

## 0.72.35 - close R33 substrate bug: evidence record accepted unknown criterion ids

R33 sub-agent found that `maestro evidence record --criterion <id>` had
no validation when the task didn't reference a Mission Spec. An agent
could record evidence with `--criterion dw-zzzzzz` (an id not in the
contract, or even a malformed id) and the row would be silently
accepted, then ignored by ProofMap. Result: agents couldn't tell their
evidence was orphaned until they ran `task proof` and saw the criterion
they thought they covered was still uncovered.

### Fix

- **`evidence record --criterion <id>` now validates against contract
  `doneWhen` ids when no Spec is linked.** Previously the validation
  branch only fired for tasks with `missionId` and a Spec. Added a
  fallback branch that loads the current contract via
  `readCurrentContractWithBackfill` and rejects unknown ids with the
  same "Unknown criterion id" error and the available ids listed in the
  hint. Tasks without a contract or with an empty `doneWhen` still
  accept any `--criterion` value (no contract context to validate
  against).

## 0.72.34 - close R32 substrate bug: user-supplied doneWhen criterion IDs were silently regenerated

R32 sub-agent found a CRITICAL bug: user-supplied `doneWhen[].id` values
in a contract draft YAML (e.g. `dw-aaaaaa`) passed schema validation but
were thrown away during contract create. Locking issued fresh
auto-generated IDs (`dw-20e179`, `dw-c5436b`). Agents who recorded
evidence with `--criterion <user-id>` ended up with orphaned evidence
because the criterion no longer existed under that ID, and `task proof`
showed 0% coverage even though every criterion had evidence.

### Fixes

- **`task contract new` now preserves user-supplied `doneWhen[].id`
  values when present and only generates IDs when absent.** Previously
  `proposeContract` mapped `input.doneWhen` and called
  `generateDoneWhenId()` unconditionally for every criterion, throwing
  away validated user input. Now uses `criterion.id ?? generateDoneWhenId()`
  to match the existing `normalizeAmendedCriteria` behavior used during
  amendments.
- **Duplicate `doneWhen[].id` in the same draft is now rejected.** Added
  a unique-id check in `proposeContract` before contract creation. Error:
  "Duplicate doneWhen.id in draft: dw-xxxxxx" with a hint to omit the id
  if uniqueness can't be guaranteed.

## 0.72.33 - close R31 substrate bugs: invisible draft contract, witness flag ignored, silent skipped paths, opaque dw-id error

R31 sub-agent surfaced four follow-up bugs against the R30 ship: a draft
contract was indistinguishable from "no contract" across `task verify`,
`verdict request`, and `plan check`, sending agents to create a duplicate
contract; `evidence record --kind command` hardcoded the witness level
and silently ignored `--witness`; mixed amendments (some redundant, some
new) succeeded without reporting which paths were skipped; and the
`doneWhen[].id` validation error said "must look like dw-xxxxxx" without
explaining the hex constraint.

### Fixes

- **Draft contracts now produce a clear "lock it first" error from
  `task verify`, `verdict request`, and `plan check`.** Previously each
  verb used `readCurrentContractWithBackfill`, which returns `undefined`
  for both "no contract" AND "draft only", so the error read "No contract
  proposed" and pointed agents to `maestro task contract new` — but a
  draft already existed, so creating another would fail. Added
  `readDraftContract(legacyStore, taskId)` and call it as a fallback in
  all three commands; when a draft exists, the error becomes
  "Contract <id> for task <id> is in draft status — lock it first" with
  the exact lock command in the hint.
- **`evidence record --kind command` now honors `--witness <level>`.**
  Previously the command-kind path hardcoded
  `"agent-claimed-locally" satisfies WitnessLevel`, ignoring `--witness`
  even though the flag was registered. Replaced with
  `parseWitnessLevel(opts.witness, "agent-claimed-locally")` to match the
  pattern already used by `--kind ai-review` and `--kind threat-model`.
  Also applied the same fix to `--kind manual-note`.
- **`contract amend` now reports paths that were silently skipped as
  already-covered.** Previously a mixed amendment (one new path + one
  redundant path) would succeed, consume budget, and say nothing about
  the skip. Now `applyPathChangesWithReport` collects skipped entries
  and the success output adds a `Skipped (already covered): <paths>`
  line. JSON output includes a `skippedAddPaths` field.
- **`doneWhen[].id` validation error now explains the hex constraint.**
  Previously said "must look like dw-xxxxxx" — agents couldn't tell that
  `x` meant lowercase hex. New message: "must be 'dw-' followed by
  exactly 6 lowercase hex chars (0-9, a-f), e.g. dw-a1b2c3" with a hint
  that omitting the id lets maestro generate one.

## 0.72.32 - close R30 substrate bugs: redundant amendment budget leak, ProofMap contract criteria gap, untracked-warning noise

R30 sub-agent surfaced four follow-up bugs against the R29 ship: redundant
path amendments still consumed amendment budget even though the path was
correctly skipped; the budget-exhaustion error message suggested an action
agents couldn't take; `task proof` ignored contract `doneWhen` criteria
when no Spec was linked; and `task verify`'s untracked-out-of-scope warning
listed maestro runtime metadata files (verdicts, contracts, evidence, runs)
that are gitignored and irrelevant to scope review.

### Fixes

- **`contract amend` no longer consumes budget when the proposed change is
  a no-op against existing scope.** R29 added skipping of paths already
  covered by globs in `applyPathChanges`, but the amendment still wrote a
  versioned record and incremented version, wasting budget. Now compares
  `before.scope.filesExpected` to the result of `applyPathChanges` and
  exits early with `MaestroError("No scope changes to apply")` when they
  match. Budget stays intact for legitimate amendments.
- **Budget-exhaustion error message now suggests reachable recovery
  commands.** Previously suggested "Increase amendmentBudget.maxAmendments
  on the contract" — but no CLI verb edits a locked contract's budget,
  leaving agents stuck. Now suggests `git checkout <base> -- <path>` to
  revert specific files OR `maestro task contract reopen <taskId>` to
  reopen the draft and re-lock with a higher budget.
- **`task proof` (ProofMap) now uses contract `doneWhen` criteria when no
  Spec is linked.** `buildProofMap` previously bailed out with
  `uncoveredCount: 0` when `args.spec` was undefined, hiding contract
  criteria from coverage analysis. Agents could record evidence with
  `--criterion` pointing to a `doneWhen.id`, but `task proof` would
  silently ignore it. Now falls back to `contract.doneWhen` when the spec
  is absent, joining evidence rows against contract criteria the same way
  it does Spec criteria. Output labels the source: `(spec, ...)` vs
  `(contract, ...)`.
- **`task verify` no longer warns about gitignored maestro runtime
  metadata as untracked out-of-scope.** `collectUntrackedFiles` returned
  raw `git ls-files --others --exclude-standard` output without applying
  the same `filterTouchedPaths` filter used by `collectTouchedFiles`.
  Files like `.maestro/contracts/<task>/v*.json`, `.maestro/tasks/NOW.md`,
  `.maestro/tasks/continuations/`, and `.maestro/tasks/contracts/` leaked
  into the untracked-out-of-scope warning even though they're maestro's
  own state. Now applies `filterTouchedPaths(output, "untracked")` so
  warnings only surface real working-tree clutter agents need to
  address.

## 0.72.31 - close R29 substrate bugs: no-op amendments, redundant paths, plan-check contract criteria gap

R29 sub-agent surfaced three bugs that blocked autonomous recovery: no-op
amendments consuming budget without making changes, redundant path
amendments wasting budget on paths already covered by globs, and
plan-check failing to validate contract criteria (only checking Spec
criteria), violating the documented pre-claim ritual.

### Fixes

- **`contract amend` now rejects no-op calls that specify neither
  `--add-path` nor `--remove-path`.** Previously calling `contract amend`
  with only `--reason` would succeed, increment the version counter,
  consume amendment budget, and record an amendment entry, but make no
  actual changes to the contract scope. Agents could accidentally exhaust
  the amendment budget without making any meaningful changes, blocking
  recovery when they later needed to legitimately amend. Now exits with
  clear error: "At least one of --add-path or --remove-path is required."
- **`contract amend` now skips paths already covered by existing glob
  patterns.** Previously adding a path like `src/features/logging/index.ts`
  when `src/features/logging/**` was already in scope would consume
  amendment budget and create a duplicate entry. Now checks if the path
  being added is already matched by existing `filesExpected` patterns and
  silently skips the addition (but still records the amendment for audit
  purposes). Prevents agents from wasting amendment budget on redundant
  additions.
- **`plan check` now validates all contract `doneWhen` criteria, not just
  Spec acceptance criteria.** Previously the `missing-proof` check only
  iterated over `spec.acceptance_criteria`, ignoring contract criteria
  entirely. This violated the documented workflow in `maestro-verify/SKILL.md`
  which states plan-check will catch missing proof strategies. Agents
  following the documented pre-claim ritual would get a clean plan-check,
  proceed to implementation, and only discover missing contract criteria at
  verdict-request time (after wasting work). Now checks both Spec and
  contract criteria, emitting a `missing-proof` finding for any criterion
  missing from the plan's `proofSet`.
- **`task verify` error message for missing contract now suggests correct
  recovery command.** Previously suggested `maestro contract amend` (which
  only works on locked contracts). Now suggests `maestro task contract new
  <taskId>` for tasks with no contract.

## 0.72.30 - close R28 substrate bugs: scope divergence, kind enum docs

R28 sub-agent surfaced two issues: a critical scope divergence where
`task verify` checked committed changes only while `task close` checked
committed + untracked, causing untracked out-of-scope files to pass
verification but block completion with no warning; and a docs gap where
the default contract template didn't explain the `kind` enum for
`doneWhen` criteria.

### Fixes

- **`task verify` now warns about untracked out-of-scope files before
  they block completion.** Previously `task verify` ran the Trust
  Verifier against `collectChangedPaths` (committed changes only) while
  `task close` ran `collectTouchedFiles` (committed + staged +
  untracked). Result: untracked files outside `scope.filesExpected`
  passed verification but blocked completion with a cryptic "contract
  broken" verdict. `task verify` now collects untracked files via
  `collectUntrackedFiles`, filters them against `scope.filesExpected`,
  and emits a warn-level `untracked-out-of-scope` finding when any are
  present. The warning message explicitly states the files will block
  completion if not committed or removed. Exit code 2 (warn) signals
  the agent to act before attempting `task close`.
- **Default contract template now documents the `kind` enum for
  `doneWhen` criteria.** Added inline comment explaining `manual`
  (human verification) vs `receipt-hint` (auto-tick from `--verified-by`
  tags at completion). Agents no longer need to guess or infer from
  examples.

## 0.72.29 - close R27 substrate bugs: TTY hang, risk divergence, criteria budget, amend path, draft strictness, broken wording

R27 sub-agent surfaced six seam bugs that left an autonomous agent
without a working recovery path: a silent editor hang, divergent risk
class between `policy check` and `verdict request`, criteria-mark
operations consuming amendmentBudget so structural amends had nowhere
left to land, the trust verifier suggesting a non-existent amend path,
half-initialized contracts produced from typo'd YAML keys, and a
"broken" verdict word with no inline reason. Together those last three
made scope-creep recovery impossible — the user couldn't even identify
why they were broken, much less fix it.

### Fixes

- **`task contract new` no longer hangs silently when $EDITOR is set
  but stdin is not a TTY.** Background processes inherit $EDITOR from
  their parent shell; without a controlling terminal the editor blocks
  on a stdin that will never produce input. The verb now refuses with
  a clear message pointing at `--from <path>`, stdin piping, or
  `--editor <cmd>` for non-blocking commands. Surfaced when
  agents/CI ran the verb in a worktree with $EDITOR exported.
- **`policy check` now resolves the same base ref as `verdict request`.**
  Previously `policy check` fell back to `resolveDefaultBase()` (which
  in greenfield repos returns the empty-tree SHA) while `verdict request`
  used `contract.claimedAtCommit ?? resolveDefaultBase()`. Result:
  `policy check` counted every committed file as "modified" and reported
  `effectiveRisk: high` while `verdict request` saw a clean diff and
  reported `medium`. Both now agree.
- **`task contract criteria mark` no longer consumes from
  `amendmentBudget`.** Marking a criterion met or unmet is workflow
  progress, not a structural contract change. Per-criterion `metAt` /
  `metBy` / `metEvidence` fields already record the audit trail. The
  amendments[] log now contains structural changes only (add/remove
  criterion, replace, scope amend). Three criteria marks no longer
  exhaust a 2-amendment budget before any real amend can land.
- **`task verify` fix-forward hint points at the working amend path.**
  Previously the printer suggested `maestro contract amend tsk-XXX
  --reason ...` (positional ref form) which does not accept `--task`,
  while `maestro task contract amend` doesn't accept `--task` at all.
  Hint now uses the canonical `maestro contract amend --task <id>
  --add-path <path> --reason "<why>"` form documented in the skill.
- **Unknown contract draft keys reject by default.** A typo such as
  `expectedPaths` (vs `filesExpected`) used to print a stderr warning
  and proceed with an empty `filesExpected[]`, producing a
  half-initialized contract that immediately failed every scope check.
  The verb now exits non-zero with every offending key listed in one
  message; pass `--allow-unknown-keys` to keep the previous
  warn-and-continue behavior. The bootstrap default contract template
  now also documents `amendmentBudget` and `costBudget` as commented
  examples.
- **Contract verdict "broken" now names the structural reasons.**
  Output previously read `Verdict: broken` alongside `Done when: 3/3
  met`, which scanned as nonsense — broken means scope/forbidden/cap
  violation. Now reads `Verdict: broken — out-of-scope files: 2,
  forbidden files: 0, unmet criteria: 0` so the reader doesn't have
  to scroll for the explanation.

## 0.72.28 - close R25 + R26 seam bugs in handoff, contract drafts, spec edit, deploy gate, task budget

R25 and R26 sub-agent passes surfaced two handoff seam bugs and four
agent-facing UX gaps that left autonomous agents stuck without an
obvious next action.

### Fixes

- **`handoff pickup --standalone` no longer poisons the source workspace.**
  Cross-project guard now runs before the `--standalone` fast-path so a
  foreign workspace cannot consume a task-linked packet by claiming it
  only wants prompt-only consumption. Marking `consumedAt` on a
  task-linked packet from the wrong workspace blocked the rightful
  workspace from ever picking it up. Standalone (task-less) foreign
  packets continue to work as before.
- **`handoff create` refuses to stack a second open task-linked packet.**
  Previously `launchHandoff` accepted multiple open packets for the same
  taskId, leading to ambiguous pickup and ownership races. New guard
  surfaces the existing packet's id and the exact pickup / show commands.
  Standalone packets without a taskId still stack since they don't claim
  a task.
- **`task contract new` now accepts `costBudget` in draft templates.**
  Previously `costBudget.maxRetries` and friends were silently dropped at
  the unknown-keys filter, making the L4 BLOCK-via-budget path
  unreachable through the documented CLI flow. Schema mirrors
  `amendmentBudget`: maxRetries (required positive int),
  maxWallClockSeconds (required positive int), maxTokens (optional
  positive int), with unknown-key warnings.
- **`spec edit --mission <fake>` no longer creates orphan spec files.**
  The verb now looks up the mission first and refuses with a clear next
  step (`mission list` / `mission new`) when the mission does not exist.
- **`deploy gate` failing checks now print inline recovery hints.**
  Previously each failing check printed bare `fail` with no path back to
  green. New output points feature_flag and canary_plan at
  `maestro spec edit`, rollback at `maestro deploy rollback`, and owner
  at `.maestro/policies/owners.yaml`.
- **`task budget` cleanly distinguishes "no costBudget set" from real
  limits.** Previous text mode showed `Retries: 0/0` and
  `Wall clock: 0s/0s` for contracts with no `costBudget`, which read as
  "0 retries allowed, exhausted." New output prints `(no limit)` and a
  YAML snippet for setting one; JSON adds `hasBudget` and omits the
  `max*` fields entirely when none is set.

## 0.72.27 - tell agents what to do next after `review ack` and cost-budget BLOCK

R24's two remaining UX gaps. Both surface "what next?" hints that
were absent before and that a sub-agent could not deduce from the
existing output.

### Fixes

- **`review ack` now tells agents the next step is `merge auto`,
  not re-running `verdict request`.** The ack is consumed by the
  auto-merge eligibility predicate, not the verdict decision tree.
  Without this hint an agent that just acknowledged review criteria
  re-runs `verdict request`, gets HUMAN again, and stalls.
- **`cost-budget-exhausted` BLOCK reason now lists recovery paths
  inline.** Previously the reason text was just "Cost budget
  exhausted; further execution blocked." with no suggestion of how
  to un-block. New text names `maestro task budget` for inspection,
  `maestro contract amend` for raising the cap, and
  `maestro handoff create` for human escalation.

## 0.72.26 - surface trust-fail paths and tailor empty-tree owners hint (R24 sweep)

R24 sub-agent walked the verdict BLOCK / HUMAN paths end-to-end and
flagged two real ergonomic gaps and one false positive.

### Fixes

- **Trust-FAIL verdicts now list the offending paths inline.** Previously
  a `verdict request` returning FAIL on a scope violation said only
  "Trust verifier found 1 error(s)" with `findingChecks: ["scope"]`,
  forcing the agent to re-run `maestro task verify` to learn which
  files were out of scope. The verdict reason now carries
  `findingPaths` (de-duplicated, sorted across all error findings)
  and the human printer dents them under the reason. Agents can
  self-correct from the verdict output alone.
- **`verdict override` and other `--base`-aware verbs now produce a
  tailored hint** when the resolved base is the empty-tree SHA
  (greenfield repo with no upstream / main / master / trunk merge-base).
  Previous behavior surfaced
  `owners.yaml not found at 4b825dc...:.maestro/policies/owners.yaml`
  with a "run maestro init" suggestion that didn't help. New error
  names the empty-tree case directly and tells the user to pass
  `--base <commit-or-ref>` explicitly.

### Investigated, not a bug

R24 also reported `verdict request --json` exiting 0 regardless of
decision. Could not reproduce: both `--json` and plain output exit
0 / 1 / 2 / 3 for PASS / FAIL / HUMAN / BLOCK in v0.72.25. The
`process.exit(exitCodeForDecision(...))` is unconditional after the
JSON branch (`src/features/verdict/commands/verdict.command.ts:171`).
Likely a shell-piping artifact in the sub-agent's environment.

## 0.72.25 - harden contract draft + evidence file readers (R22 sweep)

R22 sub-agent swept every `--from` / `--file` / `--findings` flag for
the same crash classes plan-check fixed. Found four crash paths and
two destructive silent-success paths.

### Fixes

- **`task contract new --from` and `task contract amend --from`** now
  emit a clean `[!] Contract draft path is a directory` error on
  EISDIR instead of dumping a raw Bun stack. Other read errors get
  a `[!] Cannot read contract draft: <path>` wrapper.
- **Empty contract drafts now hard-fail** instead of silently writing
  an all-empty contract (which previously wiped intent / scope /
  doneWhen on amend). `[!] Contract draft is empty` and `[!] Contract
  draft has no fields` cover the whitespace-only and parsed-empty
  cases respectively. This was the highest-impact R22 finding —
  destructive without warning.
- **`evidence record --threat-model-file` and `evidence record
  --findings`** now wrap the same EISDIR class with a clean
  `--threat-model-file: path is a directory` / `--findings: path is
  a directory` error. The YAML parse error path was already clean.

R22 also flagged `policy check` silently swallowing a malformed
`owners.yaml`. Confirmed: `policy check` doesn't actually load
owners — that file is consumed by `verdict request`, `ci verify`,
and `deploy gate`, where `parseOwners` already throws a clean
MaestroError. No fix needed.

## 0.72.24 - plan-check wraps directory-path and YAML-parse errors

R21 follow-up sub-agent (validating the v0.72.23 fixes) confirmed the
schema-validation path was clean but found two more unhandled crash
paths in the same command:

1. `--plan-file <directory>` produced a raw Bun `EISDIR` error and
   exit 1 — same crash class as the original R20 bug, just at the
   read-text step instead of the schema step.
2. A plan file containing structurally invalid YAML produced a raw
   `YAMLParseError` Bun stack trace from inside the `yaml` library.

### Fixes

- **`maestro plan check` now wraps `readText` and `parseYaml`** in
  try/catch and re-throws `MaestroError` for the EISDIR and
  YAMLParseError branches. The YAML branch surfaces the parse
  position (line/col) when the underlying library exposes it.
  Directory paths get a one-line hint pointing at the user's
  mistake; YAML errors get the original parser message plus a
  fix-and-rerun hint.

## 0.72.23 - plan-check rejects malformed plan files with field-level hints

R20 sub-agent (plan-check failure ergonomics scenario) found two
agent-facing rough edges:

1. A plan file missing `intendedFiles` crashed `maestro plan check`
   with a raw `TypeError: undefined is not an object` and exit 1.
   No user-facing error, no hint to the agent.
2. `plan check --help` only said the plan file is "JSON or YAML"
   without listing required fields. Agents had to source-dive or
   guess the shape (`PlanInput` lives in `src/features/plan/domain/`).

### Fixes

- **`maestro plan check` now validates the parsed plan file** via a
  zod schema in `src/features/plan/domain/plan-validators.ts` before
  calling the use-case. Invalid payloads throw a `MaestroError` that
  enumerates every missing/invalid field plus a hint block showing
  the canonical YAML shape. No more raw stack traces.
- **`maestro plan check --help` now embeds the plan-file schema**
  via `addHelpText("after", ...)`: required vs optional fields, an
  inline example, and a one-line description per check. The
  description also notes that `missing-proof` only fires when the
  contract is linked to a mission spec — a discoverability gap R20
  surfaced.

## 0.72.21 - generalize argv guard to catch any `<subcommand> --version <value>`

While shipping v0.72.20 a quick audit found `maestro task update --task
<id> --version 1` had the same silent-print-binary-version trap.
Verifying the source confirmed no subcommand declares `--version` as
an option, so any verb-prefix + `--version <value>` is always a user
mistake that would otherwise hit Commander's global handler.

### Fixes

- **`assertNoDeprecatedVersionFlag` now ends with a catch-all** that
  errors on any non-empty subcommand prefix followed by `--version
  <value>`, with a generic redirect to `<verb> --help`. The three
  verb-specific branches still fire first for richer redirects
  (`contract show`, `verdict show`, `task contract show`, `update`);
  the catch-all covers the rest. Bare root `maestro --version` still
  works (empty positional prefix).

## 0.72.20 - extend deprecation guard to L1 `task contract show --version`

Round-18 sub-agent flagged a remaining gap in the v0.72.19 guard. The
top-level `maestro contract show --task <id> --version <n>` (L2 verb)
correctly errored with a redirect to `--at-version`, but the aliased L1
path `maestro task contract show --task <id> --version <n>` silently
hit Commander's global `--version` handler and printed the binary
version with exit 0 — the same trap the v0.72.19 guard was meant to
close.

### Fixes

- **`task contract show --task <id> --version <n>` now errors** with
  a redirect that points at the L2 verb's correct invocation
  (`maestro contract show --task <id> --at-version <n>`) and clarifies
  that the L1 viewer takes a positional `<ref>` instead of flags.
  Same `assertNoDeprecatedVersionFlag` helper, one new branch, one new
  unit test. The L1 viewer never accepted `--version` — users typing
  it almost certainly meant the L2 versioned viewer.

## 0.72.19 - task verify prints recovery hints; deprecated --version flag now errors loudly

Round-17 ran a mixed-recovery-plus-amend explorer agent against v0.72.18.
The flow worked end-to-end but two UX gaps surfaced.

### Fixes

- **`maestro task verify` now prints fix-forward recovery hints when
  scope errors are present.** Previously only the close-flow's broken-
  contract printer emitted hints; the standalone verifier just listed
  findings, leaving agents to infer the revert/amend pattern themselves.
  The verifier now mirrors the close-flow printer: for out-of-scope
  paths it offers EITHER a per-file `git checkout <lock-sha> -- <path>
  2>/dev/null || git rm -f <path>` revert OR the `task contract amend`
  expand-scope path; for forbidden-touched paths it offers the revert
  only (forbidden paths cannot be amended). JSON output is unchanged
  (hints stay on stdout text path) so machine consumers see only the
  finding list.

- **`<subcommand> --version <value>` now errors loudly with a redirect
  to the new flag.** v0.72.18 renamed `verdict show --version`,
  `contract show --version`, and `update --version` to `--at-version`
  / `--release` to fix a Commander root-flag collision, but the old
  invocation continued to silently print the binary version and exit
  0 (Commander's global handler still wins). Agents migrating from
  the old flag had no signal. A pre-Commander argv check now detects
  the three known patterns and throws a `MaestroError` with the new
  flag name, so the failure is visible. The argv match scans only
  positional words in the prefix (skipping option flags and their
  values, both `--foo bar` and `--foo=bar` forms) so it correctly
  fires when subcommand options sit between the verb and `--version`,
  e.g. `contract show --task tsk-abc --version 1`. Extracted into
  `src/shared/lib/deprecated-version-flag.ts` with direct unit
  coverage (8 cases).



Round-16 ran a verdict-mid-amend explorer agent against v0.72.17. The
flow worked end-to-end (verdict bound to contract version, history
correctly versioned, evidence correctly attributed) but three real seam
bugs surfaced.

### Fixes

- **`--version <id>` flag on `verdict show`, `contract show`, and
  `update` now uses `--at-version` / `--release` instead.** The previous
  flag name collided with Commander's global `--version`, which is
  registered on the root program by `.version(...)`. As a result,
  `maestro verdict show --task <id> --version <verdictId>` silently
  printed the binary version string and exited 0 — the documented flow
  in AGENTS.md was unusable as written. Renamed to `--at-version` for
  the show verbs (`verdict show --at-version <id>`,
  `contract show --task <id> --at-version <n>`) and to `--release` for
  `maestro update --release <version>`. All three documented flows now
  work as intended; AGENTS.md and the `maestro-verify` skill updated.

- **`maestro task verify` now exits 0 for info-only findings.** Previously
  any non-empty findings list returned exit 2, even when every finding
  was severity `info` (e.g. unsigned commits). An agent reading exit 2
  would think verify failed when the report was purely informational.
  New behavior: exit 0 (no error/warn), 1 (any error), or 2 (any warn,
  no error). Info-only is success.

- **L1 amend (full-replace) now preserves criterion ids when the text
  is unchanged.** Previously, an amendment YAML that re-stated the same
  doneWhen text without explicit ids regenerated fresh ids for every
  criterion, silently breaking any caller tracking criteria by id across
  versions. The normalize logic now falls back to text-matching against
  the current criteria when the input doesn't carry an id, so a no-op
  re-statement keeps the existing id.



Round-15 ran a new batch of minimal-prompt sub-agents against v0.72.16.
The L1-amend scenario surfaced an asymmetric-gate bug: the L2 path
(`maestro contract amend --add-path`) correctly enforced the contract's
`amendmentBudget`, but the L1 path (`maestro task contract amend
<ref> --reason --from <yaml>`) did not consult it at all. Same contract,
same budget field, two different answers depending on which verb the
agent reached for.

### Fixes

- **L1 `maestro task contract amend` now consumes from
  `amendmentBudget`.** A new `enforceAmendmentBudget` helper runs before
  every L1 drift op (full-replace via `replace`, criterion add, criterion
  remove) and rejects the amend with a `MaestroError` plus a
  `contract-amendment-blocked` Evidence row at `witnessed-by-maestro` if
  any of the three budget gates fail: `maxAmendments` exhausted,
  `maxPathsPerAmendment` exceeded by net-new added paths, or any added
  path matches `forbiddenAmendmentPaths`. `markCriterion` is exempt by
  design — it is metadata-only (mark a manual criterion met after work
  is done) and gating it would break the standard close-flow UX. The L2
  amend path is unchanged; the gate now matches across both verbs.



Round-13 ran another batch of minimal-prompt sub-agents against v0.72.15.
The forbidden-file scenario surfaced one real seam bug; the
amendment-budget scenario reported an exit-code-0 false alarm caused by
the agent's own `2>&1 | tail -10` shell pattern (the pipe masks the
upstream exit code; direct invocation correctly returns exit 1 on budget
exhaustion).

### Fixes

- **Broken-contract recovery hint now handles files newly added after
  the lock-time commit.** Pre-fix, the printer emitted
  `git checkout <lock-sha> -- <file>` for every out-of-scope or
  forbidden path. That works for files that existed at the lock-time
  commit, but `git checkout <sha> -- <new-file>` fails with
  `error: pathspec '<new-file>' did not match any file(s) known to git`
  when the file was created after the lock. R13's forbidden-file agent
  hit exactly this: a `tests/x.test.ts` introduced after lock could not
  be reverted via the printed command and they had to improvise
  `git rm`. The recovery printer now emits one line per file in the
  form `git checkout <lock-sha> -- <file> 2>/dev/null || git rm -f <file>`
  — the `||` chains the new-file path so a verbatim copy-paste reverts
  pre-existing files (via checkout) and removes newly-added files
  (via rm). Same change applies to both the out-of-scope and
  forbidden-touched branches.

## 0.72.15 - close three trust-substrate gaps surfaced by round-12 minimal-prompt scenarios

Round-12 ran five minimal-prompt sub-agents against v0.72.14 against scenarios
the prior rounds had not exercised (reopen → re-complete loop, forbidden
file touched, amendment-budget exhaustion, empty diff, stale-claim reclaim).
Three real seam bugs surfaced; one was a wash; one was filed as a known
limitation that needs a broader fix.

### Fixes

- **Broken-contract recovery now uses the lock-time commit for revert
  hints.** When a contract closed `broken` because of out-of-scope or
  forbidden files, the recovery printer told the agent
  `git checkout HEAD -- <file>`. But by the time the broken close
  happens, HEAD is the commit containing the bad change — so the
  printed command is a no-op. The hint now uses
  `contract.claimedAtCommit` (the lock-time SHA, captured by
  `task contract lock`), falling back to `HEAD~1` when the field is
  missing. Always-correct regardless of how many commits the agent
  layered after the lock.
- **Broken-contract recovery now includes the required `--reason` flag.**
  The final command in the fix-forward sequence —
  `maestro task update --task <id> --status completed` — was missing
  the mandatory `--reason "<one-line outcome>"` flag, so an agent
  following the printout verbatim would error out at the last step.
  The recovery output now prints
  `... --status completed --reason "<one-line outcome>"`.
- **`amendmentBudget` is now parsed from `task contract new --from
  <yaml>`.** The contract draft YAML parser only knew three top-level
  keys (`intent`, `scope`, `doneWhen`). `amendmentBudget` was silently
  dropped with a `[!] Ignoring unknown contract draft key` warning —
  even though the Contract record schema and the
  `amend-contract.usecase.ts` enforcement logic both supported the
  field. Result: budgets declared in YAML were never stored, and every
  `contract amend` succeeded regardless of `maxAmendments`. The parser
  now recognises `amendmentBudget`, validates the inner shape
  (`maxAmendments`, `maxPathsPerAmendment`, `forbiddenAmendmentPaths`),
  fills sensible defaults (3 / 5 / []) for unspecified inner keys, and
  threads the budget through `services.contracts.draft` →
  `CreateContractInput` → `contractStore.create`. Subsequent
  `contract amend` calls now hit the existing budget gate and the
  second amendment fails with
  `[!] Amendment budget exhausted for task <id>: 1 of 1 amendments used`
  plus a `contract-amendment-blocked` Evidence row.

### Known limitation (not fixed in this release)

- **Stale-claim reclaim semantics are coarse.** Round-12 scenario 5
  observed that `maestro task claim <id>` on an `in_progress` task
  succeeds silently (no warning, no `--force` required) because
  `MAESTRO_SESSION_ID` does not propagate as an actor identity — the
  CLI synthesises a per-user session and emits `[info] no agent
  session detected`. Both "agents" therefore resolve to the same
  identity, no ownership transfer is recorded, and the bundled
  `maestro-task` skill defers reclaim flow explanation to a
  `reference/recovery.md` that is not in the install bundle. Filed
  for a future release that will tighten reclaim semantics and ship
  the missing reference doc.

### Regression coverage

- contract-workflows.usecase.test.ts: new test for amendmentBudget
  passthrough in `draft({...})` (covers the YAML→record→store path
  end-to-end).

## 0.72.14 - close five trust-substrate seam bugs found by round-9 + round-10 minimal-prompt scenarios

Round-9 surfaced two close-time UX gaps (recovery output too narrow,
cap counting included substrate). Round-10 ran five minimal-prompt
scenarios in parallel and surfaced three more — none caught by existing
unit/integration tests because each lived in a seam between two stores,
two policies, or two diff bases:

### Fixes

- **Recovery output covers every `broken` reason, not just unmet manual
  criteria.** v0.72.13 only printed the fix-forward sequence when a
  contract closed `broken` because of unmet `kind: manual` rows. Round-9
  surfaced two more variants (out-of-scope edits, cap exceeded) where
  the recovery message regressed to "Inspect: maestro contract show"
  with no actionable verbs. The recovery printer now enumerates ALL
  active `broken` reasons (out-of-scope files, forbidden touches, cap
  exceeded, unmet manual criteria, unmet receipt-hint criteria) and
  prints the appropriate `git checkout HEAD --` / `contract amend`
  / `criteria mark` / `--verified-by` follow-up per reason.
- **Cap counting now excludes `.maestro/` substrate metadata.** The
  `maxFilesTouched` cap counted every file in the diff, including
  `.maestro/` substrate written by the CLI itself (run-state, evidence,
  contract index updates). A user with `maxFilesTouched: 3` could
  legitimately edit two source files and still trip the cap because the
  CLI's own substrate writes pushed the count over. v0.72.12 already
  exempted substrate from the scope/forbidden checks via
  `isMaestroSubstratePath`; the cap check now reads from the same
  `auditableFiles` list rather than raw `actualFilesTouched`.
- **Contract amend → criteria mark/completion no longer drops scope.**
  `maestro contract amend --task <id>` previously wrote only to the L2
  versioned store. The L1 store (read by `task contract criteria mark`,
  `task contract criteria add/remove`, and `task update --status
  completed`) stayed at the pre-amendment scope. Each subsequent L1-driven
  save then mirrored its own un-amended scope back into L2, silently
  reverting the amendment. The amend usecase now mirrors the amended
  contract into L1 when the legacyStore exposes a `save()` method, so
  every downstream verb sees the live scope. Round-10 scenario 3 walked
  v1→v6 with two amendments — pre-fix, the final version had `src/**`
  only and contract closed `broken`; post-fix, both amendments persist.
- **Overlap detection now requires actually-touched-file intersection.**
  `closeForTask` flagged parallel worktrees as overlapping when they
  shared a `filesExpected` glob (e.g. `src/**`) even though their
  actually-touched files were disjoint (`src/add.ts` vs `src/list.ts` vs
  `src/complete.ts`). The git-window-overlap test was firing on time
  alone, not on actual file races. Open candidates (no recorded verdict
  yet) are now deferred — they have not actually raced on any file —
  and closed candidates must have a `verdict.actualFilesTouched` that
  intersects with ours before they count as overlap. Round-10 scenario 4
  (3 disjoint parallel tasks) closed 1/3 fulfilled and 2/3 broken
  pre-fix; post-fix, all three close cleanly.
- **Auto-merge HUMAN message now points at the loosening soak.** When
  the user sets `autoMergeAllowed.<class>: true` it counts as a
  "loosening" and soaks for 30 days before taking effect; the verdict
  reason previously said "set autoMergeAllowed.<class>: true" without
  noting that doing so triggers the soak window. The reason text now
  says so explicitly and points at `maestro policy pending`. Surfaced
  by round-10 scenario 5 — the user set `medium: true`, ran the task,
  and got HUMAN with a message telling them to set the flag they
  already set.

## 0.72.13 - surface fix-forward recovery on broken contract close + skill nudge to mark manual criteria up front

Round-7 minimal-prompt agents (greenfield + brownfield) both completed
their work, ran the documented pre-claim ritual, and still landed on a
`broken` contract — the cause in both cases was identical and silent:
`kind: manual` `doneWhen` criteria were never ticked. Recovery required
four undocumented commands (`task contract reopen` + `criteria mark` +
re-claim + re-complete). The pre-claim ritual nowhere told them to mark
the boxes first; nothing in the close-time output told them how to fix
forward.

### Fixes

- `task update --status completed` now surfaces explicit recovery
  commands when the contract closes `broken` due to unmet `kind: manual`
  criteria. The output names the contract id, lists the unmarked
  criterion ids, and prints the exact `task contract reopen → criteria
  mark → task update --status completed` sequence. Other broken reasons
  (out-of-scope, forbidden, cap exceeded, unmet `receipt-hint` rows)
  are also enumerated when present so triage isn't a JSON pipe.
- `maestro-task` SKILL.md adds a step `0` to the pre-claim ritual:
  inspect `contract show` for `(manual)` `[ ]` rows and tick them via
  `task contract criteria mark` *before* running `task verify`.
  `receipt-hint` criteria still auto-tick from `--verified-by` tags;
  `manual` criteria require an explicit operator action and the skill
  now says so up front rather than in passing.

## 0.72.12 - extend substrate exemption to bundled `maestro:` skills + clearer HUMAN reason

Round-6 minimal-prompt agents (greenfield + brownfield) both closed
`broken` for the same reason: `maestro init` writes ~39 files into
`.claude/skills/maestro:*/` and `.codex/skills/maestro:*/`, and when
those land in the close-time diff (the user does `git add .` after
locking the contract) every one of them shows up as out-of-scope. The
contract semantics are correct — those paths are not in the user's
`filesExpected` glob — but the surprise is total: the user only edited
`src/`, the trust-loop output is *covered in unrelated maestro
substrate*, and the only fix today is "amend the contract or reset
`.maestro/contracts/`."

### Fixes

- New shared helper `isMaestroSubstratePath` in
  `src/shared/lib/maestro-substrate-paths.ts` covers three categories:
  `.maestro/`, bundled `maestro:` skill bundles under
  `.claude/skills/`, and bundled `maestro:` skill bundles under
  `.codex/skills/` (matching both `maestro:` and the URL-encoded
  on-disk form `maestro%3A`). Both `verdict.ts` (close-path) and
  `check-scope.ts` (Trust Verifier) now share this single source of
  truth so the two layers can never drift again. Project-authored
  skills outside the `maestro:` namespace are still in scope — they're
  user code.
- `maestro init` prints a closing tip nudging users to commit the
  substrate it just wrote (`.claude/`, `.codex/`, `.maestro/`,
  `.gitignore`) before locking their first contract. The exemption
  catches the pure-substrate case automatically; this tip handles the
  user who edits a non-`maestro:` skill or a `.gitignore` line as part
  of their task and wants those changes in the diff explicitly.
- Verdict reason text for `auto-merge-not-allowed` now reads
  "Auto-merge is opt-in and not enabled for risk class … the task can
  still complete via human review" instead of the previously-terse
  "Auto-merge is not allowed". Round-6 brownfield reported the old
  wording reads as a hard block on task completion; the new wording
  distinguishes "no auto-merge" from "no completion."

## 0.72.11 - close four UX seams: contract checkbox view, task update --task alias, exit-2 + HUMAN-default skill notes

Round-5 closed cleanly (greenfield + brownfield both `fulfilled`), but
left four documented papercuts that minimal-prompt agents will keep
hitting. None block the trust flow on their own; together they account
for "I followed the skill and the output still surprised me" reports.

### Fixes

- `task contract show` now renders the per-criterion `doneWhen`
  checklist (`[x]`/`[ ]` plus criterion `kind` and the `metEvidence`
  hint when present) and a post-close `Verdict:` block citing
  out-of-scope, forbidden, and unmet rows. The aggregate `Status:` line
  alone hid the receipt-hint auto-mark trail; agents triaging a
  `broken` close had to JSON-pipe the contract to see *why*. The
  default human view now matches the JSON shape.
- `task update` accepts `--task <id>` as an alternative to the
  positional `<id-or-slug>`. Every other agent-facing trust verb
  (`task verify`, `task proof`, `verdict request`, `plan check`,
  `contract show/amend/history`, `evidence record`, `review ack`,
  `merge auto`, `deploy gate`, `runtime check`) requires `--task`;
  `task update` was the lone holdout. Passing both forms with
  conflicting ids is rejected with a clear error; passing the same id
  in both is accepted.

### Skill docs

- `skills/bundled/maestro-task/SKILL.md` now spells out
  `task verify`'s exit codes (`0` clean / `1` errors / `2`
  warnings/info-only) so agents stop interpreting `2` as a verification
  failure and looping back to step 1.
- The HUMAN verdict block now states the autopilot default explicitly:
  every risk class — `low`, `medium`, `high`, `critical` — defaults to
  `autoMergeAllowed: false`, so a clean PASS lands as HUMAN until the
  team flips the relevant entry in `policies/autopilot.yaml`. The
  pointer to `docs/auto-merge-eligibility.md` answers the inevitable
  "why is even my low-risk PR HUMAN?" follow-up.

## 0.72.10 - skill quick-start uses a working filesExpected glob

Round-4 greenfield agent followed the SKILL.md quick-start verbatim
(including `kind: receipt-hint` from v0.72.9) and still got a `broken`
contract. Cause: the quick-start's `filesExpected` example was
`src/features/foo/**` — a placeholder pulled from maestro's own repo
layout. A minimal-prompt agent writing `src/hello.ts` landed outside
that glob, the scope check flagged it, and the contract closed
`broken` despite both `receipt-hint` criteria auto-marking correctly
from `--verified-by`.

### Fix

- `skills/bundled/maestro-task/SKILL.md` quick-start now uses
  `filesExpected: - src/**` — the most common case for both greenfield
  and brownfield TS projects. Tight-scope guidance lives in surrounding
  prose; the example is now a working scaffold instead of a trap.

## 0.72.9 - skill quick-start defaults to receipt-hint criteria

Round-3 minimal-prompt agents (greenfield + brownfield) both completed
the documented contract flow successfully and then saw their contract
auto-flip to `broken` on `task update --status completed`. Cause: the
`maestro-task` SKILL.md quick-start example used `kind: manual` for the
sample `doneWhen` criterion. Per `reference/contracts.md`, `manual`
means "an operator ticks the box explicitly" via `criteria mark` —
which the SKILL.md never mentions. So a minimal-prompt agent who reads
only SKILL.md leaves the criterion unmarked, the close-path verdict
sees `unmetCriteria.length > 0`, and the contract closes `broken`.

### Fix

- `skills/bundled/maestro-task/SKILL.md` quick-start now defaults to
  `kind: receipt-hint` with two short, matchable example texts (`tests
  pass`, `manual`). Agents who follow the example and complete with
  `--verified-by "tests pass" --verified-by manual` get both criteria
  auto-marked at close.
- The same section now explicitly contrasts `receipt-hint` (auto-marked
  from `--verified-by`) and `manual` (requires explicit
  `criteria mark`), and warns that unmarked `manual` criteria close
  the contract as `broken`. The CLI semantics are unchanged.

## 0.72.8 - close-path scope exemption + post-lock empty-diff hint

Round-2 greenfield surfaced two follow-on bugs after v0.72.7:

1. `task update --status completed` ran the legacy close-path verdict
   (`computeContractVerdict`), which audited `.maestro/**` paths against
   the user's scope just like the pre-v0.72.7 Trust Verifier did.
   Greenfield agents who `git add -A` after `maestro init` and committed
   substrate alongside user code saw their contract auto-flip to
   `broken` with `outOfScopeFiles: [".maestro/contracts/...", ".maestro/
   tasks/NOW.md", ...]` on every task completion.
2. The `empty-diff` warn finding always told agents to "stage and commit
   your changes," but the most common cause is locking the contract,
   then running `task verify` before any post-lock commit exists. In
   that case `base === head`, nothing has been staged, and the advice
   is actively misleading.

### Fix

- `computeContractVerdict` filters `.maestro/**` paths out of
  `forbiddenTouched`, `expectedFilesMatched`, and `outOfScopeFiles` —
  matching the Trust Verifier exemption shipped in v0.72.7. Substrate
  metadata is owned by the CLI, not the user's task, and must not flip
  the contract verdict on close.
- `checkNonEmptyDiff` now distinguishes the base-equals-HEAD case and
  emits a hint that points at "commit work after locking the contract"
  rather than "stage and commit your changes."
- New unit test in `tests/unit/features/task/contract/verdict.test.ts`
  asserts substrate paths (including forbidden globs inside `.maestro/`)
  are exempt from close-path scope evaluation.
- New unit test in `tests/unit/features/verify/usecases/checks/check-non-empty-diff.test.ts`
  asserts the post-lock message variant.

## 0.72.7 - brownfield base resolution + .maestro/ scope exemption

Round-3 brownfield test surfaced two compounding bugs that effectively
broke the trust substrate on any repo with pre-existing files:

1. `task verify` and `verdict request` resolved the default base via the
   branch fallback chain (`main` → `master` → `trunk` → empty-tree)
   instead of the contract's own lock-commit. On a brownfield repo the
   merge-base sat well before the contract was locked, so every
   pre-existing file (`README.md`, `package.json`, `.gitignore`, etc.)
   appeared in the diff and was flagged as out-of-scope. A single bogus
   scope finding then escalated risk class from `medium` to `critical`,
   pushing the verdict from PASS to FAIL.
2. The scope check itself audited `.maestro/**` paths the same way it
   audited user code. But `.maestro/contracts/...`, `.maestro/tasks/...`,
   `.maestro/policies/...` are written by the maestro CLI itself during
   the task lifecycle. They land in the lock-commit-to-HEAD diff by
   construction, so even after fixing #1 the substrate's own bookkeeping
   triggered scope errors.

Together: the documented brownfield workflow (`init` → `task q` →
`contract new --from yaml` → `contract lock` → write code → `git commit`
→ `task verify`) was guaranteed to fail on any non-empty repo.

### Fix

- `task verify` and `verdict request` now prefer `contract.claimedAtCommit`
  (the HEAD recorded at lock time) over `resolveDefaultBase()` when no
  `--base` flag is given. Pre-existing files committed before the lock
  no longer appear in the diff. The branch fallback only fires for
  contracts locked before the field existed.
- `checkScope` exempts paths under `.maestro/` unconditionally — those
  are substrate metadata, not user code, and gating them with the user's
  contract scope produces only false positives. Forbidden patterns
  inside `.maestro/` are also exempt (consistent with the rule that the
  CLI, not the user, owns those files).
- Help text on `--base` for both `task verify` and `verdict request`
  now reads `default: contract lock-commit; falls back to merge-base
  with main/master/upstream`.
- New E2E test (`tests/e2e/l2b-contract-bridge-flow.test.ts` test 14)
  asserts a brownfield repo with pre-existing committed files produces
  no scope errors after the documented workflow.
- New unit test (`tests/unit/features/verify/usecases/checks/check-scope.test.ts`)
  locks the `.maestro/` exemption.

## 0.72.6 - contract amend propagates after.scope into the new version

Round-2 surfaced a real correctness bug: agents who ran
`maestro contract amend --add-path <p> --reason "..."` saw the new path
appear in the amended `v2.json`'s `amendments[]` array, but the
contract's effective `scope.filesExpected` was unchanged on the new
version. The Trust Verifier's scope check reads
`contract.scope.filesExpected` and saw the un-amended scope, so the
amended path kept producing `scope` error findings even after a
successful `contract amend` call. Agents looked at the error, looked
at the v2 file, and concluded amend was broken.

### Fix

- `amendContract` now applies the amendment's `after.scope`,
  `after.intent`, and `after.doneWhen` to the new version, in addition
  to appending to `amendments[]` and flipping status to `amended`. The
  amendment object is unchanged — `before`/`after` snapshots still
  capture history. Downstream readers (`task verify`, `plan check`,
  `contract show`) now see the effective post-amendment state.
- Regression test in
  `tests/unit/features/task/usecases/amend-contract.usecase.test.ts`
  covering the after.scope application path.

## 0.72.5 - Default base ref handles master-defaulting greenfield repos

Round-2 first-time-user test on a fresh `git init` (which still defaults
to `master` on macOS) hit a misleading empty-diff warn even after
committing code: `task verify`'s `resolveDefaultBase` walked
`@{u}` → `merge-base HEAD main` → literal `"main"`, which doesn't exist
on a `master` repo. Git silently treated the unknown ref as empty and
the trust verifier saw no diff.

### Fix

- `resolveDefaultBase` now walks `main` → `master` → `trunk` and falls
  back to git's empty-tree SHA (`4b825dc6...`) when none of those
  candidates differ from HEAD. On a single-branch greenfield repo, the
  empty tree is the right base — it shows every commit since creation,
  which is what a brand-new user actually wants to verify.
- New unit test (`tests/unit/shared/lib/git-base.test.ts`) locks the
  three fallback paths.

## 0.72.4 - Trust Verifier flags empty diffs

First-time-user friction. A greenfield agent who staged but never committed
saw `Trust Verifier: no findings`, `verdict request` returned a healthy-
looking HUMAN, and the verdict's `subject.tree_sha` was
`4b825dc642cb6eb9a060e54bf8d69288fbee4904` — git's well-known empty-tree
SHA. The whole trust trail bound itself to nothing because the six existing
checks all return clean trivially when there is nothing to inspect.

### Fix

- **New `empty-diff` check.** Trust Verifier now runs 7 checks. The new
  check returns a `warn` finding when both `changedPaths` and `addedLines`
  are empty, with details that name the base/head SHAs and tell the user
  to commit before verifying. Surfaces in `task verify` (exit 2) and
  `verdict request` (counts the warn in `trustVerifier.warns`). Severity is
  `warn` not `error` so legitimate pre-commit verifies still succeed
  visibly without blocking.

### Test coverage

- Unit test for the new check (`tests/unit/features/verify/usecases/checks/check-non-empty-diff.test.ts`).
- New `runTrustVerifier` integration test asserting the empty-diff finding
  surfaces alongside the other checks.

## 0.72.3 - Worktree project-root resolver fix

Bug fix. The v0.72.2 release wrapped five call sites in
`resolveMaestroProjectRoot`, but the resolver itself was wrong: it
checked `existsSync(.maestro/)` before walking through
`.git/commondir`. Linked worktrees always contain a tracked
`.maestro/` snapshot (AGENTS.md, policies/, bootstrap/,
principles.jsonl, tasks/contract-templates/), so the resolver
returned the worktree path and every L1 contract write landed in the
worktree's local `.maestro/` instead of the main repo's. A second
greenfield demo with three parallel teammates surfaced this as the
same "stranded contracts" symptom v0.72.2 was supposed to fix.

### Fix

- **Walk via `.git/commondir` before local `.maestro/` in worktrees.**
  When `.git` is a worktree pointer file, resolve to the main
  worktree's `.maestro/` first; only fall through to the local
  directory if commondir resolution fails. The main-worktree case
  (where `.git` is a directory) is unchanged.

### Test coverage

`tests/unit/shared/lib/project-root.test.ts` gains a regression test
that reproduces the bug deterministically: a linked worktree with
both a `.git` pointer file AND a tracked `.maestro/policies/`
directory must still resolve to the main repo root. The test would
fail against v0.72.2.

## 0.72.2 - Worktree config + contract draft UX fixes

Bug fixes surfaced by re-running the v0.72.1 greenfield demo with three
parallel teammates working in independent git worktrees.

### Fixes

- **Worktree-aware config resolution.** Calls like
  `services.config.load(process.cwd())` were not walking through
  `.git/commondir` to the main worktree, so contracts locked from a
  worktree captured the contract-loader's defaults instead of the
  shared `.maestro/config.yaml`. The most visible symptom was a
  contract created with `overlapPolicy: fail` even when the repo
  config opted into `annotate`. All five call sites (contract new/lock,
  mission, doctor, task, mission-control snapshot loaders) now route
  through `resolveMaestroProjectRoot`, which already existed but was
  unused at these seams.

- **Structured error when no contract exists.** `verdict request` and
  `policy check` raised bare `Error` when the requested task had no
  contract, which Bun rendered as a runtime stack trace with bunfs
  paths. Both call sites now throw `MaestroError` with hints
  (`contract new <id>`, `contract lock <id>`), matching the rest of
  the CLI's error formatting.

- **Warn on unknown contract draft YAML keys.** Keys outside the draft
  schema were silently dropped. A teammate using
  `scope.allowedPaths` (vs the real `scope.filesExpected`) produced a
  contract with no scope and no warning. The draft loader now warns
  to stderr per unknown key with did-you-mean hints for the common
  typos. Warnings are advisory; exit code is unchanged.

### Test coverage

New `tests/e2e/l2c-config-resolution-flow.test.ts` adds 4 compiled-
binary tests that lock all three fixes from the user-facing surface:
worktree config-snapshot capture, unknown-key warnings on stderr, and
structured `MaestroError` output for both `verdict request` and
`policy check` when a contract is missing.

## 0.72.1 - L1↔L2 contract bridge fix

Bug fix. The L1 contract store (`.maestro/tasks/contracts/`, written by
`task contract new/lock/amend/edit/discard/criteria *`) and the L2
versioned store (`.maestro/contracts/<taskId>/vN.json`, read by
`task verify`, `plan check`, `verdict request`, `contract show`,
`contract history`, `contract amend`, `merge auto`, `ci verify`)
shared a schema but had disjoint filesystem layouts and no bridge.
The documented agent workflow `task contract new <id> --from <yaml>
&& task contract lock <id> && task verify --task <id>` failed with
"no contract found" until v1.json was hand-seeded.

### Fix

- **Write-through mirror.** Every L1 transition that produces an
  active status (locked, amended, fulfilled, broken) now mirrors the
  saved contract into the L2 version store as the next vN.json. The
  mirror runs at the use-case layer (`contract-workflows.usecase.ts`)
  so adapter layering stays clean. Drafts and discards are skipped.
  Mirror writes serialize behind L1's existing `withFileLock`.

- **Read-time backfill.** A new helper
  (`readCurrentContractWithBackfill`) reads from L2 first; on miss,
  it pulls the active L1 record and writes it as v1. Existing
  v0.72.0 repos with locked L1-only contracts on disk continue to
  work after upgrade — no manual migration verb required.

- **Mirror is derived state.** `.maestro/contracts/` is now in
  `.gitignore` and in `git-anchor`'s runtime-path ignore list, so
  mirror writes don't pollute verdict scope analysis. The L1 store
  remains the canonical write path; the L2 store is a derived view.

### Test coverage

New `tests/e2e/l2b-contract-bridge-flow.test.ts` adds 13 compiled-
binary tests that exercise every L1 → active-state transition plus
the legacy backfill plus per-task isolation, using only documented
agent verbs. The seam that previously had zero coverage is now the
most-tested surface in the trust substrate.

## 0.72.0 - L8 (trimmed) — Cross-Task Conflict + Trust Benchmark

Honest framing: this is a trimmed L8 release. The full learning loop
(autopsy generator, `maestro ratchet` CLI, N≥2 broad-promotion guard,
sunset/decay machinery) is intentionally not in this slice. Those
phases will ship when teams ask maestro to learn from incidents.

### L8.0: `memory-ratchet` rename (internal)

`src/features/ratchet/` renamed to `src/features/memory-ratchet/`.
No CLI surface change, no behavior change. Internal restructuring only
— agents and operators are not affected.

### L8.1: Cross-task conflict detection

`maestro ci verify` now queries open PRs for overlapping changed file
paths. When overlap is found, it records a `kind=cross-task-conflict`
Evidence row at `witnessed-by-ci` and feeds the signal to the Risk
Engine.

**Risk impact:** the Risk Engine raises the effective risk class one
tier per conflict signal, capped at `critical`. Multiple
`cross-task-conflict` rows for the same verification run are
deduplicated to a single tier raise — piling up rows does not
compound the raise.

**New `EvidenceKind`:** `cross-task-conflict`
- Payload: `{ thisPr, conflictingPrs: number[], overlappingPaths: string[] }`
- Witness: `witnessed-by-ci`
- Recorded by `maestro ci verify` only (not a manual-record target)

**New port additions:**
- `ConflictDetectorPort` in `src/features/ci/ports/` — interface for
  listing open-PR file paths.
- `GhCliConflictDetectorAdapter` in `src/features/ci/adapters/` —
  Prometheus-style implementation backed by `gh api`.

**Non-fatal on API errors:** if the `gh api` call fails (missing token,
rate-limit, etc.), `ci verify` logs a warning and continues without
recording a conflict row. The verify step does not fail.

See `docs/cross-task-conflict.md` for the full reference.

### L8.2: Trust benchmark corpus seed

`tests/e2e/trust-benchmark/` is a new end-to-end regression corpus.
9 seed scenarios are included; the corpus grows demand-driven.

| File | Edge case | Mitigation |
|------|-----------|------------|
| `ec05-out-of-scope.test.ts` | EC 5 | Trust Verifier scope check |
| `ec06-generated-drift.test.ts` | EC 6 | Generated-file parity |
| `ec09-sensitive-path.test.ts` | EC 9 | `forbidden_paths` + `sensitive-paths.yaml` |
| `ec12-security-thin.test.ts` | EC 12 | Threat-model required predicate |
| `ec22-amendment-creep.test.ts` | EC 22 | Amendment-budget rules 3–7 |
| `ec23-proof-not-tied.test.ts` | EC 23 | ProofMap at L3.5 |
| `ec27-rebase-squash.test.ts` | EC 27 | Tree-SHA verdict identity |
| `ec31-decision-authority.test.ts` | EC 31 | `owners.yaml.deploy_approver` |
| `ec32-self-weakening.test.ts` | EC 32 | Rule 12 base-branch reading |

CI runs `bun test tests/e2e/trust-benchmark/` on every release.

See `docs/trust-benchmark.md` for fixture pattern and how to add
new scenarios.

### Compatibility

Fully additive. Existing repos at L5/L6/L7 are unaffected unless they
run `maestro ci verify` in CI — which will now additionally check for
cross-task conflicts and record Evidence when found. The check is
advisory by default (raises risk class; does not hard-block unless
the raised class already triggers a BLOCK condition under the team's
policy).

---

## 0.71.0 - L7 — Deploy Safety (advanced optional, trimmed)

Honest framing: L7 is reachable from L5 — teams running L5 alone can
adopt L7 without shipping L6. L7 itself is opt-in: producing
`deploy-readiness` and `runtime-signal` Evidence does not by itself
flip Verdict semantics. Teams wire the new Evidence into their
`policies/risk.yaml` if they want it to gate verdicts.

### Spec schema v2

`Spec.schema_version` bumps from 1 to 2. Two new slots:
- `runtime_signals[].{name, provider, query, threshold, severity}` —
  replaces the v1 placeholder `{ kind, source }`.
- `rollout_plan?.{feature_flag?, canary?, rollback_command?}` —
  optional rollout descriptor; consumed by `deploy gate`.

v1 specs forward-migrate at read time: `FsSpecStoreAdapter` coerces
v1 → v2 in memory without rewriting the on-disk file. Files are
rewritten only on the next `spec edit`.

### New verbs

- `maestro deploy gate --task <id> [--base <ref>] [--json]` — runs
  four deterministic checks (feature_flag, canary_plan, rollback,
  owner) against the task's mission Spec, rollback Evidence, and
  owners.yaml. Always writes a `kind=deploy-readiness` Evidence row
  with the per-check breakdown and `gate: pass | fail`. Exit 0 on
  pass, 1 on fail. Witness: `witnessed-by-ci` in CI, else
  `agent-claimed-locally`. Does not mutate the Verdict.
- `maestro deploy rollback --task <id> --command <cmd> [--json]` —
  spawns the supplied shell command, captures exit code, and writes
  a `kind=rollback-exercised` Evidence row. Witness:
  `witnessed-by-ci` in CI, else `witnessed-by-maestro` (the only
  non-CI context that warrants `witnessed-by-maestro`). Exit 0 on
  command exit 0, else 1. Evidence is written either way — the
  witness is the audit, not the success.
- `maestro runtime check --task <id> [--provider-base-url <url>]
  [--json]` — iterates `Spec.runtime_signals` and queries each
  signal via the configured monitor adapter (Prometheus is the one
  shipped reference adapter). Writes one `kind=runtime-signal`
  Evidence row per signal. Provider base URL precedence:
  `--provider-base-url` flag → `MAESTRO_PROMETHEUS_URL` env →
  `http://localhost:9090`. Signals with `provider != "prometheus"`
  log `[skip] provider <X> not supported` and record a
  runtime-signal row with `pass: false` and
  `note: "unsupported provider"`. Exit 0 always.

### owners.yaml fourth role

`owners.yaml` gains a fourth role list: `deploy_approver`. CI
Maestro (`maestro ci verify`) authorizes the PR author against this
list when a `kind=deploy-readiness` Evidence row with `gate: pass`
exists for the task. If the PR author is not in `deploy_approver`,
the GitHub check conclusion is downgraded to `failure` and the
summary appends `deploy not authorized: PR author <login> is not in
owners.yaml deploy_approver`. Owners are loaded from the base
branch (Rule 12). Outside CI, the author check is skipped.

### Two new Evidence kinds, one new producer

- `kind=deploy-readiness` (new at L7.2): payload `{ task_id,
  checks: { feature_flag, canary_plan, rollback, owner }, gate }`.
- `kind=runtime-signal` (new at L7.3): payload `{ signal_name,
  provider, query, value, threshold, operator, pass, sampled_at,
  note? }`.
- `kind=rollback-exercised` was declared in the EvidenceKind union
  at L6.1 with payload `{ command, exit }`. L7.5 ships the
  producer (`maestro deploy rollback`).

`maestro evidence show` renders payload detail for all three kinds.

### Compatibility

Fully additive. Existing repos at L5 (or L5+L6) are unaffected
unless they explicitly:
- populate `Spec.rollout_plan` (no behavior change without a
  populated rollout plan)
- add `deploy_approver` to `owners.yaml` (existing files keep
  working without it)
- set `policies/risk.yaml` predicates that consume
  `deploy-readiness` or `runtime-signal` Evidence

The Spec v1 → v2 migration is read-time forward-compat: existing v1
spec files on disk continue to load.

### Deferred to L8

Autopsy generator, ratchet review/approve/sunset CLI, N≥2
broad-promotion guard, sunset/decay machinery, cross-task conflict
detection, trust benchmark corpus.

## 0.70.0 - L6 — Auto-Merge for Declared Safe Scope (advanced optional, trimmed)

Honest framing: L6 is opt-in and applies to roughly 5–15% of merged
PRs. Eligible PRs require all of:
- PASS verdict
- `policies/autopilot.yaml.autoMergeAllowed.<risk-class>=true` (the
  `autoMergeAllowed` field has existed since L3 — L6 just consumes
  it; default is `false` for every class)
- all gating Evidence (kinds: `command`, `verifier`, `ai-review`,
  `threat-model`, `plan-check`) at `witnessed-by-ci` or stronger
- no sensitive-path edits without an `owners.yaml` `sensitive_waiver`
- rollback witnessed (`kind=rollback-exercised` Evidence at
  `witnessed-by-ci`; producer ships at L7.5)
- HUMAN-at-risk-≥-medium verdicts: `review-ack` Evidence present
- Spec score = 1.0 when a Spec is associated with the task

### New verbs

- `maestro merge auto --pr <n> [--task <id>] [--base <ref>] [--json]`
  — runs the 8-predicate eligibility gate; if eligible, calls
  `gh pr merge --auto`. Exit 0 eligible+triggered, 1 ineligible
  (with itemised reasons), 2 invocation/usage error.
- `maestro verdict override --task <id> --pr <n> --reason "<text>"
  [--verdict <id>]` — auxiliary audit-trail override. Authorization:
  invoking `whoami` must appear in the **base-branch** version of
  `.maestro/policies/owners.yaml` under `sensitive_waiver` (Rule 12).
  Writes a `verdict-override` Evidence row at
  `agent-claimed-and-not-reproducible`. The original Verdict is
  **not** rewritten — overrides are append-only Evidence. CI
  Maestro reflects the latest override in the PR check summary
  ("Verdict overridden by …"); conclusion mapping is unchanged
  (a BLOCK-overridden verdict still posts `failure`).
- `maestro review ack --task <id> --verdict <id> --criterion "<text>"
  [--criterion "<text>" …]` — explicit reviewer acknowledgement of
  checklist items. One Evidence row per invocation, kind `review-ack`,
  witness `agent-claimed-locally`. Consumed by predicate 7 of
  auto-merge eligibility.

### New EvidenceKinds

- `review-ack` — payload `{verdictId, ackedBy, criteria[]}`. Witness
  default `agent-claimed-locally`.
- `verdict-override` — payload `{verdictId, overriddenBy, reason}`.
  Witness `agent-claimed-and-not-reproducible`.
- `rollback-exercised` — payload `{command, exit}`. **Declaration
  only** in L6 (read by predicate 6). Producer ships at L7.5.

### New use-cases

- `src/features/merge/usecases/auto-merge-eligible.usecase.ts` — the
  8-predicate gate, deterministic, never short-circuits, returns
  `{ eligible, reasons[] }` with the full failure list.
- `src/features/spec/usecases/score-spec.usecase.ts` — pure
  `scoreSpec(spec)` deterministic checklist over required slots
  (`acceptance_criteria.length >= 1`, `non_goals.length >= 1`).
  No LLM-extracted half. The roadmap-mentioned
  `user_visible_behavior` slot is intentionally not added — the
  current Spec schema does not have it; adding one is a v2 schema
  bump.

### Limitations / honest framing

- `verdict override` authorization uses local `whoami`; no
  GitHub-author identity check yet (L7.9 territory).
- Spec-score threshold not configurable at L6 (defer to v0.70.x
  patch when teams ask).
- Full 7-reviewer pipeline pre-enumeration intentionally cut from
  L6.4. Reviewer kinds ship as agents emit them.
- The L7 deploy phases are not required to ship L6: predicate 6
  (`rollback-not-witnessed`) just becomes a normal ineligibility
  reason for teams that haven't shipped L7.5. That's the deliberate
  L6 ↔ L7 hand-off.

### Compat

Fully additive. Existing repos at L5 are unaffected unless they
explicitly enable `policies/autopilot.yaml.autoMergeAllowed.<class>`.
EvidenceKind union widened with `review-ack`, `verdict-override`, and
`rollback-exercised` — backward-tolerant readers continue to skip
unknown kinds.

## 0.69.0 - L5 — CI Is the Authoritative Verifier

- New verb: `maestro ci verify [--pr <n>] [--task <id>] [--base <ref>]
  [--json]`. Reads CI env (`GITHUB_ACTIONS`, `GITHUB_REPOSITORY`,
  `GITHUB_REF`, `GITHUB_SHA`, `GITHUB_BASE_REF`, `GITHUB_EVENT_PATH`,
  `GITHUB_OUTPUT`, `GITHUB_TOKEN`); flags override. Runs Trust Verifier,
  ingests CI job results as `witnessed-by-ci` Evidence, computes the
  Verdict via the existing `requestVerdict` use-case, writes
  `verdict_id`, `verdict_decision`, `effective_risk_class` to
  `$GITHUB_OUTPUT`, and (in GitHub Actions with a token) posts a
  GitHub Check via `gh api`. Exit codes: 0 PASS / 1 FAIL / 2 HUMAN /
  3 BLOCK — same as `verdict request`.
- New feature dir `src/features/ci/`: `readCiEnv` (env parser),
  `runCiVerify` (use-case), `postPrCheck` (gh-api poster), minimal
  `GithubApiPort` with a `gh-cli` adapter (`postCheckRun`,
  `patchCheckRun` only — no comment APIs in trimmed L5).
- Verdict identity by tree SHA (edge case 27): `Verdict.subject =
  { pr?, tree_sha }`, where `tree_sha` is `git rev-parse HEAD^{tree}`.
  Squash with identical content survives; force-push to a different
  tree invalidates. Backward-compatible: existing v1 verdicts without
  `subject` still parse. `GitAnchorPort` gains
  `resolveTreeSha(cwd, ref?)`. `maestro verdict show --pr <n>` finds
  verdicts by current `HEAD^{tree}` match.
- New skill asset
  `skills/bundled/maestro-setup/reference/github-workflow/maestro-verify.yml.template`:
  starter GitHub Actions workflow that installs the maestro binary
  via curl-extract and runs `maestro ci verify` on every
  `pull_request`. Permissions: `pull-requests: write`, `checks: write`,
  `contents: read`. The `maestro-setup` skill installs it into
  `.github/workflows/` when `.github/` exists; non-interactive
  otherwise. No new CLI verb — setup is skill-first.
- New reference doc `docs/ci-integration.md`: workflow template
  usage, env contract, witness-by-ci ingestion, PR check status
  semantics, verdict tree-SHA identity, troubleshooting.

Compat: additive. No breaking changes to existing CLI verbs, data
formats, evidence kinds, policies, or skill APIs. The trimmed L5
ships `ci verify` + tree-SHA verdict identity + PR check status
only. Deferred to v0.69.x patches as friction is observed: PR
comment with rendered evidence packet, `maestro pr publish`, flake
tracking, override flow, review checklist, handoff packet
`open_hypotheses` + `ruled_out_approaches`.

## 0.68.0 - L4 — Autopilot Inner Loop (No Merge)

- New verbs: `maestro plan check`, `maestro task budget`,
  `maestro evidence record --kind ai-review`,
  `maestro evidence record --kind threat-model`.
- New feature dir `src/features/plan/`: `checkPlan(plan, contract, spec,
  derived)` produces deterministic findings — `scope-widens` (plan touches
  files outside `contract.scope.filesExpected`), `missing-proof` (a Spec
  acceptance criterion has no proof entry), `risk-class-too-low` (plan's
  `riskClass` is below the deriver's class for the planned files, per
  Rule 1 plan-time gate). The `plan check` verb records a `plan-check`
  Evidence row and never blocks (exit 0 always).
- New Evidence kinds: `plan-check`, `ai-review`, `threat-model`.
  Risk Engine now consumes `ai-review` per Rule 1 (LLM veto-only): any
  error-severity reviewer finding raises `effectiveRiskClass` by one notch;
  a `security`-reviewer error always lifts to `critical`; a clean
  ai-review never lowers the deterministic baseline. Risk Engine also
  applies the `threat-model-required` predicate (Edge Case 12) — when
  `derivedRiskClass === "critical"` and the matched policy signal is
  `diff-intersects-sensitive-security`, a `threat-model` Evidence row is
  required to clear that reason; presence + schema validity is necessary
  but not sufficient.
- Cost-budget enforcement (Rule 11): run-state at
  `.maestro/runs/<task-id>/state.json` (gitignored, derived) tracks
  `retryCount`, `wallClockElapsedSeconds`, `tokensUsed?`. The Risk Engine
  short-circuits to BLOCK with reason `cost-budget-exhausted` when any
  limit is exceeded. `request-verdict` increments `retryCount` by 1 on
  FAIL/HUMAN verdicts; PASS and BLOCK do not increment. Wall-clock and
  token deltas are the agent runtime's responsibility.
- New bundled skill `maestro-verify` (canonical verification protocol);
  `maestro install` now installs 7 bundled skills (was 6). The
  `maestro-task`, `maestro-plan`, and `maestro-handoff` skills cross-
  reference `maestro-verify` at their verification points instead of
  duplicating the detail.
- Mission Control: new `autopilot` screen (mission-mode only) projects
  `{taskId, intent, latestVerdict.decision, retryCount/maxRetries,
  wallClockElapsed/maxWallClockSeconds, lastUpdatedAt}` from existing
  stores. Read-only — no mutation, no Evidence recording, no verdict
  invocation. `mission-control --render-check` now covers 14 screens.
- New docs: `docs/ai-reviewer-protocol.md` (schema for bug/security/
  architecture reviewers; confidence semantics; recording guidance) and
  `docs/threat-model-format.md` (ThreatModelPayload schema + JSON/YAML
  examples).
- Compat: additive — existing contracts, evidence, verdicts, and
  policies unchanged. The L3 `costBudgetExhausted` Risk Engine input
  (declared but unwired in L3) is now supplied by `request-verdict`.

## 0.67.0 - L3 — Risk Verdict + Policy + Witness Levels

- New verbs: `maestro verdict show`, `maestro verdict request`,
  `maestro policy check`, `maestro policy pending`, `maestro task proof`.
- New artifact: `Verdict` (`PASS` | `FAIL` | `HUMAN` | `BLOCK`) stored under
  `.maestro/verdicts/<task-id>/<verdict-id>.json` (gitignored, derived).
  `verdict request` exit codes: 0 PASS, 1 FAIL, 2 HUMAN, 3 BLOCK.
- New policy files (provisioned by `maestro init`):
  `.maestro/policies/risk.yaml` (Signal → Derived class table — risk.yaml
  absent falls back to the canonical ROADMAP-default policy),
  `.maestro/policies/autopilot.yaml` (per-class auto-merge + required witness
  level), `.maestro/policies/release.yaml`.
- Risk Engine (`src/features/risk/`): `deriveRiskClassFromDiff` is
  deterministic; `effectiveRiskClass = max(contract.riskClass, derived)` —
  per Rule 1, the agent can only raise the class, never lower it.
- ProofMap (`src/features/verify/usecases/proof-map.ts`): joins Spec
  acceptance criteria with Evidence rows on `criterion_id` and reports
  uncovered criteria.
- Asymmetric policy editing (Rule 9): tightenings (raises required witness,
  narrows scope, disables auto-merge) take effect at commit time; loosenings
  (lowers witness, widens scope, enables auto-merge) are pending for 30 days
  from commit time. `policy pending` lists currently-pending loosenings.
  `.maestro/policies/.pending-loosenings.json` is a gitignored derived cache.
- Evidence schema bumped 2 → 3; reader still accepts {1, 2, 3}. v1 rows
  missing `witness_level` are synthesized to `agent-claimed-locally` on read;
  v2 and v3 rows must carry the field.
- Skill updates: `maestro-plan` is risk-class-aware (proposing `low` for
  sensitive-path / manifest / CI / policy diffs is futile);
  `maestro-task` runs `task verify` + `verdict request` before claiming
  complete and routes FAIL/HUMAN/BLOCK appropriately.
- Compat: additive — existing contracts and evidence unchanged; `risk.yaml`
  absent falls back to the ROADMAP-default policy. BLOCK on cost-budget
  exists in the engine but the input is not wired in L3 (lands at L4.4).

## 0.66.0 - L2 — Contract-Required + Scope Check

- New verbs: `maestro contract show/amend/history`, `maestro task verify`,
  `maestro spec show/edit`.
- Spec feature: `.maestro/specs/<mission-id>.json` stores AcceptanceCriteria
  with stable ids; `evidence record` requires `--criterion` when the task's
  Mission has a Spec.
- Contract types extended with optional `missionId`, `riskClass`,
  `amendmentBudget`, `costBudget`. Schema bumped 1→2; v1 fixtures still parse.
- Versioned contract storage at `.maestro/contracts/<task-id>/v<N>.json`
  (append-only). `propose/approve/amend` use-cases enforce `amendmentBudget`.
- Trust Verifier (`src/features/verify/`) with six checks: scope, lockfile
  parity, generated-file parity (advisory), sensitive paths, commit metadata,
  secrets in diff. `maestro task verify` runs locally and writes
  `verifier`-kind Evidence rows.
- Policy feature with `Owners` loader (`policy_approver`, `ratchet_approver`,
  `sensitive_waiver`). `maestro init` provisions `policies/owners.yaml` and
  `policies/sensitive-paths.yaml` with sensible defaults.
- Evidence schema v2 adds `verifier`, `contract-amendment`, and
  `contract-amendment-blocked` kinds with typed payloads. Reader is
  backward-tolerant; v1 rows continue to parse.
- `maestro-plan` and `maestro-task` skills updated: plans must include
  `proposed_contract`; agents must amend through the CLI on genuine scope
  discovery.
- Compat: tasks without a Contract default to permissive; existing Spec-less
  Missions keep working; existing v1 contract rows continue to read.

## 0.65.0 - L1 — Evidence-only logbook

- New verbs: `maestro evidence record/list/show` for recording and inspecting
  per-task evidence under `.maestro/evidence/`.
- Mission Control Task Board surfaces evidence count and the most recent rows
  (read-only).
- `maestro-task` skill instructs agents to record evidence after verification
  commands.
- `maestro init` now adds `.maestro/evidence/` and `.maestro/runs/` to
  `.gitignore` (idempotent).
- Compat: additive only; existing data unchanged.

## 0.58.3 - Handoff scope follow-ups

- Preserve global handoff scope options through `status` and `doctor` legacy
  handoff checks instead of rebuilding partial option objects.
- Avoid resolving the current Maestro project root in Mission Control snapshot
  paths unless reply or principle rollup data is requested.
- Keep principle outcome rollups empty when a project has no scoped handoffs,
  preventing unrelated global outcomes from appearing in the current project.
- Commit repo-local Claude and Codex copies of the built-in `maestro:*`
  mission skills so the checked-out agent surfaces match the source bundle.

## 0.58.2 - Worktree-safe global handoff scoping

- Normalize git worktrees back to the owning Maestro project root before
  comparing handoff provenance, so task-linked pickup from a legitimate
  worktree is not mistaken for a foreign-project takeover.
- Scope Mission Control principle rollups to the current project's global
  handoff packets and filter handoff-backed outcomes through that scoped id
  set.
- `status` and `doctor` now also flag legacy `~/.maestro/launches/`
  artifacts left by pre-0.58 standalone global launches.

## 0.58.1 - Project-anchored task-linked handoff pickup

- Keep the handoff store global at `~/.maestro/handoff/`, but stop
  silently downgrading foreign task-linked packets to standalone pickup.
  Prompt-only packets still pick up from any working directory. Task-linked
  packets now require pickup from their source project unless the operator
  explicitly passes `--standalone`.
- `maestro handoff pickup --standalone --id <id>` is now the explicit
  escape hatch for consuming a foreign task-linked packet as prompt-only.
  Normal pickup errors with the source project path plus a concrete
  `cd <project> && maestro handoff pickup --id <id> --json` command when
  the current working directory does not match the packet provenance.
- Mission bundle export now scopes global handoffs and principle outcomes
  by both `missionId` and handoff project provenance, preventing same-id
  collisions in another repo from leaking into the current bundle.

## 0.58.0 - Single global handoff store + rename launch to handoff (BREAKING)

- Collapse the two-store handoff routing into a single global store at
  `~/.maestro/handoff/`. Every packet, task-linked or standalone, lives
  there. `--task-id` now links a packet to a task (for continuation and
  ownership transfer on pickup) without affecting storage location.
  Handoffs are globally visible from any working directory. Prompt-only
  packets can be picked up anywhere; task-linked packets must be picked up
  from their source project unless `--standalone` is passed.
- Rename internal vocabulary from "launch" to "handoff":
  `LaunchStorePort` -> `HandoffStorePort`,
  `HandoffLaunchRecord` -> `HandoffRecord`,
  `HandoffLaunchStatus` -> `HandoffStatus`,
  `FsLaunchStoreAdapter` -> `FsHandoffStoreAdapter`.
  Deleted: `CompositeLaunchStore`. Renamed files: `launch-types.ts`,
  `launch-state.ts`, `launch-store.adapter.ts`, `list-launches.usecase.ts`,
  `show-launch.usecase.ts`, `reconcile-launch-record.usecase.ts` all
  move to `handoff-*` counterparts.
- On-disk change: the per-packet metadata file renames from `launch.json`
  to `handoff.json`. The per-packet directory renames from `launches/<id>/`
  to `handoff/<id>/`. Packets are now always rooted at `~/.maestro/`.
- `Services.launchStore` renames to `Services.handoffStore`. Any code
  importing maestro internals that reads `services.launchStore` needs
  updating.
- Bundle stats JSON field `launches` renames to `handoffs`. Bundle tar
  layout changes from `<mission>.mission/launches/<id>.json` to
  `<mission>.mission/handoffs/<id>.json`.
- Init no longer creates `.maestro/launches/` in new projects and
  `.maestro/launches/` is removed from the generated `.gitignore`
  (handoffs live outside the repo now).
- No migration: any existing packets in `~/.maestro/launches/` or
  `<project>/.maestro/launches/` are orphaned. Re-issue any in-flight
  handoffs after upgrade. `doctor` still flags legacy
  `.maestro/handoffs/` folders from earlier renames.

## 0.46.0 - Rename worker to agent (runtime role) (BREAKING)

- Rename the runtime-role concept "worker" (the thing that executes a
  Feature brief) to "agent" across the code, shipped skills, on-disk
  paths, and user-facing CLI/TUI strings. The brand-classifier rename
  (`Feature.workerType` -> `Feature.agentType`) in 0.38.0 only touched
  one field; this release finishes the rename everywhere else.
- On-disk change: `.maestro/missions/<id>/workers/<featureId>/` moves to
  `.maestro/missions/<id>/agents/<featureId>/`. Run
  `bun scripts/migrate-worker-path-to-agent.ts` once to rename existing
  local directories. The script is idempotent.
- Skill slug rename: `maestro:worker-base` -> `maestro:agent-base`.
  Projects that override this slug in `.maestro/skills/` need to rename
  the local override directory; the migration script handles this too.
- Factory skill slug renames: `.factory/skills/cli-worker` ->
  `cli-agent`, `.factory/skills/backend-worker` -> `backend-agent`.
- Source API renames (breaking for anything importing maestro internals):
  `WorkerReport` -> `AgentReport`, `WorkerReply` -> `AgentReply`,
  `generateWorkerPrompt` -> `generateAgentPrompt`,
  `parseWorkerReport` -> `parseAgentReport`,
  `writeWorkerReply` -> `writeAgentReply`,
  `validateWorkerReply` -> `validateAgentReply`,
  `workerSkillNotFound` -> `agentSkillNotFound`.
- Bundle stats JSON field `workers` renamed to `agents`.
- Generated prompt H1 changes from `# Worker Assignment:` to
  `# Agent Assignment:`; error message `Worker skill '...' not found`
  becomes `Agent skill '...' not found`.
- Expanded the `AGENT_INSTRUCTION_BLOCK` handoff section injected by
  `maestro install`: documents all flags (`--provider`, `--model`,
  `--worktree`, `--base`, `--name`, `--wait`, `--json`), the detached-
  by-default behavior, persisted launch artifacts, and default models
  (`codex=gpt-5.4`, `claude=opus`).
- Intentionally preserved: the legacy-field migration glue
  (`LegacyWorkerTypeMigration`, `migrateLegacyWorkerType`, `workerType`
  key) in `src/features/mission/feature/feature-migration.ts` -- these
  describe the old on-disk field being migrated and renaming them
  would erase the signal. `hooks/pre-agent.mjs:WORKER_RULES` is a
  distinct pre-agent hook concept, unchanged.

## 0.44.3 - Native handoff launcher

- Replace the old UKI queue-based handoff flow with `maestro handoff <task>`,
  which builds a self-contained markdown brief and launches a fresh Codex or
  Claude run, optionally in a sibling worktree.
- Persist launch artifacts under `.maestro/launches/` (`prompt.md`,
  `launch.json`, `output.log`) and remove the old `handoff create`, `pickup`,
  and `list` subcommands plus pending-handoff Mission Control surfaces.
- Drop handoff replay injection from worker prompt generation and switch
  bundle export to snapshot launch records instead of UKI handoff records.
- Update built-in mission-planning guidance, bootstrap templates, README,
  AGENTS, and TUI docs to teach the new launcher-based workflow.
- Keep legacy `.maestro/handoffs/` artifacts ignored but unused, with `status`
  and `doctor` warning when they are still present.

## 0.40.0 - Install hardening and task workflow improvements

- Windows install flow: harden running-exe replacement, tolerate EBUSY/EPERM
  on prior `.old` locks, and keep local verification paths honest when the
  install directory is overridden.
- `maestro update` honors the configured install path instead of defaulting
  to the system location.
- Task workflow: auto-claim on `task update --status in_progress`, surface
  blocker errors before ownership checks, extract stale-session recovery
  into a reusable helper.
- CI: stabilize the Windows matrix (bundle/tar/rename/build paths, skill
  path encoding, boundary check root, last-mile test failures).
- Docs: sync README features, commands, storage model, and TUI screen set
  with current state.

## 0.39.0 - Windows CLI support

- Publish a Windows x64 release asset (`maestro-windows-x64.exe`) and
  extend the platform resolver to find it.
- Add a Windows install flow that handles replacing a running executable
  via a `.old`-rename-and-swap dance, including rollback on verification
  failure.
- Add a PowerShell installer (`scripts/install.ps1`) mirroring the POSIX
  `install.sh` flow and surfacing user-PATH guidance.
- Drop `sh -c` usage from cross-platform code paths and fix POSIX-leaky
  tests so the CI matrix runs green on `windows-latest`.

## 0.38.0 - Rename Feature.workerType to Feature.agentType (BREAKING)

- Rename `Feature.workerType` to `Feature.agentType` across domain types,
  Zod schemas, TUI DTOs, CLI JSON output, and generated prompt headers.
  The field names a brand classifier (codex-cli, claude-code, subagent,
  human, maestro:worker-base etc.) which is the `agent` concept in the
  conductor model, not the `worker` (role) concept.
- Rename exported `WORKER_TYPE_PATTERN` constant to `AGENT_TYPE_PATTERN`.
- CLI stdout now prints `Agent type: <value>` (was `Worker type:`).
- Generated prompt header is `**Agent Type:** <value>` (was
  `**Worker Type:**`).
- Breaking surfaces: mission plan YAML/JSON input contract, persisted
  `.maestro/missions/<id>/features/*.json` on-disk format, `feature
  prompt --json` output shape.
- Migration: run `bun scripts/migrate-feature-agent-type.ts` once to
  rewrite existing local feature JSON; the script is idempotent.
- What survives unchanged (intentional, role-level): `maestro:worker-base`
  skill name, `WorkerReport`, `WorkerReply`, `generateWorkerPrompt`,
  `.maestro/missions/<id>/workers/` directory, `# Worker Assignment:`
  prompt H1 header, `Worker skill '...' not found` error message, and
  all general worker-as-role prose in shipped skills.

## 0.37.5 - Strip pre-conductor worker-dispatch config surface

- Remove the dead worker-dispatch config surface left behind by the
  2026-04-08 conductor refactor: `execution.defaultWorker`,
  `workers: { transport, command, args, env }`, `WorkerConfig`,
  `CliWorkerConfig`, `recommendWorkerFit`, `getWorkerGuidance`,
  `formatWorkerLabel`, the Mission Control `workers` config tab, and
  the default-worker picker modal.
- Remove residual `cassAvailable` fields from `StatusReport` and
  `MissionControlConfigSummary`; `status --json` no longer emits the
  field.
- Remove dead `HIDDEN_CONFIG_KEY_PATTERNS` regex entries for config
  keys that no longer exist (`supervision.*`, `rotateWorkerOnRetry`,
  `workers.*.outputMode`).
- Net delete of ~693 lines; maestro stays a conductor that never spawns
  workers.

## 0.37.4 - Inline GitHub release notes

- Publish GitHub releases with readable inline notes instead of only the
  default compare link.
- Prefer the matching `CHANGELOG.md` section for release notes when it
  exists, and fall back to inline commit bullets when it does not.

## 0.36.0 - Mission bundles (phase 1: export + inspect)

- Add `maestro bundle export <missionId>` that packages a mission and
  its artifacts (plan, features, workers, replies, handoffs, principles,
  memory) as a portable `.mission.tar.gz` archive with a v1 manifest.
- Add `maestro bundle inspect <path>` that prints the manifest of a
  bundle without extracting it.
- Support `--out`, `--base <ref>` (include `diff.patch` from
  `<ref>..HEAD`), `--redact <memory|prompts|replies>`, and `--json` on
  the export subcommand.
- Manifest `schemaVersion: 1` captures bundle id, created-at, maestro
  version, mission summary, per-section stats, redaction flags, and an
  optional git patch descriptor so future readers can reject unknown
  versions cleanly.

## 0.35.4 — Typecheck and release metadata alignment

- Fix the repo's standing `bun run typecheck` failures across source and test
  files.
- Align the changelog back to the `0.x.y` version scheme used by
  `package.json` and `src/shared/version.ts`.
- Fix `scripts/bump.ts` so it reports the pre-bump and post-bump versions
  correctly.

## 0.35.3 — Built-in skill handoff link docs

- Add active handoff links between built-in skills.

## 0.35.2 — Mission Control cleanup

Phase 3 of the conductor refactor. The Mission Control TUI was in an
intermediate state after Phase 1 removed the worker execution layer:
the snapshot pipeline still populated empty worker / runtime /
progress-log panes and the preview screen set still advertised
`runtime`, `workers`, and `output` screens whose data stores had been
deleted. This release cleans up the dead screens, the orphaned DTOs,
and the types that described them.

### Removed preview screens

- `runtime`, `workers`, `output` are no longer valid `--preview` values.
  The TUI now renders 7 screens: `dashboard`, `features`,
  `dependencies`, `handoffs`, `config`, `memory`, `graph`.
- The `proc`, `process`, `processes`, `worker`, `out` screen aliases
  were removed. Surviving aliases: `feat`, `handoff`, `cfg`, `deps`,
  `mem`.

### Removed modal kinds

- `"processes"`, `"runtime-output"`, `"workers"` modal kinds were
  deleted from the Mission Control reducer along with their
  command-palette entries, reducer cases, modal builders, and
  input-dispatch hotkeys. The surviving modal kinds are `none`,
  `command-palette`, `feature-action`, `feature-browser`, `overview`,
  `dependencies`, `handoffs`, `config`, `memory`, `graph`.

### Orphaned types removed

From `src/domain/worker-types.ts`:

- `TransportType` union (collapsed to the literal `"cli"` on
  `CliWorkerConfig`).
- `A2aWorkerConfig`, the old `WorkerConfig` union variant.
- `WorkerResult`, `ExecutionRecord`, `RuntimeEventRecord`.
- `WorkerProgressEvent`, `WorkerProgressEventKind`, `FailureClass`.

Kept because the config inspector and `DEFAULT_CONFIG` still render
them: `CliWorkerConfig`, `WorkerConfig` (now an alias for
`CliWorkerConfig`), `WorkerOutputMode`, `ExecutionConfig`,
`SupervisionConfig`, `SupervisionLevel`, `ParallelConfig`.

From `src/tui/state/types.ts`:

- `MissionControlWorkerPane`, `MissionControlRuntimeProcessRow`.
- `MissionControlWorkerHealthRow`, `MissionControlWorkerHealthStatus`,
  `MissionControlWorkerHealthCheck`.
- `activeWorker`, `runtimeProcesses`, `workerHealth` fields on
  `MissionControlSnapshot`.
- `transport` field on `MissionControlSessionSidebar`.
- `runtimeState`, `lastSeenAgeMs`, `failureReason`, `retryCount`,
  `agent`, `sessionId` fields on `TaskPreviewPane`.

### Other removals

- `src/usecases/worker-selection.usecase.ts` and its test. No callers
  survived the Phase 1 strip.
- `validateSupervisionConfig`, `validateParallelConfig`, and
  `isCliWorkerConfig` exports in `src/domain/worker-validators.ts`.
- `workerHealth` input to `buildConfigInspector`. The inspector now
  derives worker availability directly from the CLI worker config via
  `cachedWhich`.
- `workerEvents` input to `deriveEvents`. The timeline reads only
  mission, feature, assertion, and checkpoint timestamps.
- The agent-per-feature aggregation in `buildMissionOverview`
  (`agentSummary` always returns an empty array).
- Live feature auto-follow in `createInitialState` and the reducer's
  `update-snapshot` handler (runtime state is gone, so there is no
  live feature to follow).
- Session sidebar transport/session/agent rendering in the OpenTUI
  builders.
- Faster polling cadence in `getSnapshotPollIntervalMs` (now always
  returns the 5s default because there is no live runtime to track).

### Behavior changes

- `maestro mission-control --preview output --feature <id>` now fails
  with "Unknown preview screen" instead of rendering a runtime output
  stream. The worker output pane was already empty after Phase 1.
- `maestro mission-control --preview runtime` and `--preview workers`
  fail the same way.
- The `O` hotkey inside the runtime modal no longer does anything
  (neither modal exists).

## 0.35.1 — UKI v5.2 handoff system

Phase 2 of the conductor refactor. The single `maestro handoff` subcommand
is back, producing deterministic, machine-readable UKI v5.2 records that
external workers (Claude Code children, Codex, Gemini CLI) can consume via
a single compressed string.

### Added

- `src/lib/uki-format.ts` -- deterministic compressor + parser for the
  UKI v5.2 12-slot single-string format. Pure (no clock, no random, no
  I/O). Validator returns violation list without throwing.
- `src/domain/uki-types.ts` -- `UkiHandoff`, `UkiSlots`, `CreateUkiHandoffInput`,
  `UkiHandoffStatus` domain types.
- `src/ports/handoff-store.port.ts` -- new v2 `HandoffStorePort` (different
  shape than the pre-Phase-1 port; flat-file JSON records under
  `.maestro/handoffs/<id>.json`).
- `src/adapters/handoff-store.adapter.ts` -- filesystem adapter that caches
  the compressed UKI string on each record at create time.
- `src/usecases/{create,pickup,list}-uki-handoff.usecase.ts`.
- `maestro handoff create` with structured slot flags, auto-filling agent
  and session id from `SessionDetectPort`.
- `maestro handoff pickup` with `--json` (default), `--markdown` (human
  briefing), and `--uki` (raw compressed string for piping) output modes.
  `--claim` transitions `pending -> picked-up` atomically.
- `maestro handoff list` with `--status pending|picked-up|completed`.
- Mission Control home pane now shows pending UKI handoffs sourced from
  the new store (`buildSnapshot` and `buildHomeSnapshot` both populate
  `pendingHandoffs` by listing pending records and projecting each via
  `mapUkiHandoffToHomeHandoff`).

### Notes

- The old handoff format (pre-0.35.0) was deleted in Phase 1. Existing
  `.maestro/handoffs/` contents from before the strip become orphaned
  when upgrading -- the new format is a flat-file JSON shape (not the
  previous directory-per-handoff layout) and the compressed string is
  entirely different. There is no migration path by design (documented
  in the plan at `~/.claude/plans/drifting-humming-dream.md`).
- The UKI v5.2 string must contain exactly 11 pipes, zero colons, zero
  newlines, and every `_`-joined token half is capped at 4 words (R2).
- `CS` (confidence) is scoped: `CS-work_X`, `CS-summary_Y`, or
  `CS-work_X~summary_Y`. Bare `CS-N.NN` is rejected (R5).
- `ARTIFACTS` must contain at least one of `commit_`, `branch_`,
  `version_`, or `file_` (R7).
- `STANCE_COLLAPSE` is always emitted; if the caller does not supply a
  value the compressor defaults to `NONE_DETECTED_LOW_FRICTION` (R6).

## 0.35.0 — Phase 1 strip

This release is the conductor-model cutover. Maestro is no longer a harness
that spawns workers; it is a shared mission/memory artifact that external
workers (Claude Code, Codex, Gemini CLI, etc.) read from and write to via the
CLI. The worker-execution layer has been removed wholesale.

### Removed CLI subcommands

- `maestro feature run` (the sequential feature execution engine)
- `maestro handoff`
- `maestro handoff-pickup`
- `maestro handoff-dig`
- `maestro handoff-drop`
- `maestro handoff-cleanup`
- `maestro handoff-report`
- `maestro a2a` (agent-to-agent debug command)

Phase 2 will re-introduce a single `maestro handoff` command that produces
UKI v5.2 format records; this 0.35.0 release intentionally has no handoff
surface at all.

### Removed ports and adapters

- `TransportPort` (`cli-transport`, `a2a-transport`, `multi-transport`)
- `RuntimeStorePort` / `RuntimeEventStorePort` (worker runtime + event stores)
- `ExecutionStorePort` (historic execution records)
- `HandoffStorePort` and `HandoffEnvelope` / `HandoffSession` / `HandoffPlan`
  / `Handoff` domain types (Phase 2 will re-introduce a `HandoffStorePort`
  with a completely different shape keyed on UKI records)
- `CassPort` and all CASS knowledge-store integration
- Runtime supervision stack: `runtime-supervision.usecase`,
  `runtime-recovery.usecase`, `live-runtime-tracking.usecase`
- Worker dispatch: `run-features.usecase`
- Handoff use-cases: `create-handoff`, `pickup-handoff`, `dig-handoff`,
  `report-handoff`, plus the orphaned `generate-prompt` usecase that only
  wrapped the deleted `handoff-pickup --claim` workflow

### Behavior changes

- `feature-lifecycle.updateFeature` no longer writes to a runtime store.
  Feature status updates now touch the feature store only; the runtime
  lease / last-seen / failure-reason fields are gone because there is no
  runtime to supervise.
- `session-detect` simplified: the cwd-fallback, session-id prefix resolve,
  and staleness warning flows are gone. The adapter only reads
  `CLAUDECODE` / `CODEX_THREAD_ID` env vars. Explicit `--session <id>`
  arguments are required wherever a session must be identified outside of
  those two environments.
- `generate-worker-prompt.usecase` no longer takes a `runtimeStore`
  parameter. The memory-injection path (`safeRecallMemory` ->
  `appendMemorySection`) is unchanged and continues to auto-wire into
  `maestro feature prompt <id>`.
- Mission Control worker / runtime / output panes are empty until Phase 3
  removes them outright. The dashboard, features, dependencies, config,
  memory, handoffs, and graph screens remain fully functional.
- Top-level CLI description changed from "Cross-agent handoff CLI" to
  "Conductor CLI".

### Removed devDependencies

- `@a2a-js/sdk`
- `express` and `@types/express`

These shipped only to support the now-deleted A2A transport.

### Upgrade notes

- Existing `.maestro/handoffs/` records are orphaned by this release. There
  is no migration path; they will be re-formatted by Phase 2 when the UKI
  handoff store ships.
- Any scripts that shell out to `maestro feature run`, `maestro handoff *`,
  or `maestro a2a *` must be updated. Use `maestro feature prompt <id>` to
  generate a worker prompt, then run the actual worker in a separate
  terminal (Claude Code, Codex, etc.).
