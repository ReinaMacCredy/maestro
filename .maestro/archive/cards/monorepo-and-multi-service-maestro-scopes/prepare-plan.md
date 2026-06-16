# Plan: monorepo-and-multi-service-maestro-scopes

## Task T1: Add optional `project` field to the Card base
check: a card created with `--project P` persists `project: P` in card.yaml for every card type (feature/task/bug/chore/idea/decision); a card created without the flag has no `project` key; a legacy card.yaml lacking the field still loads and round-trips with no spurious `project:`; setting `project` on a feature neither satisfies nor alters the accept readiness gate (acceptance OR affected_areas). Covers ac-1, ac-10.

## Task T2: Add optional `projects:` declaration to HarnessConfig
check: HarnessConfig round-trips an optional `projects: [..]` list; a harness.yml with NO `projects:` key loads unchanged and re-serializes without adding the key; `maestro init` writes no `projects:` by default. Covers ac-9 (and the schema half of ac-2/ac-8).

## Task T3: Declaration-gated folder->project auto-infer on card create
after: T1, T2
check: with `projects: ["*"]` a card created under `<root>/svc-pay/` stores `project=svc-pay` with no flag; with `["services/*"]` under `services/pay/` stores `pay`; with `["fe","be"]` under `docs/` stores none; `--project` always overrides; repo root stores none; with NO `projects:` key nothing is inferred; a card created from any subfolder still lands in the single root `.maestro/cards/`. Covers ac-2, ac-3 (infer half), ac-11.

## Task T4: Read surface -- `--project` filter, `[project]` badge, group-by-project, flat `--json`
after: T1
check: `maestro list --project P` / `maestro ready --project P` (and card-namespaced) return only cards whose stored project==P; unknown project => empty result, exit 0; `status` has no `--project` flag; human rows show a `[project]` badge when set and none when unset; `list` without `--project` groups under project headers ONLY when >=2 distinct projects among shown cards (no-project cards under a root/unassigned group), else flat and byte-identical to today; `list`/`ready`/`status` `--json` emit the existing dense single-line-per-item envelope with `project` as one flat field, never grouped or nested. Covers ac-4, ac-5, ac-6, ac-7.

## Task T5: Preserve user-authored `projects:` across init/sync/harness re-detect
after: T2
check: a harness.yml carrying a user-authored `projects:` declaration is preserved verbatim (not stripped, reordered, or overwritten) across `maestro init` (re-init/merge), `maestro init --force`, `maestro sync`, and any harness re-detect path; a config with no `projects:` key still loads unchanged. Covers ac-8.

## Task T6: maestro-setup skill -- per-project bounded doc/agent-spec read-in
after: T2
check: embedded/skills/maestro-setup/SKILL.md instructs enumerating a BOUNDED doc set (AGENTS.md, CLAUDE.md, README.md, docs/*.md + depth/size cap) at the repo root AND under each folder matched by the `projects:` declaration, synthesizing into the SINGLE root harness guidance with one section per project, read-in ONLY (never writes maestro-managed guidance into sub-project AGENTS.md/CLAUDE.md; install/sync stay root-only), adding no new CLI verb and no new harness schema field; the skill version is bumped and the extraction guard re-recorded; with no `projects:` declared, only root docs are read and one guidance section results. Covers ac-12, ac-13, ac-14, ac-15.
