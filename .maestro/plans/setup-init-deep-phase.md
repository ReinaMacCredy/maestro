# Add Phase 7 (init-deep) to maestro-setup bundled skill

## Objective
Add a 7th phase to the `maestro-setup` bundled skill that runs the full `init-deep` hierarchical-AGENTS.md workflow. Vendor `init-deep` as a reference doc inside `maestro-setup`. Absorb the existing root-`AGENTS.md` pointer block into init-deep's `PROJECT KNOWLEDGE BASE` body so a single managed region governs root.

## Scope

**In**
- New file: `skills/bundled/maestro-setup/reference/init-deep.md` (vendored copy of `~/.codex/skills/init-deep/SKILL.md`).
- Edit `skills/bundled/maestro-setup/SKILL.md`: add Phase 7 (pointer to reference) and fold existing step 4 (Update Root AGENTS.md) into it.
- Regenerate `src/infra/domain/bundled-skill-templates.ts` via `bun run sync:bundled-skills`.
- Update `tests/unit/infra/domain/bundled-skill-templates.test.ts` to assert Phase 7 + reference file are bundled.

**Out**
- Original `~/.codex/skills/init-deep/` (untouched).
- Any new CLI flags or commands (`Skill-first, CLI-second`).
- `--create-new` flag exposure at the maestro-setup level.
- State-tracking files, orphan-deletion logic, hash-based idempotency, hybrid block-only update modes.
- Changes to Phases 1-3 and 5-6 of maestro-setup, context-templates, styleguides, or setup-report-template.

## Research Findings

- `skills/bundled/` is the source of truth. `scripts/sync-bundled-skills.ts` walks it recursively (`scripts/skill-template-source-lib.ts:38`) and emits `src/infra/domain/bundled-skill-templates.ts`. Adding files under `reference/` is automatic.
- Drift is enforced by `tests/unit/infra/domain/bundled-skill-templates.test.ts:38`.
- The pointer-block contract is hard-coded at `tests/unit/infra/domain/bundled-skill-templates.test.ts:131-141`. The exact `<!-- maestro-setup:start -->\n## Maestro Context\n...\n<!-- maestro-setup:end -->` block is asserted as a substring. Plan keeps the markers and inner content identical, just nested inside the PROJECT KNOWLEDGE BASE body — substring match continues to pass.
- `bundled-skill-templates.test.ts:94-101` forbids `/Users/` paths in `.md`/`.yaml` files; the source init-deep `SKILL.md` is clean.
- `package.json:13-16` exposes `bun run check:bundled-skills` and `bun run sync:bundled-skills`.
- maestro-setup currently structures `## Setup Flow` as `### 1.` ... `### 6.`. Phase 7 added as `### 7. Generate Hierarchical AGENTS.md (init-deep)`; existing `### 4. Update Root AGENTS.md` removed and folded into Phase 7.
- The absorbed pointer goes inside Phase 7's PROJECT KNOWLEDGE BASE template as a `## Context Pointers` section right after `## OVERVIEW`, still wrapped by the existing `<!-- maestro-setup:start -->` / `<!-- maestro-setup:end -->` markers.

## Tasks

### Phase A — Vendor init-deep
- [ ] Copy `~/.codex/skills/init-deep/SKILL.md` content into `skills/bundled/maestro-setup/reference/init-deep.md`.
- [ ] Strip the frontmatter block (`---\nname: init-deep\n...\n---`); keep all body content (Workflow, Phases 1-4, Anti-Patterns).
- [ ] Add provenance header at top: `<!-- Vendored from upstream init-deep skill. Edit upstream first, then re-vendor. -->`

