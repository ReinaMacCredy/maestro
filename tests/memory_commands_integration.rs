pub mod card_support;
mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use card_support::cards_repo;
use serde_json::Value;

fn maestro(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_SESSION", "s1")
        .env("MAESTRO_AUTO_UPDATE", "0")
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn run(cwd: &Path, args: &[&str]) -> String {
    let output = maestro(cwd, args);
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn first_id(output: &str, prefix: &str) -> String {
    output
        .split_whitespace()
        .find(|word| word.starts_with(prefix))
        .unwrap_or_else(|| panic!("no {prefix} id in output:\n{output}"))
        .to_string()
}

#[test]
fn memory_suggest_and_create_flow_writes_visible_artifacts() {
    let temp = cards_repo("memory-suggest-create");
    let repo = temp.path();

    let created = run(
        repo,
        &[
            "memory",
            "suggest",
            "create",
            "--source-ref",
            "run_event:run-1",
            "--signal-type",
            "failure",
            "--summary",
            "Known refund gotcha",
        ],
    );
    let suggestion_id = first_id(&created, "msug-");
    assert!(
        created.contains("create: maestro memory create --from"),
        "create output gives the explicit Memory path:\n{created}"
    );

    let listed = run(repo, &["memory", "suggest", "list"]);
    assert!(
        listed.contains(&suggestion_id),
        "list shows suggestion:\n{listed}"
    );
    assert!(
        listed.contains("dismiss: maestro memory suggest dismiss"),
        "list shows dismiss path:\n{listed}"
    );

    let dismissed = run(
        repo,
        &[
            "memory",
            "suggest",
            "dismiss",
            &suggestion_id,
            "--reason",
            "not reusable",
        ],
    );
    assert!(
        dismissed.contains(&format!("dismissed {suggestion_id}")),
        "dismiss confirms id:\n{dismissed}"
    );
    let open_after_dismiss = run(repo, &["memory", "suggest", "list"]);
    assert_eq!(open_after_dismiss.trim(), "no memory suggestions");

    let second = run(
        repo,
        &[
            "memory",
            "suggest",
            "create",
            "--source-ref",
            "run_event:run-2",
            "--signal-type",
            "user_correction",
            "--summary",
            "Inspect refund ledger before approval",
        ],
    );
    let second_id = first_id(&second, "msug-");
    let memory = run(repo, &["memory", "create", "--from", &second_id]);
    let memory_id = first_id(&memory, "mem-");
    assert!(
        memory.contains(&format!("from {second_id}")),
        "memory create links the suggestion:\n{memory}"
    );

    let memory_dir = repo.join(".maestro/cards").join(&memory_id).join("memory");
    assert!(memory_dir.join("candidate.yml").is_file());
    assert!(memory_dir.join("lesson.md").is_file());
    assert!(memory_dir.join("signals.jsonl").is_file());
    assert!(memory_dir.join("receipts").is_dir());

    let candidate = fs::read_to_string(memory_dir.join("candidate.yml")).expect("candidate");
    assert!(candidate.contains("schema_version: maestro.memory.candidate.v1"));
    assert!(candidate.contains("lifecycle: proposed"));

    let scorer_contract = repo.join("schema-scorer.yml");
    fs::write(&scorer_contract, "type: schema\nname: candidate-schema\n").expect("contract");
    let attached = run(
        repo,
        &[
            "memory",
            "scorer",
            "attach",
            &memory_id,
            "--contract-file",
            scorer_contract.to_str().expect("contract path is utf8"),
        ],
    );
    assert!(
        attached.contains(&format!("attached scorer schema to {memory_id}")),
        "attach confirms scorer:\n{attached}"
    );
    let scorer_ref = format!("{memory_id}#gate.scorer_contract");
    let scorer = run(repo, &["scorer", "run", &scorer_ref]);
    let receipt_id = first_id(&scorer, "rcpt-");
    assert!(
        scorer.contains("passed") && scorer.contains(&format!("memory={memory_id}")),
        "scorer run writes a passed receipt:\n{scorer}"
    );
    let receipts = run(repo, &["scorer", "list", "--memory", &memory_id]);
    assert!(
        receipts.contains(&receipt_id),
        "receipt list shows the scorer receipt:\n{receipts}"
    );
    let receipt = run(
        repo,
        &["scorer", "show", &format!("{memory_id}#{receipt_id}")],
    );
    assert!(
        receipt.contains("\"status\": \"passed\""),
        "receipt show renders JSON:\n{receipt}"
    );

    fs::create_dir_all(repo.join(".maestro/memory")).expect("memory dir");
    fs::write(
        repo.join(".maestro/memory/target-registry.yml"),
        r#"schema_version: maestro.memory.target_registry.v1
surfaces:
  memory_note:
    target_path: .maestro/memory/approved/{memory_id}.md
    required_gate: scorer
    review_required: true
    allowed_writes:
      - replace
    allowed_scorer_types:
      - schema
    rollback_path: .maestro/memory/promotions/{memory_id}/rollback
"#,
    )
    .expect("target registry");

    let promotion = run(
        repo,
        &[
            "memory",
            "promote",
            &memory_id,
            "--plan",
            "--scorer-receipt",
            &format!("{memory_id}#{receipt_id}"),
            "--review-evidence",
            "manual:approved",
        ],
    );
    let promotion_id = first_id(&promotion, "prom-");
    assert!(
        promotion.contains("path=") && promotion.contains("gated"),
        "promotion planning records a gated plan:\n{promotion}"
    );
    let plan_path = repo
        .join(".maestro/memory/promotions")
        .join(&promotion_id)
        .join("plan.yml");
    let plan = fs::read_to_string(&plan_path).expect("promotion plan");
    assert!(plan.contains("registry_hash: sha256:"));
    assert!(plan.contains("scorer_receipt:"));
    assert!(plan.contains("review_evidence: manual:approved"));

    let applied = run(repo, &["memory", "promote", &promotion_id, "--apply"]);
    assert!(
        applied.contains(&format!("applied {promotion_id}"))
            && applied.contains(".maestro/memory/approved/"),
        "promotion apply writes target:\n{applied}"
    );
    let promoted_target = repo
        .join(".maestro/memory/approved")
        .join(format!("{memory_id}.md"));
    let promoted = fs::read_to_string(&promoted_target).expect("promoted target");
    assert!(promoted.contains(&format!("memory_id: {memory_id}")));
    assert!(promoted.contains("Inspect refund ledger before approval"));
    let promoted_candidate =
        fs::read_to_string(memory_dir.join("candidate.yml")).expect("promoted candidate");
    assert!(promoted_candidate.contains("lifecycle: promoted"));
    assert!(promoted_candidate.contains("registry_hash: sha256:"));
    let promoted_card = fs::read_to_string(
        repo.join(".maestro/cards")
            .join(&memory_id)
            .join("card.yaml"),
    )
    .expect("promoted card");
    assert!(promoted_card.contains("status: verified"));
    let health =
        fs::read_to_string(repo.join(".maestro/memory/health-ledger.jsonl")).expect("health");
    assert!(
        health.contains(&promotion_id) && health.contains("\"state\":\"healthy\""),
        "promotion appends health state: {health}"
    );
    let memory_search = run(repo, &["memory", "search", "refund", "ledger"]);
    assert!(
        memory_search.contains("APPROVED MEMORY") && memory_search.contains(&memory_id),
        "memory search returns approved Memory:\n{memory_search}"
    );
    let open_suggestion = run(
        repo,
        &[
            "memory",
            "suggest",
            "create",
            "--source-ref",
            "run_event:run-suggest-status",
            "--signal-type",
            "failure",
            "--summary",
            "Surface scoped Memory suggestion",
        ],
    );
    let open_suggestion_id = first_id(&open_suggestion, "msug-");
    let status = run(repo, &["status"]);
    assert!(
        status.contains("APPROVED MEMORY") && status.contains(&memory_id),
        "status includes bounded approved Memory:\n{status}"
    );
    assert!(
        status.contains("MEMORY SUGGESTIONS")
            && status.contains(&open_suggestion_id)
            && status.contains("sources=1")
            && status.contains("create: maestro memory create --from")
            && status.contains("dismiss: maestro memory suggest dismiss"),
        "status includes review-only Memory suggestion paths:\n{status}"
    );
    let resume = run(repo, &["resume"]);
    assert!(
        resume.contains("approved memory:") && resume.contains(&memory_id),
        "resume includes bounded approved Memory:\n{resume}"
    );
    assert!(
        resume.contains("memory suggestions:")
            && resume.contains(&open_suggestion_id)
            && resume.contains("create: maestro memory create --from")
            && resume.contains("dismiss: maestro memory suggest dismiss"),
        "resume includes review-only Memory suggestion paths:\n{resume}"
    );
    let work_card = run(
        repo,
        &[
            "card",
            "create",
            "-t",
            "task",
            "Use approved Memory",
            "--id-only",
        ],
    );
    let work_card_id = first_id(&work_card, "task-");
    let card_show = run(repo, &["card", "show", &work_card_id]);
    assert!(
        card_show.contains("APPROVED MEMORY") && card_show.contains(&memory_id),
        "card show includes scoped approved Memory:\n{card_show}"
    );
    let lease = run(repo, &["loop", "work-lease", "--json"]);
    let lease_json: Value = serde_json::from_str(&lease).expect("work lease json");
    assert_eq!(lease_json["status"], "leased");
    assert_eq!(lease_json["approved_lessons"][0]["id"], memory_id);
    assert_eq!(
        lease_json["memory_suggestions"][0]["id"].as_str(),
        Some(open_suggestion_id.as_str())
    );
    let expected_create_command = format!("maestro memory create --from {open_suggestion_id}");
    assert_eq!(
        lease_json["memory_suggestions"][0]["create_command"].as_str(),
        Some(expected_create_command.as_str())
    );
    assert!(
        lease_json["worker_prompt"]
            .as_str()
            .unwrap_or("")
            .contains(&memory_id),
        "worker prompt includes compact approved Memory:\n{lease}"
    );
    assert!(
        lease_json["worker_prompt"]
            .as_str()
            .unwrap_or("")
            .contains(&open_suggestion_id),
        "worker prompt includes review-only Memory suggestion:\n{lease}"
    );

    let third = run(
        repo,
        &[
            "memory",
            "create",
            "--from",
            "run_event:run-3",
            "--summary",
            "Race-safe apply recovery",
            "--lesson",
            "Do not apply stale promotion plans",
        ],
    );
    let third_id = first_id(&third, "mem-");
    run(
        repo,
        &[
            "memory",
            "scorer",
            "attach",
            &third_id,
            "--contract-file",
            scorer_contract.to_str().expect("contract path is utf8"),
        ],
    );
    let third_scorer_ref = format!("{third_id}#gate.scorer_contract");
    let third_scorer = run(repo, &["scorer", "run", &third_scorer_ref]);
    let third_receipt_id = first_id(&third_scorer, "rcpt-");
    let third_plan = run(
        repo,
        &[
            "memory",
            "promote",
            &third_id,
            "--plan",
            "--scorer-receipt",
            &format!("{third_id}#{third_receipt_id}"),
            "--review-evidence",
            "manual:approved",
        ],
    );
    let third_promotion_id = first_id(&third_plan, "prom-");
    let third_target = repo
        .join(".maestro/memory/approved")
        .join(format!("{third_id}.md"));
    fs::write(&third_target, "racing writer").expect("racing target");
    let failed = maestro(repo, &["memory", "promote", &third_promotion_id, "--apply"]);
    assert!(
        !failed.status.success(),
        "stale target apply should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&failed.stdout),
        String::from_utf8_lossy(&failed.stderr)
    );
    let failed_plan = fs::read_to_string(
        repo.join(".maestro/memory/promotions")
            .join(&third_promotion_id)
            .join("plan.yml"),
    )
    .expect("failed plan");
    assert!(failed_plan.contains("status: apply_failed"));
    assert!(failed_plan.contains("target .maestro/memory/approved/"));

    let suggestions =
        fs::read_to_string(repo.join(".maestro/memory/suggestions.jsonl")).expect("queue");
    let rows: Vec<Value> = suggestions
        .lines()
        .map(|line| serde_json::from_str(line).expect("suggestion row parses"))
        .collect();
    assert!(
        rows.iter()
            .any(|row| row["id"] == suggestion_id && row["status"] == "dismissed"),
        "dismissed suggestion stays inspectable: {suggestions}"
    );
    assert!(
        rows.iter().any(|row| {
            row["id"] == second_id
                && row["status"] == "created"
                && row["created_memory_id"] == memory_id
        }),
        "created suggestion links Memory id: {suggestions}"
    );
    assert!(
        rows.iter()
            .any(|row| row["id"] == open_suggestion_id && row["status"] == "open"),
        "open suggestion remains a queue row and is not auto-created: {suggestions}"
    );

    let memory_source = format!("memory:{memory_id}");
    let l0 = run(
        repo,
        &[
            "memory",
            "maintain",
            "--level",
            "L0",
            "--source-ref",
            &memory_source,
            "--reason",
            "stale scan",
        ],
    );
    let l0_id = first_id(&l0, "maint-");
    assert!(
        l0.contains("level=l0_detect") && l0.contains("contract=<none>"),
        "L0 records ledger rows without a contract:\n{l0}"
    );
    let health_after_l0 =
        fs::read_to_string(repo.join(".maestro/memory/health-ledger.jsonl")).expect("health");
    assert!(
        health_after_l0.contains(&l0_id)
            && health_after_l0.contains("\"signal\":\"maintenance_requested\"")
            && health_after_l0.contains("\"state\":\"maintenance_due\"")
            && health_after_l0.contains("\"level\":\"l0_detect\""),
        "L0 writes visible health ledger rows: {health_after_l0}"
    );

    let l1 = run(
        repo,
        &[
            "memory",
            "maintain",
            "--level",
            "L1",
            "--source-ref",
            &memory_source,
            "--reason",
            "local tidy",
            "--proof-link",
            "proof:p1",
            "--run-link",
            "run:r1",
        ],
    );
    let l1_id = first_id(&l1, "maint-");
    assert!(
        l1.contains("level=l1_local_tidy") && l1.contains("contract="),
        "L1 writes a contract:\n{l1}"
    );
    let l1_contract = fs::read_to_string(
        repo.join(".maestro/memory/maintenance")
            .join(&l1_id)
            .join("contract.yml"),
    )
    .expect("L1 maintenance contract");
    assert!(l1_contract.contains("schema_version: maestro.memory.maintenance_contract.v1"));
    assert!(l1_contract.contains("level: l1_local_tidy"));
    assert!(l1_contract.contains("tokens: 4000"));
    assert!(l1_contract.contains("max_files: 3"));
    assert!(l1_contract.contains("subagents: 0"));
    assert!(l1_contract.contains("simple_memory_note_proposal"));
    assert!(l1_contract.contains("hidden_memory"));
    assert!(l1_contract.contains("proof:p1"));
    assert!(l1_contract.contains("run:r1"));

    let l2 = run(
        repo,
        &[
            "memory",
            "dream",
            "--source-ref",
            &memory_source,
            "--reason",
            "focused repair",
        ],
    );
    let l2_id = first_id(&l2, "maint-");
    let l2_contract = fs::read_to_string(
        repo.join(".maestro/memory/maintenance")
            .join(&l2_id)
            .join("contract.yml"),
    )
    .expect("L2 maintenance contract");
    assert!(l2.contains("level=l2_focused_repair"));
    assert!(l2_contract.contains("level: l2_focused_repair"));
    assert!(l2_contract.contains("tokens: 12000"));
    assert!(l2_contract.contains("max_source_refs: 40"));
    assert!(l2_contract.contains("subagents: 1"));
    assert!(l2_contract.contains("recurrence_guard_proposal"));

    let l3_fail = maestro(
        repo,
        &[
            "memory",
            "maintain",
            "--level",
            "L3",
            "--source-ref",
            &memory_source,
            "--reason",
            "deep rebuild",
            "--tokens",
            "24000",
            "--wall-minutes",
            "60",
            "--max-source-refs",
            "80",
            "--max-files",
            "30",
            "--subagents",
            "2",
        ],
    );
    assert!(
        !l3_fail.status.success()
            && String::from_utf8_lossy(&l3_fail.stderr)
                .contains("L3 maintenance requires --human-approved"),
        "L3 without human approval should fail\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&l3_fail.stdout),
        String::from_utf8_lossy(&l3_fail.stderr)
    );

    let l3 = run(
        repo,
        &[
            "memory",
            "maintain",
            "--level",
            "L3",
            "--source-ref",
            &memory_source,
            "--reason",
            "deep rebuild",
            "--human-approved",
            "--tokens",
            "24000",
            "--wall-minutes",
            "60",
            "--max-source-refs",
            "80",
            "--max-files",
            "30",
            "--subagents",
            "2",
        ],
    );
    let l3_id = first_id(&l3, "maint-");
    let l3_contract = fs::read_to_string(
        repo.join(".maestro/memory/maintenance")
            .join(&l3_id)
            .join("contract.yml"),
    )
    .expect("L3 maintenance contract");
    assert!(l3_contract.contains("level: l3_deep_rebuild"));
    assert!(l3_contract.contains("tokens: 24000"));
    assert!(l3_contract.contains("wall_minutes: 60"));
    assert!(l3_contract.contains("max_files: 30"));
    assert!(l3_contract.contains("subagents: 2"));
    assert!(l3_contract.contains("harness_proposal"));
}
