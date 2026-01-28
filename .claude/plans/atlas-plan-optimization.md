# Atlas Plan Optimization Work Plan

## Context
### Original Request
Optimize /atlas-plan to properly integrate with Claude Code's native `EnterPlanMode()`/`ExitPlanMode()` mechanism while improving speed, quality, and user experience.

### Interview Summary
- Focus area: comprehensive optimization (speed, quality, UX).
- Codex step: optional, user chooses during interview (not after).
- Interview mode: interactive but batched for efficiency.
- Review pipeline: keep Metis and Momus.
- Plan template: keep full template.
- Architecture: main context controls plan mode lifecycle; prometheus only generates plan documents.
- @plan keyword: route to /atlas-plan (unified flow).
- PHASE: INTERVIEW remains in prometheus for backward compatibility; /atlas-plan uses DIRECT_GENERATE only.

### Metis Review
- Intent classification: Refactoring (high confidence).
- Critical questions resolved: PHASE handoff, domain questions extraction, codex choice storage, EnterPlanMode reason, @plan routing.
- Must do: test EnterPlanMode behavior (subagents can write), keep PHASE: INTERVIEW in prometheus, case-insensitive domain detection, update keyword-detector, store codex_choice with fallback.
- Must not do: touch atlas-work or execution workflow, remove Metis/Momus, simplify plan template, add dependencies, change PLAN_READY JSON, remove PHASE: INTERVIEW entirely, over-abstract domain detection.

### Assumptions (based on provided context)
- Plan will be saved to `.claude/plans/atlas-plan-optimization.md`.
- `codex_choice` stored in `.atlas/pipeline-state.json` with fallback to "standard" when missing.
- No new external dependencies; existing hook scripts and CLI tools are available (`jq`, `bash`, `rg`).

## Work Objectives
### Core Objective
Refactor /atlas-plan so the main context owns plan mode lifecycle and interview, while prometheus only generates plan documents.

### Deliverables
- Refactored `.claude/commands/atlas-plan.md` with EnterPlanMode/ExitPlanMode integration, batched interview, domain detection, codex choice capture, pipeline-state update, and DIRECT_GENERATE handoff.
- New domain question reference file extracted from prometheus.
- Slimmed `skills/atlas/references/agents/atlas-prometheus.md` with interview logic removed and plan templates preserved.
- Hook scripts updated for the new pipeline flow and codex_choice handling.
- `@plan` keyword routes to `/atlas-plan`.
- Documentation aligned with the new workflow.

### Definition of Done
- EnterPlanMode called once with a reason; ExitPlanMode called once after review pipeline completes.
- Interview runs in main context with batched AskUserQuestion including codex choice.
- Prometheus uses DIRECT_GENERATE for /atlas-plan; PHASE: INTERVIEW retained for backward compatibility.
- Metis and Momus remain in the review pipeline; PLAN_READY JSON format unchanged.
- `codex_choice` stored and honored with default fallback.
- Prometheus agent reduced to roughly 800 lines after removals.
- Hooks do not prompt for generator choice after interview for /atlas-plan.
- Manual verification scenarios pass.

### Must Have
- Clean plan mode lifecycle in `.claude/commands/atlas-plan.md`.
- Case-insensitive domain detection with explicit keyword lists.
- Domain question patterns in a dedicated reference file.
- Optional Codex step chosen during interview and persisted.
- Metis and Momus remain mandatory review steps.
- `@plan` routes to `/atlas-plan`.

### Must NOT Have
- No changes to `/atlas-work` or execution workflow.
- No removal of Metis/Momus or plan template sections.
- No new dependencies or external services.
- No change to PLAN_READY JSON format.
- No new abstraction layer for domain detection.

## Prerequisites
### Dependencies
- Claude Code plan mode tools (EnterPlanMode, ExitPlanMode).
- `jq` and `bash` available for hook scripts and verification.
- `rg` available for fast search and validation.

### Environment Setup
- `.atlas/` and `.claude/plans/` directories exist.
- Hook scripts in `scripts/` are executable in the current environment.

## Verification Strategy
### Test Decision
Manual QA plus hook-script checks.

