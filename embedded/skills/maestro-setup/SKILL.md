---
name: maestro-setup
version: 1.11.0
description: "Use when a project using or adopting Maestro needs init/install/sync/doctor, global skills, hooks, harness setup, or agent integration diagnosis or repair."
---

# Maestro Setup

Tune a Maestro-enabled repository harness from current repository evidence.

Activate with a known session id:
`maestro hook record --event skill_activation --skill maestro-setup --session <session_id>`


## Use

- After `maestro init`.
- After `maestro install`.
- When an already-initialized repo needs its Maestro setup updated to the
  running binary with minimal repeated token cost.
- When `maestro doctor` reports setup or local agent integration problems.
- When a repo's build/test/harness instructions are missing or stale.

## Do

1. Run `pwd`, then `maestro status`.
2. If the repo is already initialized and the request is only to update Maestro
   setup, take the cheap rerun path before any broader read-in:
   - Run `maestro sync --dry-run`.
   - If it reports only current/skipped resources, do not run `maestro sync`.
   - If it reports drift, run `maestro sync`, then report only changed resources,
     backup paths, and blockers.
   - Run `maestro sync --global-skills` only when the user asks for global
     skills or `maestro doctor` reports global skill drift.
   - Finish with `maestro doctor`, then `maestro status`.
3. If the repo is not initialized, run `maestro init --dry-run`.
4. If dry-run prints `operating on <path>` and that path is not the intended
   current project root, stop and ask the user to pick or create the root before
   any write.
5. Only after the root is correct, run `maestro init --yes`.
6. Run `maestro doctor`.
7. If no agent integration is installed, run `maestro install --agent codex`
   unless the user asked for another agent.
8. Inspect repo structure, build/test commands, and workflow constraints.
9. Read in the existing agent and doc instructions as a BOUNDED set: at the
   repo root, and under each folder matched by the `projects:` globs in
   `.maestro/harness/harness.yml`, read `AGENTS.md`, `CLAUDE.md`, `README.md`,
   and top-level `docs/*.md`. Stay shallow (one level per location, no deep
   crawl) and skip outsized files (roughly 64 KB and up) so a vendored doc dump
   cannot flood context. With no `projects:` declared, this is the repo root
   alone.
10. Synthesize what you read into the SINGLE root harness guidance, one section
   per project (a single section when nothing is declared). This is read-in
   only: never write maestro-managed guidance into a sub-project's own
   `AGENTS.md`/`CLAUDE.md` -- `maestro install`/`sync` write managed blocks at
   the repo root alone. Cite the inspected files; do not tune from guesses.
11. Run `maestro doctor`, then `maestro status`.

## Stop

- `maestro init --dry-run` writes nothing; use it before init writes.
- `maestro init --yes` keeps existing files and creates missing files.
- `maestro sync --dry-run` is the cheap already-initialized update check; do not
  run broader setup read-in when it proves the setup is current and no doctor
  blocker remains.
- Use `maestro init --force` only for deliberate refresh; it backs up managed
  files first.
- Do not tune the harness from guesses, package-manager defaults, or stale chat
  memory.
- Per-project docs are read-in only. Maestro owns one root scope; never write
  maestro-managed guidance into a sub-project's `AGENTS.md`/`CLAUDE.md`.

## Done

- Setup is healthy, or the remaining setup blocker is explicit.
- The next handoff is visible from `maestro status`.
- Harness guidance changes cite inspected files or commands.
