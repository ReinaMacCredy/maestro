# Plan: Spec-Compliant Skills-Only Architecture Migration

## Epic 1: Prepare Infrastructure

### 1.1 Create Target Directories
- [ ] 1.1.1 Create `skills/beads/references/`
- [ ] 1.1.2 Create `skills/design/references/party-mode/agents/{creative,product,technical}`
- [ ] 1.1.3 Create `skills/design/references/party-mode/custom/`
- [ ] 1.1.4 Create `skills/dispatching-parallel-agents/references/agent-coordination/{examples,patterns}`
- [ ] 1.1.5 Create `skills/conductor/references/{conductor,workflows,schemas}`
- [ ] 1.1.6 Create `skills/session-compaction/`

### 1.2 Create Validation Scripts
- [ ] 1.2.1 Create `scripts/validate-links.sh` for broken link detection
- [ ] 1.2.2 Create `scripts/validate-anchors.sh` for anchor verification
- [ ] 1.2.3 Test scripts work before migration

---

## Epic 2: Move Workflow Files (Commit 1)

### 2.1 Move workflows/beads/ → skills/beads/references/
- [ ] 2.1.1 `git mv workflows/beads/workflow.md skills/beads/references/`
- [ ] 2.1.2 `git mv workflows/beads/references/* skills/beads/references/`

### 2.2 Move workflows/conductor/ → skills/conductor/references/conductor/
- [ ] 2.2.1 `git mv workflows/conductor/* skills/conductor/references/conductor/`

### 2.3 Move root workflows/*.md → skills/conductor/references/workflows/
- [ ] 2.3.1 `git mv workflows/setup.md skills/conductor/references/workflows/`
- [ ] 2.3.2 `git mv workflows/newtrack.md skills/conductor/references/workflows/`
- [ ] 2.3.3 `git mv workflows/implement.md skills/conductor/references/workflows/`
- [ ] 2.3.4 `git mv workflows/status.md skills/conductor/references/workflows/`
- [ ] 2.3.5 `git mv workflows/revert.md skills/conductor/references/workflows/`
- [ ] 2.3.6 `git mv workflows/revise.md skills/conductor/references/workflows/`
- [ ] 2.3.7 `git mv workflows/finish.md skills/conductor/references/workflows/`
- [ ] 2.3.8 `git mv workflows/validate.md skills/conductor/references/workflows/`
- [ ] 2.3.9 `git mv workflows/README.md skills/conductor/references/pipeline.md`

### 2.4 Move workflows/schemas/ → skills/conductor/references/schemas/
- [ ] 2.4.1 `git mv workflows/schemas/* skills/conductor/references/schemas/`

### 2.5 Move workflows/party-mode/ → skills/design/references/party-mode/
- [ ] 2.5.1 `git mv workflows/party-mode/* skills/design/references/party-mode/`

### 2.6 Move workflows/context-engineering/ → skills/design/references/
- [ ] 2.6.1 `git mv workflows/context-engineering/session-lifecycle.md skills/design/references/`
- [ ] 2.6.2 `git mv workflows/context-engineering/references/* skills/design/references/`

### 2.7 Move workflows/agent-coordination/ → skills/dispatching-parallel-agents/references/
- [ ] 2.7.1 `git mv workflows/agent-coordination/* skills/dispatching-parallel-agents/references/agent-coordination/`

### 2.8 Cleanup Empty Directories
- [ ] 2.8.1 Remove empty workflows/ subdirectories
- [ ] 2.8.2 Remove workflows/ directory
- [ ] 2.8.3 Commit: `refactor: move workflows/ to skills/*/references/`

---

## Epic 3: Handle Commands (Commit 1 continued)

### 3.1 Delete Pure Alias Commands
- [ ] 3.1.1 `git rm commands/ds.md`
- [ ] 3.1.2 `git rm commands/fb.md`
- [ ] 3.1.3 `git rm commands/rb.md`
- [ ] 3.1.4 `git rm commands/ci.md`
- [ ] 3.1.5 `git rm commands/cn.md`
- [ ] 3.1.6 `git rm commands/ct.md`

### 3.2 Create session-compaction Skill
- [ ] 3.2.1 Create `skills/session-compaction/SKILL.md` with compact.md content
- [ ] 3.2.2 `git rm commands/compact.md`

