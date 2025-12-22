# Conductor Refresh Workflow

This document defines the refresh workflow for syncing Conductor context documentation with the current codebase state.

## Overview

Use `/conductor-refresh` when context documentation has become stale due to:
- Codebase evolution (new files, directories, modules)
- Dependency changes (new packages, version updates)
- Shipped features (completed tracks not reflected in product.md)
- Process changes (new CI/CD, tooling updates)

## Refresh Scopes

| Scope | Target | Analysis |
|-------|--------|----------|
| `all` | All context docs | Full codebase comparison |
| `tech` | `tech-stack.md` | Dependency files, new frameworks |
| `product` | `product.md` | Completed tracks, README changes |
| `workflow` | `workflow.md` | CI/CD configs, tooling changes |
| `track [id]` | Specific track | Spec/plan vs implementation |

## Analysis Details

### Tech Stack Analysis

Scan these files for changes:
- `package.json` (Node.js)
- `requirements.txt` / `pyproject.toml` (Python)
- `go.mod` (Go)
- `Cargo.toml` (Rust)
- `pom.xml` / `build.gradle` (Java)

Detect:
- **Added:** New dependencies not in tech-stack.md
- **Removed:** Dependencies in docs but not in codebase
- **Updated:** Major version changes

### Product Analysis

Compare:
- Completed tracks `[x]` in `tracks.md` vs features in `product.md`
- Current README.md vs documented product description
- New directories/modules vs documented architecture

Detect:
- **Shipped features:** Completed tracks not in product.md
- **New components:** Undocumented modules
- **Scope drift:** Divergence from original vision

### Workflow Analysis

Check:
- `.github/workflows/` for CI/CD changes
- Linting configs (`.eslintrc`, `pyproject.toml`, etc.)
- Testing setup (test directories, config files)
- Commit history for convention changes

Detect:
- **New tools:** Added CI/CD, linters, formatters
- **Process changes:** Coverage requirements, review policies

## Staleness Detection

Triggered by `/conductor-validate` or proactively on session start:

1. **Age check:** `setup_state.json` > 2 days old
2. **Refresh state:** `next_refresh_hint` in past
3. **Dependency drift:** Dependency files modified after `tech-stack.md`
4. **Track completion:** > 3 completed tracks since last refresh

## State Files

### refresh_state.json

```json
{
  "last_refresh": "2024-12-22T10:00:00Z",
  "scope": "all",
  "changes_applied": [
    {"file": "tech-stack.md", "changes": ["added React 19", "removed webpack"]},
    {"file": "product.md", "changes": ["added auth feature"]}
  ],
  "next_refresh_hint": "2024-12-24T10:00:00Z"
}
```

## Workflow Steps

### 1. Pre-Refresh Validation

```
Check:
- [ ] conductor/ directory exists
- [ ] Core files present (product.md, tech-stack.md, workflow.md, tracks.md)
- [ ] Git working directory clean (warn if dirty)
```

### 2. Scope Selection

If no argument provided, prompt:
```
What would you like to refresh?
1. all - Full refresh
2. tech - Dependencies only
3. product - Product vision only
4. workflow - Process only
5. track [id] - Specific track
```

### 3. Analysis Phase

For selected scope(s):
- Read current documentation
- Scan codebase for current state
- Compare and identify drift

### 4. Drift Report

Present findings with categories:
- **Critical:** Breaking changes, major drift
- **Recommended:** Important updates
- **Optional:** Minor updates

### 5. Confirmation

```
Apply updates?
1. All recommended updates
2. Select specific updates
3. Cancel
```

### 6. Backup & Update

For each file to update:
1. Create backup: `<file>.md.bak`
2. Apply changes
3. Add refresh marker

### 7. State Update

Update `refresh_state.json` with:
- Timestamp
- Scope
- Changes applied
- Next refresh hint (2 days)

### 8. Commit

```bash
git add conductor/
git commit -m "conductor(refresh): Sync context with codebase

Scope: [scope]
- [changes summary]"
```

## Error Handling

| Scenario | Action |
|----------|--------|
| Setup incomplete | Halt, suggest `/conductor-setup` |
| Git dirty | Warn, offer to stash or abort |
| No changes detected | Report "Context is current" |
| Backup failed | Abort, report error |

## Integration with Validate

The `/conductor-validate` command includes staleness checks:

```markdown
### Staleness Warnings
- Context docs: [N days since refresh]
- Dependency drift: [files modified after tech-stack.md]
- Shipped features: [N completed tracks since refresh]

Recommendation: Run `/conductor-refresh` to sync context
```
