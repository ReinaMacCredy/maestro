//! WS5 shared fixture harness (D6.5 condition 3): one parameterized surface
//! driving every artifact family's embedded schema-pack fixtures through the
//! REAL read entry points, proving reads never rewrite bytes (matrix item 4)
//! and that the create verbs stamp the current version (item 5).
//!
//! Families without an on-disk read or CLI writer are covered elsewhere and
//! documented inline: the proof report has no production reader (it is built
//! in memory at `task verify`; the W2 pack validator parses its fixture), and
//! the run-event / run-evidence / install writers are exercised end-to-end by
//! `hook_record_integration`, `run_evidence_integration`, and
//! `install_uninstall_integration` respectively.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use maestro::domain::schema_contracts::pack;
use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AUTO_UPDATE", "0")
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn assert_success(output: &std::process::Output, args: &[&str]) -> String {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The embedded fixture bytes for `family`/`fixtures/<name>`.
fn fixture(family: &str, name: &str) -> &'static [u8] {
    let pack = pack(family).unwrap_or_else(|| panic!("schema pack {family} ships"));
    let relative = format!("fixtures/{name}");
    pack.fixtures
        .iter()
        .find(|fixture| fixture.relative_path == relative)
        .unwrap_or_else(|| panic!("{family} ships {relative}"))
        .contents
}

/// Seed one card-dir fixture (a full card file) at `.maestro/cards/<id>/<file>`
/// and return the written path.
fn seed_card_dir(repo: &Path, family: &str, name: &str, id: &str, file: &str) -> PathBuf {
    let dir = repo.join(".maestro/cards").join(id);
    fs::create_dir_all(&dir).expect("invariant: card dir should be creatable");
    let path = dir.join(file);
    fs::write(&path, fixture(family, name)).expect("invariant: fixture should be writable");
    path
}

/// Seed an entry roster (`decisions.yaml`/`ideas.yaml`) from card fixtures and
/// return the written path.
fn seed_entries(repo: &Path, file: &str, fixtures: &[(&str, &str)]) -> PathBuf {
    let entries: Vec<serde_yaml::Value> = fixtures
        .iter()
        .map(|(family, name)| {
            serde_yaml::from_slice(fixture(family, name))
                .expect("invariant: a card fixture parses as YAML")
        })
        .collect();
    let path = repo.join(".maestro/cards").join(file);
    fs::create_dir_all(path.parent().expect("cards dir")).expect("cards dir should be creatable");
    fs::write(
        &path,
        serde_yaml::to_string(&entries).expect("invariant: entries serialize"),
    )
    .expect("invariant: roster should be writable");
    path
}

#[test]
fn run_event_fixture_identity_is_runtime_neutral() {
    let bytes = fixture("run-event", "current-events.jsonl");
    let text = std::str::from_utf8(bytes).expect("run-event fixture is UTF-8 JSONL");
    let mut saw_runtime = false;

    for (index, line) in text.lines().enumerate() {
        let event: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("line {} parses as JSON: {error}", index + 1));
        for field in ["agent", "agent_runtime"] {
            let Some(value) = event.get(field).and_then(serde_json::Value::as_str) else {
                continue;
            };
            if field == "agent_runtime" {
                saw_runtime = true;
            }
            assert!(
                value.starts_with("fixture-"),
                "run-event fixtures use neutral identity fields; line {} {field}={value:?}",
                index + 1
            );
            assert!(
                !matches!(value, "codex" | "claude" | "droid"),
                "run-event fixtures must not hardcode a real agent runtime; line {} {field}={value:?}",
                index + 1
            );
        }
    }

    assert!(
        saw_runtime,
        "run-event fixture should still cover agent_runtime"
    );
}

