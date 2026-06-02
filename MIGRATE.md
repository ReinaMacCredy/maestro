# Migrating data from the TypeScript maestro to the Rust maestro

This file is an instruction for a coding agent, not a script maestro runs. The Rust binary does
no data conversion (it only swaps itself onto your PATH). Carrying old data over is an optional,
agent-driven step you opt into by following this guide. It is best-effort and lossy: the
TypeScript and Rust products are different, so only a few record types have a clean home.

## What migrates, and what does not

The TypeScript maestro stored data in two places:
- **Global** `~/.maestro/` — only a runtime `update-check.json` cache. No user data. Not migrated.
- **Project** `.maestro/` — the real data.

Only three project record types map cleanly onto the Rust model. Everything else is **skipped**:
left intact in the backup and listed in the report. The Rust maestro is intentionally leaner
(repo-local feature / task / decision / qa) and has no home for the rest.

The Rust target model lives under `.maestro/`: features in `.maestro/features/<id>/`
(feature.yaml, baseline.md, qa-slices.yaml, notes.md), tasks in `.maestro/tasks/<id>/`.

| TypeScript source (in `.maestro/`) | Key fields | Rust target | How |
| --- | --- | --- | --- |
| `specs/*.md` (+ `tasks/contracts/index.jsonl`) | free-form spec markdown; contract events `{taskId, status, at}` | a **feature** | `maestro feature new "<title>"`, then `feature set <id> --acceptance "<from the spec>" --area "<paths>"`; paste the spec body into the feature's `notes.md`; note any contract amendments there too |
| `tasks/tasks.v2.jsonl` | `{id, slug, title, state, spec_path, blocked_by[], assignee, created_at, claimed_at, merged_at}` | a **task** | `maestro task create "<title>" --feature <id>` (link via the feature built from this task's `spec_path`), then drive `claim` -> `complete` -> `verify` as far as the source state warrants; record the original id / assignee / timestamps in the task summary |
| `evidence/<task-id>/evd-*.json` | `{kind, witness_level, payload, created_at}` | task **proof** -> `verify` | summarize the evidence into `task complete <id> --summary "..." --claim "<kind + payload>"`, then `maestro task verify <id>` |

**Skipped (backed up and listed in the report, never migrated):** global `~/.maestro/`; and in the
project: `principles.jsonl`, `plans/`, `tracks/`, `wisdom/`, `doctrine/`, `drafts/`, `policies/`,
`continuations/`, `skills/`, `settings.json`, `config.yaml`, and any directory not named in the
table above. These hold config, ephemeral session state, or free-form notes the Rust model does
not represent. They stay intact in the backup; the report tells the user exactly where.

## Safety rules (do not skip)

1. **Back up by moving — never delete.** Rename the existing `.maestro/` to
   `.maestro.ts-backup-<timestamp>/`. This preserves every TypeScript file under a new path and
   frees `.maestro/` for the Rust binary, which uses the same directory. Nothing is deleted; the
   user removes the backup themselves later, after verifying. Record the backup path.
2. **Emit a mapping report.** Produce a written report in two sections: (a) every migrated record
   and where it landed in the Rust model; (b) every skipped file, each with its path inside
   `.maestro.ts-backup-<timestamp>/`, so the user can retrieve any of it by hand.
3. **Best-effort and lossy.** When a TypeScript concept has no clean Rust home, skip it and say so
   in the report. Do not invent acceptance criteria, proof, or QA coverage the source did not
   contain, and do not force a bad fit.

## Procedure

1. Confirm the Rust binary is installed and active: `maestro version` shows a Rust build and
   `maestro doctor` is ok. (See the README for installing it.)
2. Back up by moving: `mv .maestro .maestro.ts-backup-<timestamp>` (safety rule 1).
3. Scaffold the fresh Rust workspace: `maestro init --yes`, then `maestro install --agent claude`
   (or `--agent codex`).
4. Inventory `.maestro.ts-backup-<timestamp>/`. For each `specs/*.md`, each `tasks/tasks.v2.jsonl`
   record, and each `evidence/<task-id>/` file, create the Rust artifact per the mapping table.
   Skip everything else.
5. Write the mapping report (safety rule 2), pointing the skipped list at the backup.
6. Verify: `maestro feature list --all` and `maestro task list` reflect the carried-over records;
   spot-check a few against the backup.
