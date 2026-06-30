---
name: maestro-card
version: 1.37.5
description: "Active Maestro card work after design approval: use for implement, fix, verify, QA, close, release, continue, or unattended prompts like use loop, keep looping, work while away/asleep."
---

# Maestro Card

Maestro uses three work levels: High = Card, Mid = CardKind / workflow kind,
and Low = Task. Feature, Bug, Chore, Custom, Decision, Idea, and Progress are
CardKinds, not separate high-level objects. Progress is a lightweight CardKind
that stores many low Tasks in `progress.yml`; legacy `type: task` cards remain
readable for compatibility. This skill covers the active-work cluster: the task
work loop, card/feature lifecycle, proof, and QA gates. Design
(`maestro-design`), audit (`maestro-audit`), and setup (`maestro-setup`) have
their own skills.

Activate with a known session id:
`maestro hook record --event skill_activation --skill maestro-card --session <session_id>`

First step in a session: run `maestro active` (pull-only) to see what other
live sessions are working on before you claim. If a peer is on a related card,
connect yours with `maestro link add <your-card> <their-card>`; maestro never
auto-links. Once linked, coordinate through the channel: `maestro msg send
<their-card> "<text>"` and `maestro msg read`. An `[inbox] N new (...) ->
maestro msg read` line on STDERR before any command means a linked peer is
waiting -- clear it with `maestro msg read` (see [reference/work.md](reference/work.md)).
Inbox messages are advisory coordination only: they can suggest an ordering
relationship, but they do not block work. Record an explicit Task blocker when
execution order matters. Reply when the message poses a question or needs a
decision; an FYI needs no reply.
When any other session is live as you start implementing, follow the
conflict-handoff protocol in HARNESS.md: worktree-isolate, link + `maestro
conflict` on a file you will share, merge back then `--clear`. The full dance
(including a conflicted merge-back) is `maestro loop show conflict-handoff`.

Recipe checkpoint: choose the shipped lifecycle recipe before acting. Use
`maestro loop show work` for task/card implementation, `maestro loop show ship`
before close/release/archive gates, `maestro loop show unattended` for away-mode
autonomy, and `maestro loop show learning` when recording reusable lessons.
Custom card/run recipes are allowed only when no shipped recipe fits, and must
keep the same six phases, current Maestro verbs, hard stops, and continue
output. Work Lease is only a choose-phase helper; it may select or reserve one
safe unit, but it is not a scheduler, daemon, queue, worker launcher, executor,
hidden store, or second lifecycle.

## Route

Read the reference for the job at hand; they share the ground rules below.

- Pick up, progress, finish, or unblock executable Tasks:
  [reference/work.md](reference/work.md). Its implement step is test-first
  (red-green-refactor) whenever the task's `--check` names observable
  behavior: [reference/tdd.md](reference/tdd.md).
- Track simple work with the low-ceremony Task surface (`task add` -> `task
  start` -> `task done`, no separate todo namespace): this creates or reuses a
  Progress card and stores low Tasks in `progress.yml`. Installed hooks
  auto-start this Progress path on first write-like tool use when no current
  Task is set; see the "Simple Task Board" section of
  [reference/work.md](reference/work.md).
- Tidy a card's diff before proving it (quality cleanup, applied in place):
  [reference/simplify.md](reference/simplify.md). On a test-first card this is
  the red-green-refactor step, not a second pass.
- Work the backlog unattended, including "use loop", "keep looping",
  "I am going away", "I am going to sleep", "work while I am away", or broad
  user goals that must first compile into Maestro records while the user is
  away or asleep:
  [reference/loop.md](reference/loop.md)
- Finalize, accept, prepare, amend, close, or archive a feature card after design:
  [reference/feature.md](reference/feature.md)
- Prove a claim, repair failed proof, or verify adversarially:
  [reference/verify.md](reference/verify.md)
- Capture the behavior contract before `feature accept`:
  [reference/qa-baseline.md](reference/qa-baseline.md)
- Replay scenarios and record slice evidence before `feature close`:
  [reference/qa-slice.md](reference/qa-slice.md)
