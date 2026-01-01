# Tracks

Master list of all development tracks.

## Active Tracks

## [~] Track: Test Routing
*ID: test-routing_20251230*
*Link: [spec](tracks/test-routing_20251230/spec.md) | [plan](tracks/test-routing_20251230/plan.md)*

Test track to verify /conductor-implement auto-routes to orchestrator.

---

## Completed Tracks

## [x] Track: Skill Restructure to Anthropic Standard
*ID: skill-restructure*
*Archived: [conductor/archive/skill-restructure/](archive/skill-restructure/)*

Restructured 7 skills to Anthropic's skill-creator standard using Hub-and-Spoke architecture. Created maestro-core hub (62 lines), refactored all spokes to <100 lines with references/ extraction. Total: 939 lines (70% reduction from ~3116).

## [x] Track: Scripts Restructure
*ID: scripts-restructure*
*Archived: [conductor/archive/scripts-restructure/](archive/scripts-restructure/)*

Restructured Python scripts to match claudekit-skills pattern. Moved artifact-*.py to skills/conductor/scripts/, extracted track_assigner.py to skills/beads/scripts/. All scripts self-contained with --json flag support.

## [x] Track: Orchestrator Session Brain
*ID: session-brain*
*Archived: [conductor/archive/session-brain/](archive/session-brain/)*

Add Phase 0 (Preflight) to Orchestrator for multi-session coordination. Auto-registers session identity, detects active sessions via Agent Mail, warns on conflicts, prompts for stale session takeover. Scripts: session_identity.py, preflight.py, session_cleanup.py.

## [x] Track: Continuous-Claude-v2 Integration
*ID: cc-v2-integration*
*Archived: [conductor/archive/cc-v2-integration/](archive/cc-v2-integration/)*

Full merge of Continuous-Claude-v2 patterns into Maestro. Created 15 specialized agents in 5 categories (research/review/planning/execution/debug), thin router in AGENTS.md for token efficiency, Agent Mail as primary handoff storage. Orchestrated with 3 parallel waves, 5 epics, 35 beads.

## [x] Track: Orchestrator Skill Improvements
*ID: orchestrator-improvements_20260101*
*Archived: [conductor/archive/orchestrator-improvements_20260101/](archive/orchestrator-improvements_20260101/)*

Track threads for bead-to-bead context, per-bead execution loop, AGENTS.md tool preferences, auto-detect parallel routing (≥2 independent beads → orchestrator), enhanced monitoring with bv --robot-triage, lingering beads verification before epic close.

## [x] Track: Auto-Orchestration After Filing Beads
*ID: auto-orchestrate*
*Archived: [conductor/archive/auto-orchestrate/](archive/auto-orchestrate/)*

Automatic orchestration trigger after `fb` completes. Analyzes beads dependency graph via `bv --robot-triage`, generates Track Assignments, spawns parallel workers with wave re-dispatch, runs `rb` for final review. Fallback to sequential if Agent Mail unavailable.

## [x] Track: Orchestrator Skill
*ID: orchestrator-skill_20251230*
*Archived: [conductor/archive/orchestrator-skill_20251230/](archive/orchestrator-skill_20251230/)*

Multi-agent parallel execution with autonomous workers. Mode B workers self claim/close beads. Commands: `/conductor-orchestrate`, triggers "run parallel", "spawn workers".

## [x] Track: HumanLayer-Inspired Handoff System
*ID: handoff-system_20251229*
*Archived: [conductor/archive/handoff-system_20251229/](archive/handoff-system_20251229/)*

Replaced LEDGER.md/continuity with git-committed handoffs. Commands: `/create_handoff`, `/resume_handoff`. 6 triggers: design-end, epic-start, epic-end, pre-finish, manual, idle.

## [x] Track: Research Protocol Integration
*ID: research-protocol_20251229*
*Note: Archive directory missing - track completed but not archived.*

Replaced grounding system with parallel research sub-agents. Research ALWAYS runs at:
- `ds` start (DISCOVER phase)
- DEVELOP→DELIVER transition
- `/conductor-newtrack` (pre-spec)

**Supersedes:** grounding-v2_20251228, grounding-system-redesign_20251228

*Completed tracks are archived to `conductor/archive/`.*

## [x] Track: Validation Gates
*ID: validation-gates_20251229*
*Archived: [conductor/archive/validation-gates_20251229/](archive/validation-gates_20251229/)*

Add 5 validation gates to Maestro lifecycle: design, spec, plan-structure, plan-execution, completion.

## [x] Track: Auto-Continuity for All Agents
*ID: auto-continuity*
*Archived: [conductor/archive/auto-continuity/](archive/auto-continuity/)*

