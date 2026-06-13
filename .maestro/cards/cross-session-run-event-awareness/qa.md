---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: cross-session-run-event-awareness -- new `maestro active` read verb,
  `card_touch` auto-emit on card-mutating verbs, D8 verbose/terse `record`
  echo, and the D9 `cli_run_id()` session-identity fix. Touches
  `src/interfaces/cli` (new subcommand + `hook.rs` echo + `mod.rs:1253`),
  `src/domain/run/{reader,record,event}.rs`, and card-verb call sites.
- Critical workflow chains:
  - Parallel-session awareness (trunk journey this feature exists for)
    - Steps: session A activates skill + touches a card (emits run events) ->
      session B starts -> B runs `maestro active` -> B sees A's row with card
      + progress + presence -> B runs the printed `maestro link add` hint.
    - Touched link: D7 link-hint follow-up (no link edge created by `active`
      itself).
    - Minimal proof: drive two `maestro` invocations under distinct
      `CLAUDE_CODE_SESSION_ID` values, then `maestro active` and assert two
      rows; confirm no link edge was auto-created.
  - Existing recording chain (must not regress)
    - Steps: hook fires -> `record.sh` -> `maestro hook record` (stdin) ->
      `runs/<session_id>/events.jsonl` append -> Stop writes
      `run_evidence.yaml`.
    - Touched link: `record.rs`/`event.rs` echo + `card_touch`; `cli_run_id()`.
    - Minimal proof: feed a payload with `session_id` on stdin, assert it lands
      in `runs/<session_id>/events.jsonl` (NOT a `cli-<date>` bucket).
- Scenario Matrix:
  - [bl-001] active lists live sessions, one row per session (covers: ac-1)
    - Dimensions: actor (peer session), entrypoint (new verb), data (digest row)
    - Setup: a repo with two run buckets each holding a recent event, one a
      skill_activation (mode), one with a card_touch (bound card).
    - Action: `maestro active`
    - Oracle: two rows, each showing session id, mode, bound card title +
      status, type-aware progress, last action, last-activity age, and a
      presence label; the running session is marked `you`.
    - Evidence to capture: stdout of `maestro active`.
    - Reproduction: seed buckets, run the verb.
  - [bl-002] --all reveals stale, hidden by default (covers: ac-2)
    - Dimensions: state/lifecycle (recency window), entrypoint (flag)
    - Setup: one bucket whose latest event is older than the window.
    - Action: `maestro active` then `maestro active --all`
    - Oracle: stale session absent from the first, present in the second tagged
      `[stale Nm]`.
    - Evidence to capture: both stdouts (diff shows the stale row only under
      `--all`).
    - Reproduction: backdate an event's ts, run both.
  - [bl-003] liveness is activity-aware, recent Stop = [waiting] not excluded (covers: ac-3)
    - Dimensions: state (event-kind), non-functional (no new hook)
    - Setup: a bucket whose latest event is a recent `Stop`.
    - Action: `maestro active`
    - Oracle: the session is present and labelled `[waiting]` (NOT filtered
      out); no `SessionEnd` hook exists in the event contract.
    - Evidence to capture: stdout + confirmation that
      `run::hook_event_contract()` adds no SessionEnd.
    - Reproduction: seed a recent Stop event, run the verb.
  - [bl-004] card verbs auto-emit a card_touch tagged session+card; latest-touch-wins (covers: ac-4)
    - Dimensions: integration boundary (verb side effect), data (event tag)
    - Setup: a feature/card present.
    - Action: `maestro card update <id> --description x` then a second
      card-mutating verb on a different card.
    - Oracle: each verb appends a `card_touch` event carrying the session id and
      the touched card id; the session's "current card" resolves to the most
      recent one. The verb itself returns success regardless of emit outcome.
    - Evidence to capture: tail of the session's events.jsonl; verb exit status.
    - Reproduction: run the verbs, inspect the JSONL.
  - [bl-005] active prints a copy-pasteable link hint, never auto-links (covers: ac-5)
    - Dimensions: trust (no side effect), data (hint text)
    - Setup: at least one shown peer session bound to a card.
    - Action: `maestro active`
    - Oracle: output contains a `maestro link add <your-card> <their-card>`
      template referencing the peer's card id; no `related`/link edge is created
      as a side effect (no relatedness guess computed).
    - Evidence to capture: stdout + `maestro show` of both cards before/after
      (edges unchanged).
    - Reproduction: run `active`, diff card edges.
  - [bl-006] record echo: verbose block for low-frequency, terse line for firehose (covers: ac-6)
    - Dimensions: channel (CLI echo), data (output shape)
    - Setup: none.
    - Action: `maestro hook record --event skill_activation --skill X`; then a
      synthetic high-frequency event (PostToolUse-style payload on stdin).
    - Oracle: the skill_activation prints a multi-line block (event, skill,
      session, bound card, run dir, `maestro active` tip); the PostToolUse path
      prints a single terse line.
    - Evidence to capture: both stdouts.
    - Reproduction: run both paths.
  - [bl-007] nothing fires automatically at session start; active is pull-only (covers: ac-7)
    - Dimensions: non-functional (passivity), entrypoint
    - Setup: none.
    - Action: trigger a SessionStart record.
    - Oracle: SessionStart does not invoke `active`; the awareness view runs
      only when the agent calls it.
    - Evidence to capture: no `active` output emitted by SessionStart recording.
    - Reproduction: record a SessionStart, confirm no auto-listing.
  - [bl-008] parallel sessions yield distinct rows, not one merged cli-<date> (covers: ac-8, D9)
    - Dimensions: environment (env var), integration (identity), data (bucketing)
    - Setup: two invocations differing only in `CLAUDE_CODE_SESSION_ID`.
    - Action: each runs a card-mutating verb, then `maestro active`.
    - Oracle: two distinct session buckets (named by the two ids), two rows in
      `active`; neither falls into `cli-<date>`.
    - Evidence to capture: `ls .maestro/runs/` + `maestro active` stdout.
    - Reproduction: set the env var per invocation, inspect buckets.