### 3.3 Merge Remaining Commands
- [ ] 3.3.1 Merge `commands/ground.md` → `skills/design/references/grounding.md`
- [ ] 3.3.2 Merge `commands/decompose-task.md` → `skills/conductor/references/decompose-task.md`
- [ ] 3.3.3 Merge `commands/conductor-design.md` → `skills/design/references/conductor-design-workflow.md`
- [ ] 3.3.4 Merge `commands/conductor-migrate-beads.md` → `skills/conductor/references/migrate-beads.md`
- [ ] 3.3.5 Merge remaining conductor-*.md into corresponding workflows/*.md already moved

### 3.4 Cleanup Commands Directory
- [ ] 3.4.1 `git rm` remaining command files
- [ ] 3.4.2 Remove commands/ directory
- [ ] 3.4.3 Amend commit or new commit: `refactor: remove commands/, merge to skills/`

---

## Epic 4: Update References (Commit 2)

### 4.1 Update Skill Entry Points
- [ ] 4.1.1 Update `skills/beads/SKILL.md` entry points table (workflows/beads/ → references/)
- [ ] 4.1.2 Update `skills/beads/SKILL.md` all internal paths
- [ ] 4.1.3 Update `skills/conductor/SKILL.md` workflow references
- [ ] 4.1.4 Update `skills/design/SKILL.md` party-mode and grounding paths
- [ ] 4.1.5 Update `skills/dispatching-parallel-agents/SKILL.md` agent-coordination paths
- [ ] 4.1.6 Update `skills/subagent-driven-development/SKILL.md` cross-skill references

### 4.2 Update Documentation
- [ ] 4.2.1 Update `AGENTS.md` architecture section (remove commands/, workflows/)
- [ ] 4.2.2 Update `README.md` directory structure
- [ ] 4.2.3 Update `CLAUDE.md` workflow triggers section
- [ ] 4.2.4 Update `TUTORIAL.md` path examples
- [ ] 4.2.5 Update `docs/PIPELINE_ARCHITECTURE.md` references

### 4.3 Update Templates
- [ ] 4.3.1 Remove `templates/claude-code-setup/.claude/commands/`
- [ ] 4.3.2 Create `templates/claude-code-setup/skills/audit/SKILL.md`
- [ ] 4.3.3 Update `templates/claude-code-setup/SETUP.md`
- [ ] 4.3.4 Update `templates/claude-code-setup/README.md`
- [ ] 4.3.5 Update `templates/claude-code-setup/AGENTS.md`

### 4.4 Add Cross-Skill Coupling Headers
- [ ] 4.4.1 Add coupling comment to `skills/subagent-driven-development/SKILL.md`
- [ ] 4.4.2 Add coupling comment to `skills/dispatching-parallel-agents/SKILL.md`

### 4.5 Commit Reference Updates
- [ ] 4.5.1 Run validation scripts
- [ ] 4.5.2 Fix any broken links found
- [ ] 4.5.3 Commit: `docs: update all references for new architecture`

---

## Epic 5: Finalize Release (Commit 3)

### 5.1 Version and Changelog
- [ ] 5.1.1 Verify MIGRATION_V2.md is complete
- [ ] 5.1.2 Verify MIGRATION_PATH_MAP.md is complete
- [ ] 5.1.3 Update CHANGELOG.md with 2.0.0 section

### 5.2 Final Validation
- [ ] 5.2.1 Run `rg "workflows/" --type md` (expect 0 except migration docs)
- [ ] 5.2.2 Run `rg "commands/" --type md` (expect 0 except migration docs)
- [ ] 5.2.3 Validate plugin.json: `cat .claude-plugin/plugin.json | jq .`
- [ ] 5.2.4 Test skill triggers: ds, fb, rb, /conductor-implement

### 5.3 Breaking Change Commit
- [ ] 5.3.1 Stage all changes
- [ ] 5.3.2 Commit with: `feat!: migrate to spec-compliant skills-only architecture`
- [ ] 5.3.3 Verify CI bumps to 2.0.0
- [ ] 5.3.4 Push and create release

---

## Dependencies

```
Epic 1 (Prepare) 
    ↓
Epic 2 (Move Workflows) + Epic 3 (Handle Commands) [parallel within, sequential between]
    ↓
Epic 4 (Update References)
    ↓
Epic 5 (Finalize Release)
```

## Estimated Effort

| Epic | Tasks | Complexity |
|------|-------|------------|
| Epic 1 | 9 | Low |
| Epic 2 | 21 | Medium (git mv operations) |
| Epic 3 | 14 | Medium (merge + delete) |
| Epic 4 | 17 | High (95 reference updates) |
| Epic 5 | 9 | Low |
| **Total** | **70** | |

## Risk Mitigation

- **Commit 1 (moves)**: Pure `git mv`, no content changes → easy revert
- **Commit 2 (refs)**: Content changes → validate before commit
- **Commit 3 (release)**: Version bump → feature freeze during
