# Skills global-only: remove per-repo extraction and linking

## Current state

Per-repo skill generation: extract_all (src/domain/extraction/mod.rs:51) calls extract_skills, run by maestro init (operations/init/mod.rs:94), maestro update (operations/update/mod.rs:476), maestro sync (operations/sync/mod.rs:86). This writes .maestro/skills/<4 skills> into the repo.

Per-repo symlinks: skill_symlink_for_agent (src/domain/install/mod.rs:203-214) wires .claude/skills (claude) and .codex/skills (codex) as folder symlinks -> ../.maestro/skills; created via src/domain/skills/symlink.rs, recorded as MirrorKind::Symlink in install-lock.

Global installer (src/domain/skills/global.rs:19-28) already manages BOTH agent roots from the ~/.maestro/skills cache: SUPPORTED_ROOTS = [codex -> ~/.agents/skills, claude -> ~/.claude/skills]. Verified on disk: ~/.agents/skills/maestro-card and ~/.claude/skills/maestro-card both symlink to ~/.maestro/skills/maestro-card. So both agents already discover maestro skills globally; per-repo copies are redundant. No Codex gap.

Live drift confirmed: embedded/skills/maestro-card/SKILL.md is version 1.9.0 (build source) but ~/.maestro/skills cache (read by both agents) is 1.8.0, because release-local copies the binary without running install/upgrade, so the locked auto-resync (dec-global-skill-cache-auto-resync-on-0e62) never fires. embedded vs .maestro/skills also differ on maestro-audit/reference/cli.md and maestro-setup/reference/cli.md.

## Problem

