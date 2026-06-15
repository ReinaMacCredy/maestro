# Ship only relevant-language playbook files on setup

## Current state

Extraction (src/domain/extraction/playbook.rs): extract_playbook enumerates embedded_files() over the whole embedded/playbook/ dir and writes EVERY file unconditionally. embedded/playbook/ = PLAYBOOK.md (anchor, frontmatter version: 1.0.0) + 10 language guides (cpp, csharp, dart, general, go, html-css, javascript, python, rust, typescript).

Version gate (extract.rs): one folder gate keyed on PLAYBOOK.md frontmatter version governs the whole folder. In Skip/Merge/Update modes, file_action still WRITES a file that is missing (existing.is_none()); nothing ever PRUNES a file once written. So selective shipping interacts with: (a) what the gate considers the folder set, (b) whether re-detect adds/removes files later.

Detection precedent: init already calls HarnessConfig::detect -> detect_stack(repo_root) (schema.rs:284). It picks ONE primary stack by ordered manifest probe: Cargo.toml->Rust, package.json->TypeScriptNode, pyproject/requirements->Python, else Generic. StackKind has only 4 variants {Rust, TypeScriptNode, Python, Generic}. It seeds harness.yml verify commands. It does NOT cover cpp/csharp/dart/go/html-css and is single-winner (no polyglot).

PLAYBOOK.md content: static embedded anchor with a 'How to use this' table mapping editing-X -> x.md for all 10 languages, plus an Attribution section listing the 9 vendored files. If only a subset of files ship, this static table/attribution points at absent files.

Distribution: .maestro/playbook/ is NOT gitignored (.gitignore maestro block excludes runs/channels/backups/etc but not playbook). So all 10 files get committed into every repo. The cost is committed repo clutter, NOT agent-context bloat -- the playbook is read on-demand (agent reads only the one file for the language it edits).

## Problem

## Confirmed (not forks)

TIMING: detection belongs inside plan_playbook so it re-runs on every init AND sync (verified: sync/mod.rs:97 calls extract_all in Update mode -> extract_playbook). file_action writes any MISSING file even under a Skip/Update decision (extract.rs:296), so a newly-relevant language auto-appears on the next sync with NO PLAYBOOK.md version bump. Removed languages still never prune (matches today's no-prune behavior) -- intended asymmetry: add-on-sync, never-remove.

GUARDRAIL: keep a SEPARATE playbook detector; do NOT make detect_stack multi-winner. harness.yml verify commands need single-winner semantics; forcing polyglot onto it is a speculative abstraction.

## Removal consumers (.maestro/playbook extraction)

Repoint/remove for command model: (1) src/domain/extraction/playbook.rs extract/preview/validate + plan_playbook + embedded_files + 2 tests -- embedded PLAYBOOK_DIR STAYS (command reads it), extraction fns go. (2) mod.rs:55/68/79 extract_all/validate_all/preview_all calls. (3-5) init dry-run preview, sync, update route through those (indirect). (6) paths.rs:43 playbook_dir() (only consumer was extraction). (7) HARNESS.md ## Code style paragraph (line 42) reworded to 'run: maestro playbook <lang>' -> HARNESS version bump + mirror resync + guard re-record. (8) PLAYBOOK.md frontmatter version gate becomes moot; content becomes the bare/list overview. NO skills and NO doctor checks reference the folder.

## Implementation defaults (reversible, not D-locks)

Command: top-level 'maestro playbook [<lang>]', like status/doctor. Output: the raw guide markdown to stdout (no --json; it is prose for an agent). Unknown <lang>: exit non-zero with an error listing the valid tokens (honors the existing no-dead-end-errors decision; doubles as discovery). No aliases (the index prints the exact tokens: cpp, csharp, html-css, etc.). Embedded guides stay byte-identical to today's vendored content -- this is a delivery change, not a content change. PLAYBOOK.md stays embedded as the index source; its frontmatter version is no longer a folder gate (drop or keep as inert content).