- Preserved behaviors:
  - Hook stdin recording lands events in `runs/<payload session_id>/` -> Proof:
    `printf '{"hook_event_name":"PreToolUse","session_id":"abc","tool_name":"Read"}' | maestro hook record` writes `runs/abc/events.jsonl`.
  - Card/feature/decision/claim verbs still succeed after auto-emit is added;
    emit is best-effort and never aborts the verb -> Proof: `maestro feature set
    <id> --description x` exits 0 even if event append fails (mirror the
    existing `record_hook` warn-and-continue at hook.rs:19-21).
  - `cli_run_id()` fallback chain intact when `CLAUDE_CODE_SESSION_ID` is empty
    (Codex / plain shell) -> Proof: with that var unset, a CLI-path record still
    resolves `CODEX_SESSION_ID` etc., else `cli-<date>`.
  - `run_dir_name()` encodes a UUID-form id (hyphens) to a filesystem-safe dir
    that cannot escape `.maestro/runs/` -> Proof: `df536043-8689-...` resolves
    in-tree; existing append.rs path-escape tests still pass.
  - `claim_session()` (mod.rs:1280) unchanged; claim identity stays per-process
    -> Proof: static read confirms no edit; a `maestro claim` still works.
  - `maestro doctor` passes on a correctly-installed repo -> Proof: `maestro
    doctor` exits 0 (note: doctor content-check just landed in e86fe5c3).
- Changed behaviors:
  - `cli_run_id()` resolves `CLAUDE_CODE_SESSION_ID` ahead of `cli-<date>` in a
    real Claude session (D9) -- intentional.
  - `maestro hook record` echo changes from the single line at hook.rs:58 to a
    verbose-block / terse-line split (D8) -- intentional; downstream
    `tests/cli_help.rs` and `tests/card_commands_integration.rs` assertions on
    the old output must be updated, not silently broken.
- Critical probes before commit:
  - Auto-emit is non-fatal -> simulate event-append failure, assert the
    card-mutating verb still exits 0.
  - D9 fallback with empty `CLAUDE_CODE_SESSION_ID` -> assert `cli-<date>` is
    still produced (no panic, no empty id).
  - `run_dir_name()` on a hyphenated UUID -> assert no path escape.
- Required artifacts:
  - None (no fixtures; scenarios seed `.maestro/runs/` inline).
- Baseline gaps:
  - True OS-level parallelism (ac-8) can't run in one process -> Proposed
    probe: two sequential invocations under distinct `CLAUDE_CODE_SESSION_ID`
    approximate two sessions; assert two buckets/rows.
  - The tool firehose ([working] in ac-3, terse path in ac-6) can't be
    exercised until hooks are routed to `record.sh` (Preconditions section) ->
    Proposed probe: feed a synthetic PostToolUse payload to `record` on stdin,
    bypassing the unwired hook.