### TDD/Manual QA
- Manual scenarios from the handoff:
  - `/atlas-plan "add console.log"`
  - `/atlas-plan "add OAuth login"`
  - `/atlas-plan "refactor auth system with TDD"`
- Scripted checks for hook outputs:
  - `bash scripts/test-hooks.sh --strict-json plan-ready-handler`
  - `bash scripts/test-hooks.sh --strict-json momus-loop-handler`
  - `bash scripts/test-hooks.sh --strict-json codex-completion-handler`

## Task Flow
```
T1 -> T2 -> T4 -> T5 -> T7 -> T8
T1 -> T3 -> T7
T6 --------> T7
```

## Parallelization
| Group | Tasks | Parallelizable | Notes |
| --- | --- | --- | --- |
| A | T1, T6 | YES | Independent file creation and keyword routing. |
| B | T2, T3 | YES (after T1) | Command refactor and prom trimming can proceed in parallel once domain file exists. |
| C | T4 | NO | Central pipeline behavior; avoid overlapping edits. |
| D | T5 | NO | Depends on pipeline behavior from T4. |
| E | T7 | YES (after T2-T6) | Docs update once behavior is stable. |
| F | T8 | NO | Final validation after all changes. |

## TODOs
The tasks below sequence extraction of domain questions, command refactor, agent trimming, hook updates, and docs alignment before running manual verification.

### T1: Create domain question reference file
- What: Extract AUTH/API/UI/UX/TESTING/REFACTOR/ARCHITECTURE question patterns into a standalone reference file; Complexity 4/10.
- Where: `skills/atlas/references/domain-questions.md:1-220` (new file); source from `skills/atlas/references/agents/atlas-prometheus.md:559-776`. Tools: `rg`, `sed -n`, `apply_patch`.
- Dependencies: None. Parallelizable: YES (independent; unblocks T2 and T3).
- Test-First: Manual verification.
- Acceptance Criteria: `rg -n "AUTH Domain Pattern" skills/atlas/references/domain-questions.md` returns matches; file contains trigger keywords and 5-6 questions for each of the six domains; content is ASCII and mirrors original wording.

### T2: Refactor /atlas-plan command for plan mode and interview
- What: Rewrite main flow to EnterPlanMode({ reason }) first, run batched interview with domain detection and codex choice, update pipeline-state with codex_choice, spawn prometheus DIRECT_GENERATE, run Metis and Momus, optional Codex step, then ExitPlanMode; Complexity 7/10.
- Where: `.claude/commands/atlas-plan.md:9-223` (workflow, prompts, diagram); update `.claude/commands/atlas-plan.md:1-4` only if tool list needs adjustment. Tools: `Read`, `Grep`, `apply_patch`.
- Dependencies: T1. Parallelizable: NO (foundational; defines codex_choice format used by hooks).
- Test-First: Manual verification.
- Acceptance Criteria: `rg -n "EnterPlanMode\\(\\{ reason:" .claude/commands/atlas-plan.md` shows reason parameter; `rg -n "AskUserQuestion" .claude/commands/atlas-plan.md` shows batched questions including codex choice; `rg -n "codex_choice" .claude/commands/atlas-plan.md` shows pipeline-state update with default fallback; `rg -n "DIRECT_GENERATE" .claude/commands/atlas-plan.md` shows only direct generate for prometheus; `rg -n "ExitPlanMode" .claude/commands/atlas-plan.md` appears once at the end; edge cases (multiple domains, no match, EnterPlanMode failure) are explicitly handled in text.

### T3: Slim atlas-prometheus agent
- What: Remove interview and domain-pattern blocks while preserving Phase-Based Operation and plan templates; Complexity 5/10.
- Where: `skills/atlas/references/agents/atlas-prometheus.md:189-555` and `skills/atlas/references/agents/atlas-prometheus.md:559-856` (remove); check continuity around `skills/atlas/references/agents/atlas-prometheus.md:858`. Tools: `rg`, `sed -n`, `apply_patch`, `wc -l`.
- Dependencies: T1. Parallelizable: YES (can run alongside T2 after T1).
- Test-First: Manual verification.
- Acceptance Criteria: `rg -n "Interview Mode \\(Default\\)" skills/atlas/references/agents/atlas-prometheus.md` returns no matches; `rg -n "Domain Question Patterns" skills/atlas/references/agents/atlas-prometheus.md` returns no matches; `rg -n "Phase-Based Operation" skills/atlas/references/agents/atlas-prometheus.md` still present; `wc -l skills/atlas/references/agents/atlas-prometheus.md` is roughly 750-900 lines; plan template sections remain unchanged.

