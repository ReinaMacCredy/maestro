---
amend_log_position: 1
---

### QA Baseline Contract

- Scope: self-contained-maestro-gitignore -- installer gitignore generation in `src/domain/install/mirrors.rs` (mirror_plan + gitignore_block + GitignoreSection), the legacy-root-block strip migration in `install_agent` (src/domain/install/mod.rs), plus uninstall.
- Entry point: the `.maestro/.gitignore` write AND the legacy-root-block strip are owned exclusively by `maestro install --agent` (install-lock-recorded ownership). `maestro init`, `sync`, and `upgrade` do NOT write or migrate the gitignore. The documented setup runs init -> doctor -> install (maestro-setup skill), so a fresh setup always reaches install. Existing installs migrate by re-running `maestro install`. ac-3 (which named `sync` as the migration entry point) is superseded by ac-7 and is waived at verify; the migration behavior is proved through `maestro install` per ac-7.
- Critical workflow chains:
  - install -> re-install (migration) -> uninstall (gitignore ownership + migration)
    - Steps: `maestro init` then `maestro install --agent` on a fresh repo -> `.maestro/.gitignore` written, root has no maestro block -> on a repo carrying the legacy root block, re-running `maestro install --agent` strips it (and writes `.maestro/.gitignore` in the same op) -> `maestro uninstall` removes `.maestro/.gitignore` and any legacy root residue.
    - Touched link: install mirror plan (the `.gitignore` mirror is repointed to `.maestro/.gitignore`) + the install-time strip migration + uninstall.
    - Minimal proof: grep markers / lines in root `.gitignore` and in `.maestro/.gitignore` after each step on a throwaway repo with the release binary.
- Scenario Matrix:
  - [bl-001] fresh install writes .maestro/.gitignore (covers: ac-1, ac-7)
    - Dimensions: entrypoint (install), install ownership, data shape.
    - Setup: fresh throwaway repo, no prior maestro install.
    - Action: `maestro init --yes` then `maestro install --agent claude` (repeat with codex).
    - Oracle: `.maestro/.gitignore` exists and lists the maestro-internal local paths relative to `.maestro/`: `runs/`, `channels/`, `backups/`, `index/`, `install-lock.yaml`, `update-check`, `tasks/*/evidence/`, `tasks/*/local/`, `archive/**/evidence/`, `archive/**/local/`, `archive/**/runs/`. No `.maestro/` prefix on any line. `playbook/` is absent (kept tracked for the peer feature).
    - Evidence to capture: full body of generated `.maestro/.gitignore`.
    - Reproduction: init+install in /tmp repo, `cat .maestro/.gitignore`.
  - [bl-002] root .gitignore carries no maestro block after install (covers: ac-2)
    - Dimensions: install ownership, entrypoint (install).
    - Setup: fresh throwaway repo.
    - Action: `maestro init --yes` then `maestro install --agent claude`.
    - Oracle: repo-root `.gitignore` contains no `# >>> maestro >>>` / `# <<< maestro <<<` markers and none of the formerly-managed maestro lines (no `.maestro/runs/`, no `.claude/settings.local.json`, etc.). (If the repo had no root `.gitignore` to begin with, install does not create one.)
    - Evidence to capture: `rg 'maestro|\.maestro/|settings.local' .gitignore` output (expected: empty / only user lines, or no file).
    - Reproduction: init+install in /tmp repo, inspect root `.gitignore`.
  - [bl-003] re-install migration strips legacy root block, preserves user entries (covers: ac-7; supersedes the waived ac-3)
    - Dimensions: state/lifecycle, migration, install ownership, edit-preservation.
    - Setup: repo whose root `.gitignore` already holds the OLD maestro managed block (the pre-feature `# >>> maestro >>>` ... `# <<< maestro <<<` body) plus a user-managed line outside the markers (e.g. `/target/`).
    - Action: run the NEW binary's `maestro install --agent claude`.
    - Oracle: the legacy maestro block is gone from root `.gitignore`; the user-managed line outside the markers is untouched; `.maestro/.gitignore` now carries the internal paths (written in the same operation -- no un-ignore window). A second `maestro install` is a clean no-op on the root file (idempotent).
    - Evidence to capture: root `.gitignore` before/after diff + `.maestro/.gitignore` body.
    - Reproduction: seed legacy block, `maestro install --agent claude`, diff.
  - [bl-004] update-check is covered (covers: ac-4)
    - Dimensions: data shape, install ownership, the latent-gap fix.
    - Setup: initialized + installed repo where maestro has written `.maestro/update-check`.
    - Action: `git status --porcelain`.
    - Oracle: `.maestro/update-check` does not appear as untracked/modified (it is ignored by `.maestro/.gitignore`).
    - Evidence to capture: `git status --porcelain | rg update-check` (expected: empty) + `git check-ignore .maestro/update-check`.
    - Reproduction: init+install repo, trigger update-check, `git status`.
  - [bl-005] uninstall removes .maestro/.gitignore + legacy residue (covers: ac-5)
    - Dimensions: lifecycle (uninstall), install ownership, cleanup.
    - Setup: initialized + installed repo (optionally one that went through the re-install migration; cover the preserved-`.maestro/` case where the dir is not removed wholesale).
    - Action: `maestro uninstall --agent claude`.
    - Oracle: `.maestro/.gitignore` is gone (removed via the install lock); root `.gitignore` has no orphaned maestro lines/markers.
    - Evidence to capture: `ls .maestro/.gitignore` (absent) + root `.gitignore` content.
    - Reproduction: init+install, uninstall, inspect.
  - [bl-006] agent paths dropped but files still written (covers: ac-6)
    - Dimensions: integration boundary (Claude/Codex hooks), behavior drop, install ownership.
    - Setup: fresh throwaway repo.
    - Action: `maestro init --yes` then `maestro install --agent claude` (and codex).
    - Oracle: neither `.claude/settings.local.json` nor `.codex/hooks.json` appears in any gitignore maestro writes; BUT both files are still written by hook_config_plan (hooks present under the managed `hooks` key).
    - Evidence to capture: grep both paths across `.gitignore` + `.maestro/.gitignore` (expected: absent) and confirm the two files exist with maestro hooks.
    - Reproduction: init+install, inspect gitignores + the two hook files.
