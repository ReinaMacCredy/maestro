mod support;

#[cfg(unix)]
mod unix {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use maestro::domain::install::{
        AgentInstall, FileOwnership, InstallAgent, InstallLock, MirrorKind,
    };

    use super::support::TestTempDir;

    fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_maestro"))
            .args(args)
            .current_dir(cwd)
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

    fn assert_expected_symlink(repo: &Path, relative_path: &str) {
        let path = repo.join(relative_path);
        let metadata = fs::symlink_metadata(&path).expect("invariant: skill mirror should exist");
        assert!(metadata.file_type().is_symlink());
        let target = fs::read_link(path).expect("invariant: skill mirror should be readable");
        assert_eq!(target, Path::new("../.maestro/skills"));
    }

    #[test]
    fn install_creates_claude_and_codex_skill_symlinks_and_lock_records() {
        for (agent, relative_path) in [("claude", ".claude/skills"), ("codex", ".codex/skills")] {
            let temp_dir = TestTempDir::new("maestro-skills-symlink-test");
            init_repo(temp_dir.path());

            let output = maestro(&["install", "--agent", agent], temp_dir.path());

            assert!(
                output.status.success(),
                "stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_expected_symlink(temp_dir.path(), relative_path);

            let lock = InstallLock::load(&temp_dir.path().join(".maestro/install-lock.yaml"))
                .expect("invariant: install lock should load");
            let ownership = lock.agents[agent]
                .files
                .get(relative_path)
                .expect("invariant: skill symlink ownership should be recorded");
            assert!(matches!(ownership.kind, MirrorKind::Symlink));
            assert_eq!(ownership.target.as_deref(), Some("../.maestro/skills"));
        }
    }

    #[test]
    fn install_refuses_to_overwrite_user_managed_skill_destinations() {
        for (agent, relative_path, make_dir) in [
            ("claude", ".claude/skills", true),
            ("codex", ".codex/skills", false),
        ] {
            let temp_dir = TestTempDir::new("maestro-skills-symlink-test");
            init_repo(temp_dir.path());
            let destination = temp_dir.path().join(relative_path);
            fs::create_dir_all(
                destination
                    .parent()
                    .expect("invariant: skill path should have parent"),
            )
            .expect("invariant: skill parent should be creatable");
            if make_dir {
                fs::create_dir(&destination)
                    .expect("invariant: user skill directory should be creatable");
            } else {
                fs::write(&destination, "user skills\n")
                    .expect("invariant: user skill file should be writable");
            }

            let output = maestro(&["install", "--agent", agent], temp_dir.path());

            assert!(!output.status.success());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("refusing to overwrite existing"));
            assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
            if make_dir {
                assert!(destination.is_dir());
            } else {
                assert_eq!(
                    fs::read_to_string(destination)
                        .expect("invariant: user skill file should remain readable"),
                    "user skills\n"
                );
            }
        }
    }

    #[test]
    fn install_refuses_symlinked_canonical_skills_tree() {
        let temp_dir = TestTempDir::new("maestro-skills-symlink-test");
        let external = TestTempDir::new("maestro-skills-symlink-external");
        init_repo(temp_dir.path());
        fs::create_dir_all(temp_dir.path().join(".maestro"))
            .expect("invariant: maestro dir should be creatable");
        std::os::unix::fs::symlink(external.path(), temp_dir.path().join(".maestro/skills"))
            .expect("invariant: symlinked canonical skills dir should be creatable");

        let output = maestro(&["install", "--agent", "claude"], temp_dir.path());

        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
        assert!(!temp_dir.path().join(".claude/skills").exists());
    }

    #[test]
    fn uninstall_removes_owned_expected_symlink() {
        let temp_dir = TestTempDir::new("maestro-skills-symlink-test");
        init_repo(temp_dir.path());
        let install = maestro(&["install", "--agent", "claude"], temp_dir.path());
        assert!(install.status.success());

        let uninstall = maestro(&["uninstall", "--agent", "claude"], temp_dir.path());

        assert!(
            uninstall.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&uninstall.stderr)
        );
        assert!(fs::symlink_metadata(temp_dir.path().join(".claude/skills")).is_err());
        assert!(!temp_dir.path().join(".maestro/install-lock.yaml").exists());
    }

    #[test]
    fn uninstall_preserves_changed_symlink_and_user_managed_tree() {
        let temp_dir = TestTempDir::new("maestro-skills-symlink-test");
        init_repo(temp_dir.path());
        let install = maestro(&["install", "--agent", "codex"], temp_dir.path());
        assert!(install.status.success());
        fs::remove_file(temp_dir.path().join(".codex/skills"))
            .expect("invariant: installed skill symlink should be removable");
        fs::create_dir_all(temp_dir.path().join("user-skills"))
            .expect("invariant: changed skill target should be creatable");
        std::os::unix::fs::symlink("../user-skills", temp_dir.path().join(".codex/skills"))
            .expect("invariant: changed skill symlink should be creatable");

        let uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

        assert!(
            uninstall.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&uninstall.stderr)
        );
        let changed_target = fs::read_link(temp_dir.path().join(".codex/skills"))
            .expect("invariant: changed symlink should remain");
        assert_eq!(changed_target, Path::new("../user-skills"));

        fs::remove_file(temp_dir.path().join(".codex/skills"))
            .expect("invariant: changed symlink should be removable");
        fs::create_dir(temp_dir.path().join(".codex/skills"))
            .expect("invariant: user skill tree should be creatable");
        fs::write(temp_dir.path().join(".codex/skills/SKILL.md"), "user\n")
            .expect("invariant: user skill file should be writable");
        let mut lock = InstallLock::empty();
        let previous_lock = InstallLock::load(&temp_dir.path().join(".maestro/install-lock.yaml"))
            .unwrap_or_else(|_| InstallLock::empty());
        if let Some(install) = previous_lock.agents.get("codex").cloned() {
            lock.set_agent(InstallAgent::Codex, install);
        } else {
            let mut install = AgentInstall::new("test".to_string());
            install.insert(
                ".codex/skills",
                FileOwnership::symlink("../.maestro/skills"),
            );
            lock.set_agent(InstallAgent::Codex, install);
        }
        lock.save(&temp_dir.path().join(".maestro/install-lock.yaml"))
            .expect("invariant: lock should be writable");

        let second_uninstall = maestro(&["uninstall", "--agent", "codex"], temp_dir.path());

        assert!(
            second_uninstall.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&second_uninstall.stderr)
        );
        assert!(temp_dir.path().join(".codex/skills/SKILL.md").is_file());
    }
}
