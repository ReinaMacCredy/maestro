mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use maestro::domain::loop_recipes::{self, LoopChainFacts, LoopChainSelectedUnit};
use maestro::foundation::core::hash::sha256_hex;
use maestro::foundation::core::time::format_utc_seconds_rfc3339_millis;
use serde_json::Value;

use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AUTO_UPDATE", "0")
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn maestro_with_env(cwd: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AUTO_UPDATE", "0");
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn stdout(cwd: &Path, args: &[&str]) -> String {
    let output = maestro(cwd, args);
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn stderr(cwd: &Path, args: &[&str]) -> String {
    let output = maestro(cwd, args);
    assert!(
        !output.status.success(),
        "maestro {args:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
}

fn assert_bounded_text(output: &str, max_lines: usize) {
    assert!(
        output.lines().count() <= max_lines,
        "expected at most {max_lines} lines, got {}:\n{output}",
        output.lines().count()
    );
}

const SHIPPED_RECIPE_PARITY: [(&str, &str, &str, &str); 15] = [
    (
        "adversarial-review",
        "ba675cbe7063b2fa9f06ab6068a3c92154b6ef0c4320923d1a9d19aed64df1d1",
        "ee95b9cf38c40084dd8ca59d6815c7b8e94b752405e12bb1588073a74bc9281c",
        "e27bd1a428253765597208651e0fb49a4e922f7d7ff3b8a2fab68057edd6b86b",
    ),
    (
        "audit",
        "4b445fed94188e9fb2ba445702ce49c01249666eda6d621a918ecc50bdf1db6e",
        "55b2c5a67a3aed8b73a2a7151d0fd75598a74cadbf328ee743cc21e2709b4aff",
        "b84b90b07097f1a491f5e9881b80b3baf910dcf113df76eb8bcd08f9248766d3",
    ),
    (
        "conflict-handoff",
        "e68ab6120afd87c3835769bd2c78da1accc44f4d8b31b6e7ab82459c623e3f50",
        "0c04670390de1bc11050c4e760106bebf1e3a6cb9d8b8a49d501370e7c01a59b",
        "bd0540b40ef33459bb07652d85df5504c51327cd1c731e06ea06465b7ea65fc2",
    ),
    (
        "design",
        "9a2b4bb4a8ded6d7245d16cb97e1ae72b293abcf17f9ce401789b14ab1fedfc3",
        "6c3affd6edca9713933ca04729a84d95d5226d408079e42ae5ae205b26ebe7ac",
        "ab89d9ac9499d7e4b27985cec5a51dc954d6f3736bd18ff9a46b682ec7a71c63",
    ),
    (
        "design-relay",
        "ed451e49791789ed2d084cb9f84618814a0fe71d5456934d125ae287a32dc5ea",
        "4d8051110bf97b172298ddbda9228d650ebd0188cbf914e8c98f5434ff11b458",
        "c0aa535d345871f2235d972351a9de670fb5fef257542b8ac07ff280d427bef8",
    ),
    (
        "feature-fanout",
        "40fbf1726875a7fb2239d0293686589c60bbe1a6bdff7d89e6e12d5406b6129a",
        "ff4740796a2f7be2d68f71695ad6820db7b58887e49a2058dbb5e34a09bbfbe6",
        "546659205efd634037c7c7db6f3db41113072f9281ab10ffeef7a7bfd6edd3dc",
    ),
    (
        "generate-filter",
        "6db9bfa174a8641adf7eed422e0f768036dfbc6865cf94cdee59c95119b60a0a",
        "7c6ce983b707f8113858b24fa35a8b9297352ea21cdafde5a50d64bcb4996e7d",
        "ac0b794987c593b03a6824020e2a984084c83e705035b96b564d2a145e469f5f",
    ),
    (
        "intake-triage",
        "9e21676f7605106adc2199d9df6b91ad705176e047f4b201c5c612f76e0f4181",
        "133d920d918ced438d0dcd13bbbba051f13ebc4c0223c27ef94a663b23c59ba3",
        "ee777ae8884ca64405767ce0995abf33b40b144b2b6ad3f25e5e2c54f08fa502",
    ),
    (
        "learning",
        "e95ea2de142f6f315f63aa55795c285e86799b0ee3059e22954a34746c6b2d81",
        "90308cdfc646f008914e6d4e0c42194617770efbd3d2c1664484864b9078ac93",
        "51fdaaf59540d356c4652ecdc0d1989d2409db00b3daa5c12e95506482bd8f01",
    ),
    (
        "loop-until-done",
        "84e2a583455b0ffcbf0723e125460a9116ee7b111476521450f78552785fc7a7",
        "2a59b00f9ebbe53034cb7a7914abb0867672cba6a7519fdcf0dfccd34ac9bbf9",
        "a9797f7710c188ec9a692509f3000066a1c60b414e2228944e0a29b9a1d7aca9",
    ),
    (
        "progress",
        "61c1d0031cd720a41c6e8690f2e64d64ba97cbe1cfad389fb4521318185e9185",
        "7f8a1200665b6c997dc6c76956615f3d99af2a6b06dfe40b3533c45b7d776e2b",
        "7445eb7bec581e151dde8e71b05c6731c914006dc2075584622ed97dca35dde7",
    ),
    (
        "ship",
        "51c6880598417dcdf2e93839eaa820725fdeeb124c9babc80fb7a0a7e7fc93fe",
        "43cd6d90bd94257040fe8d269adaaa24059464194f7bd7243f1abaace859ef04",
        "6e4fddca3bb04ff8d6924512f19da47d9c0e05c1d0151ceb79609073c48d8fc6",
    ),
    (
        "synthesize",
        "8e242846e03134f6c1e902b478f34db035688c244d73f84f7767ac36be2e30d8",
        "5f645a2d2821c4e8f0797061fb4379aa811068bdc7efdf76c4eeee8003355ff5",
        "13f4bc8eac884d26da61a9413c6676be0fec114842471bb8551786a65373db43",
    ),
    (
        "unattended",
        "45eb5a0a70c557a61336e48c67c0c2baf5ea190a698b9e2d1085694deb54d9ec",
        "ebbfaba2889ae7c2ab91feac0b838dec260d1bfedc5fd2c849b7e1cbcfce5ef1",
        "93d9f439444f33d970d6ae1ec40781240e9a5f2de6323c101324105c62f5724e",
    ),
    (
        "work",
        "a11a217b1980d877889a304fe88848f47771815770166fe32e6a0cc738fb9f1c",
        "8b8e1324b7ad60ce73748ccbde0334cb3a1b5cd047f2c69dca4a479e81c14ad5",
        "c70d8b224c9ad0375921a2d2fb900e38c57f82fbe77a45bf699e61360e78e4a2",
    ),
];

#[test]
fn shipped_v3_profiles_preserve_complete_v2_normalized_semantics() {
    assert_eq!(
        loop_recipes::contract_names(),
        SHIPPED_RECIPE_PARITY
            .iter()
            .map(|(name, _, _, _)| *name)
            .collect::<Vec<_>>()
    );
    for (name, expected_effective, expected_full, expected_compact) in SHIPPED_RECIPE_PARITY {
        let contract = loop_recipes::contract(name).expect("shipped v3 recipe should resolve");
        assert_eq!(
            contract.provenance.source_schema, "maestro.recipe.v3",
            "{name}"
        );
        assert_eq!(
            contract.provenance.profile, "maestro.standard-six-phase.v1",
            "{name}"
        );
        assert!(
            contract
                .invariant_ids
                .iter()
                .any(|id| id == "standard-progress-structure"),
            "{name}"
        );
        let actual = normalized_recipe_hashes(name);
        assert_eq!(actual.0, expected_effective, "{name} effective contract");
        assert_eq!(actual.1, expected_full, "{name} full rendering");
        assert_eq!(actual.2, expected_compact, "{name} compact phase packets");
    }
}

fn first_id(output: &str, prefix: &str) -> String {
    output
        .split_whitespace()
        .find(|word| word.starts_with(prefix))
        .unwrap_or_else(|| panic!("no {prefix} id in output:\n{output}"))
        .to_string()
}

fn normalized_recipe_hashes(name: &str) -> (String, String, String) {
    let contract = loop_recipes::contract(name).expect("shipped recipe should resolve");
    let mut effective =
        serde_json::to_value(contract.effective()).expect("effective recipe should serialize");
    effective
        .as_object_mut()
        .expect("effective recipe should be an object")
        .remove("schema_version");
    let effective_hash = sha256_hex(
        &serde_json::to_vec(&effective).expect("normalized effective recipe should serialize"),
    );

    let full = loop_recipes::show(name).expect("full recipe should render");
    let normalized_full = full
        .lines()
        .filter(|line| {
            ![
                "resolved_schema:",
                "schema_version:",
                "profile:",
                "resolver:",
                "source:",
                "contract_hash_schema:",
                "contract_hash:",
            ]
            .iter()
            .any(|prefix| line.starts_with(prefix))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let full_hash = sha256_hex(normalized_full.as_bytes());

    let mut compact = Vec::new();
    for phase in ["perceive", "choose", "act", "observe", "learn", "continue"] {
        let packet = loop_recipes::compact_packet_with_custom_dir(name, None, Some(phase))
            .expect("compact phase packet should render");
        let encoded = serde_json::to_vec(&packet).expect("compact phase packet should serialize");
        compact.extend_from_slice(&(encoded.len() as u64).to_le_bytes());
        compact.extend_from_slice(&encoded);
    }
    (effective_hash, full_hash, sha256_hex(&compact))
}

fn ready_loop_task(id: &str) -> loop_recipes::LoopTaskInput {
    loop_recipes::LoopTaskInput {
        id: id.to_string(),
        title: "Implement unknown gap".to_string(),
        state: "ready".to_string(),
        feature_id: None,
        blocked: false,
        ready_startable: true,
        gate: false,
        gate_kind: None,
        lane: Some("general".to_string()),
        remaining_blockers: Vec::new(),
    }
}

fn write_custom_recipe(repo: &Path, name: &str, body: &str) {
    let dir = repo.join(".maestro/loop-recipes");
    fs::create_dir_all(&dir).expect("custom recipe dir should be creatable");
    fs::write(dir.join(format!("{name}.yml")), body).expect("custom recipe should be writable");
}

fn profiled_custom_recipe() -> String {
    let body = CUSTOM_RECIPE.replacen(
        "schema_version: maestro.recipe.v2",
        "schema_version: maestro.recipe.v3\nprofile: maestro.standard-six-phase.v1",
        1,
    );
    let start = body
        .find("progress_tasks:")
        .expect("custom recipe fixture should declare progress tasks");
    let end = body
        .find("authority_scope:")
        .expect("custom recipe fixture should declare authority scope");
    format!(
        "{}progress_tasks:\n  anchor-scope:\n    title: Anchor support brief scope\n    done_check: support brief and selected card are visible\n  return-next-gate:\n    title: Finish selected brief card\n    done_check: next step or hard stop is returned\n{}",
        &body[..start],
        &body[end..]
    )
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn init_git_repo(repo: &Path) {
    let output = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(repo)
        .output()
        .expect("invariant: git init should run in test fixture");
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn ts_minutes_ago(minutes: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("invariant: clock is after the Unix epoch")
        .as_secs();
    format_utc_seconds_rfc3339_millis(now - minutes * 60)
}

fn seed_run(repo: &Path, session: &str, lines: &[String]) {
    let run_dir = repo.join(".maestro/runs").join(session);
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .expect("invariant: events fixture should be writable");
}

fn ownership_event(session: &str, card: &str, minutes_ago: u64) -> String {
    let ts = ts_minutes_ago(minutes_ago);
    format!(
        r#"{{"event_type":"ownership_acquire","session_id":"{session}","card_id":"{card}","ts":"{ts}"}}"#
    )
}

fn snapshot_dir(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    collect_snapshot(dir, dir, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn collect_snapshot(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).expect("snapshot dir should be readable") {
        let entry = entry.expect("snapshot entry should be readable");
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).expect("snapshot metadata should be readable");
        if metadata.is_dir() {
            collect_snapshot(root, &path, files);
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("snapshot path should stay under root")
                .display()
                .to_string();
            let contents = fs::read(&path).expect("snapshot file should be readable");
            files.push((relative, contents));
        }
    }
}

#[test]
fn loop_index_lists_unified_structured_recipe_catalog() {
    let temp = TestTempDir::new("maestro-loop-index");
    let out = stdout(temp.path(), &["loop"]);

    assert!(out.contains("## Shipped Recipe Catalog"), "{out}");
    assert!(out.contains("design  [lifecycle]"), "{out}");
    assert!(out.contains("design-relay  [orchestration]"), "{out}");
    assert!(out.contains("work  [lifecycle]"), "{out}");
    assert!(out.contains("unattended  [lifecycle]"), "{out}");
    assert!(out.contains("conflict-handoff  [orchestration]"), "{out}");
    assert!(out.contains("synthesize  [orchestration]"), "{out}");
    assert!(out.contains("feature-fanout"), "{out}");
    assert!(out.contains("adversarial-review"), "{out}");
    assert!(out.contains("generate-filter"), "{out}");
    assert!(out.contains("## Custom Recipe Policy"), "{out}");
    assert!(out.contains("conflict-handoff"), "{out}");
    assert!(out.contains("synthesize"), "{out}");
    assert!(out.contains("## Shipped Pattern Packs"), "{out}");
    assert!(out.contains("daily-triage"), "{out}");
    assert!(out.contains("pr-babysitter"), "{out}");
    assert!(out.contains("ci-sweeper"), "{out}");
    assert!(out.contains("dependency-sweeper"), "{out}");
    assert!(out.contains("changelog-drafter"), "{out}");
    assert!(out.contains("post-merge-cleanup"), "{out}");
    assert!(out.contains("issue-triage"), "{out}");
    assert!(!out.contains("feature-fan-out"), "{out}");
    assert!(!out.contains("adversarial-fan-out"), "{out}");
    assert!(!out.contains("generate-and-filter"), "{out}");
}

#[test]
fn loop_show_and_validate_render_recipe_native_pattern_packs() {
    let temp = TestTempDir::new("maestro-loop-pattern-packs");

    let shown = stdout(temp.path(), &["loop", "show", "pr-babysitter", "--full"]);
    assert!(
        shown.contains("schema_version: maestro.recipe_pattern.v1"),
        "{shown}"
    );
    assert!(shown.contains("id: pr-babysitter"), "{shown}");
    assert!(shown.contains("readiness_floor: L2 assisted"), "{shown}");
    assert!(shown.contains("- feature-fanout"), "{shown}");
    assert!(shown.contains("- work"), "{shown}");
    assert!(shown.contains("- synthesize"), "{shown}");
    for limit in [
        "cadence",
        "max_attempts",
        "max_subagents",
        "denylist",
        "budget",
        "kill_switch",
        "connector_permissions",
    ] {
        assert!(shown.contains(limit), "{limit} missing from {shown}");
    }

    let valid = stdout(temp.path(), &["loop", "validate", "ci-sweeper"]);
    assert!(
        valid.contains("valid shipped loop pattern: ci-sweeper"),
        "{valid}"
    );
    assert!(
        valid.contains("schema: maestro.loop_readiness.v1"),
        "{valid}"
    );
    assert!(valid.contains("readiness_floor: L1 report"), "{valid}");
    assert!(valid.contains("effective_level: L0 draft"), "{valid}");
    assert!(valid.contains("base_recipes: audit -> work"), "{valid}");
    assert!(
        valid.contains("scheduler_stance: passive_local_first"),
        "{valid}"
    );
    assert!(valid.contains("liveness:"), "{valid}");
    assert!(valid.contains("gaps:"), "{valid}");
    assert!(valid.contains("blocked_from_next_level:"), "{valid}");
    assert!(
        valid.contains("external schedulers stay external"),
        "{valid}"
    );
    assert!(valid.contains("- connector_permissions"), "{valid}");
}

#[test]
fn loop_next_json_routes_missing_maestro_without_writes() {
    let temp = TestTempDir::new("maestro-loop-next-missing");
    let out = stdout(temp.path(), &["loop", "next", "--json"]);
    let value: Value = serde_json::from_str(&out).expect("loop next JSON should parse");

    assert_eq!(value["schema"], "maestro.loop_next.v1");
    assert_eq!(value["status"], "uncertain");
    assert_eq!(value["recommended_recipe"], "intake-triage");
    assert_eq!(value["recommended_status"], "intake_triage");
    assert!(value["authority_scope"].is_array(), "{value}");
    assert!(value["autonomy"].is_array(), "{value}");
    assert!(value["edges"].is_array(), "{value}");
    assert!(value["hard_stops"].is_array(), "{value}");
    assert!(value["inspect"].is_array(), "{value}");
    assert!(value["next_verbs"].is_array(), "{value}");
    assert!(
        !temp.path().join(".maestro").exists(),
        "loop next must not initialize or write Maestro artifacts"
    );
}

#[test]
fn loop_next_json_exposes_grounded_unknown_gap_for_ready_work() {
    let temp = TestTempDir::new("maestro-loop-next-unknown-gap");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(
        temp.path(),
        &["task", "add", "--id-only", "Implement unknown gap"],
    );

    let out = stdout(temp.path(), &["loop", "next", "--json"]);
    let value: Value = serde_json::from_str(&out).expect("loop next JSON should parse");
    let unknown_gap = &value["unknown_gap"];

    assert!(unknown_gap.is_object(), "{value}");
    assert!(unknown_gap["known_knowns"].is_array(), "{unknown_gap}");
    assert!(unknown_gap["known_unknowns"].is_array(), "{unknown_gap}");
    assert!(unknown_gap["unknown_knowns"].is_array(), "{unknown_gap}");
    assert!(
        unknown_gap["unknown_unknown_risks"].is_array(),
        "{unknown_gap}"
    );
    assert_eq!(unknown_gap["action"], "probe");
    assert_eq!(unknown_gap["known_knowns"][0]["source"], "current_command");
    assert_eq!(unknown_gap["known_unknowns"][0]["source"], "proof");
}

#[test]
fn loop_next_full_text_preserves_unknown_gap_for_ready_work() {
    let temp = TestTempDir::new("maestro-loop-next-unknown-gap-text");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(
        temp.path(),
        &["task", "add", "--id-only", "Render unknown gap"],
    );

    let out = stdout(temp.path(), &["loop", "next", "--full"]);

    assert!(out.contains("unknown_gap:"), "{out}");
    assert!(out.contains("  action: probe"), "{out}");
    assert!(out.contains("  known_knowns:"), "{out}");
    assert!(out.contains("[current_command]"), "{out}");
    assert!(!out.contains("maestro unknowns"), "{out}");
}

#[test]
fn loop_next_json_keeps_return_condition_field_compatible() {
    let temp = TestTempDir::new("maestro-loop-next-edge-json-compatible");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(
        temp.path(),
        &["task", "add", "--id-only", "Render edge JSON"],
    );

    let out = stdout(temp.path(), &["loop", "next", "--json"]);
    let value: Value = serde_json::from_str(&out).expect("loop next JSON should parse");
    let edge = value["edges"]
        .as_array()
        .and_then(|edges| edges.first())
        .unwrap_or_else(|| panic!("expected at least one edge in loop next JSON:\n{value}"));

    assert!(edge["return_condition"].is_string(), "{edge}");
    assert!(edge["return_conditions"].is_array(), "{edge}");
}

#[test]
fn loop_next_unknown_gap_omits_source_less_memory_candidates() {
    let report = loop_recipes::route_next(loop_recipes::LoopRouterInput {
        repo: "test-repo".to_string(),
        initialized: true,
        tasks: vec![ready_loop_task("task-unknown-gap-source")],
        memory_hits: vec![
            loop_recipes::LoopMemoryHit {
                id: "memory-source-less".to_string(),
                kind: "user_correction".to_string(),
                reason: "source-less preference should not render".to_string(),
                source_refs: Vec::new(),
            },
            loop_recipes::LoopMemoryHit {
                id: "memory-sourced".to_string(),
                kind: "user_correction".to_string(),
                reason: "sourced preference should render".to_string(),
                source_refs: vec![loop_recipes::LoopContextRef {
                    kind: "memory".to_string(),
                    id: Some("memory-sourced".to_string()),
                    path: None,
                    command: Some("maestro grep \"unknowns\" corpus:memory".to_string()),
                }],
            },
        ],
        ..loop_recipes::LoopRouterInput::default()
    })
    .expect("loop next should route ready work");
    let gap = report.unknown_gap.expect("unknown_gap should be present");

    assert_eq!(gap.unknown_knowns.len(), 1, "{gap:?}");
    assert_eq!(gap.unknown_knowns[0].source, "memory");
    assert!(
        gap.unknown_knowns[0]
            .text
            .contains("sourced preference should render"),
        "{gap:?}"
    );
    assert!(
        !gap.unknown_knowns.iter().any(|item| item
            .text
            .contains("source-less preference should not render")),
        "{gap:?}"
    );
    assert!(gap.known_knowns.len() <= 3, "{gap:?}");
    assert!(gap.known_unknowns.len() <= 3, "{gap:?}");
    assert!(gap.unknown_unknown_risks.len() <= 3, "{gap:?}");
}

#[test]
fn loop_next_unknown_gap_action_probes_for_warn_constraints() {
    let report = loop_recipes::route_next(loop_recipes::LoopRouterInput {
        repo: "test-repo".to_string(),
        initialized: true,
        tasks: vec![ready_loop_task("task-unknown-gap-probe")],
        git: Some(loop_recipes::LoopGitInput {
            branch: Some("main".to_string()),
            code_other_dirty: 1,
            maestro_dirty: 0,
            ahead: 0,
            behind: 0,
        }),
        ..loop_recipes::LoopRouterInput::default()
    })
    .expect("loop next should route ready work");
    let gap = report.unknown_gap.expect("unknown_gap should be present");

    assert_eq!(gap.action, "probe");
    assert!(
        gap.unknown_unknown_risks
            .iter()
            .any(|item| item.source == "current_fact_gap"
                && item.text.contains("working tree has dirty")),
        "{gap:?}"
    );
}

#[test]
fn loop_show_renders_design_relay_recipe() {
    let temp = TestTempDir::new("maestro-loop-show-design-relay");
    let out = stdout(temp.path(), &["loop", "show", "design-relay", "--full"]);

    assert!(out.contains("# Design relay"), "{out}");
    assert!(out.contains("schema_version: maestro.recipe.v2"), "{out}");
    assert!(out.contains("mandate_ref"), "{out}");
    assert!(
        out.contains("advisor and subagent output is evidence, not user consent"),
        "{out}"
    );
    assert!(out.contains("return to design"), "{out}");
    assert!(out.contains("out-of-scope flags"), "{out}");
    assert!(out.contains("maestro decision lock <id>"), "{out}");
    assert!(out.contains("feature accept"), "{out}");
}

#[test]
fn loop_show_design_continue_waits_for_build_approval_before_finalize() {
    let temp = TestTempDir::new("maestro-loop-show-design-continue");
    let out = stdout(temp.path(), &["loop", "show", "design", "--full"]);

    assert!(
        out.contains("await explicit build approval before reconcile/finalize"),
        "{out}"
    );
    assert!(
        !out.contains("Allowed verbs\n\n- `maestro feature finalize <id>`"),
        "{out}"
    );
    assert!(
        !out.contains("Next\n\n- `maestro feature finalize <id>`"),
        "{out}"
    );
    assert!(
        out.contains("maestro feature finalize <id> without explicit build approval"),
        "{out}"
    );

    assert!(
        out.contains("design.continue -> work.perceive")
            && out.contains("trigger: work_ready.design_locked"),
        "{out}"
    );
    assert!(out.contains("maestro feature finalize <id>"), "{out}");
}

#[test]
fn loop_validate_and_compact_render_design_relay_card() {
    let temp = TestTempDir::new("maestro-loop-design-relay-packet");
    let valid = stdout(temp.path(), &["loop", "validate", "design-relay"]);
    assert!(
        valid.contains("valid shipped loop recipe: design-relay"),
        "{valid}"
    );

    let card = stdout(temp.path(), &["loop", "show", "design-relay", "--compact"]);
    assert_bounded_text(&card, 6);
    assert!(card.contains("design-relay · design"), "{card}");
    assert!(
        card.contains("enter: maestro loop next --compact"),
        "{card}"
    );
    assert!(
        card.contains("full: maestro loop show design-relay --full"),
        "{card}"
    );

    let full = stdout(temp.path(), &["loop", "show", "design-relay", "--full"]);
    assert!(full.contains("maestro msg send <card> <text>"), "{full}");
    assert!(full.contains("feature accept"), "{full}");
    assert!(full.contains("no recorded mandate_ref"), "{full}");
}

#[test]
fn loop_next_routes_ready_task_and_does_not_mutate_maestro_store() {
    let temp = TestTempDir::new("maestro-loop-next-ready-task");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    let task_id = stdout(
        temp.path(),
        &["task", "add", "--id-only", "Implement router"],
    );
    let task_id = task_id.trim();
    let before = snapshot_dir(&temp.path().join(".maestro"));

    let out = stdout(temp.path(), &["loop", "next", "--json"]);
    let after = snapshot_dir(&temp.path().join(".maestro"));
    let value: Value = serde_json::from_str(&out).expect("loop next JSON should parse");

    assert_eq!(before, after, "loop next must not mutate .maestro");
    assert_eq!(value["schema"], "maestro.loop_next.v1");
    assert_eq!(value["status"], "recommended");
    assert_eq!(value["recommended_recipe"], "work");
    assert_eq!(value["recommended_status"], "work");
    assert_eq!(value["recommended_phase"], "perceive");
    assert!(
        value["score"]
            .as_u64()
            .is_some_and(|score| score > 0 && score <= 100),
        "{value}"
    );
    assert!(value["attempt_policy"].is_object(), "{value}");
    assert!(value["constraints"].is_array(), "{value}");
    assert!(value["context_refs"].is_array(), "{value}");
    let expected_inspect = format!("maestro task show {task_id}");
    assert!(
        value["inspect"]
            .as_array()
            .expect("inspect should be an array")
            .iter()
            .any(|entry| entry.as_str() == Some(expected_inspect.as_str())),
        "{value}"
    );
    assert!(
        value["edges"]
            .as_array()
            .expect("edges should be an array")
            .iter()
            .any(|edge| edge["kind"] == "transition"
                && edge["trigger"] == "design_needed.scope_unclear"
                && edge["from"] == "work.act"
                && edge["to"] == "design.choose"),
        "{value}"
    );
    assert!(
        value["edges"]
            .as_array()
            .expect("edges should be an array")
            .iter()
            .any(|edge| edge["kind"] == "invocation" && edge["to"] == "audit.perceive"),
        "{value}"
    );
}

#[test]
fn loop_next_chain_text_explains_current_transition_without_mutating_store() {
    let temp = TestTempDir::new("maestro-loop-next-chain-text");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(
        temp.path(),
        &[
            "feature",
            "new",
            "Chain UX",
            "--question",
            "Which acceptance path is current?",
        ],
    );
    let task_id = stdout(
        temp.path(),
        &[
            "task",
            "create",
            "Implement chain UX",
            "--feature",
            "chain-ux",
            "--id-only",
        ],
    );
    let task_id = task_id.trim();
    stdout(temp.path(), &["task", "explore", task_id]);
    stdout(temp.path(), &["task", "accept", task_id]);
    stdout(temp.path(), &["task", "claim", task_id]);
    let before = snapshot_dir(&temp.path().join(".maestro"));

    let out = stdout(temp.path(), &["loop", "next", "--chain"]);
    let after = snapshot_dir(&temp.path().join(".maestro"));

    assert_eq!(before, after, "loop next --chain must stay read-only");
    assert!(
        out.contains("chain: design -> work -> verify -> close -> archive"),
        "{out}"
    );
    assert!(out.contains("current: work.act"), "{out}");
    assert!(out.contains("selected_unit: task:"), "{out}");
    assert!(
        out.contains("transition: work.act -> design.choose"),
        "{out}"
    );
    assert!(
        out.contains("trigger: design_needed.scope_unclear"),
        "{out}"
    );
    assert!(out.contains("next: maestro decision new"), "{out}");
    assert!(
        out.contains("- decision.all_blockers_locked: missing"),
        "{out}"
    );
}

#[test]
fn loop_next_chain_json_uses_stable_envelope() {
    let temp = TestTempDir::new("maestro-loop-next-chain-json");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(
        temp.path(),
        &[
            "feature",
            "new",
            "Chain JSON",
            "--question",
            "Which scope is current?",
        ],
    );
    let task_id = stdout(
        temp.path(),
        &[
            "task",
            "create",
            "Implement chain JSON",
            "--feature",
            "chain-json",
            "--id-only",
        ],
    );
    let task_id = task_id.trim();
    stdout(temp.path(), &["task", "explore", task_id]);
    stdout(temp.path(), &["task", "accept", task_id]);
    stdout(temp.path(), &["task", "claim", task_id]);

    let out = stdout(temp.path(), &["loop", "next", "--chain", "--json"]);
    let value: Value = serde_json::from_str(&out).expect("loop chain JSON should parse");

    assert_eq!(value["schema"], "maestro.loop_chain.v1");
    assert_eq!(value["chain"][0], "design");
    assert_eq!(value["current"], "work.act");
    assert_eq!(value["selected_unit"]["kind"], "task");
    assert_eq!(value["selected_unit"]["id"], task_id);
    assert_eq!(value["selected_feature_id"], "chain-json");
    assert_eq!(value["transition"]["from"], "work.act");
    assert_eq!(value["transition"]["to"], "design.choose");
    assert_eq!(
        value["transition"]["trigger"],
        "design_needed.scope_unclear"
    );
    assert!(
        value["return_conditions"]
            .as_array()
            .expect("return_conditions should be an array")
            .iter()
            .any(
                |condition| condition["key"] == "decision.all_blockers_locked"
                    && condition["satisfied"] == false
            ),
        "{value}"
    );
    let next = value["next"].as_array().expect("next should be an array");
    assert!(
        next.iter()
            .all(|entry| !entry.as_str().unwrap_or("").contains("<feature-id>")),
        "{value}"
    );
    assert!(
        next.iter()
            .any(|entry| entry.as_str() == Some("maestro feature reconcile chain-json")),
        "{value}"
    );
}

#[test]
fn loop_chain_matcher_selects_registered_transition_from_typed_facts() {
    let contract = loop_recipes::contract("work").expect("work recipe should validate");
    let facts = LoopChainFacts {
        selected_unit: Some(LoopChainSelectedUnit {
            kind: "feature".to_string(),
            id: "feature-x".to_string(),
            title: Some("Feature X".to_string()),
        }),
        current_recipe: "work".to_string(),
        current_phase: "act".to_string(),
        feature_status: Some("in_progress".to_string()),
        open_decisions: vec!["dec-new-scope".to_string()],
        handoff_fresh: false,
        ready_progress_rows: 2,
        ..LoopChainFacts::default()
    };

    let selected = loop_recipes::match_chain_transition(&facts, &contract)
        .expect("matcher should evaluate")
        .expect("work should transition to design");

    assert_eq!(selected.trigger, "design_needed.scope_unclear");
    assert_eq!(selected.from, "work.act");
    assert_eq!(selected.to, "design.choose");
    assert!(
        selected
            .return_conditions
            .iter()
            .any(
                |condition| condition.key == "decision.all_blockers_locked" && !condition.satisfied
            )
    );
}

#[test]
fn loop_chain_matcher_uses_recipe_order_for_multiple_matching_triggers() {
    let temp = TestTempDir::new("maestro-loop-transition-order");
    let body = include_str!("../embedded/loop-recipes/unattended.yml")
        .replace("id: unattended", "id: transition-order")
        .replace("unattended.", "transition-order.")
        .replace(
            "from: transition-order.continue",
            "from: transition-order.choose",
        );
    write_custom_recipe(temp.path(), "transition-order", &body);
    let contract = loop_recipes::custom_contract(
        &temp.path().join(".maestro/loop-recipes"),
        "transition-order",
    )
    .expect("custom transition-order recipe should validate");
    let facts = LoopChainFacts {
        selected_unit: Some(LoopChainSelectedUnit {
            kind: "task".to_string(),
            id: "task-ready".to_string(),
            title: None,
        }),
        current_recipe: "transition-order".to_string(),
        current_phase: "choose".to_string(),
        open_decisions: vec!["dec-scope".to_string()],
        handoff_fresh: false,
        ready_progress_rows: 1,
        ..LoopChainFacts::default()
    };

    let selected = loop_recipes::match_chain_transition(&facts, &contract)
        .expect("matcher should evaluate")
        .expect("custom recipe should select a transition");

    assert_eq!(
        selected.trigger, "work_ready.selected_unit",
        "recipe order should choose the first matching transition"
    );
    assert_eq!(selected.to, "work.perceive");
}

#[test]
fn loop_next_routes_ready_v2_parallel_wave_as_feature_fanout() {
    let temp = TestTempDir::new("maestro-loop-next-ready-v2-wave");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(
        temp.path(),
        &[
            "task",
            "setup",
            "--wave",
            "api=Build API",
            "--wave",
            "ui=Build UI",
            "--lane",
            "api=backend",
            "--lane",
            "ui=frontend",
        ],
    );
    let before = snapshot_dir(&temp.path().join(".maestro"));

    let out = stdout(temp.path(), &["loop", "next", "--json"]);
    let after = snapshot_dir(&temp.path().join(".maestro"));
    let value: Value = serde_json::from_str(&out).expect("loop next JSON should parse");

    assert_eq!(before, after, "loop next must not mutate .maestro");
    assert_eq!(value["recommended_recipe"], "feature-fanout");
    assert_eq!(value["recommended_status"], "feature_fanout");
    assert!(
        value["reason"].as_str().is_some_and(
            |reason| reason.contains("2 executable tasks") && reason.contains("2 lanes")
        ),
        "{value}"
    );
    assert!(
        value["inspect"]
            .as_array()
            .expect("inspect should be an array")
            .iter()
            .any(|entry| entry.as_str() == Some("maestro ready")),
        "{value}"
    );
}

#[test]
fn loop_next_compact_fanout_fails_closed_to_one_ready_inspection() {
    let temp = TestTempDir::new("maestro-loop-next-fanout-packet-json");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(
        temp.path(),
        &[
            "task",
            "setup",
            "--wave",
            "api=Build API",
            "--wave",
            "ui=Build UI",
            "--lane",
            "api=backend",
            "--lane",
            "ui=frontend",
        ],
    );
    let before = snapshot_dir(&temp.path().join(".maestro"));

    let out = stdout(temp.path(), &["loop", "next", "--compact", "--json"]);
    let after = snapshot_dir(&temp.path().join(".maestro"));
    let value: Value = serde_json::from_str(&out).expect("fanout packet JSON should parse");

    assert_eq!(before, after, "loop next --compact must stay read-only");
    assert!(out.ends_with('\n'));
    assert!(out.len() <= 512, "{} bytes: {out}", out.len());
    assert_eq!(value["schema"], "maestro.loop_action_card.v1");
    assert_eq!(value["status"], "blocked");
    assert_eq!(value["recipe"], "feature-fanout");
    assert_eq!(value["phase"], "perceive");
    assert_eq!(value["action"]["kind"], "inspect_fanout");
    assert_eq!(
        value["action"]["argv"],
        serde_json::json!(["maestro", "ready", "--json"])
    );
    assert_eq!(value["signal"]["kind"], "stop");
    assert!(value.get("selected_units").is_none(), "{value}");
}

#[test]
fn loop_next_compact_chain_text_uses_the_same_bounded_fanout_card() {
    let temp = TestTempDir::new("maestro-loop-next-fanout-packet-text");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(
        temp.path(),
        &[
            "task",
            "setup",
            "--wave",
            "api=Build API",
            "--wave",
            "ui=Build UI",
            "--lane",
            "api=backend",
            "--lane",
            "ui=frontend",
        ],
    );

    let out = stdout(temp.path(), &["loop", "next", "--compact", "--chain"]);

    assert_bounded_text(&out, 6);
    assert!(out.contains("feature-fanout/perceive"), "{out}");
    assert!(out.contains("do: maestro ready --json"), "{out}");
    assert!(out.contains("stop:"), "{out}");
    assert!(out.contains("full: maestro loop next --full"), "{out}");
    assert!(!out.contains("Build API"), "{out}");
    assert!(!out.contains("Build UI"), "{out}");
}

#[test]
fn loop_next_routes_ready_v2_serial_and_ship_gates() {
    let serial = TestTempDir::new("maestro-loop-next-ready-v2-serial-gate");
    init_git_marker(serial.path());
    stdout(serial.path(), &["init", "--yes"]);
    stdout(
        serial.path(),
        &[
            "task",
            "setup",
            "--task",
            "gate=Wire integration",
            "--lane",
            "gate=integration",
            "--gate",
            "gate=integration",
            "--atomic",
            "--reason",
            "single serial gate fixture",
        ],
    );

    let serial_out = stdout(serial.path(), &["loop", "next", "--json"]);
    let serial_value: Value =
        serde_json::from_str(&serial_out).expect("serial gate loop JSON should parse");
    assert_eq!(serial_value["recommended_recipe"], "work");
    assert!(
        serial_value["reason"].as_str().is_some_and(
            |reason| reason.contains("integration gate") && reason.contains("serially")
        ),
        "{serial_value}"
    );

    let ship = TestTempDir::new("maestro-loop-next-ready-v2-ship-gate");
    init_git_marker(ship.path());
    stdout(ship.path(), &["init", "--yes"]);
    let ship_setup = stdout(
        ship.path(),
        &[
            "task",
            "setup",
            "--task",
            "ship=Ship release",
            "--lane",
            "ship=ship",
            "--gate",
            "ship=ship",
            "--atomic",
            "--reason",
            "single ship gate fixture",
        ],
    );
    let ship_task = first_id(&ship_setup, "task-");

    let ship_out = stdout(ship.path(), &["loop", "next", "--json"]);
    let ship_value: Value =
        serde_json::from_str(&ship_out).expect("ship gate loop JSON should parse");
    assert_eq!(ship_value["recommended_recipe"], "ship");
    assert_eq!(ship_value["recommended_status"], "ship");
    assert!(
        ship_value["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("ship gate")),
        "{ship_value}"
    );

    let compact = stdout(ship.path(), &["loop", "next", "--compact", "--json"]);
    let compact_value: Value =
        serde_json::from_str(&compact).expect("ship safety card should parse");
    assert!(compact.len() <= 512, "{} bytes: {compact}", compact.len());
    assert_eq!(compact_value["status"], "blocked");
    assert_eq!(compact_value["recipe"], "ship");
    assert_eq!(compact_value["signal"]["kind"], "stop");
    assert_eq!(compact_value["signal"]["text"], "proof_ready");
    assert!(compact_value["omitted"].as_u64().unwrap_or_default() >= 2);
    assert_eq!(
        compact_value["action"]["argv"],
        serde_json::json!(["maestro", "task", "show", ship_task])
    );
}

#[test]
fn loop_next_json_includes_scoped_memory_preflight_without_mutating_store() {
    let temp = TestTempDir::new("maestro-loop-next-memory-preflight");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    let task_id = stdout(
        temp.path(),
        &["task", "add", "--id-only", "Implement memory router"],
    );
    let task_id = task_id.trim();
    let suggestion = stdout(
        temp.path(),
        &[
            "memory",
            "suggest",
            "create",
            "--source-ref",
            "run_event:run-1",
            "--signal-type",
            "failure",
            "--summary",
            "Remember loop routing failure",
            "--scope-kind",
            "task",
            "--scope-ref",
            task_id,
        ],
    );
    let suggestion_id = first_id(&suggestion, "msug-");
    let before = snapshot_dir(&temp.path().join(".maestro"));

    let out = stdout(temp.path(), &["loop", "next", "--json"]);
    let after = snapshot_dir(&temp.path().join(".maestro"));
    let value: Value = serde_json::from_str(&out).expect("loop next JSON should parse");

    assert_eq!(
        before, after,
        "loop next memory preflight must stay read-only"
    );
    assert_eq!(value["recommended_recipe"], "work");
    assert!(
        value["memory_hits"]
            .as_array()
            .expect("memory_hits should be an array")
            .iter()
            .any(|hit| hit["id"] == suggestion_id && hit["kind"] == "prior_failure"),
        "{value}"
    );
    assert!(
        value["constraints"]
            .as_array()
            .expect("constraints should be an array")
            .iter()
            .any(|constraint| constraint["id"] == "memory_relevance"
                && constraint["status"] == "pass"),
        "{value}"
    );
    assert!(
        value["constraints"]
            .as_array()
            .expect("constraints should be an array")
            .iter()
            .any(|constraint| constraint["id"] == "prior_failure_risk"
                && constraint["status"] == "warn"),
        "{value}"
    );
}

#[test]
fn loop_outcome_appends_run_event_and_routes_failure_class() {
    let temp = TestTempDir::new("maestro-loop-outcome-event");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    let task_id = stdout(temp.path(), &["task", "add", "--id-only", "Repair proof"]);
    let task_id = task_id.trim();

    let out = stdout(
        temp.path(),
        &[
            "loop",
            "outcome",
            "--recipe",
            "work",
            "--phase",
            "observe",
            "--selected-unit",
            task_id,
            "--constraint",
            "proof_ready",
            "--proof-result",
            "failed",
            "--failure-class",
            "proof_gap",
            "--blocker-class",
            "proof",
            "--retry-count",
            "1",
            "--duration-ms",
            "42",
            "--learning-candidate",
            "Similar proof gap should route to repair",
            "--source-ref",
            &format!("task:{task_id}"),
            "--run",
            "loop-outcome-test",
            "--json",
        ],
    );
    let value: Value = serde_json::from_str(&out).expect("loop outcome JSON should parse");
    assert_eq!(value["event_type"], "loop_outcome");
    assert_eq!(value["schema_version"], "maestro.loop_outcome.v1");
    assert_eq!(value["route"]["action"], "repair");
    assert_eq!(value["route"]["recipe"], "work");

    let second = stdout(
        temp.path(),
        &[
            "loop",
            "outcome",
            "--recipe",
            "work",
            "--phase",
            "observe",
            "--selected-unit",
            task_id,
            "--failure-class",
            "test_failure",
            "--run",
            "loop-outcome-test",
        ],
    );
    assert!(second.contains("recorded loop_outcome event"), "{second}");

    let events = fs::read_to_string(
        temp.path()
            .join(".maestro/runs/loop-outcome-test/events.jsonl"),
    )
    .expect("loop outcome event log should be readable");
    let rows = events.lines().collect::<Vec<_>>();
    assert_eq!(rows.len(), 2, "loop outcome appends one JSONL row per call");
    let first: Value = serde_json::from_str(rows[0]).expect("first event should parse");
    assert_eq!(first["event_type"], "loop_outcome");
    assert_eq!(first["recipe"], "work");
    assert_eq!(first["phase"], "observe");
    assert_eq!(first["selected_unit"], task_id);
    assert_eq!(first["constraints"][0], "proof_ready");
    assert_eq!(first["proof_result"], "failed");
    assert_eq!(first["failure_class"], "proof_gap");
    assert_eq!(first["blocker_class"], "proof");
    assert_eq!(first["retry_count"], 1);
    assert_eq!(first["duration_ms"], 42);
    assert_eq!(
        first["learning_candidate"],
        "Similar proof gap should route to repair"
    );
    assert_eq!(first["source_refs"][0]["kind"], "task");
    assert_eq!(first["source_refs"][0]["id"], task_id);

    let session = stdout(
        temp.path(),
        &["session", "show", "loop-outcome-test", "--json"],
    );
    let session: Value = serde_json::from_str(&session).expect("session JSON should parse");
    assert_eq!(session["lifecycle"]["counts"]["loop_outcome"], 2);
}

#[test]
fn loop_chain_facts_do_not_guess_feature_freshness_from_status() {
    let report = loop_recipes::route_next(loop_recipes::LoopRouterInput {
        repo: "/repo".to_string(),
        initialized: true,
        features: vec![loop_recipes::LoopFeatureInput {
            id: "feature-router".to_string(),
            title: "Feature Router".to_string(),
            status: "in_progress".to_string(),
            total_tasks: 0,
            verified_tasks: 0,
            open_questions: 0,
            handoff_fresh: None,
            reconcile_current: None,
        }],
        ..loop_recipes::LoopRouterInput::default()
    })
    .expect("router should recommend design for stale ungrounded feature");
    let facts = loop_recipes::chain_facts_from_router(
        &loop_recipes::LoopRouterInput {
            repo: "/repo".to_string(),
            initialized: true,
            features: vec![loop_recipes::LoopFeatureInput {
                id: "feature-router".to_string(),
                title: "Feature Router".to_string(),
                status: "in_progress".to_string(),
                total_tasks: 0,
                verified_tasks: 0,
                open_questions: 0,
                handoff_fresh: None,
                reconcile_current: None,
            }],
            ..loop_recipes::LoopRouterInput::default()
        },
        &report,
    );

    assert!(!facts.handoff_fresh, "{facts:?}");
    assert!(!facts.feature_reconcile_current, "{facts:?}");
}

#[test]
fn loop_outcome_records_structured_transition_receipt() {
    let temp = TestTempDir::new("maestro-loop-outcome-transition-receipt");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);

    let out = stdout(
        temp.path(),
        &[
            "loop",
            "outcome",
            "--recipe",
            "work",
            "--phase",
            "act",
            "--selected-unit",
            "feature-x",
            "--transition-to",
            "design.choose",
            "--transition-reason",
            "acceptance mismatch found during implementation",
            "--trigger",
            "design_needed.scope_unclear",
            "--return-condition",
            "decision.all_blockers_locked",
            "--return-condition",
            "feature.reconcile_current",
            "--return-condition",
            "feature.handoff_fresh",
            "--evidence-ref",
            "feature:feature-x",
            "--evidence-ref",
            "decision:dec-new-scope",
            "--run",
            "loop-transition-test",
            "--json",
        ],
    );
    let value: Value =
        serde_json::from_str(&out).expect("loop transition receipt JSON should parse");

    assert_eq!(value["event_type"], "loop_outcome");
    assert_eq!(value["transition_to"], "design.choose");
    assert_eq!(
        value["transition_reason"],
        "acceptance mismatch found during implementation"
    );
    assert_eq!(value["trigger"], "design_needed.scope_unclear");
    assert_eq!(value["return_condition"][0], "decision.all_blockers_locked");
    assert_eq!(value["return_condition"][1], "feature.reconcile_current");
    assert_eq!(value["return_condition"][2], "feature.handoff_fresh");
    assert_eq!(value["evidence_refs"][0]["kind"], "feature");
    assert_eq!(value["evidence_refs"][0]["id"], "feature-x");
    assert_eq!(value["failure_class"], "");
    assert_eq!(value["route"]["action"], "record");

    let events = fs::read_to_string(
        temp.path()
            .join(".maestro/runs/loop-transition-test/events.jsonl"),
    )
    .expect("loop transition event log should be readable");
    let stored: Value = serde_json::from_str(events.lines().next().unwrap())
        .expect("stored transition event should parse");
    assert_eq!(stored["transition_to"], "design.choose");
    assert_eq!(stored["evidence_refs"][1]["kind"], "decision");
}

#[test]
fn loop_outcome_rejects_unknown_transition_trigger_key() {
    let temp = TestTempDir::new("maestro-loop-outcome-transition-trigger-invalid");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);

    let error = stderr(
        temp.path(),
        &[
            "loop",
            "outcome",
            "--recipe",
            "work",
            "--phase",
            "act",
            "--selected-unit",
            "feature-x",
            "--transition-to",
            "design.choose",
            "--transition-reason",
            "bad trigger",
            "--trigger",
            "missing.trigger",
            "--return-condition",
            "feature.reconcile_current",
            "--return-condition",
            "feature.handoff_fresh",
            "--evidence-ref",
            "feature:feature-x",
            "--run",
            "loop-transition-invalid",
        ],
    );

    assert!(
        error.contains("unknown trigger key missing.trigger"),
        "{error}"
    );
}

#[test]
fn loop_outcome_rejects_transition_receipt_without_matching_edge() {
    let temp = TestTempDir::new("maestro-loop-outcome-transition-edge-invalid");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);

    let error = stderr(
        temp.path(),
        &[
            "loop",
            "outcome",
            "--recipe",
            "work",
            "--phase",
            "act",
            "--selected-unit",
            "feature-x",
            "--transition-to",
            "design.choose",
            "--transition-reason",
            "mismatched edge",
            "--trigger",
            "work_ready.selected_unit",
            "--return-condition",
            "work.accepted_or_dry",
            "--evidence-ref",
            "feature:feature-x",
            "--run",
            "loop-transition-edge-invalid",
        ],
    );

    assert!(
        error.contains("transition receipt does not match a registered edge"),
        "{error}"
    );
}

#[test]
fn loop_trace_reads_card_scoped_transition_receipts() {
    let temp = TestTempDir::new("maestro-loop-trace-receipts");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(temp.path(), &["feature", "new", "Feature X"]);
    stdout(
        temp.path(),
        &[
            "loop",
            "outcome",
            "--recipe",
            "work",
            "--phase",
            "act",
            "--selected-unit",
            "feature-x",
            "--transition-to",
            "design.choose",
            "--transition-reason",
            "acceptance mismatch found during implementation",
            "--trigger",
            "design_needed.scope_unclear",
            "--return-condition",
            "decision.all_blockers_locked",
            "--return-condition",
            "feature.reconcile_current",
            "--return-condition",
            "feature.handoff_fresh",
            "--evidence-ref",
            "feature:feature-x",
            "--run",
            "trace-receipt",
        ],
    );
    let before = snapshot_dir(&temp.path().join(".maestro"));

    let out = stdout(temp.path(), &["loop", "trace", "feature-x"]);
    let after = snapshot_dir(&temp.path().join(".maestro"));

    assert_eq!(before, after, "loop trace must stay read-only");
    assert!(out.contains("chain history: 1 recent events"), "{out}");
    assert!(out.contains("- work.act -> design.choose"), "{out}");
    assert!(
        out.contains("trigger: design_needed.scope_unclear"),
        "{out}"
    );
    assert!(out.contains("receipt: run:trace-receipt"), "{out}");
    assert!(out.contains("- decision.all_blockers_locked"), "{out}");

    let json = stdout(temp.path(), &["loop", "trace", "feature-x", "--json"]);
    let value: Value = serde_json::from_str(&json).expect("loop trace JSON should parse");
    assert_eq!(value["schema"], "maestro.loop_trace.v1");
    assert_eq!(value["card"], "feature-x");
    assert_eq!(value["events"][0]["transition_to"], "design.choose");
    assert_eq!(value["events"][0]["receipt"], "run:trace-receipt");
}

#[test]
fn loop_trace_defaults_to_recent_window_and_all_widens() {
    let temp = TestTempDir::new("maestro-loop-trace-window");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(temp.path(), &["feature", "new", "Feature X"]);
    for index in 0..6 {
        stdout(
            temp.path(),
            &[
                "loop",
                "outcome",
                "--recipe",
                "work",
                "--phase",
                "act",
                "--selected-unit",
                "feature-x",
                "--transition-to",
                "design.choose",
                "--transition-reason",
                "acceptance mismatch found during implementation",
                "--trigger",
                "design_needed.scope_unclear",
                "--return-condition",
                "decision.all_blockers_locked",
                "--return-condition",
                "feature.reconcile_current",
                "--return-condition",
                "feature.handoff_fresh",
                "--evidence-ref",
                "feature:feature-x",
                "--run",
                &format!("trace-window-{index}"),
            ],
        );
    }

    let recent = stdout(temp.path(), &["loop", "trace", "feature-x"]);
    assert!(
        recent.contains("chain history: 5 recent events"),
        "{recent}"
    );
    assert!(
        recent.contains("hidden: 1 older events; use --all"),
        "{recent}"
    );

    let all = stdout(temp.path(), &["loop", "trace", "feature-x", "--all"]);
    assert!(all.contains("chain history: 6 events"), "{all}");
    assert!(!all.contains("hidden:"), "{all}");
}

#[test]
fn loop_trace_recent_window_uses_event_timestamp_order() {
    let temp = TestTempDir::new("maestro-loop-trace-timestamp-window");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    stdout(temp.path(), &["feature", "new", "Feature X"]);

    for (session, ts) in [
        ("z-oldest", "2026-07-04T00:00:00.000Z"),
        ("a-newest", "2026-07-04T00:05:00.000Z"),
        ("b-second", "2026-07-04T00:01:00.000Z"),
        ("c-third", "2026-07-04T00:02:00.000Z"),
        ("d-fourth", "2026-07-04T00:03:00.000Z"),
        ("e-fifth", "2026-07-04T00:04:00.000Z"),
    ] {
        seed_run(
            temp.path(),
            session,
            &[format!(
                r#"{{"event_type":"loop_outcome","ts":"{ts}","recipe":"work","phase":"act","selected_unit":"feature-x","transition_to":"design.choose","transition_reason":"timestamp order","trigger":"design_needed.scope_unclear","return_condition":["decision.all_blockers_locked","feature.reconcile_current","feature.handoff_fresh"],"evidence_refs":[{{"kind":"feature","id":"feature-x"}}]}}"#
            )],
        );
    }

    let out = stdout(temp.path(), &["loop", "trace", "feature-x", "--json"]);
    let value: Value = serde_json::from_str(&out).expect("loop trace JSON should parse");
    assert_eq!(value["hidden"], 1, "{value}");
    let receipts = value["events"]
        .as_array()
        .expect("events should be an array")
        .iter()
        .map(|event| event["receipt"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();

    assert_eq!(
        receipts,
        vec![
            "run:b-second",
            "run:c-third",
            "run:d-fourth",
            "run:e-fifth",
            "run:a-newest",
        ],
        "{value}"
    );
}

#[test]
fn loop_improve_emits_typed_read_only_proposals_from_outcomes() {
    let temp = TestTempDir::new("maestro-loop-improve-proposals");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    let task_id = stdout(temp.path(), &["task", "add", "--id-only", "Improve loop"]);
    let task_id = task_id.trim();

    for (run_id, failure_class) in [
        ("loop-proof-a", "proof_gap"),
        ("loop-proof-b", "proof_gap"),
        ("loop-test-a", "test_failure"),
        ("loop-test-b", "test_failure"),
        ("loop-scope-a", "scope_ambiguity"),
        ("loop-scope-b", "scope_ambiguity"),
        ("loop-repeat-a", "repeated_failure"),
        ("loop-repeat-b", "repeated_failure"),
    ] {
        stdout(
            temp.path(),
            &[
                "loop",
                "outcome",
                "--recipe",
                "work",
                "--phase",
                "observe",
                "--selected-unit",
                task_id,
                "--failure-class",
                failure_class,
                "--source-ref",
                &format!("run_event:{run_id}"),
                "--run",
                run_id,
            ],
        );
    }

    let before = snapshot_dir(&temp.path().join(".maestro"));
    let out = stdout(temp.path(), &["loop", "improve", "--json"]);
    let after = snapshot_dir(&temp.path().join(".maestro"));
    assert_eq!(
        before, after,
        "loop improve planning must not mutate .maestro"
    );
    let value: Value = serde_json::from_str(&out).expect("loop improve JSON should parse");
    assert_eq!(value["schema"], "maestro.loop_improve.v1");
    assert_eq!(value["read_only"], true);
    let proposals = value["proposals"]
        .as_array()
        .expect("proposals should be an array");
    for kind in [
        "memory_suggestion",
        "harness_friction",
        "recipe_edit_proposal",
        "skill_update_proposal",
        "qa_guard",
        "proof_guard",
    ] {
        assert!(
            proposals.iter().any(|proposal| proposal["kind"] == kind),
            "missing {kind} proposal in {value}"
        );
    }
    for proposal in proposals {
        assert!(
            proposal["source_refs"]
                .as_array()
                .expect("source_refs should be an array")
                .len()
                >= 2,
            "proposal must carry repeated sourced outcomes: {proposal}"
        );
        assert!(
            proposal["dry_plan"]
                .as_array()
                .expect("dry_plan should be an array")
                .iter()
                .all(|step| step.as_str().is_some_and(|step| !step.trim().is_empty())),
            "proposal must carry a dry plan: {proposal}"
        );
        assert!(
            proposal["apply_command"]
                .as_str()
                .is_some_and(|command| command.starts_with("maestro ")),
            "proposal must name an explicit apply command: {proposal}"
        );
        if proposal["kind"] == "recipe_edit_proposal" || proposal["kind"] == "skill_update_proposal"
        {
            assert!(
                proposal["outcome_count"].as_u64().unwrap_or_default() >= 2
                    || proposal["severity"] == "high",
                "recipe/skill proposals need repeated outcomes or high severity: {proposal}"
            );
        }
    }
}

#[test]
fn loop_next_human_output_names_score_phase_and_blocked_alternatives() {
    let temp = TestTempDir::new("maestro-loop-next-scored-human");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    let task_id = stdout(
        temp.path(),
        &["task", "add", "--id-only", "Implement scored router"],
    );
    let task_id = task_id.trim();
    seed_run(temp.path(), "me", &[ownership_event("me", task_id, 1)]);
    seed_run(
        temp.path(),
        "other",
        &[ownership_event("other", task_id, 1)],
    );

    let output = maestro_with_env(
        temp.path(),
        &["loop", "next", "--full"],
        &[("MAESTRO_SESSION_ID", "me")],
    );
    assert!(
        output.status.success(),
        "maestro loop next failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let out = String::from_utf8(output.stdout).expect("stdout should be utf8");

    assert!(out.contains("recipe: conflict-handoff"), "{out}");
    assert!(out.contains("score: "), "{out}");
    assert!(out.contains("recommended_phase: perceive"), "{out}");
    assert!(out.contains("why_not:"), "{out}");
    assert!(out.contains("- work blocked_by: conflict_risk"), "{out}");
}

#[test]
fn loop_show_renders_structured_contracts_from_yaml() {
    let temp = TestTempDir::new("maestro-loop-show-contract");
    let out = stdout(temp.path(), &["loop", "show", "unattended", "--full"]);

    assert!(out.contains("# Unattended loop"), "{out}");
    assert!(out.contains("schema_version: maestro.recipe.v2"), "{out}");
    assert!(out.contains("## Router Metadata"), "{out}");
    assert!(out.contains("## Authority Scope"), "{out}");
    assert!(out.contains("## Autonomy"), "{out}");
    assert!(
        out.contains("perceive -> choose -> act -> observe -> learn -> continue"),
        "{out}"
    );
    assert!(out.contains("## Custom Recipe Policy"), "{out}");
    assert!(out.contains("Work Lease helper contract"), "{out}");
    assert!(out.contains("selected_unit"), "{out}");
    assert!(out.contains("maestro status --json"), "{out}");
    assert!(
        out.contains("returned inspect or status handle cannot be read"),
        "{out}"
    );
    assert!(out.contains("reconcile_handles"), "{out}");
    assert!(out.contains("run report command"), "{out}");
    assert!(out.contains("Forbidden verbs"), "{out}");
    assert!(out.contains("worker launcher"), "{out}");
}

#[test]
fn loop_show_renders_progress_recipe() {
    let temp = TestTempDir::new("maestro-loop-show-progress");
    let out = stdout(temp.path(), &["loop", "show", "progress", "--full"]);

    assert!(out.contains("# Progress loop"), "{out}");
    assert!(out.contains("maestro task setup --task"), "{out}");
    assert!(out.contains("maestro task done <ref> --proof"), "{out}");
    assert!(out.contains("escalate to full card"), "{out}");
    assert!(
        out.contains("perceive -> choose -> act -> observe -> learn -> continue"),
        "{out}"
    );
}

#[test]
fn loop_show_defaults_to_bounded_recipe_card_and_full_is_explicit() {
    let temp = TestTempDir::new("maestro-loop-show-compact-json");
    let default = stdout(temp.path(), &["loop", "show", "work"]);
    let compact = stdout(temp.path(), &["loop", "show", "work", "--compact"]);

    assert_eq!(default, compact);
    assert_bounded_text(&default, 6);
    assert_eq!(
        default,
        "work · implementation\nenter: maestro loop next --compact\nfull: maestro loop show work --full\n"
    );

    let full = stdout(temp.path(), &["loop", "show", "work", "--full"]);
    assert!(full.contains("# Work loop"), "{full}");
    assert!(full.contains("## Progress Tasks"), "{full}");
    assert!(full.contains("contract_hash: sha256:"), "{full}");
}

#[test]
fn loop_show_compact_json_is_bounded_and_full_json_is_resolved() {
    let temp = TestTempDir::new("maestro-loop-show-compact-json");
    let out = stdout(
        temp.path(),
        &["loop", "show", "work", "--compact", "--json"],
    );
    let value: Value = serde_json::from_str(&out).expect("compact recipe JSON should parse");

    assert!(out.len() <= 512, "{} bytes: {out}", out.len());
    assert_eq!(value["schema"], "maestro.loop_recipe_card.v1");
    assert_eq!(value["recipe"], "work");
    assert_eq!(value["kind"], "implementation");
    assert_eq!(value["enter"], "maestro loop next --compact");
    assert_eq!(value["full"], "maestro loop show work --full");

    let full = stdout(temp.path(), &["loop", "show", "work", "--full", "--json"]);
    let full: Value = serde_json::from_str(&full).expect("resolved recipe JSON should parse");
    assert_eq!(full["schema"], "maestro.resolved_recipe.v1");
    assert_eq!(full["contract"]["id"], "work");
    assert_eq!(
        full["contract_hash_schema"],
        "maestro.recipe-contract-hash.v1"
    );
}

#[test]
fn loop_next_compact_json_routes_ready_task_without_mutating_store() {
    let temp = TestTempDir::new("maestro-loop-next-compact-json");
    init_git_repo(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    let task_id = stdout(
        temp.path(),
        &["task", "add", "--id-only", "Implement compact packet"],
    );
    let task_id = task_id.trim();
    let before = snapshot_dir(&temp.path().join(".maestro"));

    let out = stdout(temp.path(), &["loop", "next", "--compact", "--json"]);
    let after = snapshot_dir(&temp.path().join(".maestro"));
    let value: Value = serde_json::from_str(&out).expect("compact next JSON should parse");

    assert_eq!(before, after, "loop next --compact must stay read-only");
    assert!(out.len() <= 512, "{} bytes: {out}", out.len());
    assert_eq!(value["schema"], "maestro.loop_action_card.v1");
    assert_eq!(value["status"], "ready");
    assert_eq!(value["recipe"], "work");
    assert_eq!(value["phase"], "perceive");
    assert_eq!(value["task"]["id"], task_id);
    assert_eq!(value["task"]["state"], "ready");
    assert_eq!(value["action"]["kind"], "claim_task");
    assert_eq!(
        value["action"]["argv"],
        serde_json::json!(["maestro", "task", "claim", task_id])
    );
    assert!(value["action"].get("argv_template").is_none(), "{value}");
    assert_eq!(value["full"], "maestro loop next --full");

    let default = stdout(temp.path(), &["loop", "next"]);
    let compact = stdout(temp.path(), &["loop", "next", "--compact"]);
    assert_eq!(default, compact);
    assert_bounded_text(&default, 6);
    assert!(
        default.contains(&format!("work/perceive · {task_id} [ready]")),
        "{default}"
    );
    assert!(
        default.contains(&format!("do: maestro task claim {task_id}")),
        "{default}"
    );
    assert!(
        default.contains("full: maestro loop next --full"),
        "{default}"
    );
}

#[test]
fn loop_next_compact_grounds_route_and_action_in_the_same_mixed_state_task() {
    let temp = TestTempDir::new("maestro-loop-next-compact-mixed-task-state");
    init_git_repo(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    let verify_id = stdout(
        temp.path(),
        &[
            "task",
            "create",
            "Recover proof before continuing",
            "--check",
            "observable proof exists",
            "--id-only",
        ],
    );
    let verify_id = verify_id.trim();
    stdout(temp.path(), &["task", "explore", verify_id]);
    stdout(temp.path(), &["task", "accept", verify_id]);
    stdout(temp.path(), &["task", "claim", verify_id]);
    stderr(
        temp.path(),
        &[
            "task",
            "complete",
            verify_id,
            "--summary",
            "done",
            "--claim",
            "observable proof exists",
        ],
    );
    let active_id = stdout(
        temp.path(),
        &["task", "add", "--id-only", "Continue active implementation"],
    );
    let active_id = active_id.trim();
    stdout(temp.path(), &["task", "claim", active_id]);

    let out = stdout(temp.path(), &["loop", "next", "--compact", "--json"]);
    let value: Value = serde_json::from_str(&out).expect("mixed-state card JSON should parse");

    assert_eq!(value["recipe"], "work");
    assert_eq!(value["phase"], "observe");
    assert_eq!(value["task"]["id"], verify_id);
    assert_eq!(value["task"]["state"], "needs_verification");
    assert_eq!(value["action"]["kind"], "proof_recovery");
    assert_eq!(
        value["action"]["argv"],
        serde_json::json!(["maestro", "task", "verify", verify_id])
    );
}

#[test]
fn loop_next_compact_uses_the_unknown_safety_constraints_exact_inspection() {
    let temp = TestTempDir::new("maestro-loop-next-compact-unknown-safety");
    stdout(temp.path(), &["init", "--yes"]);
    stdout(
        temp.path(),
        &["task", "add", "--id-only", "Inspect unknown git safety"],
    );

    let out = stdout(temp.path(), &["loop", "next", "--compact", "--json"]);
    let value: Value = serde_json::from_str(&out).expect("unknown-safety card JSON should parse");

    assert_eq!(value["status"], "blocked");
    assert_eq!(value["signal"]["kind"], "stop");
    assert_eq!(value["signal"]["text"], "dirty_tree_risk");
    assert_eq!(value["action"]["kind"], "inspect_safety");
    assert_eq!(
        value["action"]["argv"],
        serde_json::json!(["git", "status", "--short", "--branch"])
    );
}

#[test]
fn loop_next_progress_replay_returns_the_exact_done_template_without_help_probes() {
    let temp = TestTempDir::new("maestro-loop-next-progress-replay");
    init_git_repo(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    let setup = stdout(
        temp.path(),
        &[
            "task",
            "setup",
            "--task",
            "Implement compact replay",
            "--task",
            "Verify compact replay",
            "--start",
        ],
    );
    let first_task = first_id(&setup, "task-");
    let before = snapshot_dir(&temp.path().join(".maestro"));
    let mut invocations = Vec::new();

    invocations.push(vec!["loop", "next", "--compact", "--json"]);
    let first = stdout(temp.path(), &["loop", "next", "--compact", "--json"]);
    let after = snapshot_dir(&temp.path().join(".maestro"));
    assert_eq!(before, after, "the compact router must remain read-only");
    let first: Value = serde_json::from_str(&first).expect("first action card should parse");
    assert_eq!(first["task"]["id"], first_task);
    assert_eq!(first["task"]["progress"], true);
    assert_eq!(first["action"]["kind"], "done_task");
    assert_eq!(
        first["action"]["argv_template"],
        serde_json::json!([
            "maestro",
            "task",
            "done",
            first_task,
            "--proof",
            "<evidence>"
        ])
    );

    invocations.push(vec![
        "task",
        "done",
        first_task.as_str(),
        "--proof",
        "observed",
    ]);
    stdout(
        temp.path(),
        &["task", "done", &first_task, "--proof", "observed"],
    );
    invocations.push(vec!["loop", "next", "--compact", "--json"]);
    let second = stdout(temp.path(), &["loop", "next", "--compact", "--json"]);
    let second: Value = serde_json::from_str(&second).expect("second action card should parse");
    let second_task = second["task"]["id"]
        .as_str()
        .expect("second Progress task id should be grounded")
        .to_string();
    assert_eq!(second["action"]["kind"], "claim_task");
    assert_eq!(
        second["action"]["argv"],
        serde_json::json!(["maestro", "task", "claim", second_task])
    );

    invocations.push(vec!["task", "claim", second_task.as_str()]);
    stdout(temp.path(), &["task", "claim", &second_task]);
    invocations.push(vec!["loop", "next", "--compact", "--json"]);
    let third = stdout(temp.path(), &["loop", "next", "--compact", "--json"]);
    let third: Value = serde_json::from_str(&third).expect("third action card should parse");
    assert_eq!(third["task"]["id"], second_task);
    assert_eq!(third["action"]["kind"], "done_task");
    assert_eq!(
        third["action"]["argv_template"],
        serde_json::json!([
            "maestro",
            "task",
            "done",
            second_task,
            "--proof",
            "<evidence>"
        ])
    );
    assert_eq!(
        invocations
            .iter()
            .filter(|args| args.starts_with(&["loop", "next"]))
            .count(),
        3,
        "each observed state should use one router call"
    );
    assert!(
        invocations.iter().flatten().all(|arg| *arg != "--help"),
        "replay must not probe help: {invocations:?}"
    );
}

#[test]
fn loop_next_compact_missing_repo_fails_closed_to_one_inspection() {
    let temp = TestTempDir::new("maestro-loop-next-compact-uncertain");
    let out = stdout(temp.path(), &["loop", "next"]);

    assert_bounded_text(&out, 6);
    assert!(out.contains("intake-triage/perceive"), "{out}");
    assert!(out.contains("do: maestro status --json"), "{out}");
    assert!(out.contains("stop:"), "{out}");
    assert!(!temp.path().join(".maestro").exists());

    let json = stdout(temp.path(), &["loop", "next", "--compact", "--json"]);
    let value: Value = serde_json::from_str(&json).expect("uncertain card JSON should parse");
    assert!(json.len() <= 512, "{} bytes: {json}", json.len());
    assert_eq!(value["schema"], "maestro.loop_action_card.v1");
    assert_eq!(value["status"], "uncertain");
    assert_eq!(
        value["action"]["argv"],
        serde_json::json!(["maestro", "status", "--json"])
    );
}

#[test]
fn loop_next_compact_blocked_task_exposes_only_exact_inspection() {
    let temp = TestTempDir::new("maestro-loop-next-compact-blocked");
    init_git_marker(temp.path());
    stdout(temp.path(), &["init", "--yes"]);
    let blocker = stdout(
        temp.path(),
        &["task", "add", "--id-only", "Unblock dependency"],
    );
    let blocker = blocker.trim();
    let target = stdout(
        temp.path(),
        &["task", "add", "--id-only", "Blocked compact task"],
    );
    let target = target.trim();
    stdout(
        temp.path(),
        &[
            "task",
            "block",
            target,
            "--reason",
            "dependency is open",
            "--by",
            blocker,
        ],
    );

    let output = maestro_with_env(
        temp.path(),
        &["loop", "next", "--compact", "--json"],
        &[("MAESTRO_CURRENT_TASK", target)],
    );
    assert!(
        output.status.success(),
        "blocked compact route failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = String::from_utf8(output.stdout).expect("blocked card stdout should be UTF-8");
    let value: Value = serde_json::from_str(&json).expect("blocked card JSON should parse");
    assert!(json.len() <= 512, "{} bytes: {json}", json.len());
    assert_eq!(value["status"], "blocked");
    assert_eq!(value["task"]["id"], target);
    assert_eq!(value["action"]["kind"], "inspect_blocker");
    assert_eq!(
        value["action"]["argv"],
        serde_json::json!(["maestro", "task", "show", target])
    );
    assert!(value["action"].get("argv_template").is_none(), "{value}");
}

#[test]
fn loop_show_renders_migrated_orchestration_recipe_from_yaml() {
    let temp = TestTempDir::new("maestro-loop-show-migrated");
    let out = stdout(temp.path(), &["loop", "show", "conflict-handoff", "--full"]);

    assert!(out.contains("# Conflict handoff"), "{out}");
    assert!(out.contains("git worktree add"), "{out}");
    assert!(out.contains("schema_version: maestro.recipe.v2"), "{out}");
}

#[test]
fn loop_show_renders_synthesize_recipe() {
    let temp = TestTempDir::new("maestro-loop-show-synthesize");
    let out = stdout(temp.path(), &["loop", "show", "synthesize", "--full"]);

    assert!(out.contains("# Synthesize loop"), "{out}");
    assert!(out.contains("maestro synthesize claim"), "{out}");
    assert!(out.contains("maestro worktree cleanup"), "{out}");
    assert!(out.contains("one lane at a time"), "{out}");
}

#[test]
fn loop_rejects_old_renamed_recipe_ids() {
    let temp = TestTempDir::new("maestro-loop-old-aliases");
    for legacy in [
        "feature-fan-out",
        "adversarial-fan-out",
        "generate-and-filter",
        "unattended-loop",
    ] {
        let error = stderr(temp.path(), &["loop", "show", legacy]);
        assert!(error.contains("unknown loop recipe"), "{legacy}: {error}");
        assert!(error.contains("feature-fanout"), "{legacy}: {error}");
        assert!(!error.contains("feature-fan-out,"), "{legacy}: {error}");
    }
}

#[test]
fn loop_lists_shows_and_validates_project_custom_recipes() {
    let temp = TestTempDir::new("maestro-loop-custom");
    write_custom_recipe(temp.path(), "brief", CUSTOM_RECIPE);

    let index = stdout(temp.path(), &["loop"]);
    assert!(index.contains("## Project Custom Recipes"), "{index}");
    assert!(
        index
            .contains("brief  --  Handle one bounded support brief through current Maestro cards."),
        "{index}"
    );

    let shown = stdout(temp.path(), &["loop", "show", "brief", "--full"]);
    assert!(shown.contains("# Support brief loop"), "{shown}");
    assert!(
        shown.contains("schema_version: maestro.recipe.v2"),
        "{shown}"
    );
    assert!(
        shown.contains("perceive -> choose -> act -> observe -> learn -> continue"),
        "{shown}"
    );
    assert!(shown.contains("## Progress Tasks"), "{shown}");
    assert!(shown.contains("brief-anchor"), "{shown}");
    assert!(shown.contains("done_check"), "{shown}");
    assert!(shown.contains("## Custom Recipe Policy"), "{shown}");

    let validated = stdout(temp.path(), &["loop", "validate", "brief"]);
    assert!(
        validated.contains("valid project custom loop recipe: brief"),
        "{validated}"
    );
}

#[test]
fn mixed_custom_catalog_isolates_invalid_sources_and_keeps_valid_routes_visible() {
    let temp = TestTempDir::new("maestro-loop-custom-mixed-catalog");
    write_custom_recipe(temp.path(), "brief", CUSTOM_RECIPE);
    write_custom_recipe(
        temp.path(),
        "profiled",
        &profiled_custom_recipe()
            .replace("id: brief", "id: profiled")
            .replace("brief.", "profiled."),
    );
    write_custom_recipe(
        temp.path(),
        "bad-profile",
        &profiled_custom_recipe()
            .replace("id: brief", "id: bad-profile")
            .replace(
                "maestro.standard-six-phase.v1",
                "maestro.standard-six-phase.v2",
            ),
    );
    write_custom_recipe(
        temp.path(),
        "bad-override",
        &profiled_custom_recipe()
            .replace("id: brief", "id: bad-override")
            .replace(
                "title: Anchor support brief scope",
                "title: Anchor support brief scope\n    phase: act",
            ),
    );
    write_custom_recipe(
        temp.path(),
        "bad-effective",
        "schema_version: maestro.recipe.v2\nid: bad-effective\n",
    );
    let dependent = include_str!("../embedded/loop-recipes/unattended.yml")
        .replace("id: unattended", "id: depends-invalid")
        .replace("unattended.", "depends-invalid.")
        .replacen("to: work.perceive", "to: bad-effective.perceive", 1);
    write_custom_recipe(temp.path(), "depends-invalid", &dependent);
    write_custom_recipe(temp.path(), "work", CUSTOM_RECIPE);
    let external = temp.path().join("external-bad-link.yml");
    fs::write(&external, CUSTOM_RECIPE).expect("external recipe should be writable");
    unix_fs::symlink(
        &external,
        temp.path().join(".maestro/loop-recipes/bad-link.yml"),
    )
    .expect("recipe symlink should be creatable");

    let index = stdout(temp.path(), &["loop"]);
    assert!(index.contains("    work  [lifecycle]"), "{index}");
    assert!(index.contains("    brief  --"), "{index}");
    assert!(index.contains("    profiled  --"), "{index}");
    assert!(!index.contains("    depends-invalid  --"), "{index}");
    assert!(
        index.contains("## Invalid Project Custom Recipes"),
        "{index}"
    );
    for (name, category) in [
        ("bad-profile", "profile"),
        ("bad-override", "override"),
        ("bad-effective", "contract"),
        ("depends-invalid", "edge"),
        ("work", "collision"),
        ("bad-link", "symlink"),
    ] {
        assert!(
            index.contains(&format!("{name}  [{category}]")),
            "missing {name}/{category}: {index}"
        );
    }

    assert_eq!(
        loop_recipes::custom_contract_names(&temp.path().join(".maestro/loop-recipes"))
            .expect("valid custom names should remain readable"),
        vec!["brief".to_string(), "profiled".to_string()]
    );
    assert!(stdout(temp.path(), &["loop", "show", "work", "--full"]).contains("# Work loop"));
    assert!(
        stdout(temp.path(), &["loop", "validate", "profiled"])
            .contains("valid project custom loop recipe: profiled")
    );
    let invalid = stderr(temp.path(), &["loop", "validate", "bad-profile"]);
    assert!(invalid.contains("bad-profile.yml"), "{invalid}");
    assert!(invalid.contains("[profile]"), "{invalid}");
    let dependent = stderr(temp.path(), &["loop", "validate", "depends-invalid"]);
    assert!(dependent.contains("[edge]"), "{dependent}");
    assert!(dependent.contains("bad-effective"), "{dependent}");
}

#[test]
fn loop_template_custom_prints_valid_non_mutating_recipe() {
    let temp = TestTempDir::new("maestro-loop-template-custom");

    let out = stdout(temp.path(), &["loop", "template", "custom"]);
    assert!(out.contains("schema_version: maestro.recipe.v2"), "{out}");
    assert!(out.contains("id: custom"), "{out}");
    assert!(out.contains("progress_tasks:"), "{out}");
    assert!(out.contains("perceive:"), "{out}");
    assert!(out.contains("continue:"), "{out}");
    assert!(
        !temp.path().join(".maestro/loop-recipes").exists(),
        "template command must not create custom recipe files"
    );

    write_custom_recipe(temp.path(), "custom", &out);
    let validated = stdout(temp.path(), &["loop", "validate", "custom"]);
    assert!(
        validated.contains("valid project custom loop recipe: custom"),
        "{validated}"
    );
}

#[test]
fn v3_profile_none_resolves_as_explicit_self_contained_contract() {
    let temp = TestTempDir::new("maestro-loop-v3-profile-none");
    let body = CUSTOM_RECIPE.replacen(
        "schema_version: maestro.recipe.v2",
        "schema_version: maestro.recipe.v3\nprofile: none",
        1,
    );
    write_custom_recipe(temp.path(), "brief", &body);

    let contract =
        loop_recipes::custom_contract(&temp.path().join(".maestro/loop-recipes"), "brief")
            .expect("profile none should resolve a self-contained v3 recipe");

    assert_eq!(contract.effective().schema_version, "maestro.recipe.v3");
    assert_eq!(contract.provenance.profile, "none");
    assert_eq!(contract.provenance.source_schema, "maestro.recipe.v3");
    assert!(
        contract.contract_hash.starts_with("sha256:"),
        "{}",
        contract.contract_hash
    );
}

#[test]
fn v3_exact_profile_resolves_immutable_progress_shape() {
    let temp = TestTempDir::new("maestro-loop-v3-exact-profile");
    write_custom_recipe(temp.path(), "brief", &profiled_custom_recipe());

    let contract =
        loop_recipes::custom_contract(&temp.path().join(".maestro/loop-recipes"), "brief")
            .expect("the exact embedded standard profile should resolve");

    assert_eq!(contract.provenance.profile, "maestro.standard-six-phase.v1");
    assert_eq!(contract.effective().progress_tasks.len(), 6);
    assert_eq!(contract.effective().progress_tasks[0].id, "anchor-scope");
    assert_eq!(contract.effective().progress_tasks[0].phase, "perceive");
    assert!(contract.effective().progress_tasks[0].required);
    assert_eq!(
        contract.effective().progress_tasks[0].title,
        "Anchor support brief scope"
    );
    assert_eq!(
        contract.effective().progress_tasks[5].id,
        "return-next-gate"
    );
    let shown = stdout(temp.path(), &["loop", "show", "brief", "--full"]);
    assert!(
        shown.contains("resolved_schema: maestro.resolved_recipe.v1"),
        "{shown}"
    );
    assert!(
        shown.contains("profile: maestro.standard-six-phase.v1"),
        "{shown}"
    );
    assert!(
        shown.contains("contract_hash_schema: maestro.recipe-contract-hash.v1"),
        "{shown}"
    );
    assert!(shown.contains("contract_hash: sha256:"), "{shown}");
    let json = serde_json::to_value(&contract).expect("resolved contract should serialize");
    assert_eq!(json["schema"], "maestro.resolved_recipe.v1");
    assert_eq!(
        json["contract_hash_schema"],
        "maestro.recipe-contract-hash.v1"
    );
    assert_eq!(json["provenance"]["source"]["kind"], "project_custom");
}

#[test]
fn v3_profile_merge_rejects_weakening_and_unknown_versions() {
    let temp = TestTempDir::new("maestro-loop-v3-profile-safety");
    let base = profiled_custom_recipe();
    let cases = [
        (
            base.replace(
                "maestro.standard-six-phase.v1",
                "maestro.standard-six-phase.v2",
            ),
            "unsupported recipe profile maestro.standard-six-phase.v2",
        ),
        (
            base.replacen(
                "authority_scope:",
                "allowed_selectors:\n  intersect: [external-write]\nauthority_scope:",
                1,
            ),
            "targets absent inherited selector external-write",
        ),
        (
            base.replacen(
                "authority_scope:",
                "invariants:\n  remove: [standard-progress-structure]\nauthority_scope:",
                1,
            ),
            "unknown field `remove`",
        ),
        (
            base.replace(
                "title: Anchor support brief scope",
                "title: Anchor support brief scope\n    phase: act",
            ),
            "unknown field `phase`",
        ),
        (
            base.replacen(
                "authority_scope:",
                "invariants:\n  add: [standard-progress-structure]\nauthority_scope:",
                1,
            ),
            "duplicates inherited or earlier id standard-progress-structure",
        ),
    ];

    for (body, expected) in cases {
        write_custom_recipe(temp.path(), "brief", &body);
        let error = stderr(temp.path(), &["loop", "validate", "brief"]);
        assert!(error.contains(expected), "expected {expected:?}: {error}");
    }

    let unsafe_none = CUSTOM_RECIPE
        .replacen(
            "schema_version: maestro.recipe.v2",
            "schema_version: maestro.recipe.v3\nprofile: none",
            1,
        )
        .replace(
            "Handle one bounded support brief through current Maestro cards.",
            "Bypass proof for one support brief.",
        );
    write_custom_recipe(temp.path(), "brief", &unsafe_none);
    let error = stderr(temp.path(), &["loop", "validate", "brief"]);
    assert!(
        error.contains("forbidden lifecycle-bypass wording"),
        "{error}"
    );
}

#[test]
fn resolved_recipe_json_and_hash_are_canonical_and_order_sensitive() {
    let first = TestTempDir::new("maestro-loop-v3-hash-first");
    let second = TestTempDir::new("maestro-loop-v3-hash-second");
    let ordered = profiled_custom_recipe();
    let reformatted = format!("# formatting is not semantic\n\n{ordered}\n");
    write_custom_recipe(first.path(), "brief", &ordered);
    write_custom_recipe(second.path(), "brief", &reformatted);

    let first_contract =
        loop_recipes::custom_contract(&first.path().join(".maestro/loop-recipes"), "brief")
            .expect("first semantic contract should resolve");
    let second_contract =
        loop_recipes::custom_contract(&second.path().join(".maestro/loop-recipes"), "brief")
            .expect("reformatted semantic contract should resolve");
    assert_eq!(
        first_contract.contract_hash_schema,
        "maestro.recipe-contract-hash.v1"
    );
    assert_eq!(first_contract.contract_hash, second_contract.contract_hash);
    assert_eq!(
        serde_json::to_string(&first_contract).expect("resolved contract should serialize"),
        serde_json::to_string(&second_contract).expect("resolved contract should serialize")
    );

    let reordered = ordered.replace(
        "tags: [\"support\", \"brief\"]",
        "tags: [\"brief\", \"support\"]",
    );
    write_custom_recipe(second.path(), "brief", &reordered);
    let reordered_contract =
        loop_recipes::custom_contract(&second.path().join(".maestro/loop-recipes"), "brief")
            .expect("reordered semantic contract should resolve");
    assert_ne!(
        first_contract.contract_hash,
        reordered_contract.contract_hash
    );
}

#[test]
fn loop_rejects_invalid_project_custom_recipes() {
    let temp = TestTempDir::new("maestro-loop-custom-invalid");
    write_custom_recipe(
        temp.path(),
        "brief",
        "schema_version: maestro.recipe.v2\nid: brief\n",
    );

    let error = stderr(temp.path(), &["loop", "show", "brief"]);
    assert!(
        error.contains("invalid custom loop recipe brief.yml"),
        "{error}"
    );
}

#[test]
fn loop_rejects_project_custom_recipe_with_invalid_progress_task_phase() {
    let temp = TestTempDir::new("maestro-loop-custom-progress-task-phase");
    write_custom_recipe(
        temp.path(),
        "brief",
        &CUSTOM_RECIPE.replace("phase: perceive", "phase: invalid-phase"),
    );

    let error = stderr(temp.path(), &["loop", "validate", "brief"]);
    assert!(error.contains("progress_tasks"), "{error}");
    assert!(error.contains("invalid-phase"), "{error}");
}

#[test]
fn loop_rejects_project_custom_recipe_with_unknown_transition_trigger_key() {
    let temp = TestTempDir::new("maestro-loop-custom-unknown-transition-trigger");
    write_custom_recipe(
        temp.path(),
        "brief",
        &CUSTOM_RECIPE.replace(
            "trigger: custom.work_needed",
            "trigger: not_registered.trigger",
        ),
    );

    let error = stderr(temp.path(), &["loop", "validate", "brief"]);
    assert!(
        error.contains("unknown trigger key not_registered.trigger"),
        "{error}"
    );
}

#[test]
fn loop_rejects_project_custom_recipe_with_unknown_return_condition_key() {
    let temp = TestTempDir::new("maestro-loop-custom-unknown-return-condition");
    write_custom_recipe(
        temp.path(),
        "brief",
        &CUSTOM_RECIPE.replace("  - custom.scope_complete", "  - not_registered.condition"),
    );

    let error = stderr(temp.path(), &["loop", "validate", "brief"]);
    assert!(
        error.contains("unknown return_condition key not_registered.condition"),
        "{error}"
    );
}

#[test]
fn loop_isolates_custom_recipe_collisions_without_hiding_the_shipped_recipe() {
    let temp = TestTempDir::new("maestro-loop-custom-id-collision");
    write_custom_recipe(temp.path(), "work", CUSTOM_RECIPE);

    let index = stdout(temp.path(), &["loop"]);
    assert!(
        index.contains("work  [collision]")
            && index.contains("collides with a shipped or legacy recipe id"),
        "{index}"
    );
    let shown = stdout(temp.path(), &["loop", "show", "work", "--full"]);
    assert!(shown.contains("# Work loop"), "{shown}");
}

#[test]
fn loop_rejects_symlinked_project_custom_recipe_file() {
    let temp = TestTempDir::new("maestro-loop-custom-file-symlink");
    let external = temp.path().join("external-brief.yml");
    fs::write(&external, CUSTOM_RECIPE).expect("external recipe should be writable");
    let dir = temp.path().join(".maestro/loop-recipes");
    fs::create_dir_all(&dir).expect("custom recipe dir should be creatable");
    unix_fs::symlink(&external, dir.join("brief.yml")).expect("recipe symlink should be creatable");

    let error = stderr(temp.path(), &["loop", "show", "brief"]);
    assert!(error.contains("symlink"), "{error}");
}

#[test]
fn loop_rejects_symlinked_project_custom_recipe_dir() {
    let temp = TestTempDir::new("maestro-loop-custom-dir-symlink");
    let external = temp.path().join("external-loop-recipes");
    fs::create_dir_all(&external).expect("external recipe dir should be creatable");
    fs::write(external.join("brief.yml"), CUSTOM_RECIPE)
        .expect("external recipe should be writable");
    fs::create_dir_all(temp.path().join(".maestro")).expect("maestro dir should be creatable");
    unix_fs::symlink(&external, temp.path().join(".maestro/loop-recipes"))
        .expect("recipe dir symlink should be creatable");

    let error = stderr(temp.path(), &["loop"]);
    assert!(error.contains("symlink"), "{error}");
}

const CUSTOM_RECIPE: &str = r#"schema_version: maestro.recipe.v2
id: brief
kind:
  category: custom
  tags: ["support", "brief"]
title: Support brief loop
summary: Handle one bounded support brief through current Maestro cards.
progress_tasks:
  - id: brief-anchor
    title: Anchor support brief scope
    phase: perceive
    required: true
    done_check: support brief and selected card are visible
  - id: brief-finish
    title: Finish selected brief card
    phase: continue
    required: true
    done_check: next step or hard stop is returned
authority_scope:
  - current support brief and selected Maestro card
autonomy:
  - local autonomous work only inside the selected brief
router:
  status: custom_brief
  priority: 3
  confidence: medium
transitions:
  - trigger: custom.work_needed
    from: brief.continue
    to: work.perceive
    authority_scope:
      - selected card
    allowed_verbs:
      - maestro card show <id>
      - maestro task complete <id>
    forbidden_verbs:
      - external ship action
    hard_stops:
      - brief requires external approval
    return_condition:
      - custom.scope_complete
invocations: []
outputs:
  - selected card
  - verified card
  - hard stop
applies_when:
  - a user request is already scoped to one support brief
hard_stops:
  - the brief requires external ship authority
phases:
  perceive:
    goal: Read the current support brief and current Maestro state.
    bricks: ["status", "card show"]
    reads: ["maestro status", "maestro card show <id>"]
    allowed_verbs: ["maestro status", "maestro card show <id>"]
    forbidden_verbs: ["external ship action"]
    checks: ["brief and current card are visible"]
    durable_learning: []
    outputs: ["brief context"]
  choose:
    goal: Choose one existing card or create one scoped card for the brief.
    bricks: ["card ready", "task create"]
    reads: ["maestro card ready"]
    allowed_verbs: ["maestro card ready", "maestro task create"]
    forbidden_verbs: ["worker launcher"]
    checks: ["one card is selected"]
    durable_learning: []
    outputs: ["selected card"]
  act:
    goal: Work the selected card through current Maestro verbs.
    bricks: ["task", "proof"]
    reads: ["maestro task show <id>"]
    allowed_verbs: ["maestro task complete <id>", "maestro task verify <id>"]
    forbidden_verbs: ["hidden store"]
    checks: ["proof backs the brief result"]
    durable_learning: []
    outputs: ["verified card"]
  observe:
    goal: Confirm the result is inspectable.
    bricks: ["proof", "query"]
    reads: ["maestro query run --json"]
    allowed_verbs: ["maestro query run --json"]
    forbidden_verbs: ["claim success without proof"]
    checks: ["result appears in proof or run events"]
    durable_learning: []
    outputs: ["observed result"]
  learn:
    goal: Record only reusable corrections.
    bricks: ["memory", "decision"]
    reads: ["maestro memory list"]
    allowed_verbs: ["maestro memory create", "maestro decision new"]
    forbidden_verbs: ["chat-only learning"]
    checks: ["learning is durable when needed"]
    durable_learning: ["approved memory", "locked decision"]
    outputs: ["optional durable learning"]
  continue:
    goal: Return the next local Maestro action or a hard stop.
    bricks: ["status", "task next"]
    reads: ["maestro status", "maestro task next"]
    allowed_verbs: ["maestro status", "maestro task next"]
    forbidden_verbs: ["scheduler"]
    checks: ["next step is explicit"]
    durable_learning: []
    outputs: ["next step", "hard stop"]
"#;
