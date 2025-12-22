---
description: Sync context docs with current codebase state
---

# Conductor Refresh

Sync conductor context documentation with the current codebase state.

## 1. Verify Setup

Check conductor/ exists with core files. If not, suggest `/conductor-setup`.

## 2. Determine Scope

If no argument, ask:
- `all` - Full refresh
- `tech` - tech-stack.md only
- `product` - product.md only
- `workflow` - workflow.md only
- `track [id]` - Specific track

## 3. Analyze Drift

Compare current codebase against docs:
- **Tech:** Scan package.json, requirements.txt, etc. for new/removed deps
- **Product:** Check completed tracks not reflected in product.md
- **Workflow:** Check CI/CD changes, new tooling

## 4. Present Drift Report

Show what's changed since last setup/refresh.

## 5. Confirm Updates

Ask user to approve changes.

## 6. Apply Updates

- Create backups (*.md.bak)
- Update files with new information
- Add refresh marker to top of files

## 7. Update State

Create/update `conductor/refresh_state.json` with timestamp and changes.

## 8. Commit

```bash
git add conductor/
git commit -m "conductor(refresh): Sync context with codebase"
```