### T4: Make pipeline transitions codex_choice-aware
- What: Update pipeline transitions to read codex_choice from `.atlas/pipeline-state.json` (default to standard), skip generator choice when set, and retain legacy generator-choice path when missing; Complexity 6/10.
- Where: `scripts/lib/pipeline-state-machine.sh:11-307` (state transitions); `scripts/generator-choice-handler.sh:11-55` (guard/compat); `scripts/pipeline-transition.sh:6-12` (state-machine comments). Tools: `rg`, `sed -n`, `apply_patch`.
- Dependencies: T2. Parallelizable: YES (after T2, can run alongside T3).
- Test-First: Manual verification.
- Acceptance Criteria: `rg -n "codex_choice" scripts/lib/pipeline-state-machine.sh` shows lookup with default fallback; when codex_choice is "enhanced", generated instruction skips AskUserQuestion and goes directly to CODEX_GENERATE; when codex_choice is missing, legacy generator choice remains available; `rg -n "GENERATOR_CHOICE" scripts/lib/pipeline-state-machine.sh` only appears in the fallback path.

### T5: Align plan-ready, momus-loop, and codex-completion handlers
- What: Update hook handlers so PLAN_READY and Momus OKAY paths respect mandatory Metis/Momus review and optional Codex step without extra user prompts; Complexity 6/10.
- Where: `scripts/plan-ready-handler.sh:24-110`, `scripts/momus-loop-handler.sh:52-120`, `scripts/codex-completion-handler.sh:31-118`. Tools: `rg`, `apply_patch`, `bash scripts/test-hooks.sh`.
- Dependencies: T4. Parallelizable: NO (depends on new pipeline behavior).
- Test-First: Manual verification.
- Acceptance Criteria: plan-ready handler avoids generator-choice prompts for /atlas-plan; momus-loop handler checks codex_choice and instructs codex step only when enhanced; codex-completion handler resumes pipeline (merge and continue) instead of asking for Start Work; `bash scripts/test-hooks.sh --strict-json plan-ready-handler`, `bash scripts/test-hooks.sh --strict-json momus-loop-handler`, and `bash scripts/test-hooks.sh --strict-json codex-completion-handler` pass.

### T6: Route @plan to /atlas-plan
- What: Update keyword detector so `@plan` uses the unified /atlas-plan flow; Complexity 2/10.
- Where: `scripts/keyword-detector.sh:163-171` (atlas-plan case). Tools: `rg`, `apply_patch`.
- Dependencies: None. Parallelizable: YES (independent).
- Test-First: Manual verification.
- Acceptance Criteria: `rg -n "@plan" scripts/keyword-detector.sh` shows @plan routed to the atlas-plan handler; `@atlas-plan` behavior remains unchanged; no changes to /atlas-work routing.

### T7: Update documentation to match new flow
- What: Align documentation with main-context interview, codex choice timing, @plan routing, and trimmed prometheus responsibilities; Complexity 4/10.
- Where: `skills/atlas/SKILL.md:10-36` and `skills/atlas/SKILL.md:273-279`; `.claude/skills/atlas/SKILL.md:10-36` and `.claude/skills/atlas/SKILL.md:136-167`; `docs/ATLAS_AGENTS.md:13-58`, `docs/ATLAS_AGENTS.md:171-193`, `docs/ATLAS_AGENTS.md:318-329`, `docs/ATLAS_AGENTS.md:506-509`; `docs/HOOK_ARCHITECTURE.md:167-260`, `docs/HOOK_ARCHITECTURE.md:452-547`, `docs/HOOK_ARCHITECTURE.md:604-631`; `skills/atlas/references/workflows/prometheus.md:7-96`. Tools: `rg`, `apply_patch`.
- Dependencies: T2, T4, T5, T6. Parallelizable: YES (once behavior is stable, docs can be updated in parallel).
- Test-First: Manual verification.
- Acceptance Criteria: docs consistently show interview in main context for /atlas-plan, prom used for DIRECT_GENERATE, codex choice captured during interview, and @plan routes to /atlas-plan; `rg -n "GENERATOR_CHOICE" docs/HOOK_ARCHITECTURE.md` reflects the updated flow.

