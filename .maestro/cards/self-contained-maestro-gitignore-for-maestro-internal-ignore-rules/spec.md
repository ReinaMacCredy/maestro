# Self-contained .maestro/.gitignore for maestro-internal ignore rules

## Current state

Rules are written as ONE managed block in the repo-root .gitignore. Body: gitignore_block() at src/domain/install/mirrors.rs:972. Declared as a mirror in mirror_plan() at mirrors.rs:139-143 (path '.gitignore', MirrorKind::GitignoreSection). Rendered via upsert_managed_block(existing, HashComment, gitignore_block()) in contents_for_existing() at mirrors.rs:458-462. Markers '# >>> maestro >>>' / '# <<< maestro <<<' from ManagedBlockFormat::HashComment in src/foundation/core/managed_blocks.rs:22; upsert/remove at managed_blocks.rs:28/56.

Block body covers two zones: (a) maestro-INTERNAL local-only paths (.maestro/runs/, channels/, backups/, index/, install-lock.yaml, tasks/*/evidence/, tasks/*/local/, archive/**/evidence/, archive/**/local/, archive/**/runs/) and (b) two paths OUTSIDE .maestro/: .claude/settings.local.json and .codex/hooks.json. The (b) paths are agent-settings files maestro itself manages via hook_config_plan; they cannot move into a .maestro/.gitignore because git only applies a .gitignore to its own subtree.

Gap found while mapping: .maestro/update-check is created by maestro (update-check cache) but is NOT in gitignore_block(). This repo only ignores it via a MANUAL user-managed root entry ('.maestro/update-check'); a fresh install elsewhere would leave it untracked-but-uncovered (committable). A .maestro/.gitignore is the natural home to fold it in as 'update-check'.

## Problem

## Impact of dropping the two agent paths

Enumerated consumers of the removal (decision dec-gitignore-scope-maestro-scope-only-2fcf): (1) .claude/settings.local.json -- written by ManagedHookConfig::for_agent(Claude) (hooks.rs:17-21) merging only the 'hooks' key; the FILE also holds the user's local Claude perms. Loses maestro auto-ignore -> user/Claude own it. (2) .codex/hooks.json -- written by for_agent(Codex) (hooks.rs:22-26), maestro-generated, portable. Loses maestro auto-ignore. (3) remove_managed_block (managed_blocks.rs:56) -- reused for the root-block migration/strip and uninstall. (4) Existing installs -- carry the legacy root block; new sync must strip it. No secret leak: settings.local.json holds local perms, not credentials.
