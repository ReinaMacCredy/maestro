---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: simplify-maestro-cli-surface-for-agents and the agent-facing Maestro CLI workflow.
- Critical workflow chains:
  - Agent work loop
    - Steps: inspect next action -> optionally run one safe action -> implement/complete manually when input is required -> verify -> inspect status
    - Touched link: status/task-next read model, card claiming, task lifecycle
    - Minimal proof: CLI contract tests and cargo test output showing `maestro next` reports/refuses/runs expected actions.
  - Feature evidence flow
    - Steps: prepare tasks from structured input -> record QA/proof helper evidence -> verify feature contract -> inspect feature state
    - Touched link: feature prepare, QA sidecar, acceptance proof records
    - Minimal proof: CLI tests showing helpers write the same validated artifacts and reject empty or mismatched input.
  - Cross-agent coordination
    - Steps: inspect active peers -> print connection suggestions -> assert message sender -> handle unlinked peer remedy
    - Touched link: active read model, message sender/current-card state, related-link gate
    - Minimal proof: CLI/unit tests for advisory output and `msg send --from` assertion behavior.
- Scenario Matrix:
  - [bl-001] Canonical next action remains read-only by default (covers: ac-1)
    - Dimensions: agent/CLI/task state/card store/local repo/read-only
    - Setup: repo has at least one ready, unblocked work card.
    - Action: run `maestro next`.
    - Oracle: output names one best next action and no card state changes.
    - Evidence to capture: command output plus card status before/after or contract test fixture.
    - Reproduction: create fixture card store, invoke CLI, assert output and unchanged store.
  - [bl-002] `next --run` only performs allowlisted safe automation (covers: ac-1)
    - Dimensions: agent/CLI/ownership/automation risk
    - Setup: repo has a recommended ready card.
    - Action: run `maestro next --run`.
    - Oracle: the card is claimed for the session; input-requiring actions are refused with a template.
    - Evidence to capture: CLI output and updated card status/claim.
    - Reproduction: fixture card store with ready card and with input-required next action.
  - [bl-003] Generic status updates cannot bypass task verification (covers: ac-2)
    - Dimensions: agent/CLI/task lifecycle/safety
    - Setup: task-like card exists in in_progress or needs_verification.
    - Action: run `maestro card update <task> --status needs_verification` or `--status verified`.
    - Oracle: command fails with a typed-command remedy; existing compatible status edits still work.
    - Evidence to capture: stderr/stdout and unchanged gated status.
    - Reproduction: CLI integration tests for refused and allowed statuses.
  - [bl-004] Feature-flow helpers preserve durable artifact contracts (covers: ac-3)
    - Dimensions: agent/CLI/feature artifacts/QA/proof/prepare
    - Setup: proposed or ready feature fixture with acceptance criteria.
    - Action: run structured prepare, QA baseline/slice, and feature proof helper commands.
    - Oracle: helpers reject empty input, write expected durable artifacts, and route through existing validation.
    - Evidence to capture: generated task cards, qa.md content, proof records, verify output.
    - Reproduction: CLI integration tests using temp repo fixtures.
  - [bl-005] Coordination assertions prevent wrong-current-card sends (covers: ac-4)
    - Dimensions: agent/CLI/cross-session/current-card/channel link
    - Setup: current card differs from the requested `--from` card.
    - Action: run `maestro msg send --from <other-card> <peer> "text"`.
    - Oracle: command fails with current-card details and a remedy; no message is written.
    - Evidence to capture: CLI output and channel file absence/unchanged state.
    - Reproduction: channel fixture test with linked and unlinked card pairs.
- Preserved behaviors:
  - Hidden flat card aliases keep parsing -> Proof: `cargo test card_commands_integration`.
  - JSON stdout remains parseable while ambient banners stay on stderr -> Proof: CLI JSON integration tests.
  - Existing `feature verify --prove --evidence` remains compatible -> Proof: feature verification tests.
- Changed behaviors:
  - `maestro next` becomes the canonical loop entrypoint.
  - Generic task-like status updates reject lifecycle-owned verification states.
  - Validated helper commands reduce manual artifact syntax.
  - Coordination guidance gains explicit sender assertions.
- Critical probes before commit:
  - Rust formatting -> `cargo fmt -- --check`
  - Static checks -> `cargo clippy --all-targets -- -D warnings`
  - Test suite -> `cargo test`
  - CLI smoke -> `target/debug/maestro version`
- Required artifacts:
  - New or updated CLI contract tests for next, guardrails, helpers, and coordination.
- Baseline gaps:
  - Exact auto-safe allowlist may expand later -> Proposed probe: tests assert v1 allows only ready-card claim.

```yaml
slices:
  - scenarios: ["bl-001", "bl-002", "bl-003", "bl-004", "bl-005"]
    evidence: ["Validated implementation with cargo fmt -- --check, cargo check --all-targets, cargo clippy --all-targets -- -D warnings, cargo test, and target/debug/maestro version."]
```
