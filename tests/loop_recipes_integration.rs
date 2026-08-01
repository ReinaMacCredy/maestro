mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::{Command, Output};

use maestro::domain::loop_recipes::{self, LoopChainFacts, LoopChainSelectedUnit};
use maestro::foundation::core::hash::sha256_hex;
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

fn seed_run(repo: &Path, session: &str, lines: &[String]) {
    let run_dir = repo.join(".maestro/runs").join(session);
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .expect("invariant: events fixture should be writable");
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
    let legacy = stderr(temp.path(), &["loop", "show", "work", "--full"]);
    assert!(
        legacy.contains("unsupported_legacy_successor_surface")
            && legacy.contains("maestro packet read"),
        "{legacy}"
    );
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
fn loop_isolates_legacy_custom_recipe_collisions_and_preserves_typed_refusal() {
    let temp = TestTempDir::new("maestro-loop-custom-id-collision");
    write_custom_recipe(
        temp.path(),
        "work",
        &CUSTOM_RECIPE
            .replace("id: brief", "id: work")
            .replace("brief.", "work."),
    );

    let index = stdout(temp.path(), &["loop"]);
    assert!(
        index.contains("work  [collision]")
            && index.contains("collides with a shipped or legacy recipe id"),
        "{index}"
    );
    let refusal = stderr(temp.path(), &["loop", "show", "work", "--full"]);
    assert!(
        refusal.contains("unsupported_legacy_successor_surface")
            && refusal.contains("maestro packet read"),
        "{refusal}"
    );
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
