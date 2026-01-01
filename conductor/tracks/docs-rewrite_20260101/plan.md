# Plan: Documentation Rewrite

## Phase 1: Preparation & Cleanup

### 1.1 Archive Deprecated Docs
- [ ] 1.1.1 Create docs/archive/ directory
- [ ] 1.1.2 Move docs/MIGRATION_V2.md to docs/archive/

### 1.2 Audit Current Content
- [ ] 1.2.1 List all stale references in current docs (removed skills, old paths)
- [ ] 1.2.2 Identify content to merge from GLOBAL_CONFIG.md, manual-workflow-guide.md, handoff-system.md

## Phase 2: Core Documentation Rewrite

### 2.1 README.md (~100 lines)
- [ ] 2.1.1 Write new README with: badges, one-liner, install commands, quick start, links
- [ ] 2.1.2 Remove all Mermaid diagrams (move to ARCHITECTURE.md)
- [ ] 2.1.3 Update skill table to current 6 skills
- [ ] 2.1.4 Verify all links work

### 2.2 SETUP_GUIDE.md (~150 lines)
- [ ] 2.2.1 Rewrite installation section (all tools in one place)
- [ ] 2.2.2 Merge GLOBAL_CONFIG.md content into new "Configure Global Agent" section
- [ ] 2.2.3 Add CLI tools section with verification commands
- [ ] 2.2.4 Delete docs/GLOBAL_CONFIG.md after merge

### 2.3 TUTORIAL.md (~500 lines)
- [ ] 2.3.1 Rewrite "Why This Exists" section (concise)
- [ ] 2.3.2 Rewrite "Core Workflow" section with clear pipeline
- [ ] 2.3.3 Merge handoff-system.md content into "Understanding Handoff" section
- [ ] 2.3.4 Merge manual-workflow-guide.md useful content
- [ ] 2.3.5 Add streamlined "Common Scenarios" section
- [ ] 2.3.6 Remove verbose examples, keep 2-3 best ones
- [ ] 2.3.7 Delete docs/handoff-system.md and docs/manual-workflow-guide.md after merge

### 2.4 REFERENCE.md (NEW, ~300 lines)
- [ ] 2.4.1 Create file with header and structure
- [ ] 2.4.2 Add Commands table (all /conductor-* commands)
- [ ] 2.4.3 Add Triggers table (ds, fb, rb, tdd, debug, trace, etc.)
- [ ] 2.4.4 Add Skills reference table (6 skills with descriptions)
- [ ] 2.4.5 Add Troubleshooting quick reference table
- [ ] 2.4.6 Add bd/bv quick reference

### 2.5 AGENTS.md (~200 lines)
- [ ] 2.5.1 Rewrite with agent-only focus (remove human-facing content)
- [ ] 2.5.2 Add decision tree for workflow routing
- [ ] 2.5.3 Add session protocol section
- [ ] 2.5.4 Add critical rules section (--robot-*, --json, TDD, etc.)

### 2.6 docs/ARCHITECTURE.md (~300 lines)
- [ ] 2.6.1 Rename PIPELINE_ARCHITECTURE.md to ARCHITECTURE.md
- [ ] 2.6.2 Move Mermaid diagrams from README
- [ ] 2.6.3 Add pipeline flow explanation
- [ ] 2.6.4 Update any stale references

## Phase 3: Verification

### 3.1 Link Validation
- [ ] 3.1.1 Run link checker on all markdown files
- [ ] 3.1.2 Fix any broken internal links

### 3.2 Content Validation
- [ ] 3.2.1 Verify no references to removed skills (file-beads, review-beads)
- [ ] 3.2.2 Verify all commands work
- [ ] 3.2.3 Check line counts are within limits

### 3.3 Final Review
- [ ] 3.3.1 Read through all docs for consistency
- [ ] 3.3.2 Verify GitHub rendering of README

## Automated Verification

```bash
# Line count verification
wc -l README.md SETUP_GUIDE.md TUTORIAL.md REFERENCE.md AGENTS.md docs/ARCHITECTURE.md

# Link validation
./scripts/validate-links.sh .

# Stale reference check
rg "file-beads|review-beads" *.md docs/*.md

# Render check (manual)
# Open README.md in GitHub preview
```

## Dependencies

```
Phase 1 → Phase 2 (cleanup before rewrite)
Phase 2.2.4 depends on 2.2.2 (delete after merge)
Phase 2.3.7 depends on 2.3.3, 2.3.4 (delete after merge)
Phase 2.6.2 depends on 2.1.2 (move diagrams before removing from README)
Phase 3 depends on Phase 2 (verify after all rewrites)
```

## Estimates

| Phase | Estimated Time |
|-------|----------------|
| Phase 1: Preparation | 15 min |
| Phase 2: Core Rewrite | 2-3 hours |
| Phase 3: Verification | 30 min |
| **Total** | ~3-4 hours |
