# Design: GitHub Actions for Changelog CI/CD

**Status:** Approved  
**Date:** 2024-12-23  
**Author:** Reina MacCredy (via ds session)

## Problem Statement

Solo developer của maestro plugin cần automated changelog + versioning CI/CD để giảm manual work, đảm bảo version consistency giữa `plugin.json` và `marketplace.json`, trong khi vẫn giữ skill-level versions manual.

## Solution Overview

- **Tool:** git-cliff + custom GitHub Actions
- **Trigger:** Push to main (feat/fix commits)
- **Versioning:** SemVer auto-bump
- **Scope:** Plugin-level only (skill versions remain manual)

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    GitHub Actions                        │
├─────────────────────────────────────────────────────────┤
│  PR Opened/Updated          │  Push to main             │
│  ┌─────────────────┐        │  ┌─────────────────────┐  │
│  │ validate.yml    │        │  │ release.yml         │  │
│  │ - version sync  │        │  │ - git-cliff         │  │
│  │ - changelog     │        │  │ - version bump      │  │
│  │   preview       │        │  │ - commit + tag      │  │
│  └─────────────────┘        │  │ - GitHub Release    │  │
│                             │  └─────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Version Bump Rules

| Commit Type | Version Bump | Example |
|-------------|--------------|---------|
| `feat:` | Minor (1.5.0 → 1.6.0) | New feature |
| `fix:` | Patch (1.5.0 → 1.5.1) | Bug fix |
| `feat!:` or `BREAKING CHANGE:` | Major (1.5.0 → 2.0.0) | Breaking change |
| `docs:`, `refactor:` | None (changelog only) | Documentation |
| `chore:` | Skip entirely | Maintenance |

## Files to Create

### 1. `.github/workflows/release.yml`

Main release workflow:
- Runs on push to main
- Determines version bump type from commits
- Updates `plugin.json` and `marketplace.json`
- Generates CHANGELOG.md via git-cliff
- Creates git tag and GitHub Release
- Uses `[skip ci]` to prevent infinite loops

### 2. `.github/workflows/validate.yml`

PR validation:
- Checks version sync between plugin.json and marketplace.json
- Shows changelog preview in PR summary

### 3. `cliff.toml`

git-cliff configuration:
- Keep a Changelog format
- Conventional commit parsing
- Mapping: feat→Added, fix→Fixed, refactor→Changed, docs→Documentation

## Files to Update

### `AGENTS.md`

Update versioning section to reflect automation:
- Plugin version auto-bumped by CI
- Skill versions remain manual

## Security Considerations

- Use default `GITHUB_TOKEN` (no PAT needed)
- Pin action versions to SHA
- Pin git-cliff version
- Concurrency control to prevent race conditions
- `permissions: contents: write` explicitly set

## Edge Cases Handled

| Case | Solution |
|------|----------|
| No existing tags | Fallback to v0.0.0 |
| Concurrent merges | Concurrency group |
| Bot commit loops | `[skip ci]` marker |
| marketplace.json structure | `.plugins[0].version` path |

## Initial Setup Required

```bash
# One-time: Create initial tag matching current version
git tag v1.5.0
git push origin v1.5.0
```

## Escape Hatches

- `[skip ci]` in commit message skips all automation
- `docs:` / `chore:` commits don't bump version

## Success Criteria

- [ ] Changelog auto-generated on merge to main
- [ ] Version sync maintained (plugin.json == marketplace.json)
- [ ] SemVer auto-bump working (feat→minor, fix→patch)
- [ ] BREAKING CHANGE detection working
- [ ] GitHub Release created automatically
- [ ] PR validation showing changelog preview

## Party Mode Insights

Key feedback from design session:

1. **Separation of concerns:** Plugin versions (auto) vs skill versions (manual)
2. **Trigger refinement:** Skip bumps for docs/chore commits
3. **Security:** marketplace.json uses `.plugins[0].version` path
4. **Documentation:** Need setup instructions for initial tag