/// Every store-backed family's fixtures, seeded into one card-mode repo, read
/// through the real CLI entry points; afterwards every seeded byte must be
/// untouched (matrix item 4: reading is never a rewrite).
#[test]
fn fixtures_read_through_real_entry_points_without_rewrites() {
    let temp = TestTempDir::new("maestro-fixture-harness");
    let repo = temp.path();
    fs::create_dir_all(repo.join(".maestro/cards")).expect("cards dir");

    let mut seeded: Vec<PathBuf> = vec![
        seed_card_dir(
            repo,
            "card",
            "current-minimal.yaml",
            "card-aa11bb",
            "card.yaml",
        ),
        seed_card_dir(
            repo,
            "card",
            "current-full.yaml",
            "card-bb22cc",
            "card.yaml",
        ),
        seed_card_dir(
            repo,
            "task",
            "current-minimal.yaml",
            "card-cc33dd",
            "card.yaml",
        ),
        seed_card_dir(
            repo,
            "task",
            "current-full.yaml",
            "card-dd44ee",
            "card.yaml",
        ),
        seed_card_dir(
            repo,
            "feature",
            "current-minimal.yaml",
            "feature-minimal-fixture-0a1b",
            "card.yaml",
        ),
        seed_card_dir(
            repo,
            "feature",
            "current-full.yaml",
            "feature-full-fixture-1c2d",
            "card.yaml",
        ),
        seed_entries(
            repo,
            "decisions.yaml",
            &[
                ("decision", "current-minimal.yaml"),
                ("decision", "current-full.yaml"),
            ],
        ),
        seed_entries(
            repo,
            "ideas.yaml",
            &[
                ("backlog", "current-minimal.yaml"),
                ("backlog", "current-full.yaml"),
            ],
        ),
    ];
    let run_dir = repo.join(".maestro/runs/cli-2026-06-12");
    fs::create_dir_all(&run_dir).expect("run dir");
    let events = run_dir.join("events.jsonl");
    fs::write(&events, fixture("run-event", "current-events.jsonl")).expect("events seed");
    seeded.push(events);
    let evidence = run_dir.join("run_evidence.yaml");
    fs::write(&evidence, fixture("run-evidence", "current-full.yaml")).expect("evidence seed");
    seeded.push(evidence);

    let before: Vec<Vec<u8>> = seeded
        .iter()
        .map(|path| fs::read(path).expect("seeded file reads"))
        .collect();

    // Card envelopes and the task/feature/decision/backlog payloads, through
    // the verbs their consumers use.
    for id in [
        "card-aa11bb",
        "card-bb22cc",
        "card-cc33dd",
        "card-dd44ee",
        "card-aa66bb",
        "card-bb77cc",
        "card-cc88dd",
        "card-dd99ee",
    ] {
        assert_success(&maestro(repo, &["show", id]), &["show", id]);
    }
    for id in ["card-cc33dd", "card-dd44ee"] {
        assert_success(&maestro(repo, &["task", "show", id]), &["task", "show", id]);
    }
    let features = assert_success(
        &maestro(repo, &["feature", "list", "--all"]),
        &["feature", "list", "--all"],
    );
    assert!(
        features.contains("feature-minimal-fixture-0a1b"),
        "{features}"
    );
    assert!(features.contains("feature-full-fixture-1c2d"), "{features}");
    assert!(
        !features.contains("unreadable"),
        "every feature fixture must load cleanly: {features}"
    );
    let decisions = assert_success(&maestro(repo, &["decision", "list"]), &["decision", "list"]);
    assert!(decisions.contains("card-aa66bb"), "{decisions}");
    assert!(decisions.contains("card-bb77cc"), "{decisions}");
    assert_success(
        &maestro(repo, &["query", "friction"]),
        &["query", "friction"],
    );

    // The run-evidence read is a library seam (no CLI verb renders it today).
    let load = maestro::domain::run::load_run_evidence(
        &maestro::foundation::core::paths::MaestroPaths::new(repo),
    )
    .expect("evidence load");
    assert_eq!(load.records.len(), 1, "the evidence fixture loads");

    for (path, before) in seeded.iter().zip(&before) {
        let after = fs::read(path).expect("seeded file still reads");
        assert_eq!(
            &after,
            before,
            "reading must not rewrite {}",
            path.display()
        );
    }
}

/// The install-lock and global-skills-lock fixtures parse through their real
/// loaders (the CLI surface for both is `install`/`sync`, exercised in their
/// own integration suites).
#[test]
fn lock_fixtures_parse_through_their_loaders() {
    let temp = TestTempDir::new("maestro-fixture-locks");
    let root = temp.path();

    for name in ["install-lock-minimal.yaml", "install-lock-full.yaml"] {
        let path = root.join(name);
        fs::write(&path, fixture("install", name)).expect("lock fixture writes");
        let lock = maestro::domain::install::InstallLock::load(&path)
            .unwrap_or_else(|error| panic!("{name} must load: {error:#}"));
        if name.ends_with("full.yaml") {
            assert!(lock.agents.contains_key("claude"), "{name} carries claude");
        }
    }

    let home = root.join("home");
    fs::create_dir_all(home.join(".maestro")).expect("home dir");
    fs::write(
        home.join(".maestro/skills-lock.yaml"),
        fixture("install", "global-skills-lock-full.yaml"),
    )
    .expect("skills lock writes");
    let before = fs::read(home.join(".maestro/skills-lock.yaml")).expect("lock reads");
    maestro::domain::skills::prepare_global_skills_at(&home)
        .expect("the skills-lock fixture must parse through the global sync prepare");
    let after = fs::read(home.join(".maestro/skills-lock.yaml")).expect("lock still reads");
    assert_eq!(before, after, "preparing must not rewrite the lock");
}