### T8: Run manual verification scenarios
- What: Execute the three manual scenarios and hook checks to validate end-to-end behavior; Complexity 3/10.
- Where: Verify outputs in `.claude/commands/atlas-plan.md:9-223`, `scripts/plan-ready-handler.sh:24-110`, `scripts/momus-loop-handler.sh:52-120`, `scripts/keyword-detector.sh:163-171`, `skills/atlas/references/agents/atlas-prometheus.md:858-1900`. Tools: `/atlas-plan` commands, `bash scripts/test-hooks.sh`.
- Dependencies: T1-T7. Parallelizable: NO (final validation).
- Test-First: Manual verification.
- Acceptance Criteria: `/atlas-plan "add console.log"` enters plan mode, asks minimal questions, generates plan, exits plan mode; `/atlas-plan "add OAuth login"` triggers AUTH questions; `/atlas-plan "refactor auth system with TDD"` prompts codex choice and honors selection; `bash scripts/test-hooks.sh --strict-json plan-ready-handler` and `bash scripts/test-hooks.sh --strict-json momus-loop-handler` pass.

## Risks & Mitigation
| Risk | Impact | Mitigation |
| --- | --- | --- |
| Hook handlers still inject user prompts that conflict with main-context flow | Pipeline confusion and redundant questions | Gate handlers on pipeline-state or codex_choice and add hook tests. |
| codex_choice missing or inconsistent between command and hooks | Wrong generator path or skipped codex step | Use a single field name with default fallback to "standard" and document it. |
| Prometheus trimming removes needed plan template content | Lower plan quality or missing sections | Remove only specified sections, verify templates remain with `rg` and manual review. |
| Domain detection misclassifies or matches multiple domains | Irrelevant questions or missed requirements | Use case-insensitive keyword lists and AskUserQuestion disambiguation for multi-match. |
| User-provided plan names or requests leak into paths | Path traversal or unexpected file writes | Slugify plan names, restrict output to `.claude/plans/`, avoid shell interpolation. |
| ExitPlanMode not called due to hook errors | Plan mode stuck, user cannot approve | Ensure ExitPlanMode is explicitly called after pipeline completion and provide fallback instructions. |

## Rollback Plan
| Task | Rollback Strategy |
| --- | --- |
| T1 | Remove `skills/atlas/references/domain-questions.md` and restore from git history if needed. |
| T2 | Restore `.claude/commands/atlas-plan.md` from git history using `git show HEAD:<path> > <path>`. |
| T3 | Restore `skills/atlas/references/agents/atlas-prometheus.md` from git history and re-run line count check. |
| T4 | Revert `scripts/lib/pipeline-state-machine.sh` and related scripts from git history; confirm pipeline-state still parses. |
| T5 | Restore `scripts/plan-ready-handler.sh`, `scripts/momus-loop-handler.sh`, and `scripts/codex-completion-handler.sh` from git history; re-run hook tests. |
| T6 | Restore `scripts/keyword-detector.sh` from git history if routing breaks. |
| T7 | Revert documentation files from git history. |
| T8 | No rollback needed; rerun manual scenarios after fixes. |

## Commit Strategy
- Commit 1: `refactor(atlas-plan): move interview to main context, add domain reference`.
- Commit 2: `refactor(atlas-prometheus): remove interview and domain sections`.
- Commit 3: `chore(hooks): codex_choice-aware pipeline handlers`.
- Commit 4: `docs(atlas): update workflow and hook documentation`.

## Success Criteria
- /atlas-plan enters and exits plan mode correctly, with interview in main context and codex choice captured early.
- Prometheus only generates plans (DIRECT_GENERATE) and remains backward compatible with PHASE: INTERVIEW.
- Metis and Momus review pipeline remains intact, and PLAN_READY JSON format is unchanged.
- @plan routes to /atlas-plan, and hooks align with the updated flow.
- Manual verification scenarios and hook script checks complete without errors.
