# BMAD V6 Integration - Implementation Plan

## Overview

4-phase implementation to replace current party mode with full BMAD v6 integration.
Estimated effort: 12-16 hours.

---

## Phase 1: Foundation - Directory Structure & Configuration

**Goal:** Set up the new directory structure and configuration files.

### 1.1 Archive Old Party Mode

- [ ] 1.1.1 Create `skills/design/references/party-mode-backup/`
- [ ] 1.1.2 Move current `party-mode/` contents to backup
- [ ] 1.1.3 Verify backup complete (agents/, workflow.md, manifest.yaml)

### 1.2 Create BMAD Directory Structure

- [ ] 1.2.1 Create `skills/design/references/bmad/` directory
- [ ] 1.2.2 Create `bmad/agents/core/` directory
- [ ] 1.2.3 Create `bmad/agents/bmm/` directory
- [ ] 1.2.4 Create `bmad/agents/cis/` directory
- [ ] 1.2.5 Create `bmad/workflows/` directory
- [ ] 1.2.6 Create `bmad/teams/` directory

### 1.3 Configuration Files

- [ ] 1.3.1 Create `bmad/config.yaml` with Maestro-specific settings
- [ ] 1.3.2 Create `bmad/manifest.yaml` with 16-agent registry
- [ ] 1.3.3 Create `bmad/adapter.md` with path transform rules
- [ ] 1.3.4 Create `bmad/teams/default-party.csv` with default team

### 1.4 Phase 1 Verification

- [ ] 1.4.1 Verify all directories created
- [ ] 1.4.2 Verify config.yaml parseable
- [ ] 1.4.3 Verify manifest.yaml lists 16 agents

---

## Phase 2: Core & BMM Agents (10 Agents)

**Goal:** Create all Core and BMM module agents in native MD format.

### 2.1 Core Module (1 Agent)

- [ ] 2.1.1 Create `agents/core/bmad-master.md` - Orchestrator
  - Persona, principles, orchestration logic
  - Agent selection algorithm
  - Cross-talk coordination

### 2.2 BMM Module (9 Agents)

- [ ] 2.2.1 Create `agents/bmm/pm.md` - John (Product Manager)
- [ ] 2.2.2 Create `agents/bmm/analyst.md` - Mary (Business Analyst)
- [ ] 2.2.3 Create `agents/bmm/architect.md` - Winston (System Architect)
- [ ] 2.2.4 Create `agents/bmm/dev.md` - Amelia (Developer)
- [ ] 2.2.5 Create `agents/bmm/sm.md` - Sarah (Scrum Master)
- [ ] 2.2.6 Create `agents/bmm/tea.md` - Murat (Test Engineer)
- [ ] 2.2.7 Create `agents/bmm/ux-designer.md` - Sally (UX Designer)
- [ ] 2.2.8 Create `agents/bmm/tech-writer.md` - Paige (Technical Writer)
- [ ] 2.2.9 Create `agents/bmm/quick-flow-solo-dev.md` - Barry (Quick Flow)

### 2.3 Phase 2 Verification

- [ ] 2.3.1 Verify all 10 agent files exist
- [ ] 2.3.2 Verify each agent has valid YAML frontmatter
- [ ] 2.3.3 Verify each agent has Persona, Principles, Expertise sections

---

## Phase 3: CIS Agents & Knowledge (6 Agents + Resources)

**Goal:** Create CIS creative agents with their knowledge resources.

### 3.1 CIS Module Agents

- [ ] 3.1.1 Create `agents/cis/brainstorming-coach.md` - Carson (🧠)
- [ ] 3.1.2 Create `agents/cis/creative-problem-solver.md` - Dr. Quinn (🔬)
- [ ] 3.1.3 Create `agents/cis/design-thinking-coach.md` - Maya (🎯)
- [ ] 3.1.4 Create `agents/cis/innovation-strategist.md` - Victor (💡)
- [ ] 3.1.5 Create `agents/cis/presentation-master.md` - Leo (🎤)

### 3.2 Storyteller Agent with Sidecar

- [ ] 3.2.1 Create `agents/cis/storyteller/` directory
- [ ] 3.2.2 Create `agents/cis/storyteller/storyteller.md` - Sophia (📖)
- [ ] 3.2.3 Create `agents/cis/storyteller/sidecar/` directory
- [ ] 3.2.4 Create sidecar knowledge files (story structures, frameworks)

### 3.3 Brainstorming Resources

- [ ] 3.3.1 Create `workflows/brainstorming/brain-methods.csv` (36 techniques)
- [ ] 3.3.2 Create `workflows/brainstorming/template.md`

### 3.4 Phase 3 Verification

- [ ] 3.4.1 Verify all 6 CIS agent files exist
- [ ] 3.4.2 Verify storyteller sidecar accessible
- [ ] 3.4.3 Verify brain-methods.csv has 36 rows

---

## Phase 4: Workflows (6 CIS Workflows)

**Goal:** Create all 6 CIS workflow definitions.

### 4.1 Party Mode Workflow

- [ ] 4.1.1 Create `workflows/party-mode/workflow.md`
- [ ] 4.1.2 Create `workflows/party-mode/steps/` with step files
  - select.md, respond.md, crosstalk.md, synthesize.md

