# Simplify Maestro CLI surface for agents

## Current state

Evidence mapped 2026-06-20:
- CLI exposes both canonical grouped card verbs and hidden flat aliases: RootCommand has hidden ready/list/claim/create/show/update/close, while CardCommand repeats the same handlers under maestro card; dispatch shares byte-identical handlers (src/interfaces/cli/mod.rs:247-331, 510-540, 1620-1648).
- Status/task-next already compute a next action and copy-paste command, plus harness friction and ready-to-ship feature hints (src/interfaces/cli/status.rs:57-166, 209-361).
- task complete records proof and auto-runs task verify, but card update --status still accepts task fine states including needs_verification and verified (src/interfaces/cli/task.rs:469-521; src/interfaces/cli/card.rs:483-607; src/domain/card/query.rs:66-81).
- The task lifecycle says verified is owned by maestro task verify and direct verified transition should be refused (src/domain/task/lifecycle.rs:145-150), creating a surface mismatch with generic card status words.
- feature verify requires paired repeated flags for --prove/--evidence and --waive/--reason, then may auto-ship unless --no-ship (src/interfaces/cli/feature.rs:254-391).
- feature prepare reads a markdown plan with Task/check/covers/blocker/after fields and fails if no checks or unknown after refs exist (src/operations/feature_prepare.rs:338-400, 625-645).
- Cross-agent coordination needs active -> link add -> msg send/read and sender is implicit current card, not a named argument (README.md:215-274; src/interfaces/cli/msg.rs:1-69).
- Retired task archive/unarchive remain visible in the task command shape but immediately bail with a retirement message (src/interfaces/cli/mod.rs:790-797; src/interfaces/cli/task.rs:159-164, 735-743).

## Problem

## Proposed shape

Integrated design draft:
1. Add a canonical agent loop command: maestro next. It should be a facade over the existing status/task-next report, not a separate planner. Default output is concise human text; --json emits the same/additive report envelope; optional execution must only run one non-template, non-input command.
2. Make lifecycle guardrails impossible to bypass below the CLI. Preserve hidden flat aliases for compatibility, but generic card status updates should reject lifecycle-owned transitions such as verified and route users to maestro task verify, task complete, card close, or per-type feature/decision verbs.
3. Add validated helper surfaces instead of hand-editing artifact syntax: feature prepare inline task inputs, qa baseline/slice helpers, and proof add syntax that maps to the same domain/operation records used today.
4. Simplify coordination without auto-mutation: active can print connect-me/link suggestions; msg send can gain an explicit from assertion only if it matches the current card; message failures should suggest link add but not create links automatically.
5. Hide dead ends from visible help: retired task archive/unarchive should remain parse-compatible if needed, but no longer appear as normal discoverable verbs.
Compatibility constraints:
- Do not remove flat hidden aliases abruptly.
- Do not make next --run execute commands requiring proof, outcome text, QA baseline text, or plan review.
- Do not move domain rules into CLI-only validation.
- Do not obscure local-first durable artifacts; helpers should write or preview the same files/contracts.

## Design: maestro next modes

Mode 1: suggest-only default.
- Command: maestro next [--json]
- Contract: read-only, one best action plus blockers/warnings.
- Output shape: action kind, display command, requires_input, reason, inspect command, related feature/task, broader repo alerts.
- Purpose: the canonical command an agent runs after every step.

Mode 2: one safe step.
- Command: maestro next --run [--json]
- Contract: run exactly one action only if the next action is allowlisted as auto_safe and requires_input=false.
- Refuse with explanation when the next action needs proof text, QA text, plan review, outcome text, user choice, conflict handling, link creation, archive/ship, or external suite approval.
- Candidate auto_safe actions: claim the next ready card for the current session, refresh a read-model cache if one exists, run a pure sweep that does not ship, or execute a domain-owned recovery that is already idempotent and non-interactive.

Mode 3: bounded loop.
- Command: maestro next --loop [--max-steps N] [--json]
- Contract: repeat the same auto_safe policy as --run until blocked, no action remains, max steps reached, or a warning class requires user/agent input.
- Always prints a transcript of actions taken and the final blocker.
- Never crosses into auto-ship, archive, destructive cleanup, QA baseline authoring, feature prepare from unreviewed text, or proof/waiver invention.

Implementation principle:
- Next must reuse the existing StatusReport/NextAction path where possible. It should not become a second planner with divergent semantics.

## Design: lifecycle guardrails

Chosen policy: guided compatibility.
- Keep generic card update for low-risk, compatibility-preserving task-like status edits.
- Reject lifecycle-owned gated transitions rather than silently setting them through the generic card surface.
- Initial hard rejects:
  - verified: route to maestro task verify <id>.
  - needs_verification: route to maestro task complete <id> with summary/proof, or a typed recovery verb if one exists.
- Preserve hidden flat aliases and card grouped verbs so existing agents do not lose parsing compatibility.
- Move the guard below CLI-only validation where possible, so every adapter respects the same lifecycle ownership.
- Error style should be instructional, not punitive: name current state, refused target, owning typed verb, and copy-paste retry command.

## Design: validated feature-flow helpers

Chosen policy: validated helpers.
- Add helper commands that write existing durable artifacts from explicit structured input.
- Helpers must call the same domain/operation validation used by manual artifact paths.
- Prepare helper:
  - Supports one or more structured task inputs with title, check, covers, blocker, after.
  - Provides preview/dry-run for multi-task writes.
  - Internally produces the same task creation/prepare report as a reviewed plan file.
- QA helper:
  - Writes qa.md baseline and slice records without requiring hand-authored frontmatter/YAML.
  - Requires observed behavior text; never invents evidence.
- Proof helper:
  - Replaces paired repeated flags with explicit proof records such as ac id plus evidence text.
  - Must still warn when proof completion would trigger auto-ship unless no-ship is passed.
- Non-goal: no one-command feature advance that combines baseline, prepare, proof, and ship.

## Design: coordination simplification

Chosen policy: advisory plus assertions.
- active may gain a connect-oriented view that prints exact link/msg/conflict commands for live peer sessions.
- msg send may gain --from <card> as an assertion, not an override. If --from does not match the running session's current card, fail with the current card and touch/claim remedy.
- Sending to an unlinked peer should print the exact link add command, but must not auto-link.
- Conflict guidance remains explicit and advisory; no hidden git or worktree operations.
- JSON output must keep stdout parseable; ambient banners remain stderr-only.
