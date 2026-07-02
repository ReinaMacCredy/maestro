mod support;

use std::fs;

use git2::{IndexAddOption, Repository, Signature};
use maestro::domain::card::schema::Card;
use maestro::domain::card::{self};
use maestro::domain::feature::{self, ContractAdditions, ContractEdits};
use maestro::foundation::core::fs::ensure_dir;
use maestro::foundation::core::hash::sha256_prefixed;
use maestro::foundation::core::paths::MaestroPaths;
use support::TestTempDir;

/// Write a feature card directly into the flat card store. `contents` is the raw
/// `card.yaml` body; callers pass a well-formed card envelope (`BAD_RECORD`) or
/// deliberate garbage to exercise the strict/tolerant/diagnose read paths.
fn write_feature(paths: &MaestroPaths, id: &str, contents: &str) {
    let dir = paths.cards_dir().join(id);
    ensure_dir(&dir).expect("invariant: card dir should be creatable");
    fs::write(dir.join("card.yaml"), contents).expect("invariant: card.yaml should be writable");
}

/// Write a child task card into the flat card store. Feature ownership rides the
/// card's top-level `parent` field (no per-feature directory); the cancel cascade
/// loads and transitions the child, so `extra` carries a complete TaskRecord
/// rather than a projection-only stub.
fn write_task(paths: &MaestroPaths, id: &str, feature_id: &str, state: &str) {
    let dir = paths.cards_dir().join(id);
    ensure_dir(&dir).expect("invariant: card dir should be creatable");
    fs::write(
        dir.join("card.yaml"),
        format!(
            "schema_version: maestro.card.v1\nid: {id}\ntype: task\ntitle: {id}\nstatus: {state}\nparent: {feature_id}\ncreated_at: \"2026-06-06T00:00:00.000Z\"\nupdated_at: \"2026-06-06T00:00:00.000Z\"\nextra:\n  schema_version: maestro.task.v2\n  id: {id}\n  title: {id}\n  state: {state}\n  acceptance_locked: false\n  verification: {{}}\n  created_at: \"2026-06-06T00:00:00.000Z\"\n  updated_at: \"2026-06-06T00:00:00.000Z\"\n"
        ),
    )
    .expect("invariant: card.yaml should be writable");
}

/// A feature card whose folded record carries an incompatible schema version, so
/// the strict reads surface a schema mismatch (the card envelope itself parses;
/// the `extra` record is what classifies as incompatible).
const BAD_RECORD: &str = "schema_version: maestro.card.v1\nid: billing-csv\ntype: feature\ntitle: Billing CSV\nstatus: proposed\ncreated_at: \"1\"\nupdated_at: \"1\"\nextra:\n  schema_version: maestro.galaxy.v9\n  id: billing-csv\n  title: Billing CSV\n  status: proposed\n  created_at: \"1\"\n  updated_at: \"1\"\n";