### 4.2 Brainstorming Workflow

- [ ] 4.2.1 Create `workflows/brainstorming/workflow.md`
- [ ] 4.2.2 Create `workflows/brainstorming/steps/` with step files
  - method-selection.md, diverge.md, cluster.md, converge.md

### 4.3 Design Thinking Workflow

- [ ] 4.3.1 Create `workflows/design-thinking/workflow.md`
- [ ] 4.3.2 Create `workflows/design-thinking/steps/` with step files
  - empathize.md, define.md, ideate.md, prototype.md, test.md

### 4.4 Innovation Strategy Workflow

- [ ] 4.4.1 Create `workflows/innovation-strategy/workflow.md`
- [ ] 4.4.2 Create `workflows/innovation-strategy/steps/` with step files
  - opportunity.md, strategy.md, roadmap.md

### 4.5 Problem Solving Workflow

- [ ] 4.5.1 Create `workflows/problem-solving/workflow.md`
- [ ] 4.5.2 Create `workflows/problem-solving/steps/` with step files
  - define.md, decompose.md, solve.md, validate.md

### 4.6 Storytelling Workflow

- [ ] 4.6.1 Create `workflows/storytelling/workflow.md`
- [ ] 4.6.2 Create `workflows/storytelling/steps/` with step files
  - hero.md, structure.md, tell.md

### 4.7 Phase 4 Verification

- [ ] 4.7.1 Verify all 6 workflow.md files exist
- [ ] 4.7.2 Verify each workflow has steps/ directory
- [ ] 4.7.3 Test each workflow trigger (*brainstorm, etc.)

---

## Phase 5: Integration & SKILL.md Updates

**Goal:** Update design skill to use new BMAD system.

### 5.1 Update SKILL.md

- [ ] 5.1.1 Update `skills/design/SKILL.md` Party Mode section
  - Reference bmad/manifest.yaml instead of party-mode/
  - Add CIS workflow triggers (*brainstorm, *design-thinking, etc.)
  - Update agent count from 12 to 16

### 5.2 Update References

- [ ] 5.2.1 Update `references/party-mode/workflow.md` to point to bmad/
  - Or create redirect from old path
- [ ] 5.2.2 Update any hardcoded paths in existing files

### 5.3 Conductor Integration

- [ ] 5.3.1 Verify Party Mode works at spec stage
- [ ] 5.3.2 Verify Party Mode works at plan stage
- [ ] 5.3.3 Verify Party Mode works at implement stage

### 5.4 Phase 5 Verification

- [ ] 5.4.1 Run `ds` and verify 4 phases work
- [ ] 5.4.2 Trigger [P] at A/P/C checkpoint
- [ ] 5.4.3 Verify 2-3 agents selected from 16

---

## Phase 6: Testing & Documentation

**Goal:** Comprehensive testing and documentation updates.

### 6.1 Unit Tests (16 Agent Loading)

- [ ] 6.1.1 Test bmad-master.md loads correctly
- [ ] 6.1.2 Test all 9 BMM agents load correctly
- [ ] 6.1.3 Test all 6 CIS agents load correctly

### 6.2 Integration Tests (Party Mode)

- [ ] 6.2.1 Test agent selection with technical topic
- [ ] 6.2.2 Test agent selection with product topic
- [ ] 6.2.3 Test agent selection with creative topic
- [ ] 6.2.4 Test cross-talk limits (max 2 rounds)
- [ ] 6.2.5 Test synthesis generation

### 6.3 Integration Tests (CIS Workflows)

- [ ] 6.3.1 Test *brainstorm workflow
- [ ] 6.3.2 Test *design-thinking workflow
- [ ] 6.3.3 Test *innovate workflow
- [ ] 6.3.4 Test *problem-solve workflow
- [ ] 6.3.5 Test *story workflow
- [ ] 6.3.6 Test party-mode via [P] trigger

### 6.4 E2E Test

- [ ] 6.4.1 Full design session: ds → Party Mode → design.md

### 6.5 Regression Tests

- [ ] 6.5.1 Test DISCOVER phase works
- [ ] 6.5.2 Test DEFINE phase works
- [ ] 6.5.3 Test DEVELOP phase works
- [ ] 6.5.4 Test DELIVER phase works

### 6.6 Documentation

- [ ] 6.6.1 Update CODEMAPS overview with new structure
- [ ] 6.6.2 Document Vietnamese communication requirement
- [ ] 6.6.3 Document sync workflow with BMAD upstream

### 6.7 Phase 6 Verification

- [ ] 6.7.1 All 32 test cases pass
- [ ] 6.7.2 No regressions in existing functionality
- [ ] 6.7.3 Documentation complete

---

## Rollback Plan

If integration fails:

1. Delete `skills/design/references/bmad/`
2. Restore from `skills/design/references/party-mode-backup/`
3. Revert SKILL.md changes
4. Git history preserves all changes

---

## Dependencies

- Phase 2 depends on Phase 1 (directory structure)
- Phase 3 depends on Phase 1 (directory structure)
- Phase 4 depends on Phases 2 & 3 (agents must exist first)
- Phase 5 depends on Phase 4 (workflows must exist)
- Phase 6 depends on Phase 5 (integration must be complete)

Phases 2 and 3 can run in parallel.
