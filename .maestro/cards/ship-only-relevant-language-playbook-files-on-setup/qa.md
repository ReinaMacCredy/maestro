---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: ship-only-relevant-language-playbook-files-on-setup -- move the code
  playbook from per-repo extracted files (`.maestro/playbook/`) to an
  on-demand `maestro playbook` command served from the embedded binary.
  Surfaces: new `maestro playbook` command; `src/domain/extraction`
  (drop playbook from extract/validate/preview + shared obsolete-folder
  cleanup); `embedded/playbook/PLAYBOOK.md`; `embedded/harness/HARNESS.md`
  (`## Code style`) + HARNESS version + guard snapshot.

- Critical workflow chains:
  - Fresh setup then code-style lookup
    - Steps: `maestro init --yes` -> agent reads `## Code style` in HARNESS ->
      agent runs `maestro playbook <lang>` -> writes code to the guide
    - Touched link: HARNESS `## Code style` instruction (file-path -> command)
    - Minimal proof: init a scratch repo, confirm no `.maestro/playbook/`,
      run `maestro playbook rust`, diff against `embedded/playbook/rust.md`
  - Existing repo de-clutter on resync
    - Steps: old repo has tracked `.maestro/playbook/` -> `maestro sync`
      (or `update`, or `init --force`) -> folder removed -> `git status`
      shows a deletion the user commits
    - Touched link: shared extraction path cleanup
    - Minimal proof: seed the folder, run each entry point, confirm removal
      with no `.maestro/backups/` copy

- Scenario Matrix:
  - [bl-001] Per-language guide served byte-identical (covers: ac-1)
    - Dimensions: entrypoint (new CLI verb), data shape (file bytes), integration (embedded include_dir)
    - Setup: any directory; binary built from this branch
    - Action: `maestro playbook <lang>` for each of rust, python, go,
      typescript, javascript, cpp, csharp, dart, html-css, general
    - Oracle: stdout equals the corresponding `embedded/playbook/<lang>.md`
      byte-for-byte (frontmatter-free body as embedded), exit 0
    - Evidence to capture: `diff <(maestro playbook rust) embedded/playbook/rust.md` empty; exit codes
    - Reproduction: run the loop over all 10 tokens
  - [bl-002] Bare command prints the index (covers: ac-2)
    - Dimensions: entrypoint, actor (agent discovering guides)
    - Setup: any directory
    - Action: `maestro playbook` with no argument
    - Oracle: prints the index (how-to + the 10 available tokens + attribution), exit 0
    - Evidence to capture: stdout snapshot showing token list + attribution
    - Reproduction: `maestro playbook`
  - [bl-003] Unknown token is a no-dead-end error (covers: ac-3)
    - Dimensions: failure recovery, trust boundary (user input)
    - Setup: any directory
    - Action: `maestro playbook nosuchlang`
    - Oracle: non-zero exit; stderr/stdout lists the valid tokens
    - Evidence to capture: exit code != 0; message contains rust/python/...
    - Reproduction: `maestro playbook nosuchlang; echo $?`
  - [bl-004] Works with no repo / before init (covers: ac-4)
    - Dimensions: environment (no `.maestro`), state (uninitialized)
    - Setup: an empty dir that is not a maestro repo, and not under one
    - Action: `maestro playbook rust`
    - Oracle: serves the guide, exit 0; no "run maestro init first" error; no repo-root discovery
    - Evidence to capture: run from `/tmp/<empty>`, exit 0, content present
    - Reproduction: `cd $(mktemp -d); maestro playbook rust`
  - [bl-005] Init no longer ships the playbook folder (covers: ac-5)
    - Dimensions: lifecycle (init), data (filesystem layout)
    - Setup: fresh scratch repo
    - Action: `maestro init --yes`; then `maestro init --dry-run` in another fresh repo
    - Oracle: no `.maestro/playbook/` directory created; dry-run output has no `playbook` line
    - Evidence to capture: `ls .maestro/` lacks playbook; dry-run grep -i playbook empty
    - Reproduction: scratch init + dry-run
  - [bl-006] Stale folder removed cleanly on every extraction entry point (covers: ac-6)
    - Dimensions: migration, destructive guard, lifecycle (init --force/sync/update)
    - Setup: initialized repo with a hand-seeded tracked `.maestro/playbook/` (the pre-change layout)
    - Action: run `maestro sync`, `maestro update`, and `maestro init --force` (each from a seeded state)
    - Oracle: `.maestro/playbook/` removed; no backup copy under `.maestro/backups/`;
      a fresh `init` (no prior folder) has nothing to remove and does not error
    - Evidence to capture: folder absent post-run; backups dir has no playbook copy; git shows deletion
    - Reproduction: seed folder, run each verb, inspect
  - [bl-007] HARNESS reword + version bump, mirrors untouched (covers: ac-7)
    - Dimensions: workflow (agent instruction), compatibility (mirror blocks)
    - Setup: built binary; scratch repo
    - Action: inspect `embedded/harness/HARNESS.md` `## Code style`; init scratch; inspect CLAUDE.md/AGENTS.md mirror blocks
    - Oracle: `## Code style` instructs `maestro playbook <lang>` (no `.maestro/playbook/<lang>.md` path);
      HARNESS frontmatter version > 1.15.0; CLAUDE.md/AGENTS.md mirror blocks are unchanged `@`-pointers
    - Evidence to capture: HARNESS diff; version literal; mirror-block bytes equal to pre-change
    - Reproduction: grep HARNESS; diff mirror blocks

