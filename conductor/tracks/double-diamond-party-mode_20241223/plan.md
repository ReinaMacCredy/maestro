# Plan: Double Diamond + Party Mode

> **Last Revised:** 2024-12-23 - See [revisions.md](revisions.md) for history

## Phase 1: Foundation - Party Mode Workflow Structure
*Output: Core workflow files and agent framework*

- [x] Task 1.1: Create Party Mode workflow directory structure
  - [x] Create `workflows/party-mode/` directory
  - [x] Create `workflows/party-mode/agents/product/` directory
  - [x] Create `workflows/party-mode/agents/technical/` directory
  - [x] Create `workflows/party-mode/agents/creative/` directory
  - [x] Create `workflows/party-mode/custom/` directory

- [x] Task 1.2: Create workflow.md (all-in-one orchestration)
  - [x] Write Purpose and Prerequisites sections
  - [x] Write Agent Manifest table (12 agents)
  - [x] Write Selection Rules (Primary/Secondary/Tertiary logic)
  - [x] Write Cross-Talk Patterns section
  - [x] Write Response Format section
  - [x] Write Session Flow section

- [x] Task 1.3: Create manifest.yaml (agent registry)
  - [x] Define agent schema (id, name, icon, module, expertise, path)
  - [x] Register all 12 built-in agents
  - [x] Add custom agent discovery rules

- [x] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)

## Phase 2: Agent Personas - Product Module
*Output: 3 product-focused agent files*

- [x] Task 2.1: Create PM agent (John)
  - [x] Write YAML frontmatter (name, title, icon, role, identity, communication_style, principles)
  - [x] Write extended instructions in markdown body
  - [x] Define when this agent speaks
  - [x] Define response patterns and cross-talk behaviors

- [x] Task 2.2: Create Analyst agent (Mary)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task 2.3: Create UX agent (Sally)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task: Conductor - User Manual Verification 'Phase 2' (Protocol in workflow.md)

## Phase 3: Agent Personas - Technical Module
*Output: 4 technical-focused agent files*

- [x] Task 3.1: Create Architect agent (Winston)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task 3.2: Create Developer agent (Amelia)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task 3.3: Create QA agent (Murat)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task 3.4: Create Docs agent (Paige)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task: Conductor - User Manual Verification 'Phase 3' (Protocol in workflow.md)

## Phase 4: Agent Personas - Creative Module
*Output: 5 creative-focused agent files*

- [x] Task 4.1: Create Storyteller agent (Sophia)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task 4.2: Create Brainstorm agent (Carson)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task 4.3: Create Design Thinking agent (Maya)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task 4.4: Create Strategist agent (Victor)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task 4.5: Create Problem Solver agent (Dr. Quinn)
  - [x] Write YAML frontmatter
  - [x] Write extended instructions
  - [x] Define response patterns

- [x] Task 4.6: Verify all agents match BMAD-METHOD repository
  - [x] Compare role fields (use `+` not `&`)
  - [x] Compare identity fields verbatim
  - [x] Compare communication_style verbatim
  - [x] Source: https://github.com/bmad-code-org/BMAD-METHOD

- [ ] Task: Conductor - User Manual Verification 'Phase 4' (Protocol in workflow.md)

## Phase 5: Custom Agent Support
*Output: Template and documentation for user-defined agents*

- [ ] Task 5.1: Create agent template
  - [ ] Create `workflows/party-mode/custom/_template.md`
  - [ ] Document all required and optional fields
  - [ ] Include example values

- [ ] Task 5.2: Create custom agent README
  - [ ] Create `workflows/party-mode/custom/README.md`
  - [ ] Write step-by-step instructions for creating custom agents
  - [ ] Document how custom agents are discovered
  - [ ] Add examples of custom agent use cases

- [ ] Task: Conductor - User Manual Verification 'Phase 5' (Protocol in workflow.md)

## Phase 6: Command & Skill Updates
*Output: Updated design.toml and skill files*

- [ ] Task 6.1: Update design.toml with Double Diamond + Party Mode
  - [ ] Write new prompt with 4 explicit phases (DISCOVER, DEFINE, DEVELOP, DELIVER)
  - [ ] Add phase markers ("📍 Phase: [NAME]")
  - [ ] Add A/P/C checkpoints at 4 locations
  - [ ] Add Party Mode protocol (references workflow.md)
  - [ ] Add loop-back support ("revisit [phase]")
  - [ ] Add mini-grounding at transitions
  - [ ] Add full grounding before DELIVER
  - [ ] Add track_id generation logic
  - [ ] Add existing track handling
  - [ ] Add handoff section with fb reference

- [ ] Task 6.2: Update skills/design/SKILL.md
  - [ ] Add reference to workflows/party-mode/
  - [ ] Add Double Diamond phase descriptions
  - [ ] Keep existing principles (one question, multiple choice, grounding)

- [ ] Task 6.3: Update skills/conductor/SKILL.md
  - [ ] Add Double Diamond → Conductor phase mapping
  - [ ] Document: Discover/Define = Requirements, Develop/Deliver = Plan+Implement
  - [ ] Reference Party Mode as optional design enhancement

- [ ] Task: Conductor - User Manual Verification 'Phase 6' (Protocol in workflow.md)

## Phase 7: Documentation & Tracks Update
*Output: Updated tracks.md and documentation*

- [ ] Task 7.1: Update conductor/tracks.md
  - [ ] Add new track section for double-diamond-party-mode_20241223

- [ ] Task 7.2: Update workflows/README.md
  - [ ] Add party-mode to workflow list
  - [ ] Document relationship to design command

- [ ] Task: Conductor - User Manual Verification 'Phase 7' (Protocol in workflow.md)

## Phase 8: Verification & Testing
*Output: Verified, working implementation*

- [ ] Task 8.1: Test design → newTrack integration
  - [ ] Run /conductor:design with Party Mode
  - [ ] Verify design.md output format
  - [ ] Run /conductor:newTrack with track_id
  - [ ] Verify spec.md + plan.md generation

- [ ] Task 8.2: Test Party Mode functionality
  - [ ] Trigger [P] at each checkpoint
  - [ ] Verify 2-3 agents selected appropriately
  - [ ] Verify cross-talk patterns work
  - [ ] Verify synthesis before return

- [ ] Task 8.3: Test loop-back functionality
  - [ ] Say "revisit discover" mid-session
  - [ ] Verify phase resets correctly
  - [ ] Verify context preserved

- [ ] Task 8.4: Test custom agent support
  - [ ] Create test custom agent in custom/
  - [ ] Verify agent is discovered
  - [ ] Verify agent participates in Party Mode

- [ ] Task: Conductor - User Manual Verification 'Phase 8' (Protocol in workflow.md)
