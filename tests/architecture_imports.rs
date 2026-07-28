use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const TARGET_MODULE_ROOTS: &[&str] = &[
    "src/interfaces/mod.rs",
    "src/domain/mod.rs",
    "src/operations/mod.rs",
    "src/foundation/mod.rs",
];

const LEGACY_COMPATIBILITY_ROOTS: &[&str] = &["hooks", "mcp", "tui"];

const INTERFACE_SCAN_ROOTS: &[&str] = &["src/interfaces"];
const PRODUCTION_SCAN_ROOTS: &[&str] = &["src"];
const CLI_TRANSITIONAL_LEGACY_IMPORTS: &[(&str, &[&str])] = &[
    ("src/interfaces/cli/doctor.rs", &[]),
    ("src/interfaces/cli/harness.rs", &[]),
    ("src/interfaces/cli/init.rs", &[]),
    ("src/interfaces/cli/mod.rs", &["domain::feature"]),
    ("src/interfaces/cli/query.rs", &[]),
    ("src/interfaces/cli/task.rs", &[]),
    ("src/interfaces/cli/update.rs", &[]),
    ("src/interfaces/cli/watch.rs", &[]),
];

const DOMAIN_FACADES: &[&str] = &[
    "card",
    "decisions",
    "design",
    "extraction",
    "feature",
    "harness",
    "install",
    "playbook",
    "proof",
    "run",
    "search",
    "skills",
    "task",
];

const OPERATION_FACADES: &[&str] = &["harness", "init", "sync", "update"];

const RESOURCE_EMBED_ALLOWLIST: &[(&str, &[&str])] = &[
    (
        "src/domain/harness/templates.rs",
        &[
            "embedded/harness/HARNESS.md",
            "embedded/harness/RECOVERY.md",
        ],
    ),
    ("src/domain/run/event.rs", &["embedded/hooks/events.yaml"]),
    (
        "src/domain/extraction/hook_script.rs",
        &["embedded/hooks/record.sh"],
    ),
    ("src/domain/playbook.rs", &["embedded/playbook"]),
    ("src/domain/design/legacy.rs", &["embedded/design"]),
    ("src/domain/loop_recipes.rs", &["embedded/loop-recipes"]),
    ("src/domain/skills/catalog.rs", &["embedded/skills"]),
    (
        "src/domain/schema_contracts/catalog.rs",
        &["embedded/schemas"],
    ),
    (
        "src/domain/orchestration/runtime/catalog.rs",
        &[
            "embedded/vnext/orchestration/recipe-catalog.v1.json",
            "embedded/vnext/orchestration/profiles/bounded-continuation/",
            "embedded/vnext/orchestration/recipes/",
        ],
    ),
    ("src/interfaces/shell/mod.rs", &["embedded/shell/"]),
    (
        "src/interfaces/connectors/mod.rs",
        &[
            "embedded/vnext/hosts/",
            "embedded/vnext/bootstrap/wiring.v1.json",
        ],
    ),
    (
        "src/interfaces/mcp/mod.rs",
        &["embedded/vnext/adapter/mcp-tools.v1.json"],
    ),
];

#[test]
fn target_module_roots_exist_and_legacy_roots_remain() {
    let lib = read_source_file(Path::new("src/lib.rs"));

    for root in TARGET_MODULE_ROOTS {
        assert!(
            Path::new(root).exists(),
            "missing target module root required by the architecture plan: {root}"
        );

        let module_name = module_name_from_root(root);
        assert!(
            lib.contains(&format!("pub mod {module_name};")),
            "src/lib.rs must expose crate::{module_name}"
        );
    }

    for legacy_root in LEGACY_COMPATIBILITY_ROOTS {
        assert!(
            lib_exposes_crate_root(&lib, legacy_root),
            "legacy crate::{legacy_root} must remain during the compatibility migration"
        );
    }
}

#[test]
fn selected_compatibility_smoke_paths_resolve() {
    let _decision_file_name: fn(u32, &str) -> String =
        maestro::domain::decisions::template::decision_file_name;

    let _ = std::any::type_name::<maestro::interfaces::cli::Cli>();
    let _ = std::any::type_name::<maestro::domain::harness::HarnessConfig>();
    let _ = std::any::type_name::<maestro::domain::feature::schema::FeatureRecord>();
    let _ = std::any::type_name::<maestro::domain::task::TaskRecord>();
    let _ = std::any::type_name::<maestro::domain::proof::ProofStatusKind>();

    let _legacy_task_watch_render: fn(
        &maestro::foundation::core::paths::MaestroPaths,
        &[maestro::domain::task::TaskRecord],
    ) -> anyhow::Result<String> = maestro::tui::task_list_watch::render_snapshot;
    let _new_task_watch_render: fn(
        &maestro::foundation::core::paths::MaestroPaths,
        &[maestro::domain::task::TaskRecord],
    ) -> anyhow::Result<String> = maestro::interfaces::tui::task_list_watch::render_snapshot;

    assert_eq!(
        std::any::type_name::<maestro::interfaces::mcp::tools::ToolDefinition>(),
        std::any::type_name::<maestro::mcp::tools::ToolDefinition>()
    );
    let _legacy_tool_definitions: fn() -> Vec<maestro::mcp::tools::ToolDefinition> =
        maestro::mcp::tools::tool_definitions;
    let _new_tool_definitions: fn() -> Vec<maestro::interfaces::mcp::tools::ToolDefinition> =
        maestro::interfaces::mcp::tools::tool_definitions;
    let _legacy_mcp_serve: fn() -> anyhow::Result<()> = maestro::mcp::server::serve;
    let _new_mcp_serve: fn() -> anyhow::Result<()> = maestro::interfaces::mcp::server::serve;

    assert_eq!(
        maestro::interfaces::hooks::event::shared_hook_events(),
        maestro::hooks::event::shared_hook_events()
    );
    let legacy_run_dir_name: fn(&str) -> String = maestro::hooks::event::run_dir_name;
    let new_run_dir_name: fn(&str) -> String = maestro::interfaces::hooks::event::run_dir_name;
    assert_eq!(
        new_run_dir_name("agent/session"),
        legacy_run_dir_name("agent/session")
    );
    assert_eq!(
        maestro::domain::run::hook_event_contract().shared_events(),
        maestro::hooks::event::shared_hook_events()
    );
}

#[test]
fn task_domain_facade_does_not_publish_leaf_modules() {
    let task_facade = read_source_file(Path::new("src/domain/task/mod.rs"));
    for leaf in [
        "blockers",
        "display",
        "doctor",
        "lifecycle",
        "lookup",
        "template",
    ] {
        assert!(
            !task_facade.contains(&format!("pub mod {leaf};")),
            "src/domain/task/mod.rs should expose Task through root facade exports, not pub mod {leaf}"
        );
    }
    for persistence_type in ["TaskSnapshot", "StateHistoryEntry"] {
        assert!(
            !public_reexports(&task_facade)
                .iter()
                .any(|line| line.contains(persistence_type)),
            "src/domain/task/mod.rs should not re-export persistence-only type {persistence_type}"
        );
    }
    assert!(
        !task_facade.contains("pub fn apply_verification_outcome_with_snapshot"),
        "src/domain/task/mod.rs should not expose snapshot-shaped persistence functions"
    );
    for verification_write_surface in [
        "pub struct TaskHandle",
        "pub fn load_task_for_update",
        "pub fn apply_verification_outcome(",
        "pub fn apply_verification_outcome_to_handle",
        "pub enum VerificationOutcome",
        "pub struct VerificationPassed",
    ] {
        assert!(
            !task_facade.contains(verification_write_surface),
            "src/domain/task/mod.rs should not expose verification write surface {verification_write_surface}"
        );
    }
    for line in public_reexports(&task_facade) {
        for verification_write_surface in [
            "TaskHandle",
            "VerificationOutcome",
            "VerificationPassed",
            "apply_verification_outcome",
            "apply_verification_outcome_to_handle",
            "load_task_for_update",
        ] {
            assert!(
                !line.contains(verification_write_surface),
                "src/domain/task/mod.rs should not publicly re-export verification write surface {verification_write_surface}: {line}"
            );
        }
    }

    assert!(
        !Path::new("src/task/mod.rs").exists(),
        "legacy crate::task shim should stay retired; tests that need leaf access belong in-crate"
    );
}

