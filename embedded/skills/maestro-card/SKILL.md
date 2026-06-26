---
name: maestro-card
version: 1.29.0
description: "Use when the user wants to implement, fix, verify, QA, close, release, or continue active Maestro card/feature/task work after design is approved."
---

# Maestro Card

Maestro splits planning containers from executable work. Cards are the
mid/high-level containers and records (`feature`, `bug`, `chore`, `custom`,
`decision`, plus backlog ideas); Tasks are the atomic executable units. This
skill covers the active-work cluster: the task work loop, card/feature
lifecycle, proof, and QA gates. Design (`maestro-design`), audit
(`maestro-audit`), and setup (`maestro-setup`) have their own skills.

Activate with a known session id:
`maestro hook record --event skill_activation --skill maestro-card --session <session_id>`

First step in a session: run `maestro active` (pull-only) to see what other
live sessions are working on before you claim. If a peer is on a related card,
connect yours with `maestro link add <your-card> <their-card>`; maestro never
auto-links. Once linked, coordinate through the channel: `maestro msg send
<their-card> "<text>"` and `maestro msg read`. An `[inbox] N new (...) ->
maestro msg read` line on STDERR before any command means a linked peer is
waiting -- clear it with `maestro msg read` (see [reference/work.md](reference/work.md)).
Reply when the message poses a question or needs a decision; an FYI needs no reply.
When any other session is live as you start implementing, follow the
conflict-handoff protocol in HARNESS.md: worktree-isolate, link + `maestro
conflict` on a file you will share, merge back then `--clear`. The full dance
(including a conflicted merge-back) is `maestro loop show conflict-handoff`.

## Route

Read the reference for the job at hand; they share the ground rules below.

- Pick up, progress, finish, or unblock executable Tasks:
  [reference/work.md](reference/work.md). Its implement step is test-first
  (red-green-refactor) whenever the task's `--check` names observable
  behavior: [reference/tdd.md](reference/tdd.md).
- Track simple work with the low-ceremony Task surface (`task add` -> `task
  start` -> `task done`, no separate todo namespace): the "Simple Task Board"
  section of [reference/work.md](reference/work.md).
- Tidy a card's diff before proving it (quality cleanup, applied in place):
  [reference/simplify.md](reference/simplify.md). On a test-first card this is
  the red-green-refactor step, not a second pass.
- Work the backlog unattended, including broad user goals that must first
  compile into Maestro records while the user is away or asleep:
  [reference/loop.md](reference/loop.md)
- Finalize, accept, prepare, amend, close, or archive a feature card after design:
  [reference/feature.md](reference/feature.md)
- Prove a claim, repair failed proof, or verify adversarially:
  [reference/verify.md](reference/verify.md)
- Capture the behavior contract before `feature accept`:
  [reference/qa-baseline.md](reference/qa-baseline.md)
- Replay scenarios and record slice evidence before `feature close`:
  [reference/qa-slice.md](reference/qa-slice.md)

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
  `maestro card list` for card-container context. Take and annotate tasks with
  `maestro task start`/`maestro task claim`, `maestro task update`, and
  `maestro task note`.
- Ids are stable and opaque (`card-<hash>`; features keep their creation
  slug). The dotted alias `show` prints is display-only; never address a card
  with it.
- Never chain a guessed id: use only ids read from verb output (`task add
  --id-only`, `task list`, `card list`, `card show`). When a lookup misses,
  re-list and read the real id; do not retry spelling variations.
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
  (closed/cancelled) means archive it — run `maestro card archive <id>` directly,
  do not re-ask. `maestro card archive` moves a terminal feature's records to
  `.maestro/archive/`; the user's word "close" on a terminal card is the
  explicit intent to archive, but archive is never an automatic side effect of
  close/cancel, and a non-terminal feature is never archived.
- When the user corrects your behavior, record it:
  `maestro event intervention --note "<what was wrong>" [--topic <slug>]`.

## External intake

When the user brings a spec, plan, or PRD authored elsewhere, route open forks
through `maestro-design` first. This skill consumes the approved contract and
drives the active lifecycle; there is no CLI parser for external documents.

1. Use `maestro-design` to create the feature, preserve the source text, decide
   open forks, and author observable acceptance criteria.
2. Return here after the contract is stable.
3. Read `.maestro/cards/<id>/handoff.md` first. If it is missing or stale, run
   `maestro feature finalize <id>`.
4. Run `qa-baseline`, `feature accept`, `feature prepare`, work, verify,
   `qa-slice`, and `feature close`.

`request.md` travels with the card through archive and unarchive.

## Pipeline

`maestro-design -> feature finalize -> [maestro-card: qa-baseline -> feature accept -> prepare -> work -> verify -> qa-slice -> feature close]`
