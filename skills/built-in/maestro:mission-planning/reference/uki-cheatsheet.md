# UKI v5.2 Cheatsheet: The 12 Slots and the Command

This is the sixth step of mission planning. You have a plan, worker assignments, boundaries, and calibrated confidence scores. You need to persist the mission and emit a UKI v5.2 handoff so external workers (Codex, Claude Code children, Gemini, Aider) can pick it up.

The output of this step is a mission file persisted via `maestro mission create` and a UKI handoff string emitted via `maestro handoff create`.

## The format in one line

UKI v5.2 is 12 slots, in fixed order, joined by `|`. Each slot is `NAME-VALUE` (hyphen separates slot name from value). Tokens inside a value use `_` between words. Series use `~` (e.g. `tests_27~41_green` = tests went from 27 to 41 passing). No colons, no newlines, no arbitrary whitespace.

## The 12 slots in order

1. **`SESSION_CORE`** — one-token essence of the work. Single value, no list. Example: `auth_middleware_split`.

2. **`CAUSAL_DRIVERS`** — why this work is happening. List. Example: `user_report_duplicate_renders~memory_leak_investigation`.

3. **`DIVERGENCES`** — conflicts or disagreements during planning. List. Use `NONE` if none occurred. Example: `NONE` or `worker_type_debate_resolved_codex`.

4. **`KEY_DECISIONS`** — design calls made during planning. List. Example: `split_validation_from_permission~keep_middleware_signature`.

5. **`SIGNAL_DELTA`** — measurable changes with `~` for before/after where applicable. List. Example: `callers_14_stable~unit_tests_42_target`.

6. **`ARTIFACTS`** — commit, branch, version, and file references. List. Must contain at least one token starting with `commit_`, `branch_`, `version_`, or `file_`. Example: `branch_feat_auth_split~file_src_auth_middleware_ts`.

7. **`EXECUTION_STATE`** — final state of the work described by this handoff. Single value. For a plan-time handoff, typical values are `plan_drafted`, `plan_approved`, `ready_for_worker`. Example: `plan_drafted`.

8. **`BOUNDARY_STATE`** — what not to touch. List. Example: `preserve_middleware_signature~no_session_store_changes~no_permission_semantics_changes`.

9. **`STANCE_COLLAPSE`** — belief that changed during the work, or `NONE_DETECTED_LOW_FRICTION` if none. Single value. Always present — never omit. Example: `NONE_DETECTED_LOW_FRICTION` or `codex_cli_rejected_for_review_work`.

10. **`NEXT_ACTION`** — one concrete next step. Single value. Example: `assign_feat_001_to_codex_cli_worker`.

11. **`CS`** — confidence scores. Format: `CS-work_0.95~summary_0.90`. Both sub-scores should be present on a plan-time handoff. Never use a bare `CS-0.95`.

12. **`SUMMARY`** — human-readable summary, under 140 characters, `Essence-Progress-Risk` shape. Example: `Auth middleware split drafted; signature preserved; 14 callers need regression pass before code-review milestone.`

## Token rules

- Max 4 words per `_`-link. `auth_middleware_split` is fine. `auth_middleware_split_with_backwards_compat_preserved` is too long — shorten to `auth_split_backwards_compat`.
- No `-` inside a token. The `-` is reserved for the slot-name separator (`SLOT-VALUE`) and for the `CS-work` / `CS-summary` sub-score keys.
- No colons anywhere. Colons are reserved elsewhere in the UKI spec.
- Series use `before~after`. Example: `tests_27~41_green` reads as "tests 27 to 41 green." For enumerations without before/after, just list with `~` as the separator: `driver_one~driver_two~driver_three`.
- Lists within a slot are also `~`-joined. There is no dedicated list delimiter.

## The `maestro handoff create` command template

The exact flags (verified against `./dist/maestro handoff create --help`):

```bash
./dist/maestro handoff create \
  --session-core <token> \
  --summary "<under 140 chars>" \
  --next-action <token> \
  --driver <token> [--driver <token> ...] \
  --divergence <token> [--divergence <token> ...] \
  --decision <token> [--decision <token> ...] \
  --signal <token> [--signal <token> ...] \
  --artifact <token> [--artifact <token> ...] \
  --boundary <token> [--boundary <token> ...] \
  --execution-state <token> \
  --stance-collapse <token> \
  --confidence-work <0..1> \
  --confidence-summary <0..1>
```

Notes on the flags:
- `--session-core` takes one value; all list flags (`--driver`, `--divergence`, `--decision`, `--signal`, `--artifact`, `--boundary`) are repeatable and the command collects them into the slot list.
- `--stance-collapse` defaults to `NONE_DETECTED_LOW_FRICTION` if omitted, but pass it explicitly anyway for clarity.
- `--confidence-work` and `--confidence-summary` are numeric (0..1), not strings, and they produce the `CS-work_X~summary_Y` formatted slot internally.
- `--artifact` must contain at least one token beginning with `commit_`, `branch_`, `version_`, or `file_`. The command rejects the handoff otherwise.

## A full worked example

Plan-time handoff for the auth-middleware-split feature:

```
SESSION_CORE-auth_middleware_split|CAUSAL_DRIVERS-user_report_signature_churn~refactor_debt_audit|DIVERGENCES-NONE|KEY_DECISIONS-split_validation_from_permission~keep_middleware_signature~defer_permission_semantics|SIGNAL_DELTA-callers_14_stable~unit_tests_42_target|ARTIFACTS-branch_feat_auth_split~file_src_auth_middleware_ts|EXECUTION_STATE-plan_drafted|BOUNDARY_STATE-preserve_middleware_signature~no_session_store_changes~no_permission_semantics_changes|STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION|NEXT_ACTION-assign_feat_001_to_codex_cli_worker|CS-work_0.88~summary_0.92|SUMMARY-Auth middleware split drafted; signature preserved; 14 callers need regression pass before code-review.
```

All 12 slots present. `DIVERGENCES-NONE` because planning was smooth. `STANCE_COLLAPSE-NONE_DETECTED_LOW_FRICTION` because no belief changed. `ARTIFACTS` has two tokens, one `branch_` and one `file_`. `SUMMARY` is 111 characters, inside the 140 limit, and reads as `Essence (auth middleware split drafted) - Progress (signature preserved) - Risk (14 callers need regression pass)`.

## Common mistakes

- **Forgetting `STANCE_COLLAPSE`.** It is always present. If nothing changed your mind, use `NONE_DETECTED_LOW_FRICTION`. Do not omit the slot.
- **Empty `ARTIFACTS`.** At least one `commit_`, `branch_`, `version_`, or `file_` token is required. Plan-time handoffs usually cite a branch or a file path.
- **`SUMMARY` over 140 chars.** The slot has a hard limit. Cut words, not meaning.
- **Bare `CS-0.95` instead of scoped `CS-work_0.95`.** The v5.2 spec separates work confidence from summary confidence. Always pass both sub-scores; the bare form is v5.0 and will not validate.
- **Tokens with dashes inside them.** `no-caching-outside-scope` is wrong. Use `no_caching_outside_scope`. Dashes are reserved.