Session continuity automatic via workflow entry points (ds, /conductor-implement, /conductor-finish).

## [x] Track: maestro-core (REMOVED)
*ID: maestro-core*
*Archived: [conductor/archive/maestro-core/](archive/maestro-core/)*

Central orchestration skill with 6-level hierarchy, HALT/DEGRADE policies, and trigger routing rules.
**Note:** Skill removed in v4.3.1 - routing centralized in AGENTS.md.

## [x] Track: Skill Integration
*ID: skill-integration_20251228*
*Archived: [conductor/archive/skill-integration_20251228/](archive/skill-integration_20251228/)*

Consolidate 15 skills into 6 by merging 9 skills into conductor/references/.

## [x] Track: Grounding System Redesign (SUPERSEDED)
*ID: grounding-system-redesign_20251228*
*Archived: [conductor/archive/grounding-system-redesign_20251228/](archive/grounding-system-redesign_20251228/)*
*Superseded by: research-protocol_20251229*

Tiered grounding with enforcement, cascading router, and impact scan integration.
**Note:** Replaced by Research Protocol with parallel sub-agents.

## [x] Track: State Consolidation + Continuity Integration
*ID: state-consolidation_20251227*
*Archived: [conductor/archive/state-consolidation_20251227/](archive/state-consolidation_20251227/)*

Consolidate state files (3→1 per track) and integrate continuity with Conductor workflow.

## [x] Track: Doc-Sync Feature
*ID: doc-sync_20251227*
*Archived: [conductor/archive/doc-sync_20251227/](archive/doc-sync_20251227/)*

## [x] Track: UX Automation & State Machine
*ID: ux-automation_20251227*
*Archived: [conductor/archive/ux-automation_20251227/](archive/ux-automation_20251227/)*

## [x] Track: Continuity Integration
*ID: continuity-integration_20251227*
*Archived: [conductor/archive/continuity-integration_20251227/](archive/continuity-integration_20251227/)*

## [x] Track: BMAD V6 Integration
*ID: bmad-v6-integration*
*Archived: [conductor/archive/bmad-v6-integration/](archive/bmad-v6-integration/)*

## [x] Track: Spec-Compliant Skills-Only Architecture Migration
*ID: spec-compliant-migration_20251226*
*Archived: [conductor/archive/spec-compliant-migration_20251226/](archive/spec-compliant-migration_20251226/)*

## [x] Track: Integrate agent_mail MCP into Workflow
*ID: agent-coordination_20251224*
*Archived: [conductor/archive/agent-coordination_20251224/](archive/agent-coordination_20251224/)*

## [x] Track: /conductor-finish Phase 4 Revision
*ID: finish-phase4-revision_20251224*
*Archived: [conductor/archive/finish-phase4-revision_20251224/](archive/finish-phase4-revision_20251224/)*

## [x] Track: Conductor Track Validation System
*ID: state-files-phase1_20251224*
*Archived: [conductor/archive/state-files-phase1_20251224/](archive/state-files-phase1_20251224/)*

## [x] Track: Double Diamond + Party Mode
*ID: double-diamond-party-mode_20251223*
*Archived: [conductor/archive/double-diamond-party-mode_20251223/](archive/double-diamond-party-mode_20251223/)*

## [x] Track: /conductor-finish Integration
*ID: conductor-finish*
*Archived: [conductor/archive/conductor-finish/](archive/conductor-finish/)*

## [x] Track: Merge newTrack and File Beads
*ID: merge-newtrack-fb_20251223*
*Archived: [conductor/archive/merge-newtrack-fb_20251223/](archive/merge-newtrack-fb_20251223/)*

## [x] Track: Changelog CI/CD
*ID: changelog-cicd*
*Archived: [conductor/archive/changelog-cicd/](archive/changelog-cicd/)*

## [x] Track: CODEMAPS Integration
*ID: codemaps-integration_20251223*
*Archived: [conductor/archive/codemaps-integration_20251223/](archive/codemaps-integration_20251223/)*

## [x] Track: Beads Skill Consolidation
*ID: beads-consolidation_20251225*
*Archived: [conductor/archive/beads-consolidation_20251225/](archive/beads-consolidation_20251225/)*

## [x] Track: Beads-Conductor Integration
*ID: beads-conductor-integration_20251225*
*Archived: [conductor/archive/beads-conductor-integration_20251225/](archive/beads-conductor-integration_20251225/)*

---

## Track Format

When tracks are created, they appear as:

```markdown
## [~] Track: <title>
*ID: <shortname_YYYYMMDD>*
*Link: [spec](tracks/<id>/spec.md) | [plan](tracks/<id>/plan.md)*
```

Status markers:
- `[ ]` — Not started
- `[~]` — In progress
- `[x]` — Completed


