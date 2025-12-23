# Spec: GitHub Actions for Changelog CI/CD

**Track ID:** changelog-cicd  
**Type:** feature  
**Status:** draft  
**Created:** 2024-12-23

## Overview

Implement automated changelog generation and versioning for the Maestro plugin using git-cliff and GitHub Actions.

## Requirements

### Functional Requirements

1. **FR-1: Changelog Generation**
   - Generate CHANGELOG.md from conventional commits
   - Use Keep a Changelog format (Added, Fixed, Changed, Documentation)
   - Run automatically on push to main

2. **FR-2: Version Auto-Bump**
   - Detect commit types and bump version accordingly:
     - `feat:` → minor bump
     - `fix:` → patch bump
     - `feat!:` or `BREAKING CHANGE:` → major bump
   - Skip bump for `docs:`, `chore:` commits

3. **FR-3: Multi-File Version Sync**
   - Update `.claude-plugin/plugin.json` (`.version` field)
   - Update `.claude-plugin/marketplace.json` (`.plugins[0].version` field)
   - Both files must always have matching versions

4. **FR-4: GitHub Release**
   - Create GitHub Release with new tag
   - Include changelog content in release body

5. **FR-5: PR Validation**
   - Check version sync between JSON files
   - Show changelog preview in PR summary

### Non-Functional Requirements

1. **NFR-1: No Infinite Loops**
   - Bot commits include `[skip ci]` marker
   - Workflow has concurrency control

2. **NFR-2: Security**
   - Use default GITHUB_TOKEN
   - Pin action versions to SHA
   - Explicit permissions block

3. **NFR-3: First-Run Handling**
   - Handle case when no tags exist (fallback to v0.0.0)

## Acceptance Criteria

- [ ] AC-1: Push `feat: add feature` → CHANGELOG updated, minor version bump, release created
- [ ] AC-2: Push `fix: fix bug` → CHANGELOG updated, patch version bump, release created
- [ ] AC-3: Push `docs: update readme` → CHANGELOG updated, NO version bump
- [ ] AC-4: Push `chore: cleanup` → No changelog entry, no version bump
- [ ] AC-5: PR with version mismatch → Validation fails
- [ ] AC-6: PR shows changelog preview in summary
- [ ] AC-7: `plugin.json` and `marketplace.json` versions always match after release

## Out of Scope

- Skill-level versioning (remains manual)
- npm/package publishing
- Pre-release version handling
- Commitlint enforcement (future phase)

## Dependencies

- git-cliff CLI tool
- GitHub Actions runner
- jq for JSON manipulation

## Risks

| Risk | Mitigation |
|------|------------|
| First run with no tags | Fallback logic implemented |
| Concurrent merges | Concurrency group prevents race |
| Bot commits trigger loops | `[skip ci]` in commit message |