/// Author a complete contract on a freshly-created Proposed feature, plus a
/// present baseline with no `[bl-NNN]` scenarios. The empty Scenario Matrix
/// declares no behavioral surface (QA C), so it satisfies the accept precondition
/// (F) and the close coverage skip without entangling these transition-machinery
/// tests with slice authoring (the behavioral path lives in feature_qa_gate_integration).
fn author_contract(paths: &MaestroPaths, id: &str) {
    feature::set(
        paths,
        id,
        ContractEdits {
            acceptance: Some(vec!["exports a valid csv".to_string()]),
            affected_areas: Some(vec!["billing".to_string()]),
            ..Default::default()
        },
    )
    .expect("invariant: set should succeed on a proposed feature");

    let dir = paths.cards_dir().join(id);
    ensure_dir(&dir).expect("invariant: card dir should be creatable");
    fs::write(
        dir.join("qa.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Baseline gaps:\n  - none (no behavioral surface)\n",
    )
    .expect("invariant: qa.md should be writable");

    reconcile_clean(paths, id);
    feature::finalize(paths, id).expect("invariant: finalize should write a fresh handoff");
}

fn reconcile_clean(paths: &MaestroPaths, id: &str) {
    feature::reconcile_clean_check(paths, id, feature::ReconcileActor::agent("test", None))
        .expect("invariant: reconcile receipt should be current");
}

fn verify_contract(paths: &MaestroPaths, id: &str) {
    feature::verify_feature(
        paths,
        id,
        vec![feature::FeatureProofUpdate::Explicit {
            ac_id: "ac-1".to_string(),
            evidence: "fixture evidence".to_string(),
        }],
    )
    .expect("invariant: proof should record");
    feature::verify_feature(paths, id, Vec::new()).expect("invariant: sweep should succeed");
}

fn init_git_repo(repo: &std::path::Path) -> Repository {
    Repository::init(repo).expect("invariant: git repo should initialize")
}

fn commit_all(repository: &Repository, message: &str) -> String {
    let mut index = repository
        .index()
        .expect("invariant: git index should be readable");
    index
        .add_all(["."].iter(), IndexAddOption::DEFAULT, None)
        .expect("invariant: git index add should succeed");
    index
        .write()
        .expect("invariant: git index write should succeed");
    let tree_id = index
        .write_tree()
        .expect("invariant: git tree write should succeed");
    let tree = repository
        .find_tree(tree_id)
        .expect("invariant: git tree should exist");
    let signature = Signature::now("Maestro Test", "maestro@example.test")
        .expect("invariant: git signature should be constructable");
    let parent = repository
        .head()
        .ok()
        .and_then(|head| head.target())
        .and_then(|oid| repository.find_commit(oid).ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .expect("invariant: git commit should succeed")
        .to_string()
}

fn close_ready_feature(paths: &MaestroPaths, title: &str) -> String {
    feature::create(paths, title, None).expect("invariant: create should succeed");
    let id = title.to_ascii_lowercase().replace(' ', "-");
    author_contract(paths, &id);
    feature::accept(paths, &id, false).expect("invariant: accept should succeed");
    feature::start(paths, &id).expect("invariant: start should succeed");
    verify_contract(paths, &id);
    feature::close(paths, &id, Some("done".to_string()), false)
        .expect("invariant: close should succeed");
    id
}

#[test]
fn create_generates_slug_id_and_persists() {
    let temp = TestTempDir::new("maestro-feature-create");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV Export", None).expect("invariant: create should succeed");

    let views = feature::list(&paths).expect("invariant: list should succeed");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].id, "billing-csv-export");
    assert_eq!(views[0].title, "Billing CSV Export");
    assert_eq!(views[0].status, feature::FeatureStatus::Proposed);
}

#[test]
fn create_rejects_empty_title() {
    let temp = TestTempDir::new("maestro-feature-empty");
    let paths = MaestroPaths::new(temp.path());

    let error =
        feature::create(&paths, "   ", None).expect_err("invariant: empty title must error");
    assert!(error.to_string().contains("ASCII"));
}

#[test]
fn create_rejects_duplicate_id() {
    let temp = TestTempDir::new("maestro-feature-dup");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: first create should succeed");
    let error = feature::create(&paths, "Billing CSV", None)
        .expect_err("invariant: duplicate id must error");
    assert!(error.to_string().contains("already exists"));
}

#[test]
fn set_replaces_per_field_and_is_proposed_only() {
    let temp = TestTempDir::new("maestro-feature-set");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    let view = feature::set(
        &paths,
        "billing-csv",
        ContractEdits {
            acceptance: Some(vec!["a".to_string(), "b".to_string()]),
            affected_areas: Some(vec!["billing".to_string()]),
            description: Some("export billing rows".to_string()),
            ..Default::default()
        },
    )
    .expect("invariant: set should succeed");
    assert_eq!(view.acceptance.len(), 2);
    assert_eq!(view.affected_areas, vec!["billing".to_string()]);

    // A second set replaces the whole field rather than appending.
    let view = feature::set(
        &paths,
        "billing-csv",
        ContractEdits {
            acceptance: Some(vec!["only".to_string()]),
            ..Default::default()
        },
    )
    .expect("invariant: set should replace");
    assert_eq!(view.acceptance, vec!["only".to_string()]);
    assert_eq!(view.affected_areas, vec!["billing".to_string()]);

    // Once accepted the contract is frozen; set is rejected.
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    let error = feature::set(
        &paths,
        "billing-csv",
        ContractEdits {
            acceptance: Some(vec!["late".to_string()]),
            ..Default::default()
        },
    )
    .expect_err("invariant: set must be rejected once frozen");
    assert!(error.to_string().contains("frozen"));
}

