# Plan: Double Diamond + Party Mode

## Phase 1: Foundation - Party Mode Workflow Structure
*Output: Core workflow files and agent framework*

- [ ] Task 1.1: Create Party Mode workflow directory structure
  - [ ] Create `workflows/party-mode/` directory
  - [ ] Create `workflows/party-mode/agents/product/` directory
  - [ ] Create `workflows/party-mode/agents/technical/` directory
  - [ ] Create `workflows/party-mode/agents/creative/` directory
  - [ ] Create `workflows/party-mode/custom/` directory

- [ ] Task 1.2: Create workflow.md (all-in-one orchestration)
  - [ ] Write Purpose and Prerequisites sections
  - [ ] Write Agent Manifest table (12 agents)
  - [ ] Write Selection Rules (Primary/Secondary/Tertiary logic)
  - [ ] Write Cross-Talk Patterns section
  - [ ] Write Response Format section
  - [ ] Write Session Flow section

- [ ] Task 1.3: Create manifest.yaml (agent registry)
  - [ ] Define agent schema (id, name, icon, module, expertise, path)
  - [ ] Register all 12 built-in agents
  - [ ] Add custom agent discovery rules

- [ ] Task: Conductor - User Manual Verification 'Phase 1' (Protocol in workflow.md)

## Phase 2: Agent Personas - Product Module
*Output: 3 product-focused agent files*

- [ ] Task 2.1: Create PM agent (John)
  - [ ] Write YAML frontmatter (name, title, icon, role, identity, communication_style, principles)
  - [ ] Write extended instructions in markdown body
  - [ ] Define when this agent speaks
  - [ ] Define response patterns and cross-talk behaviors

- [ ] Task 2.2: Create Analyst agent (Mary)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task 2.3: Create UX agent (Sally)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task: Conductor - User Manual Verification 'Phase 2' (Protocol in workflow.md)

## Phase 3: Agent Personas - Technical Module
*Output: 4 technical-focused agent files*

- [ ] Task 3.1: Create Architect agent (Winston)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task 3.2: Create Developer agent (Amelia)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task 3.3: Create QA agent (Murat)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task 3.4: Create Docs agent (Paige)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task: Conductor - User Manual Verification 'Phase 3' (Protocol in workflow.md)

## Phase 4: Agent Personas - Creative Module
*Output: 5 creative-focused agent files*

- [ ] Task 4.1: Create Storyteller agent (Sophia)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task 4.2: Create Brainstorm agent (Carson)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task 4.3: Create Design Thinking agent (Maya)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task 4.4: Create Strategist agent (Victor)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

- [ ] Task 4.5: Create Problem Solver agent (Dr. Quinn)
  - [ ] Write YAML frontmatter
  - [ ] Write extended instructions
  - [ ] Define response patterns

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
