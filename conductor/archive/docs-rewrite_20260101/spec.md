# Specification: Documentation Rewrite

## Overview

Complete rewrite and restructure of Maestro's documentation to be accurate, scannable, and audience-appropriate. The current documentation is outdated, verbose, and missing coverage for v3.1 features.

## Functional Requirements

### FR-1: File Structure Reorganization

- **FR-1.1**: Create new REFERENCE.md file for quick command lookup
- **FR-1.2**: Merge docs/GLOBAL_CONFIG.md into SETUP_GUIDE.md
- **FR-1.3**: Merge docs/manual-workflow-guide.md into TUTORIAL.md
- **FR-1.4**: Merge docs/handoff-system.md into TUTORIAL.md
- **FR-1.5**: Rename docs/PIPELINE_ARCHITECTURE.md to docs/ARCHITECTURE.md
- **FR-1.6**: Archive docs/MIGRATION_V2.md (move to docs/archive/)

### FR-2: README.md Rewrite

- **FR-2.1**: Slim down to ~100 lines max
- **FR-2.2**: Focus on: quick install, quick start, links to other docs
- **FR-2.3**: Remove verbose workflow diagrams (move to ARCHITECTURE.md)
- **FR-2.4**: Update all skill references to current v3.1 structure

### FR-3: SETUP_GUIDE.md Rewrite

- **FR-3.1**: Consolidate all installation methods in one place
- **FR-3.2**: Include global config from merged GLOBAL_CONFIG.md
- **FR-3.3**: Add verification steps for each tool
- **FR-3.4**: Keep under ~150 lines

### FR-4: TUTORIAL.md Rewrite

- **FR-4.1**: Focus on concepts and "why" for humans
- **FR-4.2**: Include workflow walkthrough with practical examples
- **FR-4.3**: Merge handoff explanation from handoff-system.md
- **FR-4.4**: Add common scenarios section
- **FR-4.5**: Keep under ~500 lines

### FR-5: REFERENCE.md (New File)

- **FR-5.1**: Commands table with all /conductor-* commands
- **FR-5.2**: Triggers table (ds, fb, rb, tdd, etc.)
- **FR-5.3**: Skill reference table (6 skills, descriptions, triggers)
- **FR-5.4**: Troubleshooting quick reference
- **FR-5.5**: Keep under ~300 lines

### FR-6: AGENTS.md Rewrite

- **FR-6.1**: Focus on agent-specific instructions only
- **FR-6.2**: Include decision trees for workflow routing
- **FR-6.3**: Session protocol and rules
- **FR-6.4**: Keep under ~200 lines

### FR-7: docs/ARCHITECTURE.md

- **FR-7.1**: Consolidate all Mermaid diagrams from README
- **FR-7.2**: Pipeline flow documentation
- **FR-7.3**: Keep under ~300 lines

## Non-Functional Requirements

### NFR-1: Accuracy
- All references verified against current codebase
- No mentions of removed skills or deprecated features
- All file paths and commands tested

### NFR-2: Scannability
- Use tables over prose where possible
- Bullet points for lists
- Clear section headings
- Strict line limits per file

### NFR-3: Audience Separation
- Human docs: conceptual, "why" focused
- Agent docs: precise triggers, "how" focused

### NFR-4: Maintainability
- Single source of truth (no duplication)
- Clear ownership per file

## Acceptance Criteria

- [ ] All 6 main doc files exist with correct content
- [ ] Line counts within specified limits
- [ ] No broken internal links
- [ ] No references to removed skills (file-beads, review-beads, etc.)
- [ ] All commands/triggers verified working
- [ ] README renders correctly on GitHub

## Out of Scope

- SKILL.md files within skills/ directories
- Code comments and inline documentation
- CHANGELOG.md updates
- Demo/ directory documentation
