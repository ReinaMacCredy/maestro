---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: code-playbook-styleguides-surfaced-to-agents -- HARNESS.md content + CLAUDE.md/AGENTS.md managed mirror blocks + sync resync.
- Lean baseline per user directive "skip qa and go": three scenarios, one per acceptance criterion. No full evidence ceremony.
- Critical workflow chains:
  - install -> sync (mirror block propagation)
    - Steps: `maestro init` fresh repo -> blocks present in CLAUDE.md + AGENTS.md -> edit a block -> `maestro sync` -> block restored from shipped content.
    - Touched link: install mirror plan + sync extract path.
    - Minimal proof: grep markers after init; diff block after sync.
- Scenario Matrix:
  - [bl-001] init writes managed blocks (covers: ac-1)
    - Dimensions: entrypoint (init/install), install ownership.
    - Setup: fresh throwaway repo, no prior maestro install.
    - Action: `maestro init` (claude+codex).
    - Oracle: CLAUDE.md and AGENTS.md each contain a `<!-- maestro:start -->`/`<!-- maestro:end -->` block; CLAUDE.md block @-imports HARNESS.md, AGENTS.md block uses the Read-first line.
    - Evidence to capture: grep of both files for markers + block body.
    - Reproduction: init in /tmp repo, `rg 'maestro:start' CLAUDE.md AGENTS.md`.
  - [bl-002] sync resyncs drifted managed blocks (covers: ac-2)
    - Dimensions: state/lifecycle, edit-preservation, install ownership.
    - Setup: installed repo with both mirror blocks.
    - Action: hand-edit content inside one managed block (and add user text outside it), then `maestro sync`.
    - Oracle: block content restored to shipped body; user text outside the markers preserved; drifted copy backed up.
    - Evidence to capture: file diff before/after sync + backup file path.
    - Reproduction: edit block, `maestro sync`, inspect file + backup dir.
  - [bl-003] HARNESS.md carries Code style section (covers: ac-3)
    - Dimensions: data shape (shipped resource), entrypoint (init).
    - Setup: fresh throwaway repo.
    - Action: `maestro init`.
    - Oracle: `.maestro/harness/HARNESS.md` contains a `## Code style` section with the locked bullets; version is 1.14.0.
    - Evidence to capture: section text from generated HARNESS.md.
    - Reproduction: init in /tmp repo, open `.maestro/harness/HARNESS.md`.
- Preserved behaviors:
  - Existing HARNESS.md sections unchanged in meaning -> Proof: `maestro init` + read; sync version-gating intact -> Proof: `cargo test` sync + version-guard suites.
  - User content outside managed markers preserved -> Proof: bl-002.
- Changed behaviors:
  - sync now also refreshes the CLAUDE.md/AGENTS.md managed mirror blocks (previously install-only).
- Critical probes before commit:
  - resource version guard -> `cargo test --test resources_version_guard`.
  - full suite -> `cargo test` (background).
- Required artifacts:
  - None beyond the edited HARNESS.md + sync code.
- Baseline gaps:
  - None for this lean scope.

### QA Slice

- No blocking QA findings for the playbook + mirror-resync wave.
- Workflow chains replayed: install -> sync, on a throwaway repo with the
  release binary (g57c6449e): init --yes -> install --agent claude -> drift
  CLAUDE.md block -> sync -> second sync.
- Scenarios replayed: bl-001 (install writes both blocks), bl-002 (sync drift +
  no-op), bl-003 (HARNESS.md Code style section).
- Probes run: the e2e flow above; `cargo test` full suite + clippy (via the
  three per-task verify gates, all green); `cargo test --test sync_integration`
  (8/8); `cargo test --test resources_version_guard` (4/4).
- Artifacts captured: backup dir `...-sync/CLAUDE.md` holding the stale copy.

```yaml
slices:
  - at: "2026-06-14T14:32:00Z"
    scenarios: ["bl-001", "bl-002", "bl-003"]
    probes:
      - "release binary e2e: init --yes -> install --agent claude -> drift -> sync -> sync"
      - "cargo test --test sync_integration"
      - "cargo test --test install_mirrors"
      - "cargo test --test resources_version_guard"
    result: pass
    evidence:
      - "bl-003: .maestro/harness/HARNESS.md shows version 1.14.0 and a '## Code style' section with all 7 locked bullets"
      - "bl-001: CLAUDE.md block @-imports HARNESS.md; AGENTS.md block uses the Read-first line; both marker-wrapped"
      - "bl-002 drift: sync restored the CLAUDE.md block to shipped content, preserved '# My header'/'my footer' outside the markers, and backed up the STALE BODY copy under .maestro/backups/<ts>-sync/CLAUDE.md"
      - "bl-002 no-op: second sync on the healthy repo printed 'synced: 0 refreshed ... 3 already current' with no 'mirror blocks resynced' line and no new backup"
      - "sync_integration: 8 passed; install_mirrors incl. ac-1 marker-wrap test passed; resources_version_guard: 4 passed"
```
