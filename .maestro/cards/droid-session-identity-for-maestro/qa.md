---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: droid-session-identity-for-maestro
- Critical workflow chains:
  - CLI helper baseline
    - Steps: setup -> action -> inspect output
    - Touched link: feature QA gate
    - Minimal proof: [bl-001] Droid hook payload attribution (covers ac-1): setup a temporary Maestro repo and invoke maestro hook record with a Droid-shaped JSON stdin payload containing session_id; action records the hook event; oracle is an events.jsonl under .maestro/runs/<encoded Droid session>/ and no cli-<date> or unattributed bucket for that event; evidence is a focused integration test. [bl-002] Shipped skill guidance (covers ac-2): inspect embedded Maestro skills after sync; oracle is guidance that tells Droid users to read hook JSON stdin session_id and avoids claiming DROID_SESSION_ID exists; evidence is resource guard/content test or direct file inspection. [bl-003] Existing runtime identity preservation (covers ac-3): run session identity tests or equivalent unit/integration coverage; oracle confirms CODEX_THREAD_ID and CLAUDE_CODE_SESSION_ID precedence remain unchanged; evidence is cargo test output.
- Scenario Matrix:
  - [bl-001] observed baseline behavior
    - Dimensions: agent/CLI/local artifact
    - Setup: repo initialized with feature droid-session-identity-for-maestro
    - Action: [bl-001] Droid hook payload attribution (covers ac-1): setup a temporary Maestro repo and invoke maestro hook record with a Droid-shaped JSON stdin payload containing session_id; action records the hook event; oracle is an events.jsonl under .maestro/runs/<encoded Droid session>/ and no cli-<date> or unattributed bucket for that event; evidence is a focused integration test. [bl-002] Shipped skill guidance (covers ac-2): inspect embedded Maestro skills after sync; oracle is guidance that tells Droid users to read hook JSON stdin session_id and avoids claiming DROID_SESSION_ID exists; evidence is resource guard/content test or direct file inspection. [bl-003] Existing runtime identity preservation (covers ac-3): run session identity tests or equivalent unit/integration coverage; oracle confirms CODEX_THREAD_ID and CLAUDE_CODE_SESSION_ID precedence remain unchanged; evidence is cargo test output.
    - Oracle: behavior remains observable
    - Evidence to capture: command output or artifact diff
    - Reproduction: rerun the observed command or workflow
- Preserved behaviors:
  - [bl-001] Droid hook payload attribution (covers ac-1): setup a temporary Maestro repo and invoke maestro hook record with a Droid-shaped JSON stdin payload containing session_id; action records the hook event; oracle is an events.jsonl under .maestro/runs/<encoded Droid session>/ and no cli-<date> or unattributed bucket for that event; evidence is a focused integration test. [bl-002] Shipped skill guidance (covers ac-2): inspect embedded Maestro skills after sync; oracle is guidance that tells Droid users to read hook JSON stdin session_id and avoids claiming DROID_SESSION_ID exists; evidence is resource guard/content test or direct file inspection. [bl-003] Existing runtime identity preservation (covers ac-3): run session identity tests or equivalent unit/integration coverage; oracle confirms CODEX_THREAD_ID and CLAUDE_CODE_SESSION_ID precedence remain unchanged; evidence is cargo test output. -> Proof: manual/CLI observation
- Changed behaviors:
  - None captured at baseline
- Critical probes before commit:
  - focused CLI/helper test
- Required artifacts:
  - .maestro/cards/droid-session-identity-for-maestro/qa.md
- Baseline gaps:
  - None

```yaml
slices:
  - scenarios: ["bl-002", "bl-003"]
    evidence: ["Replayed remaining baseline scenarios: cargo test --test resources_version_guard passed 6/6, proving shipped skill guidance and updated resource versions; cargo test --test hook_record_integration passed 20/20, preserving Claude and Codex session identity behavior alongside the new Droid payload regression; final cargo fmt --check, cargo check --all-targets, cargo clippy --all-targets -- -D warnings, and cargo test all passed."]
  - scenarios: ["bl-001"]
    evidence: ["Replayed the Droid session identity wave: cargo test --test hook_record_integration passed 20/20 including droid_hook_payload_session_id_wins_over_agent_env and existing Claude/Codex session identity tests; cargo test --test resources_version_guard passed 6/6 including every_shipped_skill_explains_droid_session_identity and resource version hashes; cargo fmt --check, cargo check --all-targets, cargo clippy --all-targets -- -D warnings, and cargo test all passed."]
```