#[test]
fn set_rejects_a_blank_contract_value_so_it_cannot_satisfy_the_accept_gate() {
    let temp = TestTempDir::new("maestro-feature-set-blank");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    // A `--acceptance ""` (or whitespace) would store a `[""]` that satisfies the
    // length-only accept gate while carrying no contract; it must be refused, like
    // the sibling `task set --check ""`.
    for value in ["", "   "] {
        let error = feature::set(
            &paths,
            "billing-csv",
            ContractEdits {
                acceptance: Some(vec![value.to_string()]),
                ..Default::default()
            },
        )
        .expect_err("invariant: a blank acceptance value must be rejected");
        assert!(
            error
                .to_string()
                .contains("must not be empty or whitespace"),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn amend_rejects_a_blank_addition_value() {
    let temp = TestTempDir::new("maestro-feature-amend-blank");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");

    let error = feature::amend(
        &paths,
        "billing-csv",
        ContractAdditions {
            acceptance: vec!["   ".to_string()],
            ..Default::default()
        },
        "real reason",
    )
    .expect_err("invariant: a blank amend value must be rejected");
    assert!(
        error
            .to_string()
            .contains("must not be empty or whitespace"),
        "unexpected error: {error}"
    );
}

#[test]
fn finalize_rejects_unknown_acceptance_evidence_kind() {
    let temp = TestTempDir::new("maestro-feature-evidence-kind");
    let paths = MaestroPaths::new(temp.path());
    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    feature::set(
        &paths,
        "billing-csv",
        ContractEdits {
            acceptance: Some(vec!["exports rows".to_string()]),
            affected_areas: Some(vec!["billing".to_string()]),
            ..Default::default()
        },
    )
    .expect("invariant: set should succeed");
    reconcile_clean(&paths, "billing-csv");
    feature::finalize(&paths, "billing-csv").expect("invariant: finalize should succeed");
    feature::accept_with_qa_none(&paths, "billing-csv", "domain test", false)
        .expect("invariant: feature may be accepted");
    feature::start(&paths, "billing-csv").expect("invariant: started features may be verified");

    let reopened = feature::reopen(&paths, "billing-csv").expect("invariant: reopen should work");
    let path = reopened.path.join("card.yaml");
    let mut raw: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(&path).expect("invariant: feature should be readable"),
    )
    .expect("invariant: feature should parse");
    // The feature record rides under the card's `extra` carrier; inject the
    // poisoned evidence there so the cutover read reconstructs it.
    raw["extra"]["acceptance_evidence"] =
        serde_yaml::Value::Sequence(vec![serde_yaml::Value::Mapping(
            [
                (
                    serde_yaml::Value::String("ac_id".to_string()),
                    serde_yaml::Value::String("ac-1".to_string()),
                ),
                (
                    serde_yaml::Value::String("kind".to_string()),
                    serde_yaml::Value::String("typo".to_string()),
                ),
                (
                    serde_yaml::Value::String("text".to_string()),
                    serde_yaml::Value::String("must not count".to_string()),
                ),
                (
                    serde_yaml::Value::String("at".to_string()),
                    serde_yaml::Value::String("2026-06-06T00:00:00.000Z".to_string()),
                ),
            ]
            .into_iter()
            .collect(),
        )]);
    fs::write(
        &path,
        serde_yaml::to_string(&raw).expect("invariant: feature should serialize"),
    )
    .expect("invariant: feature should be writable");
    let error = feature::finalize(&paths, "billing-csv")
        .expect_err("unknown evidence kind must not become DB authority");
    assert!(
        format!("{error:#}").contains("unknown variant `typo`"),
        "{error:#}"
    );
}

#[test]
fn accept_gate_requires_acceptance_and_areas() {
    let temp = TestTempDir::new("maestro-feature-accept-gate");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    let error = feature::accept(&paths, "billing-csv", false)
        .expect_err("invariant: accept must block on an incomplete contract");
    let message = error.to_string();
    assert!(message.contains("acceptance"));
    assert!(message.contains("affected_areas"));

    author_contract(&paths, "billing-csv");
    let report = feature::accept(&paths, "billing-csv", false).expect("invariant: accept succeeds");
    assert!(report.changed);
    assert_eq!(report.status, feature::FeatureStatus::Ready);
}

#[test]
fn project_neither_satisfies_nor_alters_the_accept_gate() {
    let temp = TestTempDir::new("maestro-feature-project-gate");
    let paths = MaestroPaths::new(temp.path());

    // A feature minted WITH --project but no acceptance/areas is still blocked:
    // project is a base-envelope scope, distinct from affected_areas, and must
    // not satisfy the readiness gate.
    feature::create(&paths, "Billing CSV", Some("svc-pay".to_string()))
        .expect("invariant: create should succeed");
    let id = "billing-csv";
    let card = card::store::load(&card::store::card_path(&paths, id))
        .expect("load card")
        .expect("feature card exists");
    assert_eq!(
        card.project.as_deref(),
        Some("svc-pay"),
        "project is stored on the feature card"
    );
    let error = feature::accept(&paths, id, false)
        .expect_err("invariant: project must not satisfy the accept gate");
    let message = error.to_string();
    assert!(message.contains("acceptance"), "{message}");
    assert!(message.contains("affected_areas"), "{message}");

    // Authoring the real contract flips it ready regardless of project.
    author_contract(&paths, id);
    let report = feature::accept(&paths, id, false).expect("invariant: accept succeeds");
    assert!(report.changed);
    assert_eq!(report.status, feature::FeatureStatus::Ready);
}

#[test]
fn accept_dry_run_previews_without_transitioning() {
    let temp = TestTempDir::new("maestro-feature-accept-dry");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    let report =
        feature::accept(&paths, "billing-csv", true).expect("invariant: dry-run is exit 0");
    assert!(!report.changed);
    assert!(report.note.contains("would block"));

    let view = feature::show(&paths, "billing-csv").expect("invariant: show should succeed");
    assert_eq!(view.status, feature::FeatureStatus::Proposed);
}

#[test]
fn full_lifecycle_new_set_accept_start_close() {
    let temp = TestTempDir::new("maestro-feature-lifecycle");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    let started = feature::start(&paths, "billing-csv").expect("invariant: start should succeed");
    assert_eq!(started.status, feature::FeatureStatus::InProgress);
    verify_contract(&paths, "billing-csv");
    let closed = feature::close(&paths, "billing-csv", None, false)
        .expect("invariant: close should succeed");
    assert_eq!(closed.status, feature::FeatureStatus::Closed);
}

#[test]
fn archive_candidates_classify_lifecycle_states_and_apply_targets() {
    let temp = TestTempDir::new("maestro-feature-archive-candidates");
    let paths = MaestroPaths::new(temp.path());
    let evidence = feature::ArchiveGateEvidence::default();

    feature::create(&paths, "Proposed Export", None).expect("invariant: create should succeed");

    feature::create(&paths, "Ready Export", None).expect("invariant: create should succeed");
    author_contract(&paths, "ready-export");
    feature::accept(&paths, "ready-export", false).expect("invariant: accept should succeed");

    feature::create(&paths, "Active Export", None).expect("invariant: create should succeed");
    author_contract(&paths, "active-export");
    feature::accept(&paths, "active-export", false).expect("invariant: accept should succeed");
    feature::start(&paths, "active-export").expect("invariant: start should succeed");

    let closed_id = close_ready_feature(&paths, "Closed Export");
    write_task(&paths, "task-archive-child", &closed_id, "verified");
    let claimed_child = paths
        .cards_dir()
        .join("task-archive-child")
        .join("card.yaml");
    fs::write(
        &claimed_child,
        fs::read_to_string(&claimed_child)
            .expect("invariant: child card should be readable")
            .replacen(
                "updated_at: \"2026-06-06T00:00:00.000Z\"\n",
                "updated_at: \"2026-06-06T00:00:00.000Z\"\nclaimed_by: maestro\nclaimed_at: \"2026-07-02T00:00:00.000Z\"\n",
                1,
            ),
    )
    .expect("invariant: claimed child card should be writable");

    let proposed = feature::archive_candidate(&paths, "proposed-export", &evidence)
        .expect("invariant: proposed candidate should classify");
    assert_eq!(
        proposed.action,
        feature::ArchiveCandidateAction::NeedsDecision
    );
    assert!(
        proposed.reasons[0].contains("decide whether to close"),
        "{proposed:?}"
    );

    let ready = feature::archive_candidate(&paths, "ready-export", &evidence)
        .expect("invariant: ready candidate should classify");
    assert_eq!(ready.action, feature::ArchiveCandidateAction::NeedsDecision);

    let active = feature::archive_candidate(&paths, "active-export", &evidence)
        .expect("invariant: active candidate should classify");
    assert_eq!(active.action, feature::ArchiveCandidateAction::NeedsClose);

    let closed = feature::archive_candidate(&paths, &closed_id, &evidence)
        .expect("invariant: closed candidate should classify");
    assert_eq!(closed.action, feature::ArchiveCandidateAction::ArchiveNow);
    assert_eq!(closed.child_tasks, 1);

    let all_plan =
        feature::archive_apply_plan(&paths, feature::ArchiveApplySelection::All, &evidence)
            .expect("invariant: all-target plan should build");
    assert_eq!(all_plan.archive_targets(), vec![closed_id.clone()]);

    feature::archive_feature(&paths, &closed_id, false).expect("invariant: archive should succeed");
    let archived = feature::archive_candidate(&paths, &closed_id, &evidence)
        .expect("invariant: archived candidate should classify");
    assert_eq!(
        archived.action,
        feature::ArchiveCandidateAction::ReleaseOnly
    );
    assert!(archived.archived);

    let one_plan = feature::archive_apply_plan(
        &paths,
        feature::ArchiveApplySelection::One(closed_id),
        &evidence,
    )
    .expect("invariant: one-target plan should build");
    assert!(one_plan.archive_targets().is_empty());
}

#[test]
fn archive_candidate_exact_head_evidence_blocks_when_stale_or_unsafe() {
    let temp = TestTempDir::new("maestro-feature-archive-exact-head");
    let root = temp.path();
    let repository = init_git_repo(root);
    let paths = MaestroPaths::new(root);
    let id = close_ready_feature(&paths, "Bounded Export");
    let card_path = paths.cards_dir().join(&id).join("card.yaml");
    let target_card_hash = if card_path.is_file() {
        sha256_prefixed(&fs::read(&card_path).expect("invariant: target card should be readable"))
    } else {
        let db_card = card::live_db::resolve(&paths, &id)
            .expect("invariant: live DB should be readable")
            .expect("invariant: target card should exist");
        sha256_prefixed(db_card.raw.as_bytes())
    };
    let head = commit_all(&repository, "closed feature ready for archive");
    let canonical_store = paths
        .maestro_dir()
        .canonicalize()
        .expect("invariant: maestro dir should canonicalize")
        .display()
        .to_string();
    let evidence = feature::ArchiveGateEvidence {
        authority_ref: Some("feature-close:bounded-export".to_string()),
        authority_target: Some(id.clone()),
        authority_head: Some(head.clone()),
        authority_state: Some("current".to_string()),
        tested_head: Some(head.clone()),
        qa_result: Some("pass".to_string()),
        qa_evidence: vec![format!("cargo test head={head}")],
        run_id: Some("run-auto".to_string()),
        canonical_store: Some(canonical_store.clone()),
        target_card_hash: Some(target_card_hash),
        allow_dirty_target_card: false,
    };

    let current = feature::archive_candidate(&paths, &id, &evidence)
        .expect("invariant: current evidence should classify");
    assert_eq!(current.action, feature::ArchiveCandidateAction::ArchiveNow);

    let mut expired = evidence.clone();
    expired.authority_state = Some("expired".to_string());
    let expired_candidate = feature::archive_candidate(&paths, &id, &expired)
        .expect("invariant: expired authority should classify");
    assert_eq!(
        expired_candidate.action,
        feature::ArchiveCandidateAction::Blocked
    );
    assert!(
        expired_candidate
            .reasons
            .iter()
            .any(|reason| reason.contains("not current")),
        "{expired_candidate:?}"
    );

    let mut wrong_head = evidence.clone();
    wrong_head.tested_head = Some("0000000000000000000000000000000000000000".to_string());
    let wrong_head_candidate = feature::archive_candidate(&paths, &id, &wrong_head)
        .expect("invariant: wrong head should classify");
    assert_eq!(
        wrong_head_candidate.action,
        feature::ArchiveCandidateAction::Blocked
    );
    assert!(
        wrong_head_candidate
            .reasons
            .iter()
            .any(|reason| reason.contains("does not match current HEAD")),
        "{wrong_head_candidate:?}"
    );

    let mut stale_store = evidence.clone();
    stale_store.canonical_store = Some(root.join("other-store").display().to_string());
    let stale_store_candidate = feature::archive_candidate(&paths, &id, &stale_store)
        .expect("invariant: stale store should classify");
    assert_eq!(
        stale_store_candidate.action,
        feature::ArchiveCandidateAction::Blocked
    );
    assert!(
        stale_store_candidate
            .reasons
            .iter()
            .any(|reason| reason.contains("not canonical store")),
        "{stale_store_candidate:?}"
    );

    let mut stale_snapshot = evidence.clone();
    stale_snapshot.target_card_hash = Some("sha256:0000".to_string());
    let stale_snapshot_candidate = feature::archive_candidate(&paths, &id, &stale_snapshot)
        .expect("invariant: stale snapshot should classify");
    assert_eq!(
        stale_snapshot_candidate.action,
        feature::ArchiveCandidateAction::Blocked
    );
    assert!(
        stale_snapshot_candidate
            .reasons
            .iter()
            .any(|reason| reason.contains("target card changed since preflight")),
        "{stale_snapshot_candidate:?}"
    );

    let dirty_dir = paths.cards_dir().join(&id);
    ensure_dir(&dirty_dir).expect("invariant: dirty note dir should be creatable");
    fs::write(dirty_dir.join("notes.md"), "dirty\n")
        .expect("invariant: dirty note should be writable");
    let dirty_candidate = feature::archive_candidate(&paths, &id, &evidence)
        .expect("invariant: dirty state should classify");
    assert_eq!(
        dirty_candidate.action,
        feature::ArchiveCandidateAction::Blocked
    );
    assert!(
        dirty_candidate
            .reasons
            .iter()
            .any(|reason| reason.contains("relevant dirty path")),
        "{dirty_candidate:?}"
    );
}

#[test]
fn illegal_transitions_name_the_gap() {
    let temp = TestTempDir::new("maestro-feature-illegal");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    // start before accept
    let error = feature::start(&paths, "billing-csv").expect_err("invariant: start must block");
    assert!(error.to_string().contains("not accepted"));

    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    // close before start
    let error = feature::close(&paths, "billing-csv", None, false)
        .expect_err("invariant: close must block");
    assert!(error.to_string().contains("not started"));
}

#[test]
fn completed_transitions_are_idempotent_no_ops() {
    let temp = TestTempDir::new("maestro-feature-noop");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");

    // accept again: no-op at exit 0
    let report =
        feature::accept(&paths, "billing-csv", false).expect("invariant: re-accept is a no-op");
    assert!(!report.changed);
    assert!(report.note.contains("already ready"));
}

#[test]
fn close_blocks_on_live_child_task() {
    let temp = TestTempDir::new("maestro-feature-close-block");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    feature::start(&paths, "billing-csv").expect("invariant: start should succeed");
    verify_contract(&paths, "billing-csv");

    write_task(&paths, "task-001", "billing-csv", "in_progress");
    let error = feature::close(&paths, "billing-csv", None, false)
        .expect_err("invariant: close must block");
    assert!(error.to_string().contains("task-001"));

    // A verified child does not block close.
    write_task(&paths, "task-001", "billing-csv", "verified");
    let closed =
        feature::close(&paths, "billing-csv", None, false).expect("invariant: close succeeds");
    assert_eq!(closed.status, feature::FeatureStatus::Closed);
}

#[test]
fn cancel_cascades_to_live_child_tasks() {
    let temp = TestTempDir::new("maestro-feature-cancel");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    feature::start(&paths, "billing-csv").expect("invariant: start should succeed");
    write_task(&paths, "task-001", "billing-csv", "in_progress");

    // Dry-run previews the cascade without mutating the feature or its children.
    let preview = feature::cancel(&paths, "billing-csv", "scope dropped", true)
        .expect("invariant: dry-run cancel succeeds");
    assert!(!preview.changed);
    assert_eq!(preview.abandoned, vec!["task-001".to_string()]);
    assert!(preview.note.contains("would cancel"));
    let still_live = feature::show(&paths, "billing-csv").expect("invariant: show should succeed");
    assert_eq!(still_live.status, feature::FeatureStatus::InProgress);

    let report = feature::cancel(&paths, "billing-csv", "scope dropped", false)
        .expect("invariant: cancel succeeds");
    assert!(report.changed);
    assert_eq!(report.abandoned, vec!["task-001".to_string()]);

    let view = feature::show(&paths, "billing-csv").expect("invariant: show should succeed");
    assert_eq!(view.status, feature::FeatureStatus::Cancelled);
    // The audited reason is persisted on the feature record.
    assert_eq!(view.cancel_reason.as_deref(), Some("scope dropped"));
    // The child task is now abandoned. Feature ownership rides the card's
    // `parent`, so the child lives at its own flat card dir, not under a feature
    // subtree; the cascade transitions it in place.
    let task_raw = fs::read_to_string(paths.cards_dir().join("task-001/card.yaml"))
        .expect("invariant: child task should be readable");
    assert!(task_raw.contains("abandoned"));
}

#[test]
fn cannot_cancel_a_closed_feature() {
    let temp = TestTempDir::new("maestro-feature-cancel-closed");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");
    feature::start(&paths, "billing-csv").expect("invariant: start should succeed");
    verify_contract(&paths, "billing-csv");
    feature::close(&paths, "billing-csv", None, false).expect("invariant: close should succeed");

    let error = feature::cancel(&paths, "billing-csv", "too late", false)
        .expect_err("invariant: closed features cannot be cancelled");
    assert!(error.to_string().contains("terminal"));
}

#[test]
fn amend_is_append_only_with_value_dedup() {
    let temp = TestTempDir::new("maestro-feature-amend");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    author_contract(&paths, "billing-csv");
    feature::accept(&paths, "billing-csv", false).expect("invariant: accept should succeed");

    // amend grows the frozen contract
    let report = feature::amend(
        &paths,
        "billing-csv",
        ContractAdditions {
            acceptance: vec!["handles empty rows".to_string()],
            ..Default::default()
        },
        "widen scope",
    )
    .expect("invariant: amend should succeed");
    assert!(report.changed);
    assert_eq!(
        report.added.acceptance,
        vec!["handles empty rows".to_string()]
    );

    // re-adding a present value is a no-op (safe retries)
    let report = feature::amend(
        &paths,
        "billing-csv",
        ContractAdditions {
            acceptance: vec!["handles empty rows".to_string()],
            ..Default::default()
        },
        "retry",
    )
    .expect("invariant: amend retry should succeed");
    assert!(!report.changed);

    // the feature record (folded under the card's `extra`) embeds the one genuine amend
    let feature_raw = serde_yaml::to_string(
        &card::store::resolve(&paths, "billing-csv")
            .expect("invariant: feature should resolve")
            .expect("invariant: feature should exist")
            .card,
    )
    .expect("invariant: feature should serialize");
    assert!(feature_raw.contains("widen scope"));
    assert!(!feature_raw.contains("retry"));
}

#[test]
fn amend_rejects_a_proposed_feature() {
    let temp = TestTempDir::new("maestro-feature-amend-proposed");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    let error = feature::amend(
        &paths,
        "billing-csv",
        ContractAdditions {
            acceptance: vec!["x".to_string()],
            ..Default::default()
        },
        "too early",
    )
    .expect_err("invariant: amend must be rejected before accept");
    assert!(error.to_string().contains("not accepted"));
}

#[test]
fn list_joins_task_counts() {
    let temp = TestTempDir::new("maestro-feature-counts");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");
    write_task(&paths, "task-001", "billing-csv", "verified");
    write_task(&paths, "task-002", "billing-csv", "ready");

    let views = feature::list(&paths).expect("invariant: list should succeed");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].counts.total, 2);
    assert_eq!(views[0].counts.verified, 1);
}

