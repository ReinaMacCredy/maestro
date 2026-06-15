## Task T1: Repoint the .gitignore mirror to .maestro/.gitignore
check: maestro init writes .maestro/.gitignore listing the maestro-internal local paths relative to .maestro/ (runs/, channels/, backups/, index/, install-lock.yaml, update-check, tasks/*/evidence/, tasks/*/local/, archive/**/evidence/, archive/**/local/, archive/**/runs/); the repo-root .gitignore contains no maestro managed block; git check-ignore reports .maestro/update-check as ignored; .claude/settings.local.json and .codex/hooks.json are still written by hook_config_plan but appear in no gitignore maestro writes

## Task T2: Strip the legacy root block on sync + clean up on uninstall
after: T1
check: running the new binary's maestro sync on a repo whose root .gitignore still carries the old maestro managed block removes that block while leaving user-managed root lines outside the markers untouched; maestro uninstall removes .maestro/.gitignore (with .maestro/) and leaves no orphaned maestro lines/markers in the root .gitignore