### Phase B — Update maestro-setup SKILL.md
- [ ] Remove `### 4. Update Root AGENTS.md` and its body. Pointer-block text moves into Phase 7's template.
- [ ] Verify step numbering at top of `## Setup Flow` matches the new structure (1, 2, 3, 4, 5, 6 — old 5/6 become 4/5; new 7 added; or keep numbering and just add 7 — pick one and apply consistently).
- [ ] Add `### 7. Generate Hierarchical AGENTS.md (init-deep)` containing:
  - Statement that Phase 7 owns root `AGENTS.md` end-to-end and follows the workflow in `reference/init-deep.md` verbatim.
  - Context Pointers contract: Phase 7's PROJECT KNOWLEDGE BASE body must include a `## Context Pointers` section right after `## OVERVIEW`, containing the existing maestro-setup pointer block wrapped in start/end markers (text identical to current Step 4 block).
  - Failure handling: each explore agent retries once; remaining failures degrade to bash + manifest-only scoring; failures listed in `.maestro/setup-report.md`.
  - Non-interactive contract reaffirmed: Phase 7 never prompts; ambiguity becomes a `setup-report.md` warning.
  - `--create-new` is **not** exposed via maestro-setup in this slice; default is non-destructive update.
- [ ] Add the absorbed pointer block (verbatim copy of the current `<!-- maestro-setup:start -->`...`<!-- maestro-setup:end -->`) inside Phase 7's section so the test substring at `bundled-skill-templates.test.ts:131-141` still finds an exact match.
- [ ] Update `## Core Contract` if needed to keep bullets consistent with the pointer's new home.

### Phase C — Regenerate embed
- [ ] Run `bun run sync:bundled-skills`.
- [ ] Run `bun run check:bundled-skills` — must report `[ok] in sync`.

### Phase D — Test updates
- [ ] In `tests/unit/infra/domain/bundled-skill-templates.test.ts`, extend the existing `it("ships maestro-setup with managed-marker and report contracts", ...)` block:
  - Assert `setup!.files.find((f) => f.path === "reference/init-deep.md")` is defined and content includes `Init Deep` heading and the four phase names (`discovery`, `scoring`, `generate`, `review`).
  - Assert maestro-setup `SKILL.md` content includes the string `reference/init-deep.md` and the new Phase 7 heading.
- [ ] Verify pointer-block substring assertion at lines 131-141 still passes. If absorbed block changes whitespace, normalize to keep exact match intact.

### Phase E — Verify end-to-end
- [ ] `bun run check:bundled-skills` passes.
- [ ] `bun test tests/unit/infra/domain/bundled-skill-templates.test.ts` passes.
- [ ] `bun test` (full suite) passes.
- [ ] `bun run release:local` succeeds (build + install).
- [ ] `~/.claude/skills/maestro-setup/reference/init-deep.md` and `~/.codex/skills/maestro-setup/reference/init-deep.md` exist after install.
- [ ] (Sanity) Read installed `~/.claude/skills/maestro-setup/SKILL.md` — Phase 7 instructions coherent.

## Verification

```bash
bun run sync:bundled-skills
bun run check:bundled-skills
bun test tests/unit/infra/domain/bundled-skill-templates.test.ts
bun test
bun run release:local
ls -la ~/.claude/skills/maestro-setup/reference/init-deep.md
ls -la ~/.codex/skills/maestro-setup/reference/init-deep.md
```

Critical files:
- `skills/bundled/maestro-setup/reference/init-deep.md` (new)
- `skills/bundled/maestro-setup/SKILL.md` (edit: remove step 4, add Phase 7)
- `src/infra/domain/bundled-skill-templates.ts` (regenerated, do not hand-edit)
- `tests/unit/infra/domain/bundled-skill-templates.test.ts` (add Phase 7 + reference assertions)

## Notes

- **Risk — pointer-block test brittleness.** Substring at `bundled-skill-templates.test.ts:131-141` is exact. Mitigation: copy-paste verbatim; do not edit indentation or wording.
- **Cut line.** If pressure forces smaller slice, ship Phase A + B + C only (vendor + SKILL.md + sync). Drift test catches embed mismatches. Test additions in Phase D land in follow-up.
- **Assumption — section name.** Absorbed pointer goes into `## Context Pointers`. One-line change if a different name is preferred.
- **Commit shape.** `feat(setup): add init-deep Phase 7 to maestro-setup bundled skill` — minor version bump per project rule.
- **Out-of-scope follow-ups.** Wiring `--create-new` through maestro-setup; future `maestro setup` CLI mirror.