#[test]
fn interfaces_obtain_features_through_domain_facade() {
    let mut violations = Vec::new();

    for root in INTERFACE_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            let source = source_without_test_modules(&read_source_file(&file));
            let code = code_for_path_scan(&source);

            for symbol in ["FeatureRegistry", "FeatureRecord"] {
                if code.contains(symbol) {
                    violations.push(format!(
                        "{} constructs feature registry internal {symbol}; obtain features via crate::domain::feature",
                        file.display()
                    ));
                }
            }

            // `code_for_path_scan` blanks string literals, so scan the
            // comment-free source for the registry path literal directly.
            if source.contains("\"features.yaml\"") {
                violations.push(format!(
                    "{} reads the features.yaml registry directly; route through crate::domain::feature",
                    file.display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "interface code must obtain features through the domain::feature facade, not by reading or constructing the registry:\n{}",
        violations.join("\n")
    );
}

#[test]
fn interfaces_enumerate_decisions_through_domain_facade() {
    let mut violations = Vec::new();

    for root in INTERFACE_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            let source = source_without_test_modules(&read_source_file(&file));

            // The decision file-naming predicate (a `decision-` prefix plus a
            // `.md` suffix) must live only in crate::domain::decisions. An
            // interface file that encodes both literals is re-deriving the
            // registry's on-disk shape instead of routing through
            // decision_entries; bare `decision-` (a blocker-target prefix) is
            // a reference classification and stays allowed.
            if source.contains("\"decision-\"") && source.contains("\".md\"") {
                violations.push(format!(
                    "{} encodes the decision-*.md file predicate; enumerate decisions via crate::domain::decisions::decision_entries",
                    file.display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "interface code must enumerate decisions through the domain::decisions facade, not by re-deriving the decision-*.md file predicate:\n{}",
        violations.join("\n")
    );
}

#[test]
fn interfaces_record_run_events_through_domain_facade() {
    let mut violations = Vec::new();

    for root in INTERFACE_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            let source = source_without_test_modules(&read_source_file(&file));

            // The managed run event log path (`events.jsonl`) is constructed
            // only inside crate::domain::run. An interface file that names the
            // literal is hand-rolling the run-log location instead of reading
            // through run::visit_managed_events / proof and writing through
            // proof::record_claim.
            if source.contains("\"events.jsonl\"") {
                violations.push(format!(
                    "{} names the events.jsonl run-log path; record and read run events via crate::domain::run / crate::domain::proof",
                    file.display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "interface code must record and read run events through the domain facades, not by naming the events.jsonl log path:\n{}",
        violations.join("\n")
    );
}

#[test]
fn feature_domain_facade_exposes_the_deliberate_surface() {
    let feature_facade = read_source_file(Path::new("src/domain/feature/mod.rs"));
    assert_eq!(
        public_reexport_item_names(&feature_facade),
        BTreeSet::from([
            "AmendReport".to_string(),
            "AcceptanceCoverage".to_string(),
            "AcceptanceProof".to_string(),
            "AcceptanceSweepItem".to_string(),
            "AcceptanceSweepReport".to_string(),
            "AcceptanceTextEdit".to_string(),
            "ArchiveApplyPlan".to_string(),
            "ArchiveApplySelection".to_string(),
            "ArchiveCandidate".to_string(),
            "ArchiveCandidateAction".to_string(),
            "ArchiveGateEvidence".to_string(),
            "AutoArchiveReceipt".to_string(),
            "CancelReport".to_string(),
            "ContractAdditions".to_string(),
            "ContractChangeCounts".to_string(),
            "ContractEdits".to_string(),
            "FeatureArchiveReport".to_string(),
            "FeatureDiagnostic".to_string(),
            "FeatureProofUpdate".to_string(),
            "FeatureRosterEntry".to_string(),
            "FeatureStatus".to_string(),
            "FeatureVerifyReport".to_string(),
            "FeatureView".to_string(),
            "FinalizeReport".to_string(),
            "LooseSweepReport".to_string(),
            "NoteReport".to_string(),
            "QuestionRemovalEdit".to_string(),
            "ReconcileAction".to_string(),
            "ReconcileActor".to_string(),
            "ReconcileContract".to_string(),
            "ReconcileIssue".to_string(),
            "ReconcileIssueRef".to_string(),
            "ReconcileQuestions".to_string(),
            "ReconcileReceipt".to_string(),
            "ReconcileReport".to_string(),
            "ReconcileStatus".to_string(),
            "ReconcileSurfaceError".to_string(),
            "ReconcileTasks".to_string(),
            "ReconcileTextItem".to_string(),
            "ReopenReport".to_string(),
            "RETIRE_REMINDER".to_string(),
            "STALE_PROPOSED_THRESHOLD_DAYS".to_string(),
            "SetReport".to_string(),
            "SpecSectionReport".to_string(),
            "TransitionReport".to_string(),
            "accept".to_string(),
            "accept_with_qa_none".to_string(),
            "accept_with_qa_surface".to_string(),
            "acceptance_coverage".to_string(),
            "acceptance_coverage_archived".to_string(),
            "acceptance_id".to_string(),
            "age_days".to_string(),
            "amend".to_string(),
            "amend_log_position".to_string(),
            "append_auto_archive_receipt".to_string(),
            "apply_reconcile_plan".to_string(),
            "archive_apply_plan".to_string(),
            "archive_candidate".to_string(),
            "archive_candidates".to_string(),
            "archive_feature".to_string(),
            "archive_feature_with_expected_hash".to_string(),
            "archive_loose".to_string(),
            "cancel".to_string(),
            "create".to_string(),
            "current_acceptance_sweep".to_string(),
            "diagnose".to_string(),
            "ensure_exists".to_string(),
            "feature_sidecar_dir".to_string(),
            "finalize".to_string(),
            "finalize_requires_reopen".to_string(),
            "handoff_gap".to_string(),
            "is_stale_proposed".to_string(),
            "list".to_string(),
            "list_archived".to_string(),
            "list_tolerant".to_string(),
            "list_tolerant_with_entries".to_string(),
            "list_with_entries".to_string(),
            "normalize_acceptance_id".to_string(),
            "note".to_string(),
            "read_design_text".to_string(),
            "read_sidecar_text".to_string(),
            "reconcile_clean_check".to_string(),
            "reconcile_plan_template".to_string(),
            "reconcile_receipt_is_current".to_string(),
            "reconcile_report".to_string(),
            "reopen".to_string(),
            "set".to_string(),
            "set_with_report".to_string(),
            "close".to_string(),
            "close_gaps".to_string(),
            "show".to_string(),
            "show_archived".to_string(),
            "start".to_string(),
            "status".to_string(),
            "status_label".to_string(),
            "titles".to_string(),
            "unarchive_feature".to_string(),
            "uncovered_acceptance".to_string(),
            "verified_child_commit_drift".to_string(),
            "verify_feature".to_string(),
            "write_design_section".to_string(),
            "write_sidecar_text".to_string(),
            "write_spec_section".to_string(),
        ]),
        "src/domain/feature/mod.rs should expose only the deliberate feature facade surface"
    );
    assert!(
        feature_facade.contains("pub(crate) mod registry;"),
        "src/domain/feature/mod.rs should keep the registry operation surface private to the crate"
    );
}

#[test]
fn proof_domain_facade_does_not_publish_leaf_modules() {
    let proof_facade = read_source_file(Path::new("src/domain/proof/mod.rs"));
    assert_eq!(
        public_reexport_item_names(&proof_facade),
        BTreeSet::from([
            "ProofStatus".to_string(),
            "ProofStatusKind".to_string(),
            "ProofStatusSource".to_string(),
            "ProofStaleReason".to_string(),
            "RECONCILE_RECEIPT_TYPE".to_string(),
            "ReceiptExtension".to_string(),
            "TaskVerification".to_string(),
            "TaskVerificationStatus".to_string(),
            "load_receipt_extension".to_string(),
            "managed_event_files".to_string(),
            "needs_verification_proof_status_kind_for_task".to_string(),
            "proof_status".to_string(),
            "proof_status_for_task".to_string(),
            "proof_status_kind_for_task".to_string(),
            "record_claim".to_string(),
            "render_proof_status".to_string(),
            "store_receipt_extension".to_string(),
            "store_reconcile_receipt_extension".to_string(),
        ]),
        "src/domain/proof/mod.rs should expose only the deliberate Proof facade surface"
    );
    for leaf in [
        "claims",
        "commands",
        "events",
        "proof_status",
        "receipts",
        "stale",
        "verify_task",
    ] {
        assert!(
            !proof_facade.contains(&format!("pub mod {leaf};")),
            "src/domain/proof/mod.rs should expose Proof through root facade exports, not pub mod {leaf}"
        );
    }
}

#[test]
fn run_domain_facade_does_not_publish_leaf_modules() {
    let run_facade = read_source_file(Path::new("src/domain/run/mod.rs"));
    for leaf in [
        "append",
        "autonomy",
        "discovery",
        "evidence",
        "event",
        "reader",
        "record",
    ] {
        assert!(
            !run_facade.contains(&format!("pub mod {leaf};")),
            "src/domain/run/mod.rs should expose Run through root facade exports, not pub mod {leaf}"
        );
        assert!(
            !run_facade.contains(&format!("pub(crate) mod {leaf};")),
            "src/domain/run/mod.rs should keep Run leaf module {leaf} private"
        );
    }
    assert_eq!(
        public_reexport_item_names(&run_facade),
        BTreeSet::from([
            "active_sessions".to_string(),
            "active_sessions_union".to_string(),
            "assemble_autonomy_report".to_string(),
            "assemble_trace".to_string(),
            "current_bound_card".to_string(),
            "hook_event_contract".to_string(),
            "load_run_evidence".to_string(),
            "managed_event_logs".to_string(),
            "session_activity_counts".to_string(),
            "session_readout".to_string(),
            "session_readout_with_transcript".to_string(),
            "union_session_id".to_string(),
            "visit_managed_event_logs".to_string(),
            "visit_managed_events".to_string(),
            "warm_file_overlaps".to_string(),
            "write_evidence_for_session".to_string(),
            "ActivityCounts".to_string(),
            "AutonomyActionRow".to_string(),
            "AutonomyReport".to_string(),
            "FileOverlap".to_string(),
            "HookEventContract".to_string(),
            "Presence".to_string(),
            "RunEvent".to_string(),
            "RunEventLog".to_string(),
            "RunEventRecord".to_string(),
            "RunEvidenceLoad".to_string(),
            "RunEvidenceRecord".to_string(),
            "RunStatus".to_string(),
            "RunTrace".to_string(),
            "SessionActivity".to_string(),
            "SessionActivitySummary".to_string(),
            "SessionLifecycleSummary".to_string(),
            "SessionProofSummary".to_string(),
            "SessionReadout".to_string(),
            "SessionSources".to_string(),
            "SessionTaskSummary".to_string(),
            "SessionTranscript".to_string(),
            "SessionTranscriptEntry".to_string(),
            "TraceEntry".to_string(),
            "WarmEditor".to_string(),
        ]),
        "src/domain/run/mod.rs should expose only the deliberate Run contract surface"
    );
    for leaked_helper in [
        "event_files_under",
        "is_accepted_event",
        "managed_event_files",
        "managed_run_evidence_files",
        "normalized_event_type",
        "read_event_records",
        "run_dir_name",
        "run_evidence_files_under",
        "string_field",
        "visit_event_log",
        "UNATTRIBUTED_SESSION",
    ] {
        assert!(
            !public_reexport_item_names(&run_facade).contains(leaked_helper),
            "src/domain/run/mod.rs should not leak low-level Run helper {leaked_helper}"
        );
    }
}

#[test]
fn install_domain_facade_does_not_publish_leaf_modules() {
    let install_facade = read_source_file(Path::new("src/domain/install/mod.rs"));
    for leaf in ["hooks", "lock", "mirrors"] {
        assert!(
            !install_facade.contains(&format!("pub mod {leaf};")),
            "src/domain/install/mod.rs should expose Install through root facade exports, not pub mod {leaf}"
        );
        assert!(
            !install_facade.contains(&format!("pub(crate) mod {leaf};")),
            "src/domain/install/mod.rs should keep Install leaf module {leaf} private"
        );
    }
    assert_eq!(
        public_modules(&install_facade),
        BTreeSet::new(),
        "src/domain/install/mod.rs should not publish leaf modules"
    );
    assert_eq!(
        public_reexport_item_names(&install_facade),
        BTreeSet::from([
            "AgentInstall".to_string(),
            "FileOwnership".to_string(),
            "InstallLock".to_string(),
            "InstallMirrorAction".to_string(),
            "InstallMirrorPreview".to_string(),
            "InstallState".to_string(),
            "MirrorBlockFate".to_string(),
            "MirrorBlockSync".to_string(),
            "MirrorKind".to_string(),
            "MirrorPlan".to_string(),
            "mirror_plan".to_string(),
            "preview_mirror_block_resync".to_string(),
            "resync_mirror_blocks".to_string(),
        ]),
        "src/domain/install/mod.rs should expose only deliberate Install contract re-exports"
    );
    for root_item in [
        "pub enum InstallAgent",
        "pub fn install_agent",
        "pub fn uninstall_agent",
    ] {
        assert!(
            install_facade.contains(root_item),
            "src/domain/install/mod.rs should expose root facade item {root_item}"
        );
    }
}

#[test]
fn update_operation_owns_implementation() {
    assert!(
        Path::new("src/operations/update/mod.rs").is_file(),
        "Update implementation should live under src/operations/update"
    );

    for leaf in ["github_release.rs", "replace.rs"] {
        assert!(
            Path::new(&format!("src/operations/update/{leaf}")).is_file(),
            "Update implementation should split the GitHub-release and replacement \
             concerns into src/operations/update/{leaf}"
        );
    }

    let operations_facade = read_source_file(Path::new("src/operations/update/mod.rs"));
    for item in [
        "run_update",
        "run_update_with_seams",
        "detect_install_method",
        "detect_schema_mismatches",
        "InstallMethod",
        "UpdateOutcome",
        "mod github_release;",
        "mod replace;",
    ] {
        assert!(
            operations_facade.contains(item),
            "operations/update facade should expose {item}"
        );
    }
    assert_eq!(
        public_modules(&operations_facade),
        BTreeSet::new(),
        "operations/update should keep the split leaf modules private"
    );
    assert_eq!(
        public_reexport_item_names(&operations_facade),
        BTreeSet::from([
            "AtomicBinaryReplacer".to_string(),
            "GitHubCurlDownloader".to_string(),
        ]),
        "operations/update should re-export only the relocated downloader and replacer"
    );

    let mut violations = Vec::new();
    for file in rust_files_under(Path::new("src")) {
        let source = source_without_test_modules(&read_source_file(&file));
        let code = code_for_path_scan(&source);
        if code.contains("crate::update::") {
            violations.push(format!("{} imports legacy crate::update", file.display()));
            continue;
        }
        for (line_number, import_statement) in crate_import_statements(&source) {
            if contains_legacy_root_import(&import_statement, "update")
                || contains_legacy_deep_import(&import_statement, "update")
            {
                violations.push(format!(
                    "{}:{} imports legacy crate::update path",
                    file.display(),
                    line_number
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "production code should use operations::update instead of the legacy shim:\n{}",
        violations.join("\n")
    );
}

#[test]
fn harness_operation_owns_implementation() {
    for leaf in ["detect.rs", "friction.rs", "policy.rs", "propose.rs"] {
        assert!(
            Path::new(&format!("src/operations/harness/{leaf}")).is_file(),
            "Harness implementation should live under src/operations/harness"
        );
        assert!(
            !Path::new(&format!("src/harness/{leaf}")).exists(),
            "legacy src/harness should not own implementation file {leaf}"
        );
    }

    let operations_facade = read_source_file(Path::new("src/operations/harness/mod.rs"));
    for item in [
        "mod detect;",
        "mod friction;",
        "mod policy;",
        "mod propose;",
        "pub use detect::detect;",
        "pub use friction::looks_like_correction;",
        "pub use propose::{",
    ] {
        assert!(
            operations_facade.contains(item),
            "operations/harness facade should expose {item}"
        );
    }
    assert_eq!(
        public_modules(&operations_facade),
        BTreeSet::new(),
        "operations/harness should keep leaf modules private"
    );
    assert_eq!(
        public_reexport_item_names(&operations_facade),
        BTreeSet::from([
            "apply".to_string(),
            "AppliedItem".to_string(),
            "audit_overdue_hint".to_string(),
            "AuditHint".to_string(),
            "complete_readout".to_string(),
            "complete_readout_for_roots".to_string(),
            "CompleteHarnessReadout".to_string(),
            "dismiss".to_string(),
            "detect".to_string(),
            "guardrail_decision_line".to_string(),
            "guardrail_memory_line".to_string(),
            "guardrail_scorer_line".to_string(),
            "guardrail_task_check_line".to_string(),
            "load_backlog".to_string(),
            "load_backlog_in_cards".to_string(),
            "load_config".to_string(),
            "looks_like_correction".to_string(),
            "measure".to_string(),
            "over_threshold_items".to_string(),
            "OverThresholdItem".to_string(),
            "propose_agent_audit".to_string(),
            "refresh".to_string(),
            "scheduler_readout".to_string(),
            "scheduler_readout_for_roots".to_string(),
            "scheduler_surface_line".to_string(),
            "SchedulerReadout".to_string(),
            "security_decision_gate_line".to_string(),
            "security_feature_gate_line".to_string(),
            "security_qa_gate_line".to_string(),
            "security_task_gate_line".to_string(),
            "set_claims_only_verification".to_string(),
            "unapply".to_string(),
            "UnappliedItem".to_string(),
            "UnappliedTask".to_string(),
        ]),
        "operations/harness should expose only deliberate root facade symbols"
    );

    let harness_adapter = read_source_file(Path::new("src/interfaces/cli/harness.rs"));
    assert!(
        harness_adapter.contains("use crate::operations::harness;"),
        "CLI Harness adapter should call operations::harness"
    );
    let harness_detect = read_source_file(Path::new("src/operations/harness/detect.rs"));
    assert!(
        harness_detect.contains("use crate::domain::proof;"),
        "Harness detection should read Proof-owned data through the Proof facade"
    );
    for forbidden in [
        "join(\"verification.json\")",
        "serde_json::Value",
        "serde_json::from_str",
        "domain::proof::compatibility",
        "crate::verification",
    ] {
        assert!(
            !harness_detect.contains(forbidden),
            "Harness detection should not bypass the Proof facade with {forbidden}"
        );
    }
    assert_production_sources_use_operation_root_facade(
        "harness",
        &["detect", "friction", "propose"],
    );
}

#[test]
fn operations_do_not_depend_on_interfaces() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src/operations")) {
        let source = read_source_file(&file);
        let code = source_without_pub_mod_statements(&code_for_path_scan(&source));
        if code.contains("crate::interfaces") {
            violations.push(format!(
                "{} references crate::interfaces directly",
                file.display()
            ));
            continue;
        }
        if contains_bare_path_reference(&code, "interfaces") {
            violations.push(format!(
                "{} references Interfaces through a bare root path",
                file.display()
            ));
            continue;
        }
        if contains_relative_path(&code, "interfaces") {
            violations.push(format!(
                "{} references Interfaces through a relative path",
                file.display()
            ));
            continue;
        }

        for (line_number, import_statement) in crate_import_statements(&source) {
            if import_statement.contains("crate::interfaces")
                || contains_legacy_root_import(&import_statement, "interfaces")
                || contains_legacy_deep_import(&import_statement, "interfaces")
            {
                violations.push(format!(
                    "{}:{} imports Interfaces",
                    file.display(),
                    line_number
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Operations must not depend on Interfaces:\n{}",
        violations.join("\n")
    );
}

#[test]
fn domain_does_not_depend_on_interfaces_or_operations() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src/domain")) {
        let source = source_without_test_modules(&read_source_file(&file));
        let code = source_without_pub_mod_statements(&code_for_path_scan(&source));
        for upstream in ["interfaces", "operations"] {
            if code.contains(&format!("crate::{upstream}")) {
                violations.push(format!(
                    "{} references crate::{upstream} directly",
                    file.display()
                ));
                continue;
            }
            if contains_bare_path_reference(&code, upstream) {
                violations.push(format!(
                    "{} references {upstream} through a bare root path",
                    file.display()
                ));
                continue;
            }
            if contains_relative_path(&code, upstream) {
                violations.push(format!(
                    "{} references {upstream} through a relative path",
                    file.display()
                ));
            }
        }

        for (line_number, import_statement) in crate_import_statements(&source) {
            for upstream in ["interfaces", "operations"] {
                if import_statement.contains(&format!("crate::{upstream}"))
                    || contains_legacy_root_import(&import_statement, upstream)
                    || contains_legacy_deep_import(&import_statement, upstream)
                {
                    violations.push(format!(
                        "{}:{} imports {upstream}",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Domain must not depend on Interfaces or Operations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn foundation_does_not_depend_on_domain_operations_or_interfaces() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src/foundation")) {
        let source = read_source_file(&file);
        let code = source_without_pub_mod_statements(&code_for_path_scan(&source));
        for upstream in ["interfaces", "operations", "domain"] {
            if code.contains(&format!("crate::{upstream}")) {
                violations.push(format!(
                    "{} references crate::{upstream} directly",
                    file.display()
                ));
                continue;
            }
            if contains_bare_path_reference(&code, upstream) {
                violations.push(format!(
                    "{} references {upstream} through a bare root path",
                    file.display()
                ));
                continue;
            }
            if contains_relative_path(&code, upstream) {
                violations.push(format!(
                    "{} references {upstream} through a relative path",
                    file.display()
                ));
            }
        }

        for (line_number, import_statement) in crate_import_statements(&source) {
            for upstream in ["interfaces", "operations", "domain"] {
                if import_statement.contains(&format!("crate::{upstream}")) {
                    violations.push(format!(
                        "{}:{} imports {upstream}",
                        file.display(),
                        line_number
                    ));
                }
            }
            for legacy_root in LEGACY_COMPATIBILITY_ROOTS {
                if *legacy_root == "core" {
                    continue;
                }
                if contains_legacy_root_import(&import_statement, legacy_root)
                    || contains_legacy_deep_import(&import_statement, legacy_root)
                {
                    violations.push(format!(
                        "{}:{} imports Maestro-specific module {legacy_root}",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Foundation must stay domain-neutral (no domain, operation, or interface dependencies):\n{}",
        violations.join("\n")
    );
}

#[test]
fn install_production_sources_use_domain_facade_not_legacy_shim() {
    assert_production_sources_use_operation_instead_of_legacy_shim("install");

    let domain_facade = read_source_file(Path::new("src/domain/install/mod.rs"));
    for entrypoint in ["pub fn install_agent", "pub fn uninstall_agent"] {
        assert!(
            domain_facade.contains(entrypoint),
            "src/domain/install/mod.rs should expose orchestration entrypoint {entrypoint}"
        );
    }
}

#[test]
fn update_does_not_import_harness_template_writes() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src/operations/update")) {
        let source = read_source_file(&file);
        let code = code_for_path_scan(&source);
        if code.contains("crate::domain::harness::templates") {
            violations.push(format!(
                "{} references the Harness template write surface crate::domain::harness::templates",
                file.display()
            ));
        }
        if contains_bare_path_reference(&code, "harness::templates") {
            violations.push(format!(
                "{} reaches the Harness template write surface through a bare harness::templates path",
                file.display()
            ));
        }

        for (line_number, import_statement) in crate_import_statements(&source) {
            if namespaced_deep_import_segment(&import_statement, "domain", "harness")
                == Some("templates")
            {
                violations.push(format!(
                    "{}:{} imports the Harness template write surface domain::harness::templates",
                    file.display(),
                    line_number
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Update must not import Harness template writes:\n{}",
        violations.join("\n")
    );
}

#[test]
fn proof_domain_does_not_apply_or_mutate_task_directly() {
    let mut violations = Vec::new();
    let allowed_task_symbols = BTreeSet::from([
        "AcceptanceFile".to_string(),
        "AppliedVerificationReceipt".to_string(),
        "ClaimCheckReceipt".to_string(),
        "ProofSourceReceipt".to_string(),
        "TaskRecord".to_string(),
        "TaskState".to_string(),
        "VerificationBinding".to_string(),
        "VerificationCommandReceipt".to_string(),
        "VerificationFailed".to_string(),
        "VerificationOutcome".to_string(),
        "VerificationPassed".to_string(),
        "VerificationStatus".to_string(),
        "load_archived_task_record".to_string(),
        "load_task_for_update".to_string(),
        "load_task_record".to_string(),
    ]);

    for file in rust_files_under(Path::new("src/domain/proof")) {
        let source = source_without_test_modules(&read_source_file(&file));
        let code = code_for_path_scan(&source);
        for reference in module_path_references(&code, "task::") {
            let root = reference
                .split("::")
                .next()
                .expect("invariant: module reference should have a root symbol");
            let allowed_task_save_hook = reference == "template::SaveTaskHook";
            if !allowed_task_symbols.contains(root) && !allowed_task_save_hook {
                violations.push(format!(
                    "{} references Task symbol task::{reference} outside the approved read/DTO set",
                    file.display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Proof must return read data or DTOs and leave Task mutation to operations/task_verify:\n{}",
        violations.join("\n")
    );
}

#[test]
fn task_verification_application_stays_behind_task_verify_operation() {
    let mut violations = Vec::new();
    let allowed_files = [
        Path::new("src/domain/task/mod.rs"),
        Path::new("src/operations/task_verify/mod.rs"),
    ];

    for file in rust_files_under(Path::new("src")) {
        let source = source_without_test_modules(&read_source_file(&file));
        let code = code_for_path_scan(&source);
        for symbol in [
            "apply_verification_outcome(",
            "apply_verification_outcome_to_handle_after",
        ] {
            if !code.contains(symbol) || allowed_files.contains(&file.as_path()) {
                continue;
            }
            violations.push(format!(
                "{} references Task verification application path {symbol}",
                file.display()
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "Task verification outcomes should be applied only inside Task or operations/task_verify:\n{}",
        violations.join("\n")
    );
}

#[test]
fn proof_domain_reads_run_through_run_read_models() {
    let mut violations = Vec::new();
    // Managed Run seams Proof may consume: read models plus the single hardened
    // write seam. `append_manual_event` is traversal- and symlink-safe, so it is
    // the one sanctioned way for Proof to record proof events (C2 Fork A).
    let allowed_run_symbols = BTreeSet::from([
        "RunEvent".to_string(),
        "RunEventLog".to_string(),
        "RunEventRecord".to_string(),
        "RunEvidenceLoad".to_string(),
        "RunEvidenceRecord".to_string(),
        "append_manual_event".to_string(),
        "load_run_evidence".to_string(),
        "managed_event_logs".to_string(),
        "visit_managed_events".to_string(),
    ]);

    for file in rust_files_under(Path::new("src/domain/proof")) {
        let source = source_without_test_modules(&read_source_file(&file));
        let code = code_for_path_scan(&source);

        if let Some(segment) = namespaced_deep_import_segment(&code, "domain", "run") {
            violations.push(format!(
                "{} reaches into domain::run::{segment}",
                file.display()
            ));
        }

        for (line_number, import_statement) in crate_import_statements(&source) {
            if let Some(segment) =
                namespaced_deep_import_segment(&import_statement, "domain", "run")
            {
                violations.push(format!(
                    "{}:{} imports domain::run::{segment}",
                    file.display(),
                    line_number
                ));
            }
        }

        for symbol in module_symbol_references(&code, "run::") {
            if !allowed_run_symbols.contains(&symbol) {
                violations.push(format!(
                    "{} references Run symbol run::{symbol} outside the approved managed Run seam set",
                    file.display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Proof should consume Run only through managed Run seams (read models plus the hardened append), not unmanaged Run path access:\n{}",
        violations.join("\n")
    );
}

#[test]
fn run_domain_may_expose_task_ids_but_must_not_import_task() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src/domain/run")) {
        let source = source_without_test_modules(&read_source_file(&file));
        let code = code_for_path_scan(&source);

        if code.contains("crate::task") || code.contains("crate::domain::task") {
            violations.push(format!("{} references the Task aggregate", file.display()));
        }
        for (line_number, import_statement) in crate_import_statements(&source) {
            if contains_legacy_root_import(&import_statement, "task")
                || contains_legacy_deep_import(&import_statement, "task")
                || contains_root_import(&import_statement, "crate::domain::task")
                || namespaced_deep_import_segment(&import_statement, "domain", "task").is_some()
            {
                violations.push(format!(
                    "{}:{} imports the Task aggregate",
                    file.display(),
                    line_number
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Run may store or expose task_id values from events, but must not mutate or import Task:\n{}",
        violations.join("\n")
    );
}

#[test]
fn non_interface_sources_use_run_domain_instead_of_legacy_hooks() {
    let mut violations = Vec::new();

    for root in PRODUCTION_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            if file.starts_with(Path::new("src/interfaces")) {
                continue;
            }

            let source = read_source_file(&file);
            let code = source_without_allowed_interfaces_hooks_reexport(
                &file,
                &code_for_path_scan(&source),
            );
            if code.contains("crate::hooks::") {
                violations.push(format!(
                    "{} references legacy crate::hooks:: path",
                    file.display()
                ));
                continue;
            }
            for (line_number, import_statement) in crate_import_statements(&source) {
                if contains_legacy_root_import(&import_statement, "hooks")
                    || contains_legacy_deep_import(&import_statement, "hooks")
                {
                    violations.push(format!(
                        "{}:{} imports legacy crate::hooks path",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "non-interface code should use domain::run instead of legacy crate::hooks:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_sources_do_not_use_legacy_verification_imports() {
    let mut violations = Vec::new();

    for root in PRODUCTION_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            if file == Path::new("src/verification/mod.rs") {
                continue;
            }

            let source = read_source_file(&file);
            let code = code_for_path_scan(&source);
            if code.contains("crate::verification::") {
                violations.push(format!(
                    "{} references legacy crate::verification:: path",
                    file.display()
                ));
                continue;
            }

            for (line_number, import_statement) in crate_import_statements(&source) {
                if contains_legacy_root_import(&import_statement, "verification")
                    || contains_legacy_deep_import(&import_statement, "verification")
                {
                    violations.push(format!(
                        "{}:{} imports legacy crate::verification path",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production code should use domain::proof instead of legacy crate::verification:\n{}",
        violations.join("\n")
    );
}

#[test]
fn non_proof_sources_do_not_reach_into_proof_domain_leaf_modules() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src")) {
        if file.starts_with(Path::new("src/domain/proof"))
            || file == Path::new("src/verification/mod.rs")
        {
            continue;
        }

        let source = read_source_file(&file);
        let non_import_source = source_without_import_statements(&source);
        let code = code_for_path_scan(&non_import_source);
        if let Some(segment) = namespaced_deep_import_segment(&code, "domain", "proof") {
            violations.push(format!(
                "{} references domain::proof::{segment}",
                file.display()
            ));
        }

        for (line_number, import_statement) in crate_import_statements(&source) {
            if let Some(segment) =
                namespaced_deep_import_segment(&import_statement, "domain", "proof")
            {
                violations.push(format!(
                    "{}:{} imports domain::proof::{segment}",
                    file.display(),
                    line_number
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "non-Proof code must use the domain::proof root facade instead of Proof leaf modules:\n{}",
        violations.join("\n")
    );
}

#[test]
fn non_task_sources_do_not_reach_into_task_domain_leaf_modules() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src")) {
        if file.starts_with(Path::new("src/domain/task")) || file == Path::new("src/task/mod.rs") {
            continue;
        }

        let source = read_source_file(&file);
        let non_import_source = source_without_import_statements(&source);
        let code = code_for_path_scan(&non_import_source);
        if let Some(segment) = namespaced_deep_import_segment(&code, "domain", "task") {
            violations.push(format!(
                "{} references domain::task::{segment}",
                file.display()
            ));
        }

        for (line_number, import_statement) in crate_import_statements(&source) {
            if let Some(segment) =
                namespaced_deep_import_segment(&import_statement, "domain", "task")
            {
                violations.push(format!(
                    "{}:{} imports domain::task::{segment}",
                    file.display(),
                    line_number
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "non-Task code must use the domain::task root facade instead of Task leaf modules:\n{}",
        violations.join("\n")
    );
}

#[test]
fn feature_archive_stays_behind_card_persistence_seam() {
    let file = Path::new("src/domain/feature/archive.rs");
    let source = read_source_file(file);
    let mut violations = Vec::new();

    for (line_number, import_statement) in crate_import_statements(&source) {
        let compact = compact_import_statement(&import_statement);
        for forbidden in ["store", "archive_db", "live_db"] {
            let direct_root = format!("usecrate::domain::card::{forbidden}");
            let direct_import = compact.contains(&format!("{direct_root}::"))
                || compact.contains(&format!("{direct_root};"))
                || compact.contains(&format!("{direct_root},"))
                || compact.contains(&format!("{direct_root}as"));
            let grouped_import = compact
                .split("usecrate::domain::card::{")
                .skip(1)
                .any(|grouped| contains_grouped_root_reference_segment(grouped, forbidden));

            if direct_import || grouped_import {
                violations.push(format!(
                    "{}:{} imports card persistence implementation detail `{forbidden}` via `{import_statement}`",
                    file.display(),
                    line_number
                ));
            }
        }
    }

    let code = code_for_path_scan(&source_without_import_statements(&source));
    for (line_index, line) in code.lines().enumerate() {
        for forbidden_path in [
            "crate::domain::card::store::",
            "crate::domain::card::archive_db::",
            "crate::domain::card::live_db::",
        ] {
            if line.contains(forbidden_path) {
                violations.push(format!(
                    "{}:{} references card persistence implementation path `{forbidden_path}`",
                    file.display(),
                    line_index + 1
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "feature archive movement must go through the card-domain persistence seam, not card store/live-db/archive-db internals:\n{}",
        violations.join("\n")
    );
}

#[test]
fn transitional_public_surfaces_match_phase_policy() {
    assert_public_modules(
        Path::new("src/domain/mod.rs"),
        &[
            "capability",
            "authority",
            "card",
            "channel",
            "conflict",
            "contract",
            "decisions",
            "design",
            "distribution",
            "evidence",
            "execution",
            "extraction",
            "feature",
            "gate",
            "gate_lock",
            "harness",
            "identity",
            "install",
            "intake",
            "integration",
            "lean",
            "loop_recipes",
            "maturity",
            "memory",
            "migration",
            "orchestration",
            "persistence",
            "playbook",
            "proof",
            "repository",
            "research",
            "resource_contracts",
            "run",
            "schema_contracts",
            "search",
            "skills",
            "step",
            "task",
            "work",
        ],
        &[],
    );
    assert_public_modules(Path::new("src/foundation/mod.rs"), &["core"], &[]);
    assert_public_modules(
        Path::new("src/interfaces/mod.rs"),
        &["cli", "hooks", "mcp", "shell", "tui"],
        &[],
    );
    assert_public_modules(
        Path::new("src/operations/mod.rs"),
        &[
            "card_migrate",
            "container_migrate",
            "feature_prepare",
            "feature_close",
            "harness",
            "init",
            "memory",
            "migrate",
            "sync",
            "update",
        ],
        &[],
    );
}

#[test]
fn canonical_refoundation_facades_have_exact_visibility() {
    assert_module_visibility(
        Path::new("src/domain/mod.rs"),
        &[
            "authority",
            "capability",
            "card",
            "channel",
            "conflict",
            "contract",
            "decisions",
            "design",
            "distribution",
            "evidence",
            "execution",
            "extraction",
            "feature",
            "gate",
            "gate_lock",
            "harness",
            "identity",
            "install",
            "intake",
            "integration",
            "lean",
            "loop_recipes",
            "maturity",
            "memory",
            "migration",
            "orchestration",
            "persistence",
            "playbook",
            "proof",
            "repository",
            "research",
            "resource_contracts",
            "run",
            "schema_contracts",
            "search",
            "skills",
            "step",
            "task",
            "work",
        ],
        &[
            "coordination",
            "installation",
            "planning",
            "projection",
            "transport",
        ],
    );
    assert_module_visibility(
        Path::new("src/interfaces/mod.rs"),
        &["cli", "hooks", "mcp", "shell", "tui"],
        &["connectors"],
    );
    assert_module_visibility(
        Path::new("src/operations/mod.rs"),
        &[
            "card_migrate",
            "container_migrate",
            "feature_close",
            "feature_prepare",
            "harness",
            "init",
            "memory",
            "migrate",
            "sync",
            "update",
        ],
        &[
            "action",
            "adapters",
            "installation",
            "migration",
            "observation",
            "orchestration",
        ],
    );
    assert_module_visibility(
        Path::new("src/domain/capability/mod.rs"),
        &["literals"],
        &["generated_catalog", "runtime"],
    );
    assert_module_visibility(
        Path::new("src/domain/distribution/mod.rs"),
        &[],
        &["runtime"],
    );
    assert_module_visibility(
        Path::new("src/domain/evidence/mod.rs"),
        &["submission_claim"],
        &["diagnostics"],
    );
    assert_module_visibility(Path::new("src/domain/migration/mod.rs"), &[], &["runtime"]);
    assert_module_visibility(
        Path::new("src/domain/orchestration/mod.rs"),
        &["literals"],
        &["runtime"],
    );

    for path in [
        "src/domain/capability/generated_catalog/mod.rs",
        "src/domain/capability/runtime/mod.rs",
        "src/domain/coordination/mod.rs",
        "src/domain/distribution/runtime/mod.rs",
        "src/domain/evidence/diagnostics/mod.rs",
        "src/domain/installation/mod.rs",
        "src/domain/intake/mod.rs",
        "src/domain/maturity/mod.rs",
        "src/domain/memory/mod.rs",
        "src/domain/migration/runtime/mod.rs",
        "src/domain/orchestration/runtime/mod.rs",
        "src/domain/planning/mod.rs",
        "src/domain/projection/mod.rs",
        "src/domain/research/mod.rs",
        "src/domain/transport/mod.rs",
        "src/interfaces/connectors/mod.rs",
        "src/operations/action/mod.rs",
        "src/operations/adapters/mod.rs",
        "src/operations/installation/mod.rs",
        "src/operations/migration/mod.rs",
        "src/operations/observation/mod.rs",
        "src/operations/orchestration/mod.rs",
    ] {
        assert!(
            public_modules(&read_source_file(Path::new(path))).is_empty(),
            "canonical facade {path} must keep implementation children private"
        );
    }

    let mut violations = Vec::new();
    for file in rust_files_under(Path::new("src/domain")) {
        let source = source_without_test_modules(&read_source_file(&file));
        if source.contains("crate::interfaces") || source.contains("crate::operations") {
            violations.push(format!(
                "{} imports a higher canonical layer from the domain layer",
                file.display()
            ));
        }
    }
    for file in rust_files_under(Path::new("src/operations")) {
        let source = source_without_test_modules(&read_source_file(&file));
        if source.contains("crate::interfaces") {
            violations.push(format!(
                "{} imports the canonical interface layer from operations",
                file.display()
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "canonical imports must preserve interfaces -> operations -> domain:\n{}",
        violations.join("\n")
    );
}

fn lib_exposes_crate_root(lib: &str, root: &str) -> bool {
    let public_module = format!("pub mod {root};");

    lib.lines()
        .map(str::trim)
        .any(|line| line == public_module || compatibility_reexport_exposes_root(line, root))
}

fn compatibility_reexport_exposes_root(line: &str, root: &str) -> bool {
    match root {
        "mcp" => line == "pub use interfaces::mcp;" || line == "pub use crate::interfaces::mcp;",
        "hooks" => {
            line == "pub use interfaces::hooks;" || line == "pub use crate::interfaces::hooks;"
        }
        "tui" => line == "pub use interfaces::tui;" || line == "pub use crate::interfaces::tui;",
        _ => false,
    }
}

fn assert_production_sources_use_operation_instead_of_legacy_shim(legacy_root: &str) {
    let mut violations = Vec::new();
    let legacy_shim = PathBuf::from(format!("src/{legacy_root}/mod.rs"));

    for file in rust_files_under(Path::new("src")) {
        if file == legacy_shim {
            continue;
        }

        let source = read_source_file(&file);
        let code = code_for_path_scan(&source);
        if code.contains(&format!("crate::{legacy_root}::")) {
            violations.push(format!(
                "{} references legacy crate::{legacy_root}:: path",
                file.display()
            ));
            continue;
        }

        for (line_number, import_statement) in crate_import_statements(&source) {
            if contains_legacy_root_import(&import_statement, legacy_root)
                || contains_legacy_deep_import(&import_statement, legacy_root)
            {
                violations.push(format!(
                    "{}:{} imports legacy crate::{legacy_root} path",
                    file.display(),
                    line_number
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production code should use operations::{legacy_root} instead of the legacy shim:\n{}",
        violations.join("\n")
    );
}

fn assert_production_sources_use_operation_root_facade(operation_root: &str, leaves: &[&str]) {
    let mut violations = Vec::new();
    let operation_root_dir = PathBuf::from(format!("src/operations/{operation_root}"));
    let legacy_shim = PathBuf::from(format!("src/{operation_root}/mod.rs"));

    for file in rust_files_under(Path::new("src")) {
        if file.starts_with(&operation_root_dir) || file == legacy_shim {
            continue;
        }

        let source = read_source_file(&file);
        let non_import_source = source_without_import_statements(&source);
        let code = code_for_path_scan(&non_import_source);
        for leaf in leaves {
            let needle = format!("crate::operations::{operation_root}::{leaf}::");
            if code.contains(&needle) {
                violations.push(format!(
                    "{} references deep operation path {needle}",
                    file.display()
                ));
            }
        }

        for (line_number, import_statement) in crate_import_statements(&source) {
            if let Some(leaf) =
                namespaced_deep_import_segment(&import_statement, "operations", operation_root)
                && leaves.contains(&leaf)
            {
                violations.push(format!(
                    "{}:{} imports deep operation path operations::{operation_root}::{leaf}",
                    file.display(),
                    line_number
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production code should use operations::{operation_root} root facade symbols instead of operation leaf modules:\n{}",
        violations.join("\n")
    );
}

#[test]
fn moved_interface_sources_respect_facade_and_transition_policy() {
    let mut violations = Vec::new();

    for root in INTERFACE_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            let source = read_source_file(&file);
            let non_import_source = source_without_import_statements(&source);
            if let Some(path) = protected_interface_path_reference(&non_import_source) {
                if is_allowed_transitional_interface_path_reference(&file, &path) {
                    continue;
                }

                violations.push(format!(
                    "{} references protected implementation path {path}",
                    file.display()
                ));
            }

            for (line_number, import_statement) in crate_import_statements(&source) {
                if is_allowed_transitional_interface_import(&file, &import_statement) {
                    continue;
                }

                if let Some(import) = protected_interface_import(&import_statement) {
                    violations.push(format!(
                        "{}:{} uses disallowed facade or compatibility path {import}",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "interface code must call facades directly except explicit transitional allowances:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_sources_prefer_foundation_core_imports() {
    let mut violations = Vec::new();

    for root in PRODUCTION_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            let source = read_source_file(&file);
            let code = code_for_path_scan(&source);
            if code.contains("crate::core::") {
                violations.push(format!(
                    "{} references legacy crate::core:: path",
                    file.display()
                ));
                continue;
            }

            for (line_number, import_statement) in crate_import_statements(&source) {
                if contains_legacy_root_import(&import_statement, "core")
                    || contains_legacy_deep_import(&import_statement, "core")
                {
                    violations.push(format!(
                        "{}:{} imports legacy crate::core path",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production code should import Core through crate::foundation::core during the migration:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_sources_prefer_interfaces_shell_imports() {
    let mut violations = Vec::new();

    for root in PRODUCTION_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            let source = read_source_file(&file);
            let code = code_for_path_scan(&source);
            if code.contains("crate::shell::") {
                violations.push(format!(
                    "{} references legacy crate::shell:: path",
                    file.display()
                ));
                continue;
            }

            for (line_number, import_statement) in crate_import_statements(&source) {
                if contains_legacy_root_import(&import_statement, "shell")
                    || contains_legacy_deep_import(&import_statement, "shell")
                {
                    violations.push(format!(
                        "{}:{} imports legacy crate::shell path",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production code should import Shell through crate::interfaces::shell during the migration:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_sources_prefer_interfaces_mcp_imports() {
    let mut violations = Vec::new();

    for root in PRODUCTION_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            let source = read_source_file(&file);
            let code = code_for_path_scan(&source);
            if code.contains("crate::mcp::") {
                violations.push(format!(
                    "{} references legacy crate::mcp:: path",
                    file.display()
                ));
                continue;
            }

            for (line_number, import_statement) in crate_import_statements(&source) {
                if contains_legacy_root_import(&import_statement, "mcp")
                    || contains_legacy_deep_import(&import_statement, "mcp")
                {
                    violations.push(format!(
                        "{}:{} imports legacy crate::mcp path",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production code should import MCP through crate::interfaces::mcp during the migration:\n{}",
        violations.join("\n")
    );
}

#[test]
fn production_sources_prefer_interfaces_tui_imports() {
    let mut violations = Vec::new();

    for root in PRODUCTION_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            let source = read_source_file(&file);
            let code = source_without_pub_mod_statements(&code_for_path_scan(&source));
            if code.contains("crate::tui::") {
                violations.push(format!(
                    "{} references legacy crate::tui:: path",
                    file.display()
                ));
                continue;
            }
            if contains_bare_path_reference(&code, "tui") {
                violations.push(format!(
                    "{} references legacy tui root through a bare root path",
                    file.display()
                ));
                continue;
            }

            for (line_number, import_statement) in crate_import_statements(&source) {
                if contains_legacy_root_import(&import_statement, "tui")
                    || contains_legacy_deep_import(&import_statement, "tui")
                {
                    violations.push(format!(
                        "{}:{} imports legacy crate::tui path",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production code should import TUI through crate::interfaces::tui during the migration:\n{}",
        violations.join("\n")
    );
}

#[test]
fn interface_sources_prefer_interfaces_hooks_imports() {
    let mut violations = Vec::new();

    for root in INTERFACE_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            let source = read_source_file(&file);
            let code = source_without_pub_mod_statements(&code_for_path_scan(&source));
            if code.contains("crate::hooks::") {
                violations.push(format!(
                    "{} references legacy crate::hooks:: path",
                    file.display()
                ));
                continue;
            }
            if contains_bare_path_reference(&code, "hooks") {
                violations.push(format!(
                    "{} references legacy hooks root through a bare root path",
                    file.display()
                ));
                continue;
            }
            if contains_relative_path(&code, "hooks") {
                violations.push(format!(
                    "{} references legacy hooks root through a relative path",
                    file.display()
                ));
                continue;
            }

            for (line_number, import_statement) in crate_import_statements(&source) {
                if contains_legacy_root_import(&import_statement, "hooks")
                    || contains_legacy_deep_import(&import_statement, "hooks")
                {
                    violations.push(format!(
                        "{}:{} imports legacy crate::hooks path",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "interface code should import Hooks through crate::interfaces::hooks during the migration:\n{}",
        violations.join("\n")
    );
}

#[test]
fn non_interface_sources_do_not_depend_on_interfaces_hooks() {
    let mut violations = Vec::new();

    for root in PRODUCTION_SCAN_ROOTS {
        for file in rust_files_under(Path::new(root)) {
            if file.starts_with(Path::new("src/interfaces")) {
                continue;
            }

            let source = read_source_file(&file);
            let code = source_without_allowed_interfaces_hooks_reexport(
                &file,
                &code_for_path_scan(&source),
            );
            if contains_crate_interfaces_hooks_reference(&code) {
                violations.push(format!(
                    "{} references crate::interfaces::hooks directly",
                    file.display()
                ));
                continue;
            }
            if contains_bare_interfaces_hooks_reference(&code) {
                violations.push(format!(
                    "{} references interfaces::hooks through a bare root path",
                    file.display()
                ));
                continue;
            }
            if contains_relative_path(&code, "interfaces::hooks") {
                violations.push(format!(
                    "{} references interfaces::hooks through a relative path",
                    file.display()
                ));
                continue;
            }

            for (line_number, import_statement) in crate_import_statements(&source) {
                if contains_crate_interfaces_hooks_reference(&import_statement) {
                    violations.push(format!(
                        "{}:{} imports crate::interfaces::hooks directly",
                        file.display(),
                        line_number
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "non-interface code should avoid interface Hooks paths and use domain::run for Run behavior:\n{}",
        violations.join("\n")
    );
}

#[test]
fn resource_embeds_stay_in_owning_modules() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src")) {
        let source = read_source_file(&file);
        for (line_number, line) in source.lines().enumerate() {
            let embeds_resource = line.contains("include_str!") || line.contains("include_dir!");
            if !embeds_resource || !line.contains("embedded/") {
                continue;
            }
            if resource_embed_is_allowed(&file, line) {
                continue;
            }

            violations.push(format!(
                "{}:{} embeds a resource outside its owning module",
                file.display(),
                line_number + 1
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "resource files should be embedded only by their owning modules:\n{}",
        violations.join("\n")
    );
}

#[test]
fn bundled_skill_resources_are_skill_directories_without_evals() {
    let root = Path::new("embedded/skills");
    let mut violations = Vec::new();

    for entry in sorted_dir_entries(root) {
        if !entry.is_dir() {
            violations.push(format!(
                "{} is not a bundled skill directory",
                entry.display()
            ));
            continue;
        }
        if !entry.join("SKILL.md").is_file() {
            violations.push(format!("{} is missing SKILL.md", entry.display()));
        }
        for path in paths_under(&entry) {
            if path
                .components()
                .any(|component| component.as_os_str().to_string_lossy() == "evals")
            {
                violations.push(format!(
                    "{} contains development-only evals content",
                    path.display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "bundled skill resources must be installable skill directories:\n{}",
        violations.join("\n")
    );
}

fn protected_interface_import(line: &str) -> Option<String> {
    if is_relative_import_statement(line) && contains_relative_deep_reference(line) {
        return Some("relative deep path under src/interfaces".to_string());
    }

    for facade in DOMAIN_FACADES {
        if contains_namespaced_root_alias(line, "domain", facade) {
            return Some(format!("domain::{facade} root alias"));
        }
        if let Some(segment) = namespaced_deep_import_segment(line, "domain", facade) {
            return Some(format!("domain::{facade}::{segment}"));
        }
    }

    for facade in OPERATION_FACADES {
        if contains_namespaced_root_alias(line, "operations", facade) {
            return Some(format!("operations::{facade} root alias"));
        }
        if let Some(segment) = namespaced_deep_import_segment(line, "operations", facade) {
            return Some(format!("operations::{facade}::{segment}"));
        }
    }

    for legacy_root in LEGACY_COMPATIBILITY_ROOTS {
        if contains_legacy_root_import(line, legacy_root) {
            return Some(format!("legacy crate::{legacy_root} root import"));
        }
        if contains_legacy_deep_import(line, legacy_root) {
            return Some(format!("legacy crate::{legacy_root}::"));
        }
    }

    None
}

fn is_allowed_transitional_interface_import(file: &Path, line: &str) -> bool {
    if file == Path::new("src/interfaces/hooks/event.rs") && line == "use crate::domain::run;" {
        return true;
    }

    transitional_interface_legacy_roots(file).is_some_and(|roots| {
        roots.iter().any(|root| {
            contains_legacy_root_import(line, root) || contains_legacy_deep_import(line, root)
        })
    })
}

fn is_allowed_transitional_interface_path_reference(file: &Path, path: &str) -> bool {
    transitional_interface_legacy_roots(file).is_some_and(|roots| {
        roots
            .iter()
            .any(|root| path == format!("legacy crate::{root}::"))
    })
}

fn transitional_interface_legacy_roots(file: &Path) -> Option<&'static [&'static str]> {
    CLI_TRANSITIONAL_LEGACY_IMPORTS
        .iter()
        .find_map(|(allowed_file, roots)| (file == Path::new(allowed_file)).then_some(*roots))
}

fn assert_public_modules(path: &Path, expected_modules: &[&str], expected_reexports: &[&str]) {
    let source = read_source_file(path);
    let actual = public_modules(&source);
    let expected = expected_modules
        .iter()
        .map(|module| module.to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        actual,
        expected,
        "{} must expose exactly the expected public modules",
        path.display()
    );

    let actual_reexports = crate_reexports(&source);
    let expected_reexports = expected_reexports
        .iter()
        .map(|root| root.to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        actual_reexports,
        expected_reexports,
        "{} must expose exactly the expected public re-exports",
        path.display()
    );
}

fn assert_module_visibility(
    path: &Path,
    expected_public: &[&str],
    expected_crate_visible: &[&str],
) {
    let source = read_source_file(path);
    let expected_public = expected_public
        .iter()
        .map(|module| module.to_string())
        .collect::<BTreeSet<_>>();
    let expected_crate_visible = expected_crate_visible
        .iter()
        .map(|module| module.to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        public_modules(&source),
        expected_public,
        "{} must expose exactly the approved public modules",
        path.display()
    );
    assert_eq!(
        crate_visible_modules(&source),
        expected_crate_visible,
        "{} must expose exactly the approved crate-internal modules",
        path.display()
    );
}

fn crate_reexports(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("pub use ")
                .and_then(|root| root.strip_suffix(';'))
                .map(str::to_string)
        })
        .collect()
}

fn public_reexports(source: &str) -> Vec<String> {
    let mut reexports = Vec::new();
    let mut current: Option<String> = None;

    for line in source.lines().map(str::trim) {
        if let Some(statement) = current.as_mut() {
            statement.push(' ');
            statement.push_str(line);
            if line.ends_with(';') {
                reexports.push(current.take().expect("invariant: re-export exists"));
            }
            continue;
        }

        if !line.starts_with("pub use ") {
            continue;
        }

        if line.ends_with(';') {
            reexports.push(line.to_string());
        } else {
            current = Some(line.to_string());
        }
    }

    if let Some(statement) = current {
        reexports.push(statement);
    }

    reexports
}

fn public_reexport_item_names(source: &str) -> BTreeSet<String> {
    public_reexports(source)
        .into_iter()
        .flat_map(|statement| public_reexport_item_names_from_statement(&statement))
        .collect()
}

fn public_reexport_item_names_from_statement(statement: &str) -> Vec<String> {
    let statement = statement
        .trim()
        .trim_start_matches("pub use ")
        .trim_end_matches(';')
        .trim();
    if let Some((_, grouped)) = statement.split_once("::{") {
        return grouped
            .trim_end_matches('}')
            .split(',')
            .filter_map(public_reexport_leaf_name)
            .collect();
    }
    public_reexport_leaf_name(statement)
        .into_iter()
        .collect::<Vec<_>>()
}

fn public_reexport_leaf_name(item: &str) -> Option<String> {
    let item = item.trim();
    if item.is_empty() {
        return None;
    }
    let item = item
        .split_once(" as ")
        .map(|(_, alias)| alias)
        .unwrap_or(item);
    item.rsplit("::").next().map(str::to_string)
}

fn public_modules(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let module = line.strip_prefix("pub mod ")?;
            module
                .strip_suffix(';')
                .or_else(|| module.strip_suffix(" {"))
                .map(str::to_string)
        })
        .collect()
}

fn crate_visible_modules(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let module = line.strip_prefix("pub(crate) mod ")?;
            module
                .strip_suffix(';')
                .or_else(|| module.strip_suffix(" {"))
                .map(str::to_string)
        })
        .collect()
}

fn protected_interface_path_reference(source: &str) -> Option<String> {
    let source = code_for_path_scan(source);
    let source = source_without_pub_mod_statements(&source);

    if contains_relative_deep_reference(&source) {
        return Some("relative deep path under src/interfaces".to_string());
    }

    for facade in DOMAIN_FACADES {
        if let Some(segment) = namespaced_deep_import_segment(&source, "domain", facade) {
            return Some(format!("domain::{facade}::{segment}"));
        }
    }

    for facade in OPERATION_FACADES {
        if let Some(segment) = namespaced_deep_import_segment(&source, "operations", facade) {
            return Some(format!("operations::{facade}::{segment}"));
        }
    }

    for legacy_root in LEGACY_COMPATIBILITY_ROOTS {
        if contains_legacy_deep_path_reference(&source, legacy_root) {
            return Some(format!("legacy crate::{legacy_root}::"));
        }
    }

    None
}

fn contains_relative_deep_reference(source: &str) -> bool {
    LEGACY_COMPATIBILITY_ROOTS
        .iter()
        .any(|root| contains_relative_path(source, root))
        || contains_relative_path(source, "domain")
        || contains_relative_path(source, "operations")
}

fn contains_relative_path(source: &str, root: &str) -> bool {
    if source.contains(&format!("self::{root}::")) {
        return true;
    }
    if source.contains(&format!("self::{{{root}")) {
        return true;
    }

    source.match_indices("super::").any(|(index, _)| {
        let mut tail = &source[index..];
        while let Some(rest) = tail.strip_prefix("super::") {
            tail = rest;
        }
        tail.starts_with(&format!("{root}::")) || tail.starts_with(&format!("{{{root}"))
    })
}

fn contains_crate_interfaces_hooks_reference(source: &str) -> bool {
    let compact = compact_import_statement(source);
    compact.contains("crate::interfaces::hooks")
        || compact.contains("crate::interfaces::{hooks")
        || compact.contains("crate::{interfaces::hooks")
        || compact.contains("crate::{interfaces::{hooks")
}

fn contains_bare_interfaces_hooks_reference(source: &str) -> bool {
    contains_bare_path_reference(source, "interfaces::hooks")
        || contains_bare_grouped_path_reference(source, "interfaces", "hooks")
}

fn contains_bare_path_reference(source: &str, path: &str) -> bool {
    source.match_indices(path).any(|(index, _)| {
        let previous = source[..index].chars().next_back();
        let next = source[index + path.len()..].chars().next();

        is_bare_path_prefix_boundary(previous) && is_bare_path_suffix_boundary(next)
    })
}

fn contains_bare_grouped_path_reference(source: &str, namespace: &str, root: &str) -> bool {
    let needle = format!("{namespace}::{{");
    source.match_indices(&needle).any(|(index, _)| {
        let previous = source[..index].chars().next_back();
        if !is_bare_path_prefix_boundary(previous) {
            return false;
        }

        let grouped = &source[index + needle.len()..];
        contains_grouped_root_reference_segment(&compact_import_statement(grouped), root)
    })
}

fn is_bare_path_prefix_boundary(character: Option<char>) -> bool {
    match character {
        None => true,
        Some(':') => false,
        Some(character) => !is_path_identifier_character(character),
    }
}

fn is_bare_path_suffix_boundary(character: Option<char>) -> bool {
    match character {
        None => true,
        Some(':') => true,
        Some(character) => !is_path_identifier_character(character),
    }
}

fn is_path_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn code_for_path_scan(source: &str) -> String {
    let chars = source.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(source.len());
    let mut index = 0;
    let mut block_comment_depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;

    while index < chars.len() {
        let current = chars[index];
        let next = chars.get(index + 1).copied();

        if block_comment_depth > 0 {
            if current == '/' && next == Some('*') {
                block_comment_depth += 1;
                output.push(' ');
                output.push(' ');
                index += 2;
                continue;
            }
            if current == '*' && next == Some('/') {
                block_comment_depth -= 1;
                output.push(' ');
                output.push(' ');
                index += 2;
                continue;
            }
            output.push(if current == '\n' { '\n' } else { ' ' });
            index += 1;
            continue;
        }

        if in_string {
            if escaped {
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == '"' {
                in_string = false;
            }
            output.push(if current == '\n' { '\n' } else { ' ' });
            index += 1;
            continue;
        }

        if current == '/' && next == Some('/') {
            output.push(' ');
            output.push(' ');
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                output.push(' ');
                index += 1;
            }
            continue;
        }

        if current == '/' && next == Some('*') {
            block_comment_depth = 1;
            output.push(' ');
            output.push(' ');
            index += 2;
            continue;
        }

        if let Some((hash_count, prefix_len)) = raw_string_start(&chars, index) {
            for _ in 0..prefix_len {
                output.push(' ');
            }
            index += prefix_len;
            while index < chars.len() {
                if raw_string_end(&chars, index, hash_count) {
                    for _ in 0..=hash_count {
                        output.push(' ');
                    }
                    index += hash_count + 1;
                    break;
                }
                output.push(if chars[index] == '\n' { '\n' } else { ' ' });
                index += 1;
            }
            continue;
        }

        if current == '"' {
            in_string = true;
            escaped = false;
            output.push(' ');
            index += 1;
            continue;
        }

        output.push(current);
        index += 1;
    }

    output
}

fn raw_string_start(chars: &[char], index: usize) -> Option<(usize, usize)> {
    if chars.get(index).copied() != Some('r') {
        return None;
    }

    let mut cursor = index + 1;
    let mut hash_count = 0;
    while chars.get(cursor).copied() == Some('#') {
        hash_count += 1;
        cursor += 1;
    }

    if chars.get(cursor).copied() == Some('"') {
        Some((hash_count, cursor - index + 1))
    } else {
        None
    }
}

fn raw_string_end(chars: &[char], index: usize, hash_count: usize) -> bool {
    if chars.get(index).copied() != Some('"') {
        return false;
    }

    (0..hash_count).all(|offset| chars.get(index + offset + 1).copied() == Some('#'))
}

fn contains_legacy_root_import(line: &str, legacy_root: &str) -> bool {
    contains_root_import(line, &format!("crate::{legacy_root}"))
        || contains_grouped_root_import(line, legacy_root)
}

fn contains_legacy_deep_import(line: &str, legacy_root: &str) -> bool {
    line.contains(&format!("crate::{legacy_root}::"))
        || line.contains(&format!("{{{legacy_root}::"))
        || line.contains(&format!(" {legacy_root}::"))
        || line.contains(&format!(",{legacy_root}::"))
}

fn contains_legacy_deep_path_reference(source: &str, legacy_root: &str) -> bool {
    source.contains(&format!("crate::{legacy_root}::"))
}

fn contains_namespaced_root_alias(line: &str, namespace: &str, facade: &str) -> bool {
    let compact = compact_import_statement(line);

    contains_root_alias(line, &format!("crate::{namespace}::{facade}"))
        || compact.contains(&format!("crate::{{{namespace}::{facade}as"))
        || compact.contains(&format!("crate::{namespace}::{facade}::{{selfas"))
        || compact.contains(&format!("crate::{namespace}::{{{facade}::{{selfas"))
        || compact.contains(&format!("crate::{{{namespace}::{facade}::{{selfas"))
        || compact.contains(&format!("crate::{{{namespace}::{{{facade}::{{selfas"))
        || line
            .split(&format!("crate::{namespace}::{{"))
            .nth(1)
            .is_some_and(|grouped| contains_grouped_root_alias_segment(grouped, facade))
}

fn contains_root_import(line: &str, path: &str) -> bool {
    let compact = compact_import_statement(line);
    let path = path.replace(' ', "");

    compact.contains(&format!("{path};"))
        || compact.contains(&format!("{path},"))
        || compact.contains(&format!("{path}as"))
}

fn contains_root_alias(line: &str, path: &str) -> bool {
    let compact = compact_import_statement(line);
    let path = path.replace(' ', "");

    compact.contains(&format!("{path}as"))
}

fn contains_grouped_root_import(line: &str, root: &str) -> bool {
    let compact = compact_import_statement(line);
    let Some(grouped) = compact.split("crate::{").nth(1) else {
        return false;
    };

    contains_grouped_root_segment(grouped, root)
}

fn contains_grouped_root_alias_segment(grouped: &str, root: &str) -> bool {
    grouped.contains(&format!("{root}as"))
}

fn contains_grouped_root_segment(grouped: &str, root: &str) -> bool {
    grouped.contains(&format!("{root};"))
        || grouped.contains(&format!("{root},"))
        || grouped.contains(&format!("{root}as"))
        || grouped.contains(&format!("{root}}}"))
}

fn contains_grouped_root_reference_segment(grouped: &str, root: &str) -> bool {
    contains_grouped_root_segment(grouped, root) || grouped.contains(&format!("{root}::"))
}

fn compact_import_statement(line: &str) -> String {
    line.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn namespaced_deep_import_segment<'a>(
    line: &'a str,
    namespace: &str,
    facade: &str,
) -> Option<&'a str> {
    let direct_needle = format!("crate::{namespace}::{facade}::");
    for import_tail in line.split(&direct_needle).skip(1) {
        if let Some(segment) = first_domain_child_segment(import_tail) {
            return Some(segment);
        }
    }

    let grouped_needle = format!("{namespace}::{facade}::");
    for import_tail in line.split(&grouped_needle).skip(1) {
        if let Some(segment) = first_domain_child_segment(import_tail) {
            return Some(segment);
        }
    }

    let domain_root_grouped_needle = format!("{facade}::");
    for grouped in line.split(&format!("crate::{namespace}::{{")).skip(1) {
        for import_tail in grouped.split(&domain_root_grouped_needle).skip(1) {
            if let Some(segment) = first_domain_child_segment(import_tail) {
                return Some(segment);
            }
        }
    }

    None
}

fn first_domain_child_segment(import_tail: &str) -> Option<&str> {
    let import_tail = import_tail.trim_start();
    let import_tail = import_tail.strip_prefix('{').unwrap_or(import_tail);

    for candidate in import_tail.split(',') {
        let candidate = candidate.trim_start();
        let candidate = candidate.strip_prefix('{').unwrap_or(candidate);
        let segment = candidate
            .split(|character: char| {
                character == ':'
                    || character == '}'
                    || character == ';'
                    || character.is_whitespace()
            })
            .next()
            .unwrap_or("");

        if is_private_child_segment(segment) {
            return Some(segment);
        }
    }

    None
}

fn is_private_child_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "self"
        && segment
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
}

fn is_crate_import_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("use crate::")
        || trimmed.starts_with("pub use crate::")
        || trimmed.starts_with("pub(crate) use crate::")
        || trimmed.starts_with("pub(super) use crate::")
}

fn is_relative_import_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("use super::")
        || trimmed.starts_with("use self::")
        || trimmed.starts_with("pub use super::")
        || trimmed.starts_with("pub use self::")
        || trimmed.starts_with("pub(crate) use super::")
        || trimmed.starts_with("pub(crate) use self::")
        || trimmed.starts_with("pub(super) use super::")
        || trimmed.starts_with("pub(super) use self::")
}

fn is_relative_import_statement(line: &str) -> bool {
    is_relative_import_line(line)
}

fn crate_import_statements(source: &str) -> Vec<(usize, String)> {
    let mut imports = Vec::new();
    let mut current: Option<(usize, String)> = None;

    for (line_index, line) in source.lines().enumerate() {
        if let Some((_, statement)) = current.as_mut() {
            statement.push(' ');
            statement.push_str(line.trim());
            if line.contains(';') {
                imports.push(current.take().expect("invariant: current import exists"));
            }
            continue;
        }

        if !is_crate_import_line(line) && !is_relative_import_line(line) {
            continue;
        }

        let statement = line.trim().to_string();
        if statement.contains(';') {
            imports.push((line_index + 1, statement));
        } else {
            current = Some((line_index + 1, statement));
        }
    }

    if let Some(import) = current {
        imports.push(import);
    }

    imports
}

fn source_without_import_statements(source: &str) -> String {
    let mut output = String::new();
    let mut in_import = false;

    for line in source.lines() {
        if in_import {
            if line.contains(';') {
                in_import = false;
            }
            continue;
        }

        if is_crate_import_line(line) || is_relative_import_line(line) {
            in_import = !line.contains(';');
            continue;
        }

        output.push_str(line);
        output.push('\n');
    }

    output
}

fn source_without_test_modules(source: &str) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0;

    while index < lines.len() {
        if lines[index].trim_start().starts_with("#[cfg(test)]") {
            let mut candidate = index + 1;
            while candidate < lines.len() && lines[candidate].trim().is_empty() {
                candidate += 1;
            }
            if candidate < lines.len() && lines[candidate].trim_start().starts_with("mod ") {
                index = skip_braced_item(&lines, candidate);
                continue;
            }
        }
        output.push_str(lines[index]);
        output.push('\n');
        index += 1;
    }

    output
}

fn skip_braced_item(lines: &[&str], start: usize) -> usize {
    let mut index = start;
    let mut depth = 0usize;
    let mut saw_open = false;

    while index < lines.len() {
        for character in lines[index].chars() {
            match character {
                '{' => {
                    depth += 1;
                    saw_open = true;
                }
                '}' if depth > 0 => {
                    depth -= 1;
                }
                _ => {}
            }
        }
        index += 1;
        if saw_open && depth == 0 {
            break;
        }
    }

    index
}

fn source_without_pub_mod_statements(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.starts_with("pub mod ") && !line.starts_with("pub(crate) mod ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn module_symbol_references(source: &str, prefix: &str) -> BTreeSet<String> {
    source
        .match_indices(prefix)
        .filter(|(index, _)| at_module_boundary(source, *index))
        .filter_map(|(index, _)| {
            let tail = &source[index + prefix.len()..];
            let symbol = tail
                .chars()
                .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                .collect::<String>();
            (!symbol.is_empty()).then_some(symbol)
        })
        .collect()
}

fn module_path_references(source: &str, prefix: &str) -> BTreeSet<String> {
    source
        .match_indices(prefix)
        .filter(|(index, _)| at_module_boundary(source, *index))
        .filter_map(|(index, _)| {
            let tail = &source[index + prefix.len()..];
            let path = tail
                .chars()
                .take_while(|character| {
                    character.is_ascii_alphanumeric() || *character == '_' || *character == ':'
                })
                .collect::<String>()
                .trim_end_matches(':')
                .to_string();
            (!path.is_empty()).then_some(path)
        })
        .collect()
}

/// True when a `module::` prefix match begins a module path rather than the
/// tail of a compound identifier (e.g. `verify_task::` must not count as a
/// `task::` reference). Path separators (`:`) are allowed before the prefix so
/// nested paths like `domain::task::` still match.
fn at_module_boundary(source: &str, index: usize) -> bool {
    match source[..index].chars().last() {
        None => true,
        Some(previous) => !(previous.is_ascii_alphanumeric() || previous == '_'),
    }
}

fn source_without_allowed_interfaces_hooks_reexport(file: &Path, source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            file != Path::new("src/lib.rs")
                || (!import_line_reexports_root_only(line, "interfaces::hooks")
                    && !import_line_reexports_root_only(line, "crate::interfaces::hooks"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn import_line_reexports_root_only(line: &str, root: &str) -> bool {
    let trimmed = line.trim();
    let Some(suffix) = trimmed.strip_prefix(&format!("pub use {root}")) else {
        return false;
    };

    suffix == ";" || suffix.starts_with(" as ")
}

#[test]
fn protected_import_parser_catches_grouped_imports_and_root_aliases() {
    assert_eq!(
        protected_interface_import("use crate::hooks as legacy_hooks;"),
        Some("legacy crate::hooks root import".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::{hooks as legacy_hooks};"),
        Some("legacy crate::hooks root import".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::domain::task as task_domain;"),
        Some("domain::task root alias".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::{domain::task as task_domain};"),
        Some("domain::task root alias".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::domain::task::{self as task_domain};"),
        Some("domain::task root alias".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::domain::{task::{self as task_domain}};"),
        Some("domain::task root alias".to_string())
    );
    assert_eq!(protected_interface_import("use crate::domain::task;"), None);
    assert_eq!(
        protected_interface_import("use crate::domain::task::{template::TaskRecord};"),
        Some("domain::task::template".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::domain::{task::template::TaskRecord};"),
        Some("domain::task::template".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::domain::task::{self, template::TaskRecord};"),
        Some("domain::task::template".to_string())
    );
    assert_eq!(
        protected_interface_import("use super::super::domain::task::template::TaskRecord;"),
        Some("relative deep path under src/interfaces".to_string())
    );
    assert_eq!(protected_interface_import("use super::{render};"), None);
    assert_eq!(
        protected_interface_path_reference(
            "fn render() { crate::domain::task::template::render_task_body(); }"
        ),
        Some("domain::task::template".to_string())
    );
    assert_eq!(
        protected_interface_path_reference(
            "fn render() { super::super::domain::task::template::render_task_body(); }"
        ),
        Some("relative deep path under src/interfaces".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::operations::harness::propose::apply;"),
        Some("operations::harness::propose".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::operations::harness::{self as harness_ops};"),
        Some("operations::harness root alias".to_string())
    );
    assert_eq!(
        protected_interface_path_reference(
            "fn render() { crate::operations::harness::propose::apply(); }"
        ),
        Some("operations::harness::propose".to_string())
    );
    assert_eq!(
        protected_interface_path_reference("fn run() { feature::run(args); }"),
        None
    );
    assert_eq!(
        protected_interface_path_reference("fn run() { crate::hooks::event::run_dir_name(); }"),
        Some("legacy crate::hooks::".to_string())
    );
    assert_eq!(
        protected_interface_path_reference(
            "fn render() { super::super::super::super::domain::task::template::render_task_body(); }"
        ),
        Some("relative deep path under src/interfaces".to_string())
    );
    assert_eq!(
        protected_interface_path_reference("const PATH: &str = \"crate::domain::task::template\";"),
        None
    );
    assert_eq!(
        protected_interface_path_reference("// crate::domain::task::template::TaskRecord"),
        None
    );
    assert_eq!(
        protected_interface_path_reference(
            "const RAW: &str = r#\"crate::domain::task::template\"#;"
        ),
        None
    );
    assert!(contains_relative_path(
        "use super::super::hooks::event::run_dir_name;",
        "hooks"
    ));
    assert!(contains_relative_path(
        "use super::super::interfaces::hooks::event::run_dir_name;",
        "interfaces::hooks"
    ));
    assert!(contains_relative_path(
        "use super::{interfaces::hooks::event::run_dir_name};",
        "interfaces::hooks"
    ));
    assert!(contains_relative_path(
        "use self::{interfaces::hooks::{self}};",
        "interfaces::hooks"
    ));
    assert!(contains_bare_path_reference(
        "use hooks::event::run_dir_name;",
        "hooks"
    ));
    assert!(contains_bare_path_reference(
        "fn run() { hooks::event::run_dir_name(\"agent/session\"); }",
        "hooks"
    ));
    assert!(!contains_bare_path_reference(
        "use crate::interfaces::hooks::event::run_dir_name;",
        "hooks"
    ));
    assert!(contains_bare_interfaces_hooks_reference(
        "use interfaces::hooks;"
    ));
    assert!(contains_bare_interfaces_hooks_reference(
        "use interfaces::{hooks::{self}};"
    ));
    assert!(contains_bare_interfaces_hooks_reference(
        "fn run() { interfaces::hooks::event::run_dir_name(\"agent/session\"); }"
    ));
    assert!(!contains_bare_interfaces_hooks_reference(
        "pub use crate::interfaces::hooks;"
    ));
    assert!(contains_root_import(
        "use crate::interfaces::hooks;",
        "crate::interfaces::hooks"
    ));
    assert!(contains_crate_interfaces_hooks_reference(
        "use crate::interfaces::hooks::{self};"
    ));
    assert!(contains_crate_interfaces_hooks_reference(
        "use crate::interfaces::{hooks::{self}};"
    ));
    assert_eq!(
        namespaced_deep_import_segment(
            "use crate::interfaces::hooks::event::run_dir_name;",
            "interfaces",
            "hooks"
        ),
        Some("event")
    );
}

#[test]
fn module_reference_scanners_only_match_at_a_module_boundary() {
    // A compound identifier ending in the prefix is not a module reference.
    assert!(
        module_path_references("use super::verify_task::VerificationReport;", "task::").is_empty()
    );
    assert!(module_symbol_references("fn go() { overrun::start(); }", "run::").is_empty());

    // Real module paths still match, including nested and leading positions.
    assert!(
        module_path_references("use crate::domain::task::Status;", "task::").contains("Status")
    );
    assert!(module_path_references("task::Status", "task::").contains("Status"));
    assert!(module_symbol_references("run::append_event()", "run::").contains("append_event"));
}

#[test]
fn vnext_authority_and_store_keep_one_directional_ownership() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src/domain/persistence")) {
        let source = source_without_test_modules(&read_source_file(&file));
        if source.contains("domain::vnext::authority")
            || source.contains("super::super::authority")
            || source.contains("super::authority")
        {
            violations.push(format!(
                "{} imports semantic Authority into Persistence",
                file.display()
            ));
        }
    }

    for file in rust_files_under(Path::new("src/domain/authority")) {
        let relative = file.strip_prefix("src/domain/authority").unwrap();
        if relative == Path::new("facade_tests.rs") {
            continue;
        }
        let source = source_without_test_modules(&read_source_file(&file));
        if ![
            Path::new("facade.rs"),
            Path::new("facade/repository_admission.rs"),
            Path::new("facade/repository_leaf_authority.rs"),
            Path::new("publication.rs"),
        ]
        .contains(&relative)
            && source.contains("domain::vnext::persistence")
        {
            violations.push(format!(
                "{} bypasses the sole Authority-to-Store facade edge",
                file.display()
            ));
        }
        for forbidden in [
            "rusqlite",
            "std::fs",
            "std::net",
            "std::process",
            "SystemTime",
            "crate::interfaces",
        ] {
            if source.contains(forbidden) {
                violations.push(format!(
                    "{} imports forbidden Authority mechanism {forbidden}",
                    file.display()
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "vNext Authority owns semantics and only its publication facade boundary may call the canonical Store:\n{}",
        violations.join("\n")
    );
}

#[test]
fn stage5_protected_diagnostic_ports_are_sealed_test_only_and_non_bearer() {
    let integration = read_source_file(Path::new(
        "src/domain/integration/trusted_host_diagnostic.rs",
    ));
    let persistence = read_source_file(Path::new("src/domain/persistence/protected_diagnostic.rs"));
    let stage10_seed = read_source_file(Path::new(
        "src/domain/integration/trusted_host_diagnostic_stage10_seed.rs",
    ));
    let stage9_seed = read_source_file(Path::new(
        "src/domain/persistence/protected_diagnostic_stage9_seed.rs",
    ));
    let envelope = read_source_file(Path::new(
        "src/domain/authority/protected_diagnostic_envelope.rs",
    ));
    let stage8_seed = read_source_file(Path::new(
        "src/domain/authority/protected_diagnostic_envelope_stage8_seed.rs",
    ));
    let integration_module = read_source_file(Path::new("src/domain/integration/mod.rs"));
    let persistence_module = read_source_file(Path::new("src/domain/persistence/mod.rs"));
    let store = read_source_file(Path::new("src/domain/persistence/store.rs"));
    let facade = read_source_file(Path::new("src/domain/authority/facade.rs"));
    let diagnostic_entry = facade
        .split("pub(crate) fn protected_continuity_diagnostic_with_ports")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn protected_continuity_diagnostic_with_mode")
                .next()
        })
        .expect("Stage 5 diagnostic entry must remain a bounded production-neutral facade seam");
    let diagnostic_entry_and_mode = facade
        .split("pub(crate) fn protected_continuity_diagnostic_with_ports")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn build_protected_continuity_diagnostic")
                .next()
        })
        .expect("Stage 5 diagnostic entry and closed mode must remain locally reviewable");
    let diagnostic_kernel = facade
        .split("fn build_protected_continuity_diagnostic")
        .nth(1)
        .and_then(|tail| tail.split("fn validate_post_cut_current_authority").next())
        .expect("Stage 5 diagnostic kernel must remain locally reviewable");
    let diagnostic_operation = diagnostic_kernel
        .split("fn protected_diagnostic_invocation_nonce")
        .next()
        .expect("diagnostic operation must precede its private nonce helpers");
    let issuer_before_view = diagnostic_entry_and_mode
        .find("ProtectedDiagnosticInvocationIssuerV1::fresh()")
        .expect("process and invocation entropy must be acquired before the Store view");
    let serialized_view = diagnostic_entry_and_mode
        .find("with_serialized_active_view")
        .expect("diagnostic must execute inside one serialized Store view");
    assert!(issuer_before_view < serialized_view);
    assert!(!diagnostic_operation.contains("RandomState::new()"));

    assert!(integration.contains("pub(crate) trait TrustedHostDiagnosticConnectionPortV1"));
    assert!(integration.contains("pub(crate) trait TrustedHostDiagnosticAttestationPortV1"));
    assert!(integration.contains("pub(crate) trait TrustedHostDiagnosticPresentationPortV1"));
    assert!(integration.contains("pub(in crate::domain::integration) mod sealed"));
    assert!(!integration_module.contains("PortSealedV1"));
    assert!(stage10_seed.contains("impl sealed::Connection"));
    assert!(stage10_seed.contains("impl sealed::Attestation"));
    assert!(stage10_seed.contains("impl sealed::Presentation"));
    assert!(
        stage10_seed
            .contains("connection: &'connection mut Stage10OwnerLocalConnectionSeedV1<'host>")
    );
    assert!(stage10_seed.contains("challenge: TrustedHostDiagnosticChallengeV1"));
    assert!(integration.contains(
        "#[cfg(test)]\npub(crate) struct TrustedHostDiagnosticAttestationV1<'connection, 'anchor, 'view>"
    ));
    assert!(!integration.contains("TrustedHostDiagnosticTestInvocationV1"));
    assert!(!integration.contains("begin_invocation"));
    assert!(integration.contains("fn attest_in_current_view<'scope, 'view>"));
    assert!(!integration.contains("AtomicU64"));
    assert!(
        integration.contains(
            "current_view_anchor: &'anchor ProtectedDiagnosticCurrentViewAnchorV1<'view>"
        )
    );
    assert!(!integration.contains("operator_mapping_commitment"));
    assert!(!integration.contains("request_commitment"));
    assert!(!integration.contains("fn matches_operator"));
    assert!(!integration.contains("challenge.protected_subject_commitment() == [0; 32]"));
    assert!(!integration.contains("challenge.anchor_commitment() !="));
    assert!(!integration.contains("challenge.authority_commitment() == [0; 32]"));
    assert!(!integration.contains("identity.principal_identity =="));
    assert!(!integration.contains("identity.binding_identity =="));
    assert!(!integration.contains("identity.session_identity =="));
    assert!(!integration.contains("identity.domain_role =="));
    assert!(!integration.contains("operator_identity =="));
    assert!(!integration.contains("pub(crate) fn binds"));
    assert!(integration.contains("pub(crate) fn present_once"));
    assert!(
        integration
            .contains("#[cfg(test)]\npub(crate) struct TrustedHostDiagnosticTestConnectionV1")
    );
    assert!(
        persistence
            .contains("pub(crate) trait ProtectedDiagnosticCurrentViewProviderV1: sealed::Sealed")
    );
    assert!(persistence.contains("pub(in crate::domain::persistence) mod sealed"));
    assert!(!persistence_module.contains("ProviderSealedV1"));
    assert!(persistence.contains("pub(in crate::domain::persistence) fn from_live_provider"));
    assert!(stage9_seed.contains("impl sealed::Sealed"));
    assert!(stage9_seed.contains("ProtectedDiagnosticProviderCurrentnessV1::from_live_provider"));
    assert!(
        persistence
            .contains("provider: Option<&'view mut dyn ProtectedDiagnosticCurrentViewProviderV1>")
    );
    assert!(persistence.contains("provider_binding: Option<ProtectedDiagnosticProviderBindingV1>"));
    assert!(persistence.contains("fn final_recheck_current_view("));
    assert!(persistence.contains("fn abandon_current_view(&mut self)"));
    assert!(persistence.contains("provider_currentness_revision"));
    assert!(!persistence.contains("PhantomData"));
    let provider_trait = persistence
        .split("pub(crate) trait ProtectedDiagnosticCurrentViewProviderV1")
        .nth(1)
        .and_then(|tail| tail.split("#[cfg(test)]").next())
        .expect("sealed current-view provider trait must remain locally reviewable");
    assert!(!provider_trait.contains("Option<[u8; 32]>"));
    assert!(store.contains(
        "ProtectedDiagnosticCurrentViewAnchorV1::bind(provider, &observed, self.root.path())"
    ));
    assert!(store.contains("consume_protected_diagnostic_current_view_anchor"));
    assert!(store.contains("anchor.consume_final_recheck(&observed)"));
    assert!(
        persistence.contains(
            "#[cfg(test)]\npub(crate) struct ProtectedDiagnosticTestCurrentViewProviderV1"
        )
    );
    assert!(facade.contains(
        "#[cfg(test)]\n    pub(crate) fn protected_continuity_diagnostic_reference_envelope"
    ));
    assert!(
        !facade
            .contains("#[cfg(test)]\n    pub(crate) fn protected_continuity_diagnostic_with_ports")
    );
    assert!(
        diagnostic_entry.contains("connection: &mut dyn TrustedHostDiagnosticConnectionPortV1")
    );
    assert!(
        diagnostic_entry
            .contains("current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1")
    );
    assert!(!diagnostic_entry.contains("envelope_builder"));
    assert!(!facade.contains("ProtectedContinuityDiagnosticEnvelopeBuilderV1"));
    assert!(!facade.contains("ProtectedContinuityDiagnosticPreparedEnvelopeV1"));
    assert!(envelope.contains("pub(crate) struct ProtectedContinuityDiagnosticReleasedEnvelopeV1"));
    assert!(envelope.contains("struct ProtectedContinuityDiagnosticCandidateEnvelopeV1"));
    assert!(envelope.contains("pub(super) struct ProtectedContinuityDiagnosticPreparedCarrierV1"));
    assert!(envelope.contains("const MAX_ENVELOPE_BYTES_V1: usize = 1024"));
    assert!(envelope.contains("struct BoundedEnvelopeWriterV1"));
    assert!(
        envelope.contains("candidate.bytes[..candidate.len] != expected.bytes[..expected.len]")
    );
    assert!(!envelope.contains("Box<dyn ProtectedContinuityDiagnosticPreparedEnvelopeV1"));
    assert!(!envelope.contains("fn into_bytes(self: Box<Self>) -> Vec<u8>"));
    assert!(envelope.contains("ProtectedContinuityDiagnosticEnvelopeInputV1"));
    for getter in [
        "admission_ref",
        "attempt_ref",
        "generation_ref",
        "reason_class",
        "expected_carrier_state",
        "observed_carrier_state",
        "freshness_class",
        "remediation_class",
        "fence_subject_ref",
        "fence_carrier_ref",
        "semantic_point_ref",
        "covered_closure_ref",
        "conservative_point_envelope_ref",
        "carrier_revision_ref",
        "current_view_anchor_ref",
        "authority_snapshot_ref",
        "attestation_carrier_ref",
    ] {
        assert!(envelope.contains(&format!("fn {getter}")));
    }
    assert!(diagnostic_kernel.contains("ProtectedContinuityDiagnosticEnvelopeInputV1::new("));
    assert!(diagnostic_kernel.contains("prepare_current_protected_snapshot("));
    assert!(stage8_seed.contains("encode_canonical_envelope(input)?"));
    assert!(
        stage8_seed.contains("ProtectedContinuityDiagnosticAssemblerModeV1::SubstituteAdmission")
    );
    assert!(stage8_seed.contains("ProtectedContinuityDiagnosticAssemblerModeV1::IgnoreInput"));
    assert!(
        stage8_seed
            .contains("fn stage8_owner_local_descendant_is_the_only_concrete_assembler_seed")
    );
    assert!(stage8_seed.contains("const _: () = {"));
    assert!(stage8_seed.contains("protected_continuity_diagnostic_with_ports("));
    assert!(stage8_seed.contains(".map(|released| released.into_bytes())"));

    for forbidden in [
        "RepositoryAuthenticatedHumanV1",
        "request_commitment",
        "authenticated_host_connection_context_ref",
        "ScopeAtomV1",
        "ActionRequestIdV1",
        "AuthorizationReceiptV1",
        "FnOnce",
        "println!",
        "eprintln!",
        "panic!",
        "tracing::",
        "log::",
    ] {
        assert!(
            !diagnostic_entry.contains(forbidden) && !diagnostic_kernel.contains(forbidden),
            "protected diagnostic entry must not derive authority from {forbidden}"
        );
    }
    for forbidden in [
        "Assessment",
        "Gate",
        "Evidence",
        "Audit",
        "ScopeAtom",
        "ActionRequest",
        "AuthorizationReceipt",
        "ActionResult",
        "Idempotency",
    ] {
        assert!(
            !diagnostic_kernel.contains(forbidden),
            "protected diagnostic kernel must not introduce {forbidden} authority or effects"
        );
    }
    let attestation_definition = integration
        .split("pub(crate) struct TrustedHostDiagnosticAttestationV1")
        .nth(1)
        .expect("attestation port must exist");
    let prefix = integration
        .split("pub(crate) struct TrustedHostDiagnosticAttestationV1")
        .next()
        .unwrap();
    let nearby_prefix = &prefix[prefix.len().saturating_sub(160)..];
    assert!(!nearby_prefix.contains("derive("));
    for forbidden in [
        "Serialize",
        "Default",
        "canonical_bytes",
        "pub fn new",
        "pub(crate) fn new",
        "impl From",
        "impl TryFrom",
    ] {
        assert!(
            !attestation_definition
                .lines()
                .take(80)
                .any(|line| line.contains(forbidden)),
            "attestation must not expose {forbidden}"
        );
    }
    assert!(!integration.contains("pub mod trusted_host_diagnostic"));
    assert!(!persistence.contains("pub mod protected_diagnostic"));
    assert!(integration_module.contains("TrustedHostDiagnosticConnectionPortV1"));
    assert!(integration_module.contains(
        "#[cfg(test)]\npub(crate) use trusted_host_diagnostic::{\n    TrustedHostDiagnosticTestClaimsV1"
    ));
    assert!(persistence_module.contains("ProtectedDiagnosticCurrentViewProviderV1"));
    assert!(persistence_module.contains(
        "#[cfg(test)]\npub(crate) use protected_diagnostic::{\n    ProtectedDiagnosticTestAnchorMutationV1"
    ));
    assert!(!integration_module.contains("pub use trusted_host_diagnostic"));
    assert!(!integration_module.contains("TrustedHostDiagnosticChallengeV1"));
    assert!(!persistence_module.contains("pub use protected_diagnostic"));
    assert!(!diagnostic_entry.contains("pub(crate) const fn witness"));
    assert!(!diagnostic_entry.contains("ProtectedContinuityDiagnosticReadGuardV1<'view>"));
    assert!(!diagnostic_kernel.contains("operator_mapping_commitment"));
    assert!(!integration.contains("claims_commitment"));
    assert!(!integration.contains("witness_carrier_commitment"));
    assert!(integration.contains("fn attestation_commitment(&self) -> [u8; 32]"));
    assert!(facade.contains("struct ProtectedDiagnosticPresentedHostFactsV1"));
    assert!(
        diagnostic_kernel
            .contains("ProtectedDiagnosticPresentedHostFactsV1::capture(presentation)")
    );
    let diagnostic_build = diagnostic_kernel
        .split("struct ProtectedDiagnosticPresentedHostFactsV1")
        .next()
        .expect("the Authority build must precede its private captured host-facts carrier");
    assert_eq!(
        diagnostic_build
            .matches("ProtectedDiagnosticPresentedHostFactsV1::capture(presentation)")
            .count(),
        1
    );
    assert_eq!(diagnostic_build.matches("presentation.").count(), 0);
    let host_fact_capture = facade
        .split("fn capture(presentation: &dyn TrustedHostDiagnosticPresentationPortV1)")
        .nth(1)
        .and_then(|tail| tail.split("fn attestation_commitment(&self)").next())
        .expect("Authority must snapshot every presented host fact exactly once");
    for getter in [
        "anchor_commitment",
        "authority_commitment",
        "protected_subject_commitment",
        "invocation_nonce",
        "challenge_commitment",
        "attestation_commitment",
        "provider_identity",
        "profile_identity",
        "profile_revision",
        "process_incarnation",
        "connection_incarnation",
        "channel_incarnation",
        "issuer_identity",
        "realm_identity",
        "audience_identity",
        "authentication_event_identity",
        "host_currentness_revision",
        "revocation_revision",
        "freshness_identity",
        "carrier_commitment",
        "principal_identity",
        "binding_identity",
        "session_identity",
        "context_identity",
        "trust_root_revision",
        "assurance_revision",
        "human_capable",
        "binding_not_before",
        "binding_expires_at",
        "session_not_before",
        "session_expires_at",
        "store_generation",
        "authority_epoch",
        "domain_identity",
        "domain_role",
        "incarnation_revision",
    ] {
        assert_eq!(
            host_fact_capture
                .matches(&format!("presentation.{getter}()"))
                .count(),
            1,
            "Authority must capture {getter} exactly once"
        );
    }
    assert!(diagnostic_kernel.contains("presented_host_facts.presented_attestation_commitment"));
    assert!(diagnostic_kernel.contains("!= recomputed_attestation_commitment"));
    assert!(
        diagnostic_kernel
            .contains("final_attestation_commitment != verified_attestation_commitment")
    );
    assert!(
        diagnostic_kernel
            .contains("ContinuityReferenceV1::from_digest(verified_attestation_commitment)")
    );
    let active_anchor = diagnostic_kernel
        .find("protected_diagnostic_current_view_anchor")
        .expect("Active current-view anchor must be established");
    let issuance = diagnostic_kernel
        .find("protected_diagnostic_invocation_nonce(")
        .expect("Authority must mint the invocation identity in the current view");
    let challenge = diagnostic_kernel
        .find("TrustedHostDiagnosticChallengeV1::from_authority_issuance(")
        .expect("Authority must construct the same-view challenge");
    let attestation = diagnostic_kernel
        .find(".attest_in_current_view(")
        .expect("Integration must answer the Authority-issued invocation in the same view");
    assert!(active_anchor < issuance && issuance < challenge && challenge < attestation);
    let prepared_freeze = diagnostic_kernel
        .find("prepare_current_protected_snapshot(&envelope_input, assembler_mode)")
        .expect("Authority must prepare and validate the fixed envelope before final rechecks");
    let host_final_recheck = diagnostic_kernel
        .find(".final_recheck()")
        .expect("host currentness must be rechecked before diagnostic return");
    let store_final_recheck = diagnostic_kernel
        .find("consume_protected_diagnostic_current_view_anchor(current_view_anchor)")
        .expect("Persistence currentness lease must be consumed before diagnostic return");
    assert!(
        attestation < prepared_freeze
            && prepared_freeze < host_final_recheck
            && host_final_recheck < store_final_recheck
    );
    let authority_release = diagnostic_kernel
        .find("Ok(prepared.release())")
        .expect("only Authority may release the prepared envelope");
    assert!(store_final_recheck < authority_release);
    assert!(facade.contains("AtomicU64"));
    assert!(facade.contains("invocation_entropy"));
    assert!(facade.contains("process_incarnation"));
    assert!(facade.contains("sequence.checked_add(1)"));
    assert!(!facade.contains("ISSUED_INVOCATIONS"));
    assert!(facade.contains("ProtectedDiagnosticInvocationIssuerV1"));
    assert!(facade.contains("RandomState::new()"));
    assert!(facade.contains("protected_diagnostic_process_incarnation"));
    assert!(facade.contains("protected_diagnostic_invocation_sequence"));
    assert!(
        facade.contains(
            "_current_view_anchor: &'anchor ProtectedDiagnosticCurrentViewAnchorV1<'view>"
        )
    );
    assert!(!facade.contains("PhantomData<&'view ()>"));
    assert!(!integration.contains("struct TrustedHostDiagnosticChallengeV1"));
    assert!(!integration.contains("from_authority_issuance"));

    let connection_port_prefix = integration
        .split("pub(crate) trait TrustedHostDiagnosticConnectionPortV1")
        .next()
        .expect("trusted-host port prefix must exist");
    assert!(!connection_port_prefix.ends_with("#[cfg(test)]\n"));
    let provider_port_prefix = persistence
        .split("pub(crate) trait ProtectedDiagnosticCurrentViewProviderV1")
        .next()
        .expect("current-view provider prefix must exist");
    assert!(!provider_port_prefix.ends_with("#[cfg(test)]\n"));
    assert!(
        !store.contains("#[cfg(test)]\n    pub(crate) fn protected_diagnostic_current_view_anchor")
    );
    assert!(!store.contains(
        "#[cfg(test)]\n    pub(crate) fn consume_protected_diagnostic_current_view_anchor"
    ));
    assert!(integration.contains("pub(in crate::domain::integration) mod sealed"));
    assert!(!integration_module.contains("PortSealedV1"));
    assert!(!persistence_module.contains("ProviderSealedV1"));
    assert!(persistence_module.contains("ProtectedDiagnosticObservedCurrentViewV1"));
    assert!(persistence_module.contains("ProtectedDiagnosticProviderCurrentnessV1"));
    assert!(persistence.contains("pub(in crate::domain::persistence) fn from_live_provider"));
    assert!(!persistence.contains("provider_currentness_commitment: [u8; 32]"));

    let mut challenge_constructors = Vec::new();
    let mut presentation_consumers = Vec::new();
    let mut connection_port_implementors = Vec::new();
    let mut current_view_provider_implementors = Vec::new();
    let mut integration_seal_implementors = Vec::new();
    let mut persistence_seal_implementors = Vec::new();
    let mut obsolete_builder_contracts = Vec::new();
    let mut released_envelope_constructors = Vec::new();
    for file in rust_files_under(Path::new("src")) {
        let source = read_source_file(&file);
        if source.contains("TrustedHostDiagnosticChallengeV1::from_authority_issuance(") {
            challenge_constructors.push(file.clone());
        }
        if source.contains(".present_once(") {
            presentation_consumers.push(file.clone());
        }
        if source.contains("impl TrustedHostDiagnosticConnectionPortV1 for") {
            connection_port_implementors.push(file.clone());
        }
        if source.contains("impl ProtectedDiagnosticCurrentViewProviderV1 for") {
            current_view_provider_implementors.push(file.clone());
        }
        if source.contains("impl sealed::Connection")
            || source.contains("impl sealed::Attestation")
            || source.contains("impl sealed::Presentation")
        {
            integration_seal_implementors.push(file.clone());
        }
        if source.contains("impl sealed::Sealed") {
            persistence_seal_implementors.push(file.clone());
        }
        if source.contains("ProtectedContinuityDiagnosticEnvelopeBuilderV1")
            || source.contains("ProtectedContinuityDiagnosticPreparedEnvelopeV1")
        {
            obsolete_builder_contracts.push(file.clone());
        }
        if source.contains("ProtectedContinuityDiagnosticReleasedEnvelopeV1 { prepared }") {
            released_envelope_constructors.push(file);
        }
    }
    connection_port_implementors.sort();
    current_view_provider_implementors.sort();
    integration_seal_implementors.sort();
    persistence_seal_implementors.sort();
    obsolete_builder_contracts.sort();
    released_envelope_constructors.sort();
    assert_eq!(
        challenge_constructors,
        [PathBuf::from("src/domain/authority/facade.rs")]
    );
    assert_eq!(
        presentation_consumers,
        [PathBuf::from("src/domain/authority/facade.rs")]
    );
    assert_eq!(
        connection_port_implementors,
        [
            PathBuf::from("src/domain/integration/trusted_host_diagnostic.rs"),
            PathBuf::from("src/domain/integration/trusted_host_diagnostic_stage10_seed.rs")
        ]
    );
    assert_eq!(
        current_view_provider_implementors,
        [
            PathBuf::from("src/domain/persistence/protected_diagnostic.rs"),
            PathBuf::from("src/domain/persistence/protected_diagnostic_stage9_seed.rs")
        ]
    );
    assert_eq!(
        integration_seal_implementors,
        [
            PathBuf::from("src/domain/integration/trusted_host_diagnostic.rs"),
            PathBuf::from("src/domain/integration/trusted_host_diagnostic_stage10_seed.rs")
        ]
    );
    assert_eq!(
        persistence_seal_implementors,
        [
            PathBuf::from("src/domain/persistence/protected_diagnostic.rs"),
            PathBuf::from("src/domain/persistence/protected_diagnostic_stage9_seed.rs")
        ]
    );
    assert!(obsolete_builder_contracts.is_empty());
    assert_eq!(
        released_envelope_constructors,
        [PathBuf::from(
            "src/domain/authority/protected_diagnostic_envelope.rs"
        )]
    );
    assert!(integration.contains(
        "#[cfg(test)]\nimpl TrustedHostDiagnosticConnectionPortV1 for TrustedHostDiagnosticTestConnectionV1"
    ));
    assert!(persistence.contains(
        "#[cfg(test)]\nimpl ProtectedDiagnosticCurrentViewProviderV1 for ProtectedDiagnosticTestCurrentViewProviderV1"
    ));
    let vnext_module = read_source_file(Path::new("src/domain/mod.rs"));
    assert!(!vnext_module.contains("protected_diagnostic_sibling_port_compile_probe"));
    assert!(!vnext_module.contains("impl TrustedHostDiagnosticConnectionPortV1"));
    assert!(!vnext_module.contains("impl ProtectedDiagnosticCurrentViewProviderV1"));
    assert!(!vnext_module.contains("from_live_provider"));
    assert!(
        stage10_seed
            .contains("fn stage10_owner_local_descendant_can_implement_the_sealed_host_ports")
    );
    assert!(stage10_seed.contains("claim_authenticated_invocation_no_io"));
    assert!(stage10_seed.contains("recheck_authenticated_invocation_no_io"));
    assert!(stage10_seed.contains("acquire_from_authenticated_host"));
    assert!(!stage10_seed.contains("fixed_digest_getters"));
    assert!(!stage10_seed.contains("fixed_revision_getters"));
    assert!(
        stage9_seed
            .contains("fn stage9_owner_local_descendant_can_mint_only_structured_live_currentness")
    );
}

#[test]
fn stage5_successor_seams_are_owner_private_and_production_replaceable() {
    let authority_facade = read_source_file(Path::new("src/domain/authority/facade.rs"));
    let authority = read_source_file(Path::new("src/domain/authority/governance_attestation.rs"));
    let materialization = read_source_file(Path::new("src/domain/authority/materialization.rs"));
    let governance_floor = read_source_file(Path::new("src/domain/authority/governance_floor.rs"));
    let publication = read_source_file(Path::new("src/domain/authority/publication.rs"));
    let authority_seed = read_source_file(Path::new(
        "src/domain/authority/governance_attestation_stage7_seed.rs",
    ));
    let persistence = read_source_file(Path::new(
        "src/domain/persistence/protected_locator_lease.rs",
    ));
    let persistence_stage9 = read_source_file(Path::new(
        "src/domain/persistence/protected_locator_stage9_seed.rs",
    ));
    let foundation = read_source_file(Path::new("src/foundation/core/aggregate_census.rs"));
    let foundation_seed = read_source_file(Path::new(
        "src/foundation/core/aggregate_census_stage11_seed.rs",
    ));
    let installation = read_source_file(Path::new("src/domain/installation/durable_finality.rs"));
    let installation_stage9 = read_source_file(Path::new(
        "src/domain/installation/durable_finality_stage9_seed.rs",
    ));
    let installation_stage11 = read_source_file(Path::new(
        "src/domain/installation/durable_finality_stage11_seed.rs",
    ));
    let authority_mod = read_source_file(Path::new("src/domain/authority/mod.rs"));
    let persistence_mod = read_source_file(Path::new("src/domain/persistence/mod.rs"));
    let foundation_mod = read_source_file(Path::new("src/foundation/core/mod.rs"));
    let installation_mod = read_source_file(Path::new("src/domain/installation/mod.rs"));
    let singular = read_source_file(Path::new(
        "src/foundation/core/descriptor_census_platform.rs",
    ));
    let vnext_module = read_source_file(Path::new("src/domain/mod.rs"));

    assert!(authority.contains("GovernanceAttestationV1<'tx"));
    assert!(authority.contains("RepositoryGovernanceFloorCurrentViewV1<'tx>"));
    assert!(materialization.contains("VerifiedSchedulingPolicyDowngradeMandateUseV1"));
    assert!(materialization.contains("AdmittedRepositoryActionBindingV1"));
    assert!(!authority.contains("supplemental_mandate_present: bool"));
    assert!(!authority.contains("governance_floor: [u64; 4]"));
    assert!(!authority.contains("classifier_result"));
    assert!(!authority.contains("safety_floor: [u64; 4]"));
    assert!(!governance_floor.contains("AUTHORITY_PINNED_SCHEDULING_SAFETY_FLOOR_V1"));
    assert!(!governance_floor.contains("= [1; 4]"));
    assert!(governance_floor.contains("AUTHORITY_SCHEDULING_SAFETY_MAXIMUMS_V1"));
    assert!(governance_floor.contains("checked_sub(maximum)"));
    assert!(governance_floor.contains("AuthoritySchedulingSafetyStateV1"));
    assert!(governance_floor.contains("floor_identity"));
    assert!(governance_floor.contains("floor_semantic_hash"));
    assert!(governance_floor.contains("classifier_semantic_hash"));
    assert!(governance_floor.contains("scheduling_safety.currentness"));
    assert!(governance_floor.contains("resolve_authority_scheduling_safety_state"));
    assert!(governance_floor.contains("REPOSITORY_GOVERNANCE_FLOOR_SCHEMA_TAG_V1: usize = 25"));
    assert!(governance_floor.contains("\"maestro.vnext.repository-governance-floor-snapshot.v1\""));
    assert!(governance_floor.contains("restore_requires_exact_same_domain_chain"));
    assert!(publication.contains("RepositoryGovernanceFloorSnapshot"));
    assert!(!authority.contains("pub struct GovernanceAttestationV1"));
    assert!(authority_seed.contains("publish_scheduling_policy_from_stage7"));
    assert!(authority_seed.contains("SchedulingPolicyPublicationInputV1"));
    assert!(authority_seed.contains("PlanningSchedulingPolicyInputV1"));
    assert!(authority_seed.contains("SchedulingPolicyPublicationInputV1::new"));
    assert!(authority_seed.contains("const _: fn() = ||"));
    assert!(!authority_seed.contains("#[expect(\n    dead_code,"));
    assert!(authority_facade.contains("pub(super) struct SchedulingPolicyPublicationInputV1"));
    assert!(
        !authority_facade
            .contains("pub(in crate::domain) struct SchedulingPolicyPublicationInputV1")
    );
    assert!(authority_facade.contains("pub(super) fn new("));
    assert!(authority_facade.contains("pub(super) fn publish_scheduling_policy("));
    assert!(
        !authority_facade.contains("pub(super) fn publish_scheduling_policy_without_downgrade")
    );
    assert!(!authority_facade.contains("pub(super) fn publish_scheduling_policy_with_downgrade"));
    assert!(vnext_module.contains("stage7_governance_seed_compile_probe"));
    assert!(vnext_module.contains("stage7_sibling_can_name_and_call_only_the_seed_owned_entry"));

    assert!(persistence.contains("ProtectedLocatorLeaseV1<'locator>"));
    assert!(persistence.contains("ProtectedLocatorAcquisitionRequestV2"));
    assert!(persistence.contains("ProtectedLocatorLeaseV2<'locator>"));
    assert!(persistence.contains("ProtectedLocatorCandidateTransitionV2<'locator>"));
    assert!(persistence.contains("acquire_pre_candidate"));
    assert!(persistence.contains("bind_inert_candidate"));
    assert!(persistence.contains("dispatch_exact_transition"));
    assert!(!persistence.contains("#[derive(Clone, Copy, Eq, PartialEq)]\npub(in crate::domain) struct ProtectedLocatorAcquisitionRequestV2"));
    assert!(!persistence.contains("#[derive(Clone, Copy, Eq, PartialEq)]\npub(in crate::domain) struct ProtectedLocatorObservedStateV2"));
    assert!(!persistence.contains("#[derive(Clone, Copy, Eq, PartialEq)]\npub(in crate::domain) struct ProtectedLocatorCandidateInputV2"));
    assert!(!persistence.contains("impl Clone for ProtectedLocatorLeaseV2"));
    assert!(!persistence.contains("impl Copy for ProtectedLocatorLeaseV2"));
    assert!(persistence.contains("begin_pre_store"));
    assert!(persistence.contains("ProtectedLocatorCeremonyContinuationV1<'locator>"));
    assert!(persistence.contains("dispatch_expected_old"));
    assert!(
        !persistence.contains(
            "#[derive(Clone, Copy)]\npub(in crate::domain) struct ProtectedLocatorLeaseV1"
        )
    );
    assert!(!persistence.contains("pub struct ProtectedLocatorLeaseV1"));

    assert!(foundation.contains("AggregateCensusLeaseV1<'scan"));
    assert!(foundation.contains("RepositoryRootAdmissionV2"));
    assert!(foundation.contains("InstallationRootAdmissionV2"));
    assert!(foundation.contains("CensusInvocationV2"));
    assert!(foundation.contains("FoundationAdmittedRootSourceV2"));
    assert!(foundation.contains("AggregateCensusLeaseV2<'scan>"));
    assert!(foundation.contains("AggregateCensusResultV2<'scan>"));
    assert!(foundation.contains("MigrationClassificationContinuationV2<'scan>"));
    assert!(foundation.contains("acquire_complete_admitted_root_source"));
    assert!(!foundation.contains("impl Clone for AggregateCensusLeaseV2"));
    assert!(!foundation.contains("impl Copy for AggregateCensusLeaseV2"));
    assert!(foundation.contains("acquire_complete_root_set"));
    assert!(foundation.contains("final_root_set_recheck"));
    assert!(foundation_seed.contains("Stage11AggregateCensusBackendSeedV1"));
    assert!(foundation_seed.contains("Stage11AggregateCensusOutputV1<'scan>"));
    assert!(foundation_seed.contains("pub(crate) struct Stage11AggregateCensusBackendSeedV1"));
    assert!(foundation_seed.contains("pub(crate) fn test_unavailable()"));
    assert!(foundation_seed.contains("pub(super) fn census_from_stage11_owner"));
    assert!(foundation_seed.contains("pub(super) fn into_parts"));
    assert!(!foundation_seed.contains("pub(super) fn acquire()"));
    assert!(!foundation_seed.contains("pub fn census_from_stage11_owner"));
    assert!(!foundation_seed.contains("#[derive(Clone"));
    assert!(singular.contains("singular-root production route is intentionally retired"));
    assert!(!foundation.contains("pub struct AggregateCensusLeaseV1"));

    assert!(installation.contains("DurableInstallationFinalityBackendV1<'effect"));
    assert!(installation.contains("DurableInstallationFinalityBackendV2<'effect"));
    assert!(installation.contains("ActiveStoreFinalityRequestV2"));
    assert!(installation.contains("PreStoreFinalityRequestV2"));
    assert!(installation.contains("ProtectedLocatorLeaseV2<'_>"));
    assert!(installation.contains("validate_inert_candidate"));
    assert!(!installation.contains("impl Clone for DurableInstallationFinalityBackendV2"));
    assert!(!installation.contains("impl Copy for DurableInstallationFinalityBackendV2"));
    assert!(installation.contains("ProtectedLocatorLeaseV1<'locator>"));
    assert!(installation.contains("ActiveStoreDecisionTupleV1"));
    assert!(installation.contains("PreStoreDecisionTupleV1"));
    assert!(installation.contains("commit_and_readback"));
    assert!(installation.contains("ProtectedLocatorCeremonyContinuationV1<'locator>"));
    assert!(installation_stage9.contains("impl ActiveStoreFinalityOwnerV1"));
    assert!(installation_stage11.contains("impl PreStoreFinalityOwnerV1"));
    assert!(
        installation_stage9
            .contains("pub(in crate::domain) struct Stage9ActiveStoreFinalitySeedV1")
    );
    assert!(
        installation_stage11.contains("pub(in crate::domain) struct Stage11PreStoreFinalitySeedV1")
    );
    assert!(!installation_stage9.contains("pub(super) fn acquire()"));
    assert!(!installation_stage11.contains("pub(super) fn acquire()"));
    assert!(installation.contains("Stage11PreStoreFinalityOperationV1<'effect>"));
    assert!(installation.contains("Stage9ActiveStoreFinalityOperationV1<'effect>"));
    assert!(installation.contains("prepare_active_from_stage9_owner"));
    assert!(installation.contains("prepare_pre_store_from_stage11_owner"));
    assert!(installation.contains("execute_pre_store_from_stage11_owner"));
    assert!(!installation.contains("pub struct Stage11PreStoreFinalityOperationV1"));
    assert!(!installation.contains(
        "#[derive(Clone, Copy)]\npub(in crate::domain) struct Stage11PreStoreFinalityOperationV1"
    ));
    assert!(!installation.contains("impl FnOnce"));
    assert!(!installation.contains("pub struct DurableInstallationFinalityBackendV1"));

    assert!(!authority_mod.contains("pub mod governance_attestation"));
    assert!(!persistence_mod.contains("pub mod protected_locator_lease"));
    assert!(foundation_mod.contains("\nmod aggregate_census;\n"));
    assert!(!foundation_mod.contains("pub mod aggregate_census"));
    assert!(!foundation_mod.contains("pub(crate) mod aggregate_census"));
    assert!(!foundation_mod.contains("pub(in "));
    assert!(!foundation_mod.contains("pub(crate) mod aggregate_census_stage11_seed"));
    assert!(installation_mod.contains("\nmod durable_finality;\n"));
    assert!(!installation_mod.contains("pub mod durable_finality"));
    assert!(!installation_mod.contains("pub(crate) mod durable_finality"));
    assert!(!installation_mod.contains("pub(in crate::domain) mod durable_finality_stage11_seed"));
    assert!(!installation_mod.contains("pub(in crate::domain) mod durable_finality_stage9_seed"));
    assert!(foundation_mod.contains("pub(crate) mod stage11_aggregate_census"));
    assert!(foundation_mod.contains("Stage11AggregateCensusProviderSeedV2"));
    assert!(foundation_mod.contains("Stage11AggregateCensusComponentV2"));
    assert!(foundation_mod.contains("bind_owner_provider_v2"));
    assert!(foundation_mod.contains("census_from_stage11_owner_v2"));
    assert!(foundation_mod.contains("Vec<InventoryRowV1>"));
    assert!(foundation_mod.contains("pub(crate) fn bind_owner_provider"));
    assert!(foundation_mod.contains("Stage11AggregateCensusProviderSeedV1"));
    assert!(foundation_mod.contains("pub(crate) fn census_from_stage11_owner"));
    assert!(foundation_mod.contains("pub(crate) fn into_parts"));
    assert!(!foundation_mod.contains("pub(crate) fn acquire_seed()"));
    assert!(!foundation_mod.contains("fn test_census_provider()"));
    assert!(!foundation_mod.contains("StoreV1"));
    assert!(!foundation_mod.contains("impl FnOnce"));
    assert!(!foundation_mod.contains("OnceLock"));
    assert!(installation_mod.contains("pub(in crate::domain) mod stage9_finality"));
    assert!(installation_mod.contains("pub(in crate::domain) mod stage11_finality"));
    assert!(installation_mod.contains("pub(in crate::domain) mod stage9_finality_v2"));
    assert!(installation_mod.contains("pub(in crate::domain) mod stage11_finality_v2"));
    assert!(installation_mod.contains("Stage9ActiveStoreFinalityProviderSeedV2"));
    assert!(installation_mod.contains("Stage11PreStoreFinalityProviderSeedV2"));
    assert!(persistence_mod.contains("pub(in crate::domain) mod protected_locator_v2"));
    assert!(persistence_mod.contains("pub(in crate::domain) fn capture_pre_candidate"));
    assert!(persistence_mod.contains("pub(in crate::domain) fn acquire_pre_candidate"));
    assert!(
        persistence_mod.contains("pub(in crate::domain) type Stage9ProtectedLocatorProviderSeedV2")
    );
    assert!(persistence.contains("pub(super) mod v2_owner_sealed"));
    assert!(
        persistence_stage9
            .contains("impl v2_owner_sealed::Sealed for Stage9ProtectedLocatorBackendSeedV2")
    );
    assert!(persistence_mod.contains("\nmod protected_locator_stage9_seed;\n"));
    assert!(!persistence_mod.contains("bind_stage9_owner_provider"));
    assert_eq!(
        installation_mod
            .matches("fn bind_finality_provider")
            .count(),
        2
    );
    assert!(!installation_mod.contains("fn acquire_finality_seed()"));
    assert!(!installation_mod.contains("fn test_finality_provider()"));
    assert!(installation_mod.contains("Stage9ActiveStoreFinalityProviderBindingV1<'effect>"));
    assert!(installation_mod.contains("Stage11PreStoreFinalityProviderBindingV1<'effect>"));
    assert!(installation_mod.contains("Stage9ActiveStoreFinalityProviderSeedV1"));
    assert!(installation_mod.contains("Stage11PreStoreFinalityProviderSeedV1"));
    assert!(!installation_mod.contains("StoreV1"));
    assert!(!installation_mod.contains("AtomicGenerationPublicationV1"));
    assert!(!installation_mod.contains("impl FnOnce"));
    assert!(!installation_mod.contains("OnceLock"));
    assert!(!installation_mod.contains(
        "#[derive(Clone)]\n    pub(in crate::domain) struct Stage9ActiveStoreFinalityProviderBindingV1"
    ));
    assert!(!installation_mod.contains(
        "#[derive(Clone)]\n    pub(in crate::domain) struct Stage11PreStoreFinalityProviderBindingV1"
    ));
    assert!(installation_mod.contains("pub(in crate::domain) fn prepare_active_from_stage9_owner"));
    assert!(
        installation_mod.contains("backend: Stage9ActiveStoreFinalityProviderBindingV1<'effect>,")
    );
    assert!(
        !installation_mod
            .contains("backend: &'borrow mut Stage9ActiveStoreFinalityProviderBindingV1")
    );
    assert!(
        installation_mod.contains("pub(in crate::domain) fn prepare_pre_store_from_stage11_owner")
    );
    assert!(
        installation_mod.contains("backend: Stage11PreStoreFinalityProviderBindingV1<'effect>,")
    );
    assert!(
        !installation_mod
            .contains("backend: &'borrow mut Stage11PreStoreFinalityProviderBindingV1")
    );
    assert!(vnext_module.contains("stage11_frozen_owner_seed_compile_probe"));
    assert!(vnext_module.contains("Stage11AggregateCensusProviderSeedV1::test_unavailable()"));
    assert!(vnext_module.contains("Stage11PreStoreFinalityProviderSeedV1::test_unavailable()"));
    assert!(
        vnext_module.contains("stage11_sibling_can_name_and_call_only_the_frozen_owner_entries")
    );
    assert!(vnext_module.contains("stage9_frozen_owner_seed_compile_probe"));
    assert!(vnext_module.contains("Stage9ActiveStoreFinalityProviderSeedV1::test_unavailable()"));
    assert!(vnext_module.contains("stage9_sibling_can_name_and_call_only_the_frozen_owner_entry"));

    let all_production_source = rust_files_under(Path::new("src"))
        .into_iter()
        .map(|path| read_source_file(&path))
        .collect::<Vec<_>>()
        .join("\n");
    for provider in [
        "Stage9ActiveStoreFinalityProviderBindingV1",
        "Stage11PreStoreFinalityProviderBindingV1",
        "Stage11AggregateCensusProviderBindingV1",
    ] {
        for forbidden_trait in ["Clone", "Copy", "Debug", "Serialize"] {
            assert!(
                !all_production_source.contains(&format!("{forbidden_trait} for {provider}")),
                "{provider} must not gain a manual {forbidden_trait} implementation"
            );
        }
    }

    let mut finality_owner_implementors = rust_files_under(Path::new("src"))
        .into_iter()
        .filter(|path| {
            let source = read_source_file(path)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            source.contains("ActiveStoreFinalityOwnerV1 for")
                || source.contains("PreStoreFinalityOwnerV1 for")
        })
        .collect::<Vec<_>>();
    finality_owner_implementors.sort();
    assert_eq!(
        finality_owner_implementors,
        [
            PathBuf::from("src/domain/installation/durable_finality.rs"),
            PathBuf::from("src/domain/installation/durable_finality_stage11_seed.rs"),
            PathBuf::from("src/domain/installation/durable_finality_stage9_seed.rs")
        ]
    );

    let mut aggregate_owner_implementors = rust_files_under(Path::new("src"))
        .into_iter()
        .filter(|path| {
            let source = read_source_file(path)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            source.contains("AggregateCensusBackendV1 for")
        })
        .collect::<Vec<_>>();
    aggregate_owner_implementors.sort();
    assert_eq!(
        aggregate_owner_implementors,
        [
            PathBuf::from("src/foundation/core/aggregate_census.rs"),
            PathBuf::from("src/foundation/core/aggregate_census_stage11_seed.rs")
        ]
    );
}

#[test]
fn stage11_v3_quarantine_route_is_current_owner_private_and_does_not_adapt_v2() {
    let installation_census = read_source_file(Path::new("src/domain/installation/census.rs"));
    let installation_admission =
        read_source_file(Path::new("src/domain/installation/legacy_quarantine.rs"));
    let migration_runtime =
        read_source_file(Path::new("src/domain/migration/runtime/live_set_v3.rs"));
    let migration_runtime_mod = read_source_file(Path::new("src/domain/migration/runtime/mod.rs"));
    let persistence = read_source_file(Path::new("src/domain/persistence/legacy_quarantine.rs"));
    let persistence_mod = read_source_file(Path::new("src/domain/persistence/mod.rs"));
    let repository = read_source_file(Path::new(
        "src/domain/repository/legacy_quarantine_admission.rs",
    ));
    let repository_mod = read_source_file(Path::new("src/domain/repository/mod.rs"));
    let foundation = read_source_file(Path::new("src/foundation/core/legacy_quarantine.rs"));
    let foundation_mod = read_source_file(Path::new("src/foundation/core/mod.rs"));
    let migration_operation =
        read_source_file(Path::new("src/operations/migration/live_set_v3.rs"));
    let migration_operation_mod = read_source_file(Path::new("src/operations/migration/mod.rs"));

    for required_route in [
        "pub(crate) fn from_live_owners(",
        "RepositoryRootAdmissionV3::mint_from_store",
        "installation_census.admit_legacy_quarantine_roots_v3()",
        "FoundationLegacyQuarantineLeaseV1::acquire(",
        "self.physical.copy_once(token)",
        "self.physical.rollback()",
        ".finish(self.quarantine.identity().into_bytes())",
        "Stage11PhysicalClosureV3::new(",
    ] {
        assert!(
            migration_operation.contains(required_route),
            "Stage 11 V3 route is missing {required_route}"
        );
    }
    assert!(
        installation_census.contains("#[path = \"legacy_quarantine.rs\"]\nmod legacy_quarantine;")
    );
    assert!(installation_admission.contains("pub(crate) struct InstallationRootAdmissionV3"));
    assert!(repository.contains("pub(crate) struct RepositoryRootAdmissionV3"));
    assert!(repository_mod.contains("\nmod legacy_quarantine_admission;\n"));
    assert!(repository_mod.contains("pub(crate) use legacy_quarantine_admission::{"));
    assert!(persistence_mod.contains("\npub(crate) mod legacy_quarantine;\n"));
    assert!(foundation_mod.contains("\npub(crate) mod legacy_quarantine;\n"));
    assert!(migration_runtime_mod.contains("\nmod live_set_v3;\n"));
    assert!(migration_runtime_mod.contains("pub use live_set_v3::{"));
    assert!(migration_operation_mod.contains("\nmod live_set_v3;\n"));
    assert!(migration_operation_mod.contains("pub(crate) use live_set_v3::{"));
    assert!(!repository_mod.contains("pub mod legacy_quarantine_admission"));
    assert!(!persistence_mod.contains("pub mod legacy_quarantine"));
    assert!(!foundation_mod.contains("pub mod legacy_quarantine"));
    assert!(!migration_runtime_mod.contains("pub mod live_set_v3"));
    assert!(!migration_operation_mod.contains("pub mod live_set_v3"));

    let v3_sources = [
        installation_admission.as_str(),
        migration_runtime.as_str(),
        persistence.as_str(),
        repository.as_str(),
        foundation.as_str(),
        migration_operation.as_str(),
    ];
    for historical_authority in [
        "LegacyQuarantineEpochV2",
        "AggregatePhysicalCensusV2",
        "Stage11CensusContinuationV2",
        "stage11_aggregate_census",
    ] {
        assert!(
            v3_sources
                .iter()
                .all(|source| !source.contains(historical_authority)),
            "current Stage 11 V3 code must not import or adapt {historical_authority}"
        );
    }
    for historical in [
        "src/foundation/core/aggregate_census.rs",
        "src/foundation/core/aggregate_census_stage11_seed.rs",
    ] {
        let source = read_source_file(Path::new(historical));
        assert!(source.contains("AggregateCensus"));
        assert!(!source.contains("FoundationLegacyQuarantineLeaseV1"));
        assert!(!source.contains("LegacyQuarantineEpochV3"));
    }

    for (move_only, source) in [
        ("FoundationLegacyQuarantineLeaseV1", foundation.as_str()),
        ("FoundationSourceCopyContinuationV1", foundation.as_str()),
        ("Stage11LiveSetContinuationV3", migration_operation.as_str()),
        (
            "Stage11SealedCopyContinuationV3",
            migration_operation.as_str(),
        ),
    ] {
        let declaration = format!("pub(crate) struct {move_only}");
        let declaration_offset = source
            .find(&declaration)
            .unwrap_or_else(|| panic!("missing move-only declaration {declaration}"));
        let attributes = source[..declaration_offset]
            .rsplit_once("\n\n")
            .map_or("", |(_, attributes)| attributes);
        for forbidden_trait in ["Clone", "Copy", "Serialize", "Deserialize"] {
            assert!(
                !attributes.contains(forbidden_trait),
                "{move_only} must not derive {forbidden_trait}"
            );
            assert!(
                v3_sources
                    .iter()
                    .all(|source| !source
                        .contains(&format!("impl {forbidden_trait} for {move_only}"))),
                "{move_only} must not gain {forbidden_trait}"
            );
        }
    }

    for interface in rust_files_under(Path::new("src/interfaces")) {
        let source = read_source_file(&interface);
        for forbidden_leaf in [
            "foundation::core::legacy_quarantine",
            "domain::persistence::legacy_quarantine",
            "domain::repository::legacy_quarantine_admission",
            "domain::installation::legacy_quarantine",
            "operations::migration::live_set_v3",
        ] {
            assert!(
                !source.contains(forbidden_leaf),
                "{} must not import Stage 11 leaf {forbidden_leaf}",
                interface.display()
            );
        }
    }
}

fn rust_files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path.to_path_buf());
        }
        return;
    }

    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to scan {}: {error}", path.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!("failed to read entry under {}: {error}", path.display())
        });
        collect_rust_files(&entry.path(), files);
    }
}

fn module_name_from_root(root: &str) -> &str {
    root.trim_start_matches("src/").trim_end_matches("/mod.rs")
}

fn read_source_file(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn resource_embed_is_allowed(file: &Path, line: &str) -> bool {
    RESOURCE_EMBED_ALLOWLIST
        .iter()
        .any(|(allowed_file, allowed_fragments)| {
            file == Path::new(allowed_file)
                && allowed_fragments
                    .iter()
                    .any(|fragment| line.contains(fragment))
        })
}

fn sorted_dir_entries(root: &Path) -> Vec<PathBuf> {
    let mut entries = fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to scan {}: {error}", root.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| {
                    panic!("failed to read entry under {}: {error}", root.display())
                })
                .path()
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn paths_under(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_paths(root, &mut paths);
    paths
}

fn collect_paths(path: &Path, paths: &mut Vec<PathBuf>) {
    paths.push(path.to_path_buf());
    if path.is_dir() {
        for entry in sorted_dir_entries(path) {
            collect_paths(&entry, paths);
        }
    }
}
