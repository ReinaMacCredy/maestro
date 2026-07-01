mod support;

#[cfg(unix)]
mod unix {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use maestro::domain::install::{
        AgentInstall, FileOwnership, InstallAgent, InstallLock, MirrorKind,
    };

    use super::support::TestTempDir;

    fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_maestro"))
            .args(args)
            .current_dir(cwd)
            .env("HOME", cwd.join("home"))
            .env("MAESTRO_INSTALL_METHOD", "local")
            .output()
            .expect("invariant: compiled maestro binary should be runnable in symlink tests")
    }

    fn init_repo(repo: &Path) {
        fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
        fs::create_dir_all(repo.join(".maestro/harness"))
            .expect("invariant: harness dir should be creatable");
        fs::write(
            repo.join(".maestro/harness/HARNESS.md"),
            "# Maestro Harness Protocol\n",
        )
        .expect("invariant: harness protocol should be writable");
        fs::create_dir_all(repo.join(".maestro/hooks"))
            .expect("invariant: hooks dir should be creatable");
        fs::write(
            repo.join(".maestro/hooks/record.sh"),
            "# maestro:hook-version: 1.0.0\nexec maestro hook record\n",
        )
        .expect("invariant: hook recorder script should be writable");
    }

    fn lock_path(repo: &Path) -> PathBuf {
        repo.join(".maestro/install-lock.yaml")
    }

    /// Seed an install lock that records a legacy per-repo skills symlink for
    /// `agent`, the way an older maestro version wrote it, and materialize the
    /// symlink on disk pointing at the now-removed `../.maestro/skills`.
    fn seed_legacy_symlink(repo: &Path, agent: InstallAgent, relative_path: &str) {
        let link = repo.join(relative_path);
        fs::create_dir_all(link.parent().expect("invariant: symlink path has a parent"))
            .expect("invariant: agent dir should be creatable");
        symlink("../.maestro/skills", &link)
            .expect("invariant: legacy skills symlink should be creatable");
        let mut lock = InstallLock::empty();
        let mut install = AgentInstall::new("legacy".to_string());
        install.insert(relative_path, FileOwnership::symlink("../.maestro/skills"));
        lock.set_agent(agent, install);
        lock.save(&lock_path(repo))
            .expect("invariant: synthetic lock should be writable");
    }

    fn no_symlink_entry(lock: &InstallLock, agent_key: &str) -> bool {
        lock.agents[agent_key]
            .files
            .values()
            .all(|ownership| ownership.kind != MirrorKind::Symlink)
    }

    /// ac-2: skills are global-only, so install creates no per-repo skills
    /// symlink and records no Symlink-kind entry in the install lock.
    #[test]
    fn install_creates_no_skill_symlink_and_records_no_symlink_lock_entry() {
        for (agent, relative_path) in [("claude", ".claude/skills"), ("codex", ".codex/skills")] {
            let temp_dir = TestTempDir::new("maestro-skills-symlink-test");
            init_repo(temp_dir.path());

            let output = maestro(&["install", "--agent", agent], temp_dir.path());
            assert!(
                output.status.success(),
                "stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );

            assert!(
                fs::symlink_metadata(temp_dir.path().join(relative_path)).is_err(),
                "install must not create a per-repo {relative_path} symlink"
            );
            let lock = InstallLock::load(&lock_path(temp_dir.path()))
                .expect("invariant: install lock should load");
            assert!(
                no_symlink_entry(&lock, agent),
                "install lock must record no Symlink-kind entry"
            );
            assert!(
                !lock.agents[agent].files.contains_key(relative_path),
                "install lock must not record {relative_path}"
            );
        }
    }

    /// ac-3 (install entry point): a lock that still records a legacy per-repo
    /// skills symlink has it pruned on install -- the symlink is removed, its lock
    /// entry dropped, and `.maestro/skills` is not re-created.
    #[test]
    fn install_prunes_a_legacy_skill_symlink_recorded_in_the_lock() {
        for (agent_key, agent, relative_path) in [
            ("claude", InstallAgent::Claude, ".claude/skills"),
            ("codex", InstallAgent::Codex, ".codex/skills"),
        ] {
            let temp_dir = TestTempDir::new("maestro-skills-symlink-test");
            init_repo(temp_dir.path());
            seed_legacy_symlink(temp_dir.path(), agent, relative_path);

            let output = maestro(&["install", "--agent", agent_key], temp_dir.path());
            assert!(
                output.status.success(),
                "stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );

            assert!(
                fs::symlink_metadata(temp_dir.path().join(relative_path)).is_err(),
                "install must prune the legacy {relative_path} symlink"
            );
            let lock = InstallLock::load(&lock_path(temp_dir.path()))
                .expect("invariant: install lock should load");
            assert!(
                no_symlink_entry(&lock, agent_key),
                "the legacy Symlink lock entry must be dropped"
            );
            assert!(
                !temp_dir.path().join(".maestro/skills").exists(),
                "install must not re-create .maestro/skills"
            );
        }
    }

    /// ac-3 (update entry point): `maestro upgrade` prunes the same legacy symlink.
    /// `update` never rewrites the per-agent lock, so this exercises the
    /// standalone migration the upgrade flow calls, not the install rewrite.
    #[test]
    fn upgrade_prunes_a_legacy_skill_symlink_recorded_in_the_lock() {
        let temp_dir = TestTempDir::new("maestro-skills-symlink-test");
        init_repo(temp_dir.path());
        seed_legacy_symlink(temp_dir.path(), InstallAgent::Claude, ".claude/skills");

        let output = maestro(&["upgrade"], temp_dir.path());
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(
            fs::symlink_metadata(temp_dir.path().join(".claude/skills")).is_err(),
            "upgrade must prune the legacy .claude/skills symlink"
        );
        let lock = InstallLock::load(&lock_path(temp_dir.path()))
            .expect("invariant: install lock should load");
        assert!(
            no_symlink_entry(&lock, "claude"),
            "the legacy Symlink lock entry must be dropped on upgrade"
        );
        assert!(
            !temp_dir.path().join(".maestro/skills").exists(),
            "upgrade must not re-create .maestro/skills"
        );
    }

    /// Migration ownership safety: the lock records a maestro symlink, but on disk
    /// the user repointed it elsewhere. Ownership no longer matches, so migration
    /// drops the stale lock entry but leaves the user's symlink in place.
    #[test]
    fn migration_preserves_a_user_repointed_symlink() {
        let temp_dir = TestTempDir::new("maestro-skills-symlink-test");
        init_repo(temp_dir.path());
        fs::create_dir_all(temp_dir.path().join("user-skills"))
            .expect("invariant: user skill target should be creatable");
        fs::create_dir_all(temp_dir.path().join(".codex"))
            .expect("invariant: codex dir should be creatable");
        symlink("../user-skills", temp_dir.path().join(".codex/skills"))
            .expect("invariant: user symlink should be creatable");
        let mut lock = InstallLock::empty();
        let mut install = AgentInstall::new("legacy".to_string());
        install.insert(
            ".codex/skills",
            FileOwnership::symlink("../.maestro/skills"),
        );
        lock.set_agent(InstallAgent::Codex, install);
        lock.save(&lock_path(temp_dir.path()))
            .expect("invariant: synthetic lock should be writable");

        let output = maestro(&["install", "--agent", "codex"], temp_dir.path());
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let target = fs::read_link(temp_dir.path().join(".codex/skills"))
            .expect("invariant: user-repointed symlink should remain");
        assert_eq!(target, Path::new("../user-skills"));
    }
}
