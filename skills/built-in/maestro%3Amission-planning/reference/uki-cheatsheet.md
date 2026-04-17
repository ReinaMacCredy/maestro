# UKI v5.4 Plan Cheatsheet: The 15 Slots and the Command

This is the sixth step of mission planning. You have a plan, worker assignments, boundaries, and calibrated confidence scores. You need to persist the mission and emit a UKI v5.4 plan handoff so external workers (Codex, Claude Code children, Gemini, Aider) can pick it up.

The output of this step is a mission file persisted via `maestro mission create` and a UKI handoff string emitted via `maestro handoff create`.

## The format in one line

UKI v5.4 plan mode is 15 slots, in fixed order, joined by `|`. Each slot is `NAME-VALUE`. List slots render their tokens with `-` separators inside the value. The `CS` slot uses `~` only between `work_` and `summary_` sub-scores. Series inside a single token can still use `~` (e.g. `tests_27~41_green`). No colons, no newlines, no backticks, and no `|` characters.

## The 15 plan-mode slots in order

1. **`MODE`** — `plan` for this skill. Single value. Example: `plan`.

2. **`CURRENT_STATE`** — the baton-pass state. Plan handoffs usually use `plan_ready`. Example: `plan_ready`.

3. **`SESSION_CORE`** — one-token essence of the work. Single value. Example: `auth_middleware_split`.

4. **`CAUSAL_DRIVERS`** — why this work is happening. List. Use `NONE` if empty. Example: `user_report_duplicate_renders-refactor_debt_audit`.

5. **`DIVERGENCES`** — conflicts or disagreements during planning. List. Use `NONE` if none occurred. Example: `NONE` or `worker_type_debate_resolved_codex`.

6. **`MAESTRO_REFS`** — structured Maestro anchors. Tokens are auto-prefixed as `mission_`, `feature_`, `milestone_`, `plan_`, or `spec_`. Use `NONE` if you truly have no refs. Example: `mission_mis_01h8k2f9-plan_plan_auth_split_json`.

7. **`PLAN_PATHS`** — token-safe plan anchors. List. Use `NONE` if no plan token exists, but this skill should usually emit one. Example: `plan_auth_split_json`.

8. **`MAESTRO_SYNC`** — state sync milestones worth carrying forward. List. Use `NONE` if none apply. Example: `mission_created-mission_approved`.

9. **`DECISIONS`** — design calls made during planning. List. Use `NONE` if empty. Example: `split_validation_from_permission-keep_middleware_signature`.

10. **`SIGNAL_DELTA`** — measurable changes. Each token can still carry a `before~after` series when useful. Example: `callers_14_stable-tests_27~41_green`.

11. **`ARTIFACTS`** — commit, branch, version, file, mission, and related anchors. List. This slot must not be empty. Example: `branch_feat_auth_split-file_plans_auth_split_json-mission_mis_01h8k2f9`.

12. **`READ_MORE`** — follow-up anchors for the next worker. List. This slot must not be empty. Example: `plan_auth_split_json-file_plans_auth_split_json`.

13. **`NEXT_ACTION`** — one concrete next step. Single value. Example: `assign_feat_001_codex_cli`.

14. **`CS`** — confidence scores. Format: `CS-work_0.95~summary_0.90`. Plan handoffs should include both sub-scores.

15. **`SUMMARY`** — human-readable summary, under 140 characters, `Essence-Progress-Risk` shape. Example: `Auth middleware split drafted; signature preserved; 14 callers need regression pass before code-review.`

## Token rules

- Max 6 words per token. `auth_middleware_split` is fine. `auth_middleware_split_with_backwards_compat_preserved_everywhere` is too long — shorten it.
- No `-` inside a token. In v5.4, `-` is both the slot-name separator and the list-item separator inside list slots.
- No colons, newlines, backticks, or `|` anywhere in slot values.
- Use `_` inside tokens and reserve `~` for sub-series inside one token (`tests_27~41_green`) or inside `CS`.
- `ARTIFACTS` must contain at least one anchor. `READ_MORE` must contain at least one anchor.

## The `maestro handoff create` command template

The exact flags (verified against `maestro handoff create --help`):