- Intake an external spec, plan, or PRD after design approval:
  [reference/intake.md](reference/intake.md)

## Shared Ground

- Prefer native Maestro MCP tools for lifecycle reads and writes when the host
  exposes them. The host-loaded tool schema is authoritative. Use CLI commands
  when MCP is unavailable, for verbs not yet exposed as MCP tools, or when
  debugging unsupported behavior.
- Exact command signatures live in [reference/cli.md](reference/cli.md),
  generated from the binary. A verb or flag not listed there does not exist;
  read it instead of probing `--help`. CLI remains the compatibility and
  human-facing contract; MCP is the agent ergonomic contract.
- Discover executable work with `maestro task list`, `maestro task next`, and
  `maestro card list` for card-container context. Progress-backed low Tasks
  appear in task views as routine `REF` rows; use `task list --json` when you
  need stable ids. The Progress card itself appears in card views. Take and annotate tasks with
  `maestro task start`/`maestro task claim`, `maestro task update`, and
  `maestro task note`.
- Ids are stable and opaque (`card-<hash>`; features keep their creation
  slug). The dotted alias `show` prints is display-only; never address a card
  with it.
- Never chain a guessed id: use only refs read from `task list` for immediate
  Task subcommands, or ids read from verb output (`task add --id-only`, `task
  list --json`, `card list`, `card show`). When a lookup misses, re-list and
  read the real id; do not retry spelling variations.
- Do not hand-edit `card.yaml` or the verb-guarded sidecars (`qa.md`,
  state history). Use verbs so gates and audit trails stay intact.
- Terminal words are per type — feature `closed`/`cancelled`, task
  `verified`/`rejected`/`abandoned`/`superseded`, decision
  `locked`/`superseded`, and Bug/Chore/Custom card containers `closed` after
  owned tasks verify — and all of them read as coarse `closed` on the board.
  `maestro card close` fits only legacy task cards or task-owning
  Bug/Chore/Custom containers whose owned tasks are verified.
  When the user says "close" a feature, branch on its state: a live feature
  means `feature close` or `feature cancel`; a feature already terminal
  (closed/cancelled) means archive it. If the current request, accepted
  contract, SPEC, or run record grants bounded ship or auto-archive authority
  for this target, do not finish at "closed" and do not ask again: after the
  requested push/publish/release/local-install/handoff boundary completes and
  the delivered commit hash is known, run `maestro feature auto-archive <id>`
  as the next lifecycle step. If no such authority exists, run
  `maestro card archive <id>` only when the user's terminal "close" wording is
  explicit archive intent. Archive is never a blind close/cancel side effect,
  and a non-terminal feature is never archived.
- For preauthorized auto-archive, use `maestro feature auto-archive <id>` with
  a current target-scoped authority (`--authority-ref`, `--authority-target`,
  `--authority-head`, `--authority-state current`), exact QA evidence
  (`--tested-head`, `--qa-result pass`, repeat `--qa-evidence`), the owning
  run/worktree disposition (`--run`, `--multi-agent`, `--worker-source`), and
  the current store that owns the target card
  (`--canonical-store <path-to-current/.maestro>`). A linked implementation
  worktree may auto-archive when its current `.maestro` store owns the target,
  the work is done, and evidence names the exact current `HEAD`. Stop instead of
  archiving if the helper refuses, if relevant worktree state is dirty, if
  worker changes are not represented in the current target `HEAD`, if the
  current store does not own the target card, if relevant Maestro conflicts are
  still asserted, or if terminal archive preflight fails.
- When the user corrects or steers active work, do not pause just because they
  corrected you. If the correction is clear, record it with `maestro event
  intervention --note "<what changed>" [--topic <slug>]` and apply it. If it is
  unclear but low-risk, state the assumption, record it, and continue. Ask only
  when the ambiguity can change scope, contract, schema, lifecycle, release
  behavior, or other hard-to-reverse work. Full routing lives in
  [reference/work.md](reference/work.md).

## Pipeline

`maestro-design -> feature finalize -> [maestro-card: qa-baseline -> feature accept -> prepare -> work -> verify -> qa-slice -> feature close]`