/// The harness fixtures parse through the doctor's real harness read in an
/// init'd repo, without rewriting the file.
#[test]
fn harness_fixtures_read_through_doctor() {
    let temp = TestTempDir::new("maestro-fixture-harness-yml");
    let repo = temp.path();
    fs::create_dir_all(repo.join(".git")).expect("git marker");
    assert_success(&maestro(repo, &["init", "--yes"]), &["init", "--yes"]);

    let harness_yml = repo.join(".maestro/harness/harness.yml");
    for name in ["current-minimal.yaml", "current-full.yaml"] {
        fs::write(&harness_yml, fixture("harness", name)).expect("harness fixture writes");
        let before = fs::read(&harness_yml).expect("harness reads");
        assert_success(&maestro(repo, &["doctor"]), &["doctor"]);
        let after = fs::read(&harness_yml).expect("harness still reads");
        assert_eq!(
            before, after,
            "doctor must not rewrite harness.yml ({name})"
        );
    }
}

/// Matrix item 5: every create verb stamps the family's current version --
/// the write side of the bounded read set.
#[test]
fn create_verbs_stamp_current_versions() {
    let temp = TestTempDir::new("maestro-fixture-write-current");
    let repo = temp.path();
    fs::create_dir_all(repo.join(".git")).expect("git marker");
    assert_success(&maestro(repo, &["init", "--yes"]), &["init", "--yes"]);

    let harness = fs::read_to_string(repo.join(".maestro/harness/harness.yml"))
        .expect("init writes harness.yml");
    assert!(
        harness.contains("schema_version: maestro.harness.v1"),
        "init stamps the current harness version: {harness}"
    );

    assert_success(
        &maestro(repo, &["feature", "new", "Write Stamp"]),
        &["feature", "new", "Write Stamp"],
    );
    let feature = fs::read_to_string(repo.join(".maestro/cards/write-stamp/card.yaml"))
        .expect("feature new writes the card");
    assert!(
        feature.contains("schema_version: maestro.card.v1"),
        "{feature}"
    );
    assert!(
        feature.contains("schema_version: maestro.feature.v2"),
        "the payload stamps the current feature version: {feature}"
    );

    assert_success(
        &maestro(repo, &["task", "create", "Write stamp task"]),
        &["task", "create", "Write stamp task"],
    );
    let pool = repo.join(".maestro/cards/tasks");
    let task_dir = fs::read_dir(&pool)
        .expect("task pool exists")
        .filter_map(Result::ok)
        .find(|entry| entry.path().is_dir())
        .expect("task create lands in the pool");
    let task = fs::read_to_string(task_dir.path().join("task.yaml")).expect("task record written");
    assert!(task.contains("schema_version: maestro.card.v1"), "{task}");
    assert!(
        task.contains("schema_version: maestro.task.v2"),
        "the payload stamps the current task version: {task}"
    );

    assert_success(
        &maestro(repo, &["decision", "new", "Write stamp ruling"]),
        &["decision", "new", "Write stamp ruling"],
    );
    let decisions = fs::read_to_string(repo.join(".maestro/cards/decisions.yaml"))
        .expect("decision new writes the roster");
    assert!(
        decisions.contains("schema_version: maestro.card.v1"),
        "the decision entry stamps the current card version: {decisions}"
    );

    assert_success(
        &maestro(repo, &["create", "Write stamp idea", "--type", "idea"]),
        &["create", "Write stamp idea", "--type", "idea"],
    );
    let ideas = fs::read_to_string(repo.join(".maestro/cards/ideas.yaml"))
        .expect("create --type idea writes the roster");
    assert!(
        ideas.contains("schema_version: maestro.card.v1"),
        "the idea entry stamps the current card version: {ideas}"
    );
}

/// V5.5 through the shipped fixture itself: the below-floor task payload the
/// pack carries refuses with the explicit migrate route, not a parse error.
#[test]
fn legacy_task_fixture_refuses_with_the_migrate_route() {
    let temp = TestTempDir::new("maestro-fixture-legacy-task");
    let repo = temp.path();
    fs::create_dir_all(repo.join(".maestro/cards")).expect("cards dir");
    seed_card_dir(
        repo,
        "task",
        "legacy-task-v1.yaml",
        "card-ee55ff",
        "card.yaml",
    );

    let show = maestro(repo, &["task", "show", "card-ee55ff"]);
    assert!(
        !show.status.success(),
        "a below-floor payload must refuse: {}",
        String::from_utf8_lossy(&show.stdout)
    );
    let message = String::from_utf8_lossy(&show.stderr);
    assert!(message.contains("schema mismatch"), "{message}");
    assert!(
        message.contains("fix: run maestro migrate-v2"),
        "the refusal must carry the pack route: {message}"
    );
}
