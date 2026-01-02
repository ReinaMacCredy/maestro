# Plan: Unify SA/MA into FULL Mode

## Phase 1: Core Skill Updates (Foundation)

### 1.1 Update AGENTS.md Root Files
- [ ] **1.1.1** Update `AGENTS.md` - Remove SA vs MA decision tree, add FULL mode description
  - File: `AGENTS.md`
  - Remove lines 22-29 (SA/MA routing)
  - Add: "Always FULL mode via orchestrator"
- [ ] **1.1.2** Update `~/.config/amp/AGENTS.md` - Remove `<!-- BEGIN maestro-village -->` section
  - File: `~/.config/amp/AGENTS.md`

### 1.2 Update Conductor Skill
- [ ] **1.2.1** Update `conductor/SKILL.md` - Remove SA/MA mode table
  - File: `.claude/skills/conductor/SKILL.md`
  - Replace SA/MA Beads Integration table with unified approach
- [ ] **1.2.2** Update `preflight-beads.md` - Remove Village MCP check, mode detection
  - File: `.claude/skills/conductor/references/preflight-beads.md`
  - Remove Step 3 (Village MCP Availability)
  - Remove mode field from session state
- [ ] **1.2.3** Update `implement.md` - Remove SINGLE_AGENT routing
  - File: `.claude/skills/conductor/references/workflows/implement.md`
  - Always route to orchestrator
- [ ] **1.2.4** Update `beads-session.md` - Remove SA/MA Mode sections
  - File: `.claude/skills/conductor/references/beads-session.md`
  - Consolidate to single unified flow
- [ ] **1.2.5** Update `beads-integration.md` - Remove mode-specific flows
  - File: `.claude/skills/conductor/references/beads-integration.md`
  - Remove SA vs MA Mode Flows sections

### 1.3 Update Orchestrator Skill
- [ ] **1.3.1** Update `orchestrator/SKILL.md` - Remove Village references
  - File: `.claude/skills/orchestrator/SKILL.md`
- [ ] **1.3.2** Update `preparation.md` - Replace Village with Agent Mail
  - File: `.claude/skills/orchestrator/references/preparation.md`
- [ ] **1.3.3** Update `monitoring.md` - Replace Village with Agent Mail
  - File: `.claude/skills/orchestrator/references/monitoring.md`
- [ ] **1.3.4** Update `graceful-fallback.md` - Change to HALT policy
  - File: `.claude/skills/orchestrator/references/patterns/graceful-fallback.md`
  - Remove SA fallback, add HALT on Agent Mail failure
- [ ] **1.3.5** Update `worker-prompt.md` - Remove Village commands
  - File: `.claude/skills/orchestrator/references/worker-prompt.md`

## Phase 2: Beads & Design Skills

### 2.1 Update Beads Skill
- [ ] **2.1.1** Update `workflow.md` - Remove team/role/leader concepts
  - File: `.claude/skills/beads/references/workflow.md`
- [ ] **2.1.2** Update `WORKFLOWS.md` - Remove Multi-Agent Workflows section
  - File: `.claude/skills/beads/references/WORKFLOWS.md`
- [ ] **2.1.3** Update `workflow-integration.md` - Remove mode detection events
  - File: `.claude/skills/beads/references/workflow-integration.md`
- [ ] **2.1.4** Update `conductor-integration.md` - Remove SA/MA references
  - File: `.claude/skills/beads/references/conductor-integration.md`
- [ ] **2.1.5** Update `auto-orchestrate.md` - Remove mode routing
  - File: `.claude/skills/beads/references/auto-orchestrate.md`
- [ ] **2.1.6** Update `GIT_INTEGRATION.md` - Remove Team Branch Pattern
  - File: `.claude/skills/beads/references/GIT_INTEGRATION.md`
- [ ] **2.1.7** Update `CONFIG.md` - Remove Team ID config
  - File: `.claude/skills/beads/references/CONFIG.md`
- [ ] **2.1.8** Update `LABELS.md` - Remove team-prefixed labels
  - File: `.claude/skills/beads/references/LABELS.md`
- [ ] **2.1.9** Update `BOUNDARIES.md` - Remove role notes
  - File: `.claude/skills/beads/references/BOUNDARIES.md`

### 2.2 Update Design Skill
- [ ] **2.2.1** Update `session-lifecycle.md` - Remove mode references
  - File: `.claude/skills/design/references/session-lifecycle.md`

### 2.3 Update Maestro Core
- [ ] **2.3.1** Update `maestro-core/SKILL.md` - Update Fallback Policies
  - File: `.claude/skills/maestro-core/SKILL.md`
  - Remove Village degrade policy
- [ ] **2.3.2** Update `glossary.md` - Remove SA/MA definitions
  - File: `.claude/skills/maestro-core/references/glossary.md`

## Phase 3: Documentation Updates

### 3.1 Root Documentation
- [ ] **3.1.1** Update `REFERENCE.md` - Remove Village section
  - File: `REFERENCE.md`
- [ ] **3.1.2** Update `SETUP_GUIDE.md` - Remove Village setup
  - File: `SETUP_GUIDE.md`
- [ ] **3.1.3** Update `README.md` - Remove Village mentions
  - File: `README.md`
- [ ] **3.1.4** Update `CLAUDE.md` - Update fallback policy
  - File: `CLAUDE.md`
- [ ] **3.1.5** Update `TUTORIAL.md` - Remove mode references
  - File: `TUTORIAL.md`

