# Skill-version drift detection: surface sync when on-disk skills lag the binary

## Current state

Hook point: src/main.rs:32 run_auto_check() runs after each command (gated by should_auto_check_after; exclusion list at main.rs:44). The passive after-command check already exists but today only nags about a newer GitHub release; it never touches the skill cache.

Curl-gate gap (verified): run_auto_check (update.rs) bails when detect_install_method != Curl and calls run_update with global_skills_home: None + check_only: true, so a side-loaded or dev-rebuilt binary (cp target/release/maestro ~/.local/bin) gets zero skill-cache signal. A real cache resync happens only inside a full maestro upgrade/update, where global_skills_home is Some.

Drift primitive exists: src/domain/skills/global.rs global_skills_status_at (line 290) compares each cached skill version vs the running binary's embedded version by EQUALITY (installed == embedded); maestro sync --global-skills resyncs. Reuse, do not reinvent.

Lock: ~/.maestro/skills-lock.yaml (maestro.global_skills_lock.v1) records each cached skill version + per-file sha256. Drift = cache version != binary embedded version; offline + cheap (version compare, no hashing for the nudge).

Prior art: dec-global-skill-cache-auto-resync-on-0e62 (locked, parent shipped feature skill-time-cli-discovery) chose auto-resync on install/upgrade. Confirmed correct; the gap is the out-of-band binary advance (dev rebuild/side-load) that bypasses that trigger. doctor never compares cache to embedded versions.

Scope (narrowed): the global cache only (~/.maestro/skills), the sole copy both Claude (~/.claude/skills) and Codex (~/.agents/skills) load. The earlier 'cover the per-repo .maestro/skills too' scope is dead: sibling feature skills-global-only-remove-per-repo-extraction-and-linking (decision dec-skills-are-global-only, locked) removes the per-repo copy entirely, so there is no project-scope copy to track.

Two copies after the sibling card lands: embedded/skills (baked into the binary by build.rs = authoritative source / freshness oracle) and ~/.maestro/skills (the global cache agents load). Drift = cache version != binary embedded version; offline, version-compare.

Boundary: sibling feature skills-global-only-... owns the storage/linking architecture (removing per-repo extraction + linking). Its non-goal 'release-local auto-resync trigger -> sibling feature' hands the out-of-band resync trigger to THIS card. This card owns drift DETECTION + the resync/nudge trigger only; it does not decide storage or linking.

Locked behavior: on drift, auto-resync the cache inline (dec-drift-response-auto-resync-the-cache-4dac), once per drift event. Equality compare, cache mirrors binary (dec-cache-ahead-equality-compare-cache-a597): drift = cache version != binary embedded version (reuse global_skills_status_at); on ANY mismatch the cache is resynced to the running binary, including downgrade when the binary is older -- the cache is a pure projection of whatever maestro you ran. This reverses the earlier 'silent when ahead' lean (which assumed a monotonic cache); alternating binaries churn, accepted as rare.

## Problem

The global skill cache (~/.maestro/skills) is the single copy both Claude and Codex load, but it silently lags the binary whenever the binary advances out-of-band -- a dev rebuild + cp to ~/.local/bin, or any side-load -- instead of through maestro install/update. The shipped auto-resync (dec-global-skill-cache-auto-resync-on-0e62) fires only on install/upgrade, and the passive after-command check is curl-only and never touches the cache, so this path produces no signal at all. Agents then load stale skills (e.g. maestro-design 1.10.0 while embedded is 1.12.0) and never see the new instructions, with nothing surfacing the lag.
