# v1 feature gap closure for v2

The master plan and ADRs 0001-0014 named the primary v1 surfaces (`mission`, `intake`, `session/notes`, `spec`) and the kept-as-is non-goals (`mission-control`, `mcp`, `hooks`, `ci`, `gitnexus`, `gc`/`recover`/`bundle`, verdict/witness/policy). They did not name several v1 features that have to be explicitly addressed before v2 is locked: `memory`, `memory-ratchet`, `agent`, `graph`, `session/detect`, `intake/classify`, `qa`, `ralph`, `notes`, `inspect`, `state`, `skills`, `deploy`, and the seven `maestro:*` colon-namespaced built-in skills. This ADR fixes their fate.

The unifying principle: v1 carried conductor-era features (memory store, graph store, mission-planning, conductor mode) that don't fit the article's harness-OS shape. Where a v1 feature has a clean article-aligned home, it is absorbed and the feature directory is deleted. Where it doesn't, it is dropped. Explicit non-goal entries are added for surfaces kept in v2 unchanged. The result is a smaller, sharper v2 surface that matches "what Codex can't see doesn't exist": knowledge lives as committed markdown that the agent reads at session start, not as runtime stores the agent has to query.

## Decisions

**Memory feature deleted; one verb survives.** v1 `memory` + `memory-ratchet` + `agent` (7 verbs: memory-compile/correct/learn/lint/recall/search/stats plus ratchet-check/promote plus generateAgentPrompt) disappear as a runtime store. Corrections become entries in `docs/principles/<rule>.md` (committed markdown with scan command + fix recipe per the principles primitive). Durable learnings become entries in `docs/design-docs/learnings/` (a carved-out agent-writable subdirectory; see §6 layout). Agent-prompt synthesis collapses into AGENTS.md as the article's ~100-line table of contents.

**Tradeoff stated plainly:** this is not absorption-in-shape, it is *deletion of the auto-promotion machinery*. The article supports static markdown principles authored by humans/maintainers; it never described auto-promoted corrections. The agent context-switches to author a markdown file every correction-worthy moment, instead of running `memory-learn` and letting the harness pipe the entry to a store. We accept this UX cost because the runtime store violated "what Codex can't see doesn't exist": corrections in a store are invisible to a fresh agent session unless explicitly recalled.

**One verb survives: `maestro principle promote`.** Takes an ad-hoc correction (typically from `verify` FAIL evidence or from a transition-evidence row) and materializes a draft principle markdown file at `docs/principles/<slug>.md` for the agent or human to finalize. This is the only piece of the correction loop that load-bears: the moment of "FAIL just happened; capture this before it's lost." No recall, no search, no compile; only promote. Recall happens by reading the directory at session start, which is the article's shape.

**Graph feature absorbed into references; feature deleted.** v1 `graph link` + `graph context` disappear. Project-to-project relationships become `docs/references/project-graph.yaml` (or markdown). Agent reads at session start like any other reference. No `graph link` verb (just edit the file); no `graph context` verb (the file IS the context). Parallel to the memory decision.

**Session-detect absorbed into worktree primitive.** v1 `session detect` (env-based identification of agent harness: claude-code / codex / gemini / etc) becomes internal utility used by worktree, evidence, and handoff to record agent identity on transitions. No standalone `session` feature; no `whoami` verb. Worktree metadata at claim time captures `agent: claude-code`. Notes-portion of session already folded into handoff per ADR-0002; this completes the session feature deletion.

**`maestro-classify` skill folds into `maestro-design`.** Work-type classification (6-type taxonomy: new-spec, spec-slice, change-request, initiative, maintenance, harness-improvement) happens at spec-authoring time. The design skill's Q&A includes work-type selection and writes `work_type:` to spec frontmatter alongside `risk_class:` and `mode:`. Telemetry collection moves to ad-hoc evidence rows. No standalone classify skill in v2.

**`maestro-qa` skill folds into `maestro-setup` as `setup --qa`.** QA install is bootstrap-shaped, not runtime-shaped, so it belongs under setup. Same scaffolding logic (`.maestro/qa/` config, sub-skills, sidecars, GitHub Actions workflow), accessed via `maestro setup --qa`. The 5-skill agent runtime bundle stays at 5.

**Verb deletions:**

- `ralph` / `ralph-review`: dropped. v2 `loop` primitive owns the iterate-until-PASS pattern. Multi-perspective review never load-beared a gate; no replacement.
- `note` / `notes`: standalone create/list verb dropped. Agent writes notes as markdown files directly into `docs/design-docs/` (durable) or as handoff entries (session-scoped). Article shape: notes live as committed files, not ledger entries.
- `inspect`, `state`: both dropped. Per-primitive show verbs (`task get`, `plan show`, `spec validate`, `evidence list`) cover every inspection need. Grab-bag verbs were redundant.

**Explicit non-goals (kept in v2 unchanged, surfaced in §11):**

- `skills`: keeps `list` + `sync` for the agent skill bundle. Tiny surface.
- `deploy`: keeps L7 deploy-gate evidence verbs unchanged. Already alluded to under "CI integration" but now named explicitly.

**Colon-namespaced built-in skills (`skills/built-in/maestro:*`): piecemeal migration.**

- `maestro:agent-base` (startup/cleanup procedures) → folds into `maestro-task` task-startup section.
- `maestro:mission-planning` → folds into `maestro-plan` (heavy mode).
- `maestro:scrutiny-validator` and `maestro:user-testing-validator` → fold into `maestro-verify`.
- `maestro:conduct` (orchestrator mode) → deleted. Violates v2 passive-harness rule.
- `maestro:blueprint` (HTML blueprint generator) → deleted. Not an article concept.
- `maestro:define-mission-skills` → deleted. Mission-era meta-skill, no v2 equivalent.

After migration, `skills/built-in/` becomes empty and the directory is removed. The 5-skill `skills/bundled/` bundle is the only skill surface in v2.

## Migration impact

`setup --migrate-v2` gains the following steps (extending the §9 sequence in the master plan):

1. Migrate any v1 correction-store entries in `.maestro/memory/` into `docs/principles/legacy/<id>.md`, preserving rule + rationale + scan command + fix recipe where present. Add to `docs/design-docs/learnings/` if the entry is a learning, not a rule. Delete `.maestro/memory/` and `.maestro/memory/ratchet/`.
2. Migrate any v1 project-graph at `.maestro/graph.json` into `docs/references/project-graph.yaml`. Delete the source file.
3. Strip session feature directory; preserve last session-detect output as `.maestro/runs/<id>/agent.json` for any in-flight worktree.
4. Move `.maestro/qa/` config to v2 layout (if present); the directory itself stays, only sub-skill paths normalize.

`setup --migrate-v2` Phase 1 migration tests must cover each of these in addition to the task and exec-plan state mappings already specified.

## Rejected

- Keep memory as a fourth port (MemoryPort) (preserves v1 verbs but reintroduces a runtime store that the article explicitly says shouldn't exist; "what Codex can't see doesn't exist" means knowledge lives as files).
- Keep graph as a standalone feature (single TUI caller, dormant data; feature is over-engineered for the workload).
- Keep colon-namespaced tier as "advanced skills" alongside the 5-bundle (defeats the small-stable-context principle of ADR-0012; advanced tier is where vocabulary regressions hide).
- Defer any of the above to post-2.0 (would orphan v1 users on big-bang flip and leave conductor-era vocabulary lurking in the codebase).
