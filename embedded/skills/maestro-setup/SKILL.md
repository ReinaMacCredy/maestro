---
name: maestro-setup
version: 1.4.2
description: "Use after Maestro init/install or doctor warnings to tune a repository harness from verified repo evidence."
---

# Maestro Setup

Tune a Maestro-enabled repository harness from current repository evidence.

Activate:
`maestro hook record --event skill_activation --skill maestro-setup`

## Use

- After `maestro init`.
- After `maestro install`.
- When `maestro doctor` reports setup or local agent integration problems.
- When a repo's build/test/harness instructions are missing or stale.

## Do

1. Run `pwd`, then `maestro status`.
2. If the repo is not initialized, run `maestro init --dry-run`.
3. If dry-run prints `operating on <path>` and that path is not the intended
   current project root, stop and ask the user to pick or create the root before
   any write.
4. Only after the root is correct, run `maestro init --yes`.
5. Run `maestro doctor`.
6. If no agent integration is installed, run `maestro install --agent codex`
   unless the user asked for another agent.
7. Inspect repo structure, build/test commands, existing agent instructions,
   and workflow constraints.
8. Update harness guidance only from verified files or command output.
9. Run `maestro doctor`, then `maestro status`.

## Stop

- `maestro init --dry-run` writes nothing; use it before init writes.
- `maestro init --yes` keeps existing files and creates missing files.
- Use `maestro init --force` only for deliberate refresh; it backs up managed
  files first.
- Do not tune the harness from guesses, package-manager defaults, or stale chat
  memory.

## Done

- Setup is healthy, or the remaining setup blocker is explicit.
- The next handoff is visible from `maestro status`.
- Harness guidance changes cite inspected files or commands.