```bash
maestro handoff create \
  --mode plan \
  --session-core <token> \
  --current-state <token> \
  --summary "<under 140 chars>" \
  --next-action <token> \
  --driver <token> [--driver <token> ...] \
  --divergence <token> [--divergence <token> ...] \
  --mission-id <token> \
  --feature-id <token> \
  --milestone-id <token> \
  --plan-ref <token> \
  --spec-ref <token> \
  --plan-path-item <token> [--plan-path-item <token> ...] \
  --maestro-sync <token> [--maestro-sync <token> ...] \
  --decision <token> [--decision <token> ...] \
  --signal <token> [--signal <token> ...] \
  --artifact <token> [--artifact <token> ...] \
  --read-more <token> [--read-more <token> ...] \
  --boundary <token> [--boundary <token> ...] \
  --risk <token> [--risk <token> ...] \
  --confidence-work <0..1> \
  --confidence-summary <0..1>
```

Notes on the flags:
- `--mode plan` is required for the plan packet described here.
- `--current-state` defaults to `plan_ready` for plan mode if omitted, but pass it explicitly when you want the packet to be fully obvious in the command itself.
- `--session-core`, `--summary`, and `--next-action` are single values. Most other slot flags are repeatable and become list slots.
- `--mission-id`, `--feature-id`, `--milestone-id`, `--plan-ref`, and `--spec-ref` do not render as standalone slots; they populate `MAESTRO_REFS`.
- `--plan-path-item` populates `PLAN_PATHS`. `--maestro-sync` populates `MAESTRO_SYNC`.
- `--artifact` and `--read-more` can be supplied manually, but the command also auto-collects branch, file, mission, and plan-path anchors when it can.
- `--confidence-work` and `--confidence-summary` are numeric (0..1), not strings, and they produce the `CS-work_X~summary_Y` formatted slot internally.

## A full worked example

Plan-time handoff for the auth-middleware-split feature:

```
MODE-plan|CURRENT_STATE-plan_ready|SESSION_CORE-auth_middleware_split|CAUSAL_DRIVERS-user_report_signature_churn-refactor_debt_audit|DIVERGENCES-NONE|MAESTRO_REFS-mission_mis_01h8k2f9-plan_plan_auth_split_json|PLAN_PATHS-plan_auth_split_json|MAESTRO_SYNC-mission_created|DECISIONS-split_validation_from_permission-keep_middleware_signature-defer_permission_semantics|SIGNAL_DELTA-callers_14_stable-unit_tests_42_target|ARTIFACTS-branch_feat_auth_split-file_plans_auth_split_json-mission_mis_01h8k2f9|READ_MORE-plan_auth_split_json-file_plans_auth_split_json|NEXT_ACTION-assign_feat_001_codex_cli|CS-work_0.88~summary_0.92|SUMMARY-Auth middleware split drafted; signature preserved; 14 callers need regression pass before code-review.
```

All 15 plan slots are present. `DIVERGENCES-NONE` because planning was smooth. `MAESTRO_REFS` carries the mission and plan anchors. `ARTIFACTS` is non-empty and `READ_MORE` points the next worker to the plan. `SUMMARY` stays under 140 characters and reads as `Essence (auth middleware split drafted) - Progress (signature preserved) - Risk (14 callers need regression pass)`.

## Common mistakes

- **Writing a v5.2 packet.** Plan mode now starts with `MODE-plan|CURRENT_STATE-plan_ready|...` and includes `MAESTRO_REFS`, `PLAN_PATHS`, `MAESTRO_SYNC`, and `READ_MORE`.
- **Using `~` as the list delimiter.** In v5.4 list slots are hyphen-joined. Keep `~` for `CS` and intra-token series such as `tests_27~41_green`.
- **Empty `ARTIFACTS` or `READ_MORE`.** Both slots are required in the rendered packet. Give the next worker something concrete to inspect.
- **`SUMMARY` over 140 chars.** The slot has a hard limit. Cut words, not meaning.
- **Bare `CS-0.95` instead of scoped `CS-work_0.95`.** v5.4 still requires scoped confidence values. Plan handoffs should usually carry both `work` and `summary`.
- **Tokens with dashes inside them.** `no-caching-outside-scope` is wrong. Use `no_caching_outside_scope`. Dashes are reserved.
