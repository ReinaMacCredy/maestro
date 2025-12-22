# Spec: Double Diamond + Party Mode

## Overview

Enhance the `/conductor:design` command with a Double Diamond methodology and multi-agent Party Mode for collaborative design sessions. This brings BMAD v6 patterns into Maestro's Conductor workflow.

## Functional Requirements

### FR1: Double Diamond Flow
- **FR1.1:** Implement 4 explicit phases: DISCOVER, DEFINE, DEVELOP, DELIVER
- **FR1.2:** Announce current phase with visual marker: "📍 Phase: [NAME]"
- **FR1.3:** Define exit criteria for each phase
- **FR1.4:** Support loop-back commands ("revisit [phase]")
- **FR1.5:** Fast path option for detailed inputs or "quick" mode

### FR2: A/P/C Checkpoints
- **FR2.1:** Present menu at 4 checkpoints (end of each phase):
  - [A] Advanced - deeper analysis, assumption audit
  - [P] Party - multi-perspective review
  - [C] Continue - proceed to next phase
  - [↩ Back] - return to previous phase
- **FR2.2:** Define specific behaviors for [A] at each phase

### FR3: Party Mode Workflow
- **FR3.1:** Create `workflows/party-mode/workflow.md` with all orchestration rules
- **FR3.2:** Create `workflows/party-mode/manifest.yaml` with agent registry
- **FR3.3:** Implement 12 agent personas across 3 modules:
  - Product: PM (John), Analyst (Mary), UX (Sally)
  - Technical: Architect (Winston), Developer (Amelia), QA (Murat), Docs (Paige)
  - Creative: Storyteller (Sophia), Brainstorm (Carson), Design (Maya), Strategist (Victor), Solver (Dr. Quinn)
- **FR3.4:** Use hybrid MD + YAML frontmatter format for agents

### FR4: Agent Selection & Cross-Talk
- **FR4.1:** Implement relevance analysis for agent selection
- **FR4.2:** Select Primary (best match), Secondary (complementary), Tertiary (devil's advocate)
- **FR4.3:** Response format: `[Icon] **[Name]**: [Response]`
- **FR4.4:** Cross-talk patterns: reference each other, build on ideas, respectfully disagree

### FR5: Custom Agent Support
- **FR5.1:** Create `workflows/party-mode/custom/` directory
- **FR5.2:** Create `_template.md` with all required fields
- **FR5.3:** Create `README.md` with instructions
- **FR5.4:** Auto-discover custom agents in manifest

### FR6: Grounding Integration
- **FR6.1:** Mini-ground at DISCOVER → DEFINE transition
- **FR6.2:** Mini-ground at DEFINE → DEVELOP transition
- **FR6.3:** Mini-ground at DEVELOP → DELIVER transition
- **FR6.4:** Full grounding before DELIVER completion
- **FR6.5:** Record grounding notes in design.md output

### FR7: Conductor Integration
- **FR7.1:** Output design.md compatible with newTrack parsing
- **FR7.2:** Generate track_id as `{shortname}_{YYYYMMDD}`
- **FR7.3:** Handle existing track detection (append vs overwrite)
- **FR7.4:** Provide handoff message with track_id for `fb` command
- **FR7.5:** Map Double Diamond to Conductor phases in skills/conductor/SKILL.md

## Non-Functional Requirements

- **NFR1:** Follow Maestro conventions (MD + YAML frontmatter for agents)
- **NFR2:** No external runtime dependencies (LLM-as-orchestrator)
- **NFR3:** Compatible with Claude Code, Amp, Codex
- **NFR4:** Workflows are single source of truth; commands mirror them

## Acceptance Criteria

- [ ] `/conductor:design` shows "📍 Phase: [NAME]" markers throughout
- [ ] A/P/C menu appears at 4 checkpoints
- [ ] [P] triggers Party Mode with 2-3 relevant agents
- [ ] Agents respond in character with cross-talk
- [ ] "revisit [phase]" loops back correctly
- [ ] Mini-grounding occurs at phase transitions
- [ ] Final design.md works with `/conductor:newTrack`
- [ ] Custom agents in `custom/` folder are discovered
- [ ] skills/conductor/SKILL.md documents DD → Conductor mapping

## Out of Scope

- Text-to-Speech integration (BMAD feature)
- External API hooks for custom GPTs/Gems
- Persistent Party Mode session state
- Game Development agents (BMGD module from BMAD)