- Preserved behaviors:
  - hook_config_plan still writes `.claude/settings.local.json` (hooks key merge) and `.codex/hooks.json` unchanged -> Proof: bl-006 + `cargo test` install/hooks suites.
  - User-managed root `.gitignore` entries outside the maestro block are never touched -> Proof: bl-003.
  - Managed-block upsert/remove machinery (managed_blocks.rs) still works for CLAUDE.md/AGENTS.md/.codex/config.toml -> Proof: `cargo test` mirror/sync suites.
- Changed behaviors:
  - The `.gitignore` mirror now targets `.maestro/.gitignore` (was repo-root `.gitignore`); the legacy root maestro block is removed on re-running `maestro install`; `.claude/settings.local.json` and `.codex/hooks.json` are no longer gitignored by maestro.
  - Migration is install-owned, not sync-owned (ac-3 corrected to ac-7): existing repos migrate on their next `maestro install`; `sync`/`upgrade` leave the legacy block in place until then (harmless -- it keeps ignoring the maestro paths until install rewrites the layout).
- Critical probes before commit:
  - install mirrors / gitignore unit tests -> `cargo test --test install_mirrors` (and any gitignore-specific test).
  - migration/strip path -> targeted test seeding a legacy root block then asserting removal at install time.
  - full suite -> `cargo test` (background).
- Required artifacts:
  - None beyond the edited `mirrors.rs` + `install/mod.rs` + the generated `.maestro/.gitignore`.
- Baseline gaps:
  - None for this scope; all six scenarios are exercised through the real `init`+`install`/`uninstall` entry points with the release binary at qa-slice.

```yaml
slices:
  - at: "2026-06-15T15:04:12Z"
    scenarios: ["bl-001", "bl-002", "bl-003", "bl-004", "bl-005", "bl-006"]
    probes:
      - "release binary 0.107.0.1781535578-g94a352bf, isolated HOME, real git repos in /tmp, MAESTRO_AUTO_UPDATE=0"
      - "cargo test (full suite, background): CARGO_EXIT=0, 53 'test result: ok' blocks, 0 failed"
      - "cargo test --test install_mirrors / install_uninstall_integration: 40/40 green"
    result: pass
    evidence:
      - "bl-001 (ac-1): fresh `init --yes` + `install --agent claude` (and codex) -> .maestro/.gitignore body = runs/ channels/ backups/ index/ install-lock.yaml update-check tasks/*/evidence/ tasks/*/local/ archive/**/evidence/ archive/**/local/ archive/**/runs/ (HashComment markers). No line starts with .maestro/; playbook/ absent."
      - "bl-002 (ac-2): after fresh install no repo-root .gitignore is created; `grep -E 'maestro|>>> maestro|settings.local|hooks.json'` finds nothing (no file)."
      - "bl-003 (ac-7): seeded root .gitignore with legacy maestro block + user lines (/target/, .DS_Store); `install --agent claude` stripped the block to leave only /target/ + .DS_Store, wrote .maestro/.gitignore in the same op; 2nd install left root .gitignore byte-identical (idempotent)."
      - "bl-004 (ac-4): with .maestro/update-check present, `git check-ignore .maestro/update-check` echoes the path (ignored); `git status --porcelain | grep update-check` is empty."
      - "bl-005 (ac-5): `uninstall --agent claude` removed .maestro/.gitignore in both the fresh-install case and the migrated/preserved-.maestro case; root .gitignore kept /target/ with no maestro residue; .maestro/ dir preserved."
      - "bl-006 (ac-6): no gitignore (root or .maestro/) mentions .claude/settings.local.json or .codex/hooks.json; both files are still written by hook_config_plan with the maestro hooks key (settings.local.json 8x \"hooks\", .codex/hooks.json present)."
```