#[test]
fn strict_list_errors_on_incompatible_record() {
    let temp = TestTempDir::new("maestro-feature-strict");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", BAD_RECORD);

    let error = feature::list(&paths).expect_err("invariant: strict read must error on bad schema");
    assert!(error.to_string().contains("schema"));
}

#[test]
fn strict_show_errors_on_incompatible_record() {
    let temp = TestTempDir::new("maestro-feature-strict-show");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", BAD_RECORD);

    let error = feature::show(&paths, "billing-csv")
        .expect_err("invariant: strict read must error on bad schema");
    assert!(error.to_string().contains("schema"));
}

#[test]
fn tolerant_titles_degrade_to_empty_on_incompatible_record() {
    let temp = TestTempDir::new("maestro-feature-tolerant");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", BAD_RECORD);

    let titles = feature::titles(&paths);
    assert!(titles.is_empty(), "tolerant titles must skip a bad record");
}

#[test]
fn tolerant_titles_degrade_to_empty_on_unparseable_record() {
    let temp = TestTempDir::new("maestro-feature-tolerant-parse");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", "this: is: not: valid: yaml: [");

    let titles = feature::titles(&paths);
    assert!(titles.is_empty(), "tolerant titles must skip a parse error");
}

#[test]
fn tolerant_titles_return_id_to_title_map() {
    let temp = TestTempDir::new("maestro-feature-titles");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");

    let titles = feature::titles(&paths);
    assert_eq!(
        titles.get("billing-csv").map(String::as_str),
        Some("Billing CSV")
    );
}

#[test]
fn tolerant_roster_marks_incompatible_record_without_dropping_healthy_rows() {
    let temp = TestTempDir::new("maestro-feature-tolerant-roster");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Healthy Feature", None).expect("invariant: create should succeed");
    write_feature(&paths, "billing-csv", BAD_RECORD);

    let roster = feature::list_tolerant(&paths);
    assert!(
        roster
            .iter()
            .any(|entry| matches!(entry, feature::FeatureRosterEntry::Loaded(view) if view.id == "healthy-feature")),
        "healthy rows must survive a sibling schema mismatch: {roster:#?}"
    );
    assert!(
        roster
            .iter()
            .any(|entry| matches!(entry, feature::FeatureRosterEntry::Unreadable { id, error, .. } if id == "billing-csv" && error.contains("schema mismatch"))),
        "bad rows must be marked instead of silently dropped: {roster:#?}"
    );
}

/// A card.yaml that fails to even parse as a card must still surface on the
/// roster (outer load error), while a corrupt non-feature card stays off the
/// feature board -- its own surfaces report it.
#[test]
fn tolerant_roster_surfaces_a_card_that_fails_to_load() {
    let temp = TestTempDir::new("maestro-feature-tolerant-load-err");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Healthy Feature", None).expect("invariant: create should succeed");
    write_feature(&paths, "billing-csv", "type: feature\nbroken: [");
    write_feature(&paths, "card-broken1", "type: task\nbroken: [");

    let roster = feature::list_tolerant(&paths);
    assert!(
        roster
            .iter()
            .any(|entry| matches!(entry, feature::FeatureRosterEntry::Loaded(view) if view.id == "healthy-feature")),
        "healthy rows must survive a sibling load failure: {roster:#?}"
    );
    assert!(
        roster
            .iter()
            .any(|entry| matches!(entry, feature::FeatureRosterEntry::Unreadable { id, .. } if id == "billing-csv")),
        "a feature card that fails to load is marked, not dropped: {roster:#?}"
    );
    assert!(
        !roster
            .iter()
            .any(|entry| matches!(entry, feature::FeatureRosterEntry::Unreadable { id, .. } if id == "card-broken1")),
        "a corrupt card declaring a non-feature type stays off the feature board: {roster:#?}"
    );
}

/// Load the card set the way doctor does (one shared store walk), keeping only
/// the loadable cards -- envelope failures are the walk's caller's to report.
fn loaded_cards(paths: &MaestroPaths) -> Vec<(Card, std::path::PathBuf)> {
    maestro::domain::card::query::scan_with_failures(paths)
        .expect("invariant: card store should be walkable")
        .cards
}

#[test]
fn diagnose_reports_count_on_compatible_store() {
    let temp = TestTempDir::new("maestro-feature-diag-ok");
    let paths = MaestroPaths::new(temp.path());

    feature::create(&paths, "Billing CSV", None).expect("invariant: create should succeed");

    assert_eq!(feature::diagnose(&loaded_cards(&paths)).found, Ok(1));
}

#[test]
fn diagnose_reports_absent_card_store_as_zero_features() {
    let temp = TestTempDir::new("maestro-feature-diag-absent");
    let paths = MaestroPaths::new(temp.path());

    // Feature cards live in the flat card store, not a per-entity directory, so an
    // absent store reads as zero features rather than a missing-directory error
    // (the directory-shaped diagnostic retired with the cutover).
    assert_eq!(feature::diagnose(&loaded_cards(&paths)).found, Ok(0));
}

#[test]
fn diagnose_reports_incompatible_record_as_error_data() {
    let temp = TestTempDir::new("maestro-feature-diag-bad");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", BAD_RECORD);

    assert!(
        feature::diagnose(&loaded_cards(&paths)).found.is_err(),
        "an incompatible record must report as error data"
    );
}

#[test]
fn unparseable_envelope_is_the_shared_walks_failure_not_a_feature_error() {
    let temp = TestTempDir::new("maestro-feature-diag-parse");
    let paths = MaestroPaths::new(temp.path());
    write_feature(&paths, "billing-csv", "this: is: not: valid: yaml: [");

    // An unparseable card.yaml has an unknowable type, so it surfaces once as
    // the shared walk's failure; diagnose only sees the loadable cards.
    let scan = maestro::domain::card::query::scan_with_failures(&paths)
        .expect("invariant: card store should be walkable");
    assert_eq!(
        scan.failures.len(),
        1,
        "the corrupt envelope is the walk's failure"
    );
    assert_eq!(feature::diagnose(&scan.cards).found, Ok(0));
}

#[test]
fn status_label_renders_snake_case() {
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::Proposed),
        "proposed"
    );
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::Ready),
        "ready"
    );
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::InProgress),
        "in_progress"
    );
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::Closed),
        "closed"
    );
    assert_eq!(
        feature::status_label(&feature::FeatureStatus::Cancelled),
        "cancelled"
    );
}
