# Migrating data from the TypeScript maestro to the Rust maestro

Status: SCAFFOLD. The procedure and safety rules below are settled. The field-by-field
mapping table is not filled in yet (it needs an inventory of the TypeScript data schema).
Do not rely on this for a lossless migration until the mapping section is complete.

This file is an instruction for a coding agent, not a script maestro runs. The Rust binary
does no data conversion (it only swaps itself onto your PATH). Carrying old data over is an
optional, agent-driven step that you opt into by following this guide.

## Your job (agent)

Read the existing TypeScript maestro data in this environment, map it into the Rust maestro
repo-local model under `.maestro/`, and write the result, without losing or destroying the
original. The TypeScript and Rust products are different (TS used global config and a broader
mission/spec/principle model; Rust is repo-local with feature/task/decision/qa), so this is a
best-effort structural map, not a 1:1 copy.

## Safety rules (do not skip)

1. **Back up first.** Copy all TypeScript maestro data to a timestamped backup directory
   before touching anything. Record the backup path in your report.
2. **Never delete TypeScript data.** Migration only reads the old data and writes new files.
   The user removes the old data themselves, later, once they have verified the result.
3. **Emit a mapping report.** Produce a written report listing every source record, where it
   landed in the Rust model (or that it was intentionally dropped, with the reason). The user
   reviews this before trusting the migration.
4. **Best-effort and lossy.** When a TypeScript concept has no clean Rust home, say so in the
   report rather than forcing a bad fit. Do not invent acceptance criteria, proof, or QA
   coverage that the source did not contain.

## Procedure

1. Confirm the Rust binary is installed and active: `maestro version` shows a Rust build, and
   `maestro doctor` is ok. (See the README for installing the Rust binary.)
2. Back up the TypeScript data (safety rule 1).
3. Inventory the TypeScript data: enumerate every record and its fields.
4. For each source record, apply the mapping table below and write the corresponding Rust
   artifact via the `maestro` CLI (`feature new/set`, `task create`, `decision new`, ...) or as
   files under `.maestro/` where the CLI has no direct verb.
5. Write the mapping report (safety rule 3).
6. Verify: `maestro feature list --all`, `maestro task ...`, and `maestro decision list` should
   reflect the carried-over records. Spot-check a few against the backup.

## Mapping table (TODO: blocked on TypeScript schema inventory)

The Rust target model is repo-local under `.maestro/`:
- features: `.maestro/features/<id>/` (feature.yaml, baseline.md, qa-slices.yaml, notes.md)
- tasks: `.maestro/tasks/<id>/`
- decisions: `.maestro/decisions/`

| TypeScript source | Fields | Rust target | Notes |
| --- | --- | --- | --- |
| mission | TODO | feature? | TODO |
| spec | TODO | feature contract (acceptance, areas) | TODO |
| principle | TODO | decision? | TODO |
| task | TODO | task | TODO |
| evidence / verdict | TODO | proof / verify | TODO |

Fill this in after inventorying the TypeScript schema (its data locations and field shapes).
