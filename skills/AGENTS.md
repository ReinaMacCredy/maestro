# Skills

Shipped skill source tree. Use this file with the repo-root [AGENTS.md](../AGENTS.md).

## Ownership

- `built-in/*/SKILL.md` is the source of truth for repo-shipped built-in skills.
- `bundled/*/SKILL.md` is the source of truth for the global installed Maestro skill bundle.
- `src/infra/domain/built-in-skill-templates.ts` and `src/infra/domain/bundled-skill-templates.ts` are generated from this tree.
- `.factory/skills/` is reference material for authors and reviewers, not the runtime lookup path.

## Workflow

- Edit repo-shipped skills under `skills/built-in/`.
- Edit installed global-skill content under `skills/bundled/`.
- Regenerate built-in templates with `bun scripts/sync-built-in-skills.ts`.
- Regenerate bundled templates with `bun scripts/sync-bundled-skills.ts`.
- Check for drift with `bun run check:skills` and `bun run check:bundled-skills`.
- `bun run build` syncs built-in templates before compile; bundled templates still need their dedicated sync/check flow.

## Lookup Rules

- Runtime agent prompt lookup resolves `.maestro/skills/{agentType}/SKILL.md` first.
- If no project-local skill exists, runtime falls back to `skills/built-in/{agentType}/SKILL.md`.
- `maestro install` publishes the bundled skill set from `skills/bundled/` into user-level skill directories.

## Skill Changes (L1)
- `maestro-task` (`bundled/maestro-task/SKILL.md`) was updated in L1 to document evidence recording: agents should call `maestro evidence record` after running verification commands, linking evidence to the active task and optionally to a contract criterion.

## Skill Changes (L2)
- `maestro-mission` (`bundled/maestro-mission/SKILL.md`) was updated in L2 to require a `proposed_contract` in every plan. The plan must include `allowed_files`, `forbidden_paths`, `done_when`, and `amendment_budget`. This contract is not itself an amendment (Rule 6); it is the plan-time proposal that gets locked when the agent claims the task.
- `maestro-task` (`bundled/maestro-task/SKILL.md`) was updated in L2 with a "Stay in scope; amend on genuine discovery" section. When an agent discovers a file outside the locked contract scope, it must call `maestro contract amend --task <id> --add-path <path> --reason "<why>"` before touching that file. Each amendment consumes from `amendmentBudget` (Rules 3–7). Amendments write a `contract-amendment` Evidence row automatically. Use `maestro task verify --task <id>` to run the Trust Verifier locally before completing.

## Skill Changes (L3)
- `maestro-mission` (`bundled/maestro-mission/SKILL.md`) was updated in L3 to be risk-class-aware. Plans must propose a `risk_class` (`low | medium | high | critical`). The Risk Engine derives a class from diff signals and takes the higher of agent-proposed vs Maestro-derived; agents cannot lower the derived class (Rule 1). See `docs/risk-class-derivation.md` for the signal-to-class mapping table.
- `maestro-task` (`bundled/maestro-task/SKILL.md`) was updated in L3 to require `maestro task verify --task <id>` followed by `maestro verdict request --task <id>` before an agent claims complete. An exit code of 0 (PASS) allows completion; 1 (FAIL), 2 (HUMAN), or 3 (BLOCK) must be resolved first. ProofMap coverage is computed inside `verdict request` and surfaces as `proof-map-incomplete` reasons; there is no standalone `task proof` verb.

## Skill Changes (L4)
- **New skill: `maestro-verify`** (`bundled/maestro-verify/SKILL.md`) ships as the canonical verification protocol. It documents the full pre-claim ritual (plan → implement → verify → ProofMap → verdict → branch on exit code), witness levels, Trust Verifier scope, plan-check, verdict semantics, cost-budget monitoring, AI Reviewer protocol (Rule 1 veto-only), and threat-model production. Total bundled skills: 7 (was 6).
- `maestro-task` (`bundled/maestro-task/SKILL.md`) was sharpened in L4.2 to include the self-check loop (verify → verdict → fix/handoff/stop) and cross-references `maestro-verify` for the full protocol.
- `maestro-mission` (`bundled/maestro-mission/SKILL.md`) was updated in L4 to cross-reference `maestro-verify` for the plan-check step (`maestro plan check --task <id> --plan-file <path>`) which catches `scope-widens`, `missing-proof`, and `risk-class-too-low` before coding starts.
- `maestro-handoff` (`bundled/maestro-handoff/SKILL.md`) cross-references `maestro-verify` for the handoff gate protocol.

## Skill Changes (L7)
- `maestro-verify` (`bundled/maestro-verify/SKILL.md`) was updated in L7 to add three new sections covering deploy/runtime/rollback: `## Deploy Gate` (the four checks, advisory wiring), `## Runtime Signals` (`Spec.runtime_signals` schema and `runtime-signal` Evidence), and `## Witnessed Rollback` (Rule 10, how the rollback check consumes witnessed evidence). These sections are slotted between `## Verdict Override` and `## The Pre-Claim Ritual`.

## Local Gotchas

- Do not hand-edit `src/infra/domain/built-in-skill-templates.ts` or `src/infra/domain/bundled-skill-templates.ts`.
- Keep directory names aligned with the decoded skill name expected by the corresponding sync script.

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- none

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
