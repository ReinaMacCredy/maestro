# Safety Rules

## File Deletion

Never delete files without explicit user permission in this session.

This includes:
- Files you just created (tests, temp files, scripts)
- Empty directories
- Backup files

If you think something should be removed, stop and ask.

## Destructive Commands

Forbidden without explicit approval in the same message:
- `git reset --hard`
- `git clean -fd`
- `rm -rf`
- Any command that deletes or overwrites code/data

Before running destructive commands:
1. If unsure what it will delete, ask first
2. Prefer safe alternatives: `git status`, `git diff`, `git stash`
3. After approval, restate the command and list what it affects

## Code Modifications

Do not run scripts that bulk-modify code:
- Codemods
- One-off transformation scripts
- Giant sed/regex refactors

For large changes:
- Break into smaller, explicit edits
- Review diffs before proceeding
- Edit file-by-file with careful reasoning