- Preserved behaviors:
  - Guide CONTENT is exactly the previously-shipped bytes (no content regression) -> Proof: `maestro playbook <lang>` diffed against `embedded/playbook/<lang>.md`
  - Other extraction folders still extract/refresh under the version gate (harness, hooks, skills cache) -> Proof: `maestro init --yes` still writes `.maestro/harness/HARNESS.md`; `maestro sync` still version-gates remaining folders
  - `maestro sync` still backs up drifted non-playbook folders before overwrite -> Proof: existing sync backup tests stay green
  - Attribution / Apache-2.0 licensing for the vendored guides still surfaces -> Proof: it appears in the `maestro playbook` index output
- Changed behaviors:
  - `maestro init`/`sync`/`update` no longer write `.maestro/playbook/` (intended; was: extract all 11 files)
  - New `maestro playbook [<lang>]` command (intended; did not exist)
  - Stale `.maestro/playbook/` is removed on the extraction path with no backup (intended)
  - HARNESS `## Code style` points at the command, not the file path (intended)
- Critical probes before commit:
  - Full suite green -> `cargo test` (run in background; slow)
  - Fresh init writes no playbook folder -> scratch `maestro init --yes` + `ls .maestro/`
  - Guide served byte-identical -> `diff <(maestro playbook rust) embedded/playbook/rust.md`
  - Harness extraction not collateral-damaged -> scratch init still writes `.maestro/harness/HARNESS.md`
- Required artifacts:
  - None beyond the shipped binary and embedded resources.
- Baseline gaps:
  - Current behavior (this binary, g855a058c): `init --yes` extracts all 11 files
    into a TRACKED `.maestro/playbook/`; `init --dry-run` prints `create   playbook`;
    no `maestro playbook` command exists; HARNESS:42-45 says read
    `.maestro/playbook/<lang>.md`; HARNESS version 1.15.0.

```yaml
slices:
  - at: "2026-06-15T15:31:41Z"
    scenarios: ["bl-001", "bl-002", "bl-003", "bl-004", "bl-005", "bl-006", "bl-007"]
    probes:
      - "diff <(maestro playbook <lang>) embedded/playbook/<lang>.md  (loop 10 langs)"
      - "maestro playbook  (index) / maestro playbook nosuchlang / cd $(mktemp -d); maestro playbook rust"
      - "scratch maestro init --yes + init --dry-run; seed .maestro/playbook + sync / init --force"
      - "maestro install claude / maestro install codex (scratch) -> inspect CLAUDE.md/AGENTS.md mirror blocks"
      - "cargo test (full suite, background)"
    result: pass
    evidence:
      - "bl-001 (ac-1): all 10 guides (rust python go typescript javascript cpp csharp dart html-css general) byte-identical to embedded/playbook/<lang>.md, exit 0"
      - "bl-002 (ac-2): bare `maestro playbook` prints index with 10 `    maestro playbook <lang>` token lines + 'How to use this' + Apache attribution, exit 0"
      - "bl-003 (ac-3): `maestro playbook nosuchlang` exit 1, stderr lists valid tokens (cpp, csharp, dart, general, go, html-css, javascript, python, rust, typescript)"
      - "bl-004 (ac-4): `maestro playbook rust` from empty non-repo dir serves '# Idiomatic Rust Style Guide Summary', exit 0, empty stderr, no init error"
      - "bl-005 (ac-5): scratch `init --yes` .maestro={cards harness hooks RECOVERY.md} -- no playbook/; `init --dry-run | grep -ic playbook` = 0"
      - "bl-006 (ac-6): seeded+committed .maestro/playbook/ removed by sync and init --force, no .maestro/backups/ copy, `git status` shows 'D .maestro/playbook/rust.md'; fresh init (no folder) exit 0; update shares extract_all->remove_obsolete_playbook_folder path"
      - "bl-007 (ac-7): HARNESS.md version 1.16.0, `## Code style` instructs `maestro playbook <lang>` with no `.maestro/playbook/<lang>.md` path; install claude CLAUDE.md block '@.maestro/harness/HARNESS.md' + install codex AGENTS.md 'Read .maestro/harness/HARNESS.md first...' -- unchanged @-pointers (mirrors.rs untouched)"
      - "full suite: 53 test binaries all ok, 0 failed; resources_version_guard re-recorded harness 1.16.0"
```