### 3.2 Architecture & CODEMAPS
- [ ] **3.2.1** Update `docs/ARCHITECTURE.md` - Remove SA/MA mode box from diagrams
  - File: `docs/ARCHITECTURE.md`
- [ ] **3.2.2** Update `conductor/CODEMAPS/overview.md` - Remove SA/MA section
  - File: `conductor/CODEMAPS/overview.md`
- [ ] **3.2.3** Update `conductor/CODEMAPS/skills.md` - Update orchestrator description
  - File: `conductor/CODEMAPS/skills.md`

### 3.3 Conductor Context Files
- [ ] **3.3.1** Update `conductor/AGENTS.md` - Remove Village learnings/commands
  - File: `conductor/AGENTS.md`
- [ ] **3.3.2** Update `conductor/tech-stack.md` - Remove Village from dependencies
  - File: `conductor/tech-stack.md`
- [ ] **3.3.3** Update `conductor/workflow.md` - Remove mode references if any
  - File: `conductor/workflow.md`
- [ ] **3.3.4** Update `conductor/tracks.md` - Remove mode references
  - File: `conductor/tracks.md`

### 3.4 Delete Village Documentation
- [ ] **3.4.1** Delete `docs/VILLAGE.md`
  - File: `docs/VILLAGE.md`
  - Action: Delete file entirely

## Phase 4: Additional Updates

### 4.1 More Conductor References
- [ ] **4.1.1** Update `remember.md` - Remove mode-specific patterns
  - File: `.claude/skills/conductor/references/remember.md`
- [ ] **4.1.2** Update `pipeline.md` - Remove mode routing
  - File: `.claude/skills/conductor/references/pipeline.md`
- [ ] **4.1.3** Update `beads-facade.md` - Remove mode field
  - File: `.claude/skills/conductor/references/beads-facade.md`
- [ ] **4.1.4** Update `track-init-beads.md` - Remove mode references
  - File: `.claude/skills/conductor/references/track-init-beads.md`
- [ ] **4.1.5** Update `decompose-task.md` - Remove Village commands
  - File: `.claude/skills/conductor/references/decompose-task.md`
- [ ] **4.1.6** Update `doc-sync/integration.md` - Remove mode references
  - File: `.claude/skills/conductor/references/doc-sync/integration.md`

### 4.2 Validation & Structure
- [ ] **4.2.1** Update `validation/lifecycle.md` - Update HALT/DEGRADE
  - File: `.claude/skills/conductor/references/validation/lifecycle.md`
- [ ] **4.2.2** Update `validation/beads/checks.md` - Remove mode checks
  - File: `.claude/skills/conductor/references/validation/beads/checks.md`

### 4.3 Writing Skills
- [ ] **4.3.1** Update `skill-structure.md` - Update HALT/DEGRADE guidelines
  - File: `.claude/skills/writing-skills/references/skill-structure.md`

### 4.4 Orchestrator Examples
- [ ] **4.4.1** Update `dispatch-three-agents.md` - Remove Village coordination
  - File: `.claude/skills/orchestrator/references/examples/dispatch-three-agents.md`
- [ ] **4.4.2** Update `parallel-dispatch.md` - Remove fallback to SA
  - File: `.claude/skills/orchestrator/references/patterns/parallel-dispatch.md`
- [ ] **4.4.3** Update `architecture.md` - Remove Village architecture
  - File: `.claude/skills/orchestrator/references/architecture.md`
- [ ] **4.4.4** Update `preflight.md` - Remove mode detection
  - File: `.claude/skills/orchestrator/references/preflight.md`

### 4.5 Templates
- [ ] **4.5.1** Update `templates/workflow.md` - Remove mode references
  - File: `templates/workflow.md`
- [ ] **4.5.2** Update `templates/SETUP.md` - Remove Village setup
  - File: `templates/SETUP.md`

### 4.6 FILE_BEADS Reference
- [ ] **4.6.1** Update `FILE_BEADS.md` - Remove mode-specific filing
  - File: `.claude/skills/beads/references/FILE_BEADS.md`

## Phase 5: Verification

### 5.1 Link Validation
- [ ] **5.1.1** Run `./scripts/validate-links.sh .` - Fix any broken links
- [ ] **5.1.2** Run `./scripts/validate-anchors.sh .` - Fix any broken anchors

### 5.2 Search Verification
- [ ] **5.2.1** Grep for remaining SA/MA references
  - Command: `rg -i "single.?agent|multi.?agent|\bsa\b|\bma\b" --type md -l`
  - Exclude: CHANGELOG.md, archive/
- [ ] **5.2.2** Grep for remaining Village references
  - Command: `rg -i "village|\.beads-village|bv --robot|init.*team" --type md -l`
  - Exclude: CHANGELOG.md, archive/
- [ ] **5.2.3** Grep for remaining mode detection
  - Command: `rg "mode.*(detect|SA|MA)|SINGLE_AGENT" --type md -l`
  - Exclude: CHANGELOG.md, archive/

### 5.3 Functional Verification
- [ ] **5.3.1** Test single-task execution via orchestrator
- [ ] **5.3.2** Test multi-task parallel execution

## Automated Verification

```bash
# Run after all changes
./scripts/validate-links.sh .
./scripts/validate-anchors.sh .

# Verify no stray references
rg -i "village|\.beads-village|bv --robot" --type md -l | grep -v CHANGELOG | grep -v archive
rg "SINGLE_AGENT|mode.*detect" --type md -l | grep -v CHANGELOG | grep -v archive
```
