use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const TARGET_MODULE_ROOTS: &[&str] = &[
    "src/interfaces/mod.rs",
    "src/domain/mod.rs",
    "src/operations/mod.rs",
    "src/foundation/mod.rs",
];

const LEGACY_COMPATIBILITY_ROOTS: &[&str] = &[
    "commands",
    "core",
    "decisions",
    "evidence",
    "feature",
    "harness",
    "task",
    "hooks",
    "improver",
    "install",
    "mcp",
    "metrics",
    "migrate",
    "shell",
    "skills",
    "tui",
    "update",
    "verification",
];

const INTERFACE_COMPATIBILITY_REEXPORTS: &[(&str, &str)] = &[];

const INTERFACE_SCAN_ROOTS: &[&str] = &["src/interfaces"];
const PRODUCTION_SCAN_ROOTS: &[&str] = &["src"];
const CLI_TRANSITIONAL_LEGACY_IMPORTS: &[(&str, &[&str])] = &[
    ("src/interfaces/cli/decision.rs", &["decisions"]),
    ("src/interfaces/cli/doctor.rs", &["feature", "harness"]),
    ("src/interfaces/cli/feature.rs", &["feature"]),
    ("src/interfaces/cli/improve.rs", &["harness", "improver"]),
    ("src/interfaces/cli/init.rs", &["harness", "skills"]),
    ("src/interfaces/cli/install.rs", &["install"]),
    ("src/interfaces/cli/metrics.rs", &["metrics"]),
    ("src/interfaces/cli/migrate.rs", &["migrate"]),
    (
        "src/interfaces/cli/query.rs",
        &["decisions", "feature", "harness", "metrics"],
    ),
    ("src/interfaces/cli/task.rs", &[]),
    ("src/interfaces/cli/uninstall.rs", &["install"]),
    ("src/interfaces/cli/update.rs", &["update"]),
    ("src/interfaces/cli/watch.rs", &[]),
];

const MCP_TRANSITIONAL_LEGACY_IMPORTS: &[(&str, &[&str])] =
    &[("src/interfaces/mcp/tools.rs", &["metrics"])];

const HOOKS_TRANSITIONAL_LEGACY_IMPORTS: &[(&str, &[&str])] =
    &[("src/interfaces/hooks/record.rs", &["evidence"])];

const TUI_TRANSITIONAL_LEGACY_IMPORTS: &[(&str, &[&str])] =
    &[("src/interfaces/tui/task_list_watch.rs", &["feature"])];

const DOMAIN_FACADES: &[&str] = &[
    "decisions",
    "feature",
    "harness",
    "install",
    "proof",
    "skills",
    "task",
];

const OPERATION_FACADES: &[&str] = &["improver", "metrics", "migrate", "update"];

const RESOURCE_EMBED_ALLOWLIST: &[(&str, &[&str])] = &[
    (
        "src/domain/harness/templates.rs",
        &["resources/harness/HARNESS.md"],
    ),
    (
        "src/domain/skills/bundled.rs",
        &["resources/skills/bundled/"],
    ),
    ("src/interfaces/shell/mod.rs", &["resources/shell/"]),
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
    assert_eq!(
        maestro::foundation::core::schema::TASK_SCHEMA_VERSION,
        maestro::core::schema::TASK_SCHEMA_VERSION
    );
    assert_eq!(
        std::any::type_name::<maestro::foundation::core::paths::MaestroPaths>(),
        std::any::type_name::<maestro::core::paths::MaestroPaths>()
    );
    assert_eq!(
        std::any::type_name::<maestro::foundation::core::error::MaestroError>(),
        std::any::type_name::<maestro::core::error::MaestroError>()
    );
    assert_eq!(
        std::any::type_name::<maestro::foundation::core::git::GitSnapshot>(),
        std::any::type_name::<maestro::core::git::GitSnapshot>()
    );
    assert_eq!(
        std::any::type_name::<maestro::foundation::core::managed_blocks::ManagedBlockFormat>(),
        std::any::type_name::<maestro::core::managed_blocks::ManagedBlockFormat>()
    );
    assert_eq!(
        std::any::type_name::<maestro::domain::harness::schema::HarnessConfig>(),
        std::any::type_name::<maestro::harness::schema::HarnessConfig>()
    );
    assert_eq!(
        std::any::type_name::<maestro::domain::feature::schema::FeatureRecord>(),
        std::any::type_name::<maestro::feature::schema::FeatureRecord>()
    );
    assert_eq!(
        std::any::type_name::<maestro::domain::skills::bundled::BundledSkill>(),
        std::any::type_name::<maestro::skills::bundled::BundledSkill>()
    );
    let _legacy_decision_file_name: fn(u32, &str) -> String =
        maestro::decisions::template::decision_file_name;
    let _new_decision_file_name: fn(u32, &str) -> String =
        maestro::domain::decisions::template::decision_file_name;

    let _legacy_ensure_dir = |path: &Path| maestro::core::fs::ensure_dir(path);
    let _new_ensure_dir = |path: &Path| maestro::foundation::core::fs::ensure_dir(path);
    let _legacy_write_string_atomic = |path: &Path, contents: &str| {
        maestro::core::safe_write::write_string_atomic(path, contents)
    };
    let _new_write_string_atomic = |path: &Path, contents: &str| {
        maestro::foundation::core::safe_write::write_string_atomic(path, contents)
    };
    let _legacy_head = |path: &Path| maestro::core::git::head(path);
    let _new_head = |path: &Path| maestro::foundation::core::git::head(path);
    let _legacy_backup_file = |paths: &maestro::core::paths::MaestroPaths,
                               source: &Path,
                               operation: &str,
                               timestamp: &str| {
        maestro::core::backup::backup_file_with_timestamp(paths, source, operation, timestamp)
    };
    let _new_backup_file = |paths: &maestro::foundation::core::paths::MaestroPaths,
                            source: &Path,
                            operation: &str,
                            timestamp: &str| {
        maestro::foundation::core::backup::backup_file_with_timestamp(
            paths, source, operation, timestamp,
        )
    };

    let _ = std::any::type_name::<maestro::commands::Cli>();
    let _ = std::any::type_name::<maestro::interfaces::cli::Cli>();
    let _ = std::any::type_name::<maestro::task::template::TaskRecord>();
    let _legacy_load_task = |path: &Path| maestro::task::template::load_task(path);
    let _legacy_load_task_with_snapshot =
        |tasks_dir: &Path, id: &str| maestro::task::lookup::load_task_with_snapshot(tasks_dir, id);
    let _legacy_render_task: fn(&maestro::task::template::TaskRecord) -> String =
        maestro::task::display::render_task;
    let _legacy_render_task_list: fn(&[maestro::task::template::TaskRecord]) -> String =
        maestro::task::display::render_task_list;
    let _legacy_load_task_records =
        |tasks_dir: &Path| maestro::task::doctor::load_task_records(tasks_dir);
    let _legacy_load_task_entries =
        |tasks_dir: &Path| maestro::task::doctor::load_task_entries(tasks_dir);
    let _legacy_check_blocker_graph =
        |tasks_dir: &Path| maestro::task::doctor::check_blocker_graph(tasks_dir);
    let _legacy_render_task_doctor_report: fn(&maestro::task::doctor::TaskDoctorReport) -> String =
        maestro::task::doctor::render_report;
    let _legacy_resolve_task_yaml_path =
        |tasks_dir: &Path, id: &str| maestro::task::lookup::resolve_task_yaml_path(tasks_dir, id);
    let _legacy_task_yaml_path_for_entry =
        |entry: &std::fs::DirEntry| maestro::task::lookup::task_yaml_path_for_entry(entry);
    let _legacy_valid_task_yaml_path =
        |path: &Path| maestro::task::lookup::valid_task_yaml_path(path);
    let _ = std::any::type_name::<maestro::verification::proof_status::ProofStatusKind>();
    let _legacy_proof_status_fields =
        |status: &maestro::verification::proof_status::ProofStatus| {
            let _report: &Option<maestro::verification::verify_task::VerificationReport> =
                &status.report;
            let _stale_reasons: &Vec<maestro::verification::stale::StaleReason> =
                &status.stale_reasons;
            let _task_id: &String = &status.task_id;
            let _path: &String = &status.verification_path;
            let _kind: &maestro::verification::proof_status::ProofStatusKind = &status.kind;
        };
    let _legacy_proof_status: fn(
        &maestro::foundation::core::paths::MaestroPaths,
        &str,
    ) -> anyhow::Result<
        maestro::verification::proof_status::ProofStatus,
    > = maestro::verification::proof_status::proof_status;
    let _legacy_render_proof_status: fn(
        &maestro::verification::proof_status::ProofStatus,
    ) -> String = maestro::verification::proof_status::render_proof_status;
    let _legacy_loaded_task_fields = |loaded: &maestro::verification::verify_task::LoadedTask| {
        let _task: &maestro::domain::task::TaskRecord = &loaded.task;
        let _task_dir: &PathBuf = &loaded.task_dir;
    };
    let _legacy_verify_task: fn(
        &maestro::foundation::core::paths::MaestroPaths,
        &str,
        &str,
    ) -> anyhow::Result<
        maestro::verification::verify_task::VerificationReport,
    > = maestro::verification::verify_task::verify_task;
    let _ = std::any::type_name::<maestro::domain::task::TaskRecord>();
    let _ = std::any::type_name::<maestro::domain::proof::ProofStatusKind>();
    let _ = std::any::type_name::<maestro::operations::metrics::summary::MetricsSummary>();
    let _ = std::any::type_name::<maestro::operations::update::InstallMethod>();
    assert_eq!(
        std::any::type_name::<maestro::interfaces::shell::Shell>(),
        std::any::type_name::<maestro::shell::Shell>()
    );
    let legacy_render_shell_init: fn(maestro::shell::Shell) -> &'static str =
        maestro::shell::render_shell_init;
    let new_render_shell_init: fn(maestro::interfaces::shell::Shell) -> &'static str =
        maestro::interfaces::shell::render_shell_init;
    assert_eq!(
        new_render_shell_init(maestro::interfaces::shell::Shell::Bash),
        legacy_render_shell_init(maestro::shell::Shell::Bash)
    );
    let _legacy_detect_shell: fn() -> maestro::shell::Shell = maestro::shell::Shell::detect;
    let _new_detect_shell: fn() -> maestro::interfaces::shell::Shell =
        maestro::interfaces::shell::Shell::detect;

    let _legacy_task_watch_render: fn(
        &maestro::foundation::core::paths::MaestroPaths,
        &[maestro::task::template::TaskRecord],
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
        maestro::interfaces::hooks::event::SHARED_HOOK_EVENTS,
        maestro::hooks::event::SHARED_HOOK_EVENTS
    );
    let legacy_run_dir_name: fn(&str) -> String = maestro::hooks::event::run_dir_name;
    let new_run_dir_name: fn(&str) -> String = maestro::interfaces::hooks::event::run_dir_name;
    assert_eq!(
        new_run_dir_name("agent/session"),
        legacy_run_dir_name("agent/session")
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

    let legacy_shim = read_source_file(Path::new("src/task/mod.rs"));
    assert!(
        !legacy_shim.contains("pub use crate::domain::task::*"),
        "legacy crate::task shim should explicitly re-export the compatibility surface"
    );
    assert!(
        !legacy_shim.contains("pub fn append_history"),
        "legacy crate::task shim should not grow lifecycle helpers outside the old leaf surface"
    );
}

#[test]
fn proof_domain_facade_does_not_publish_leaf_modules() {
    let proof_facade = read_source_file(Path::new("src/domain/proof/mod.rs"));
    let proof_root_facade = proof_facade
        .split("pub(crate) mod compatibility")
        .next()
        .expect("invariant: split always yields root facade prefix");
    assert_eq!(
        public_reexport_item_names(proof_root_facade),
        BTreeSet::from([
            "ProofStatus".to_string(),
            "ProofStatusKind".to_string(),
            "ProofStatusSource".to_string(),
            "ProofStaleReason".to_string(),
            "TaskVerification".to_string(),
            "TaskVerificationStatus".to_string(),
            "latest_proof_failed_for_task".to_string(),
            "managed_event_files".to_string(),
            "proof_status".to_string(),
            "proof_status_for_task".to_string(),
            "proof_status_kind_for_task".to_string(),
            "render_proof_status".to_string(),
            "verify_task".to_string(),
        ]),
        "src/domain/proof/mod.rs should expose only the deliberate Proof facade surface"
    );
    for leaf in ["events", "proof_status", "stale", "verify_task"] {
        assert!(
            !proof_facade.contains(&format!("pub mod {leaf};")),
            "src/domain/proof/mod.rs should expose Proof through root facade exports, not pub mod {leaf}"
        );
    }

    let verification_shim = read_source_file(Path::new("src/verification/mod.rs"));
    for leaf in ["events", "proof_status", "stale", "verify_task"] {
        assert!(
            verification_shim.contains(&format!("pub mod {leaf}")),
            "legacy crate::verification shim should preserve compatibility module {leaf}"
        );
    }
    let legacy_proof_status_module = verification_shim
        .split("pub mod proof_status {")
        .nth(1)
        .and_then(|body| body.split("\npub mod stale {").next())
        .expect("legacy crate::verification shim should contain proof_status before stale");
    for duplicated_domain_logic in [
        "read_report",
        "freshness_inputs",
        "stale_reasons(",
        "VerificationStatus::",
        "format_claims",
        "format_sources",
    ] {
        assert!(
            !legacy_proof_status_module.contains(duplicated_domain_logic),
            "legacy crate::verification proof_status shim should adapt domain::proof compatibility helpers, not duplicate Proof logic: {duplicated_domain_logic}"
        );
    }
    assert!(
        !verification_shim.contains("mod events;")
            && !verification_shim.contains("mod proof_status;")
            && !verification_shim.contains("mod stale;")
            && !verification_shim.contains("mod verify_task;"),
        "legacy crate::verification shim should not own Proof implementation files"
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
fn transitional_public_surfaces_match_phase_policy() {
    assert_public_modules(
        Path::new("src/domain/mod.rs"),
        &["decisions", "feature", "harness", "proof", "skills", "task"],
        &["crate::install"],
    );
    assert_public_modules(Path::new("src/foundation/mod.rs"), &["core"], &[]);
    assert_public_modules(
        Path::new("src/interfaces/mod.rs"),
        &["cli", "hooks", "mcp", "shell", "tui"],
        &[],
    );
    assert_reexports(
        Path::new("src/operations/mod.rs"),
        &[
            "crate::improver",
            "crate::metrics",
            "crate::migrate",
            "crate::update",
        ],
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
        "commands" => {
            line == "pub use interfaces::cli as commands;"
                || line == "pub use crate::interfaces::cli as commands;"
        }
        "core" => line == "pub use foundation::core;" || line == "pub use crate::foundation::core;",
        "shell" => {
            line == "pub use interfaces::shell;" || line == "pub use crate::interfaces::shell;"
        }
        "mcp" => line == "pub use interfaces::mcp;" || line == "pub use crate::interfaces::mcp;",
        "hooks" => {
            line == "pub use interfaces::hooks;" || line == "pub use crate::interfaces::hooks;"
        }
        "tui" => line == "pub use interfaces::tui;" || line == "pub use crate::interfaces::tui;",
        "decisions" => {
            line == "pub use domain::decisions;" || line == "pub use crate::domain::decisions;"
        }
        "feature" => {
            line == "pub use domain::feature;" || line == "pub use crate::domain::feature;"
        }
        "harness" => {
            line == "pub use domain::harness;" || line == "pub use crate::domain::harness;"
        }
        "skills" => line == "pub use domain::skills;" || line == "pub use crate::domain::skills;",
        _ => false,
    }
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
                if is_allowed_compatibility_reexport(&file, &import_statement)
                    || is_allowed_transitional_interface_import(&file, &import_statement)
                {
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
        "non-interface code should keep using crate::hooks until Run exposes a non-interface facade:\n{}",
        violations.join("\n")
    );
}

#[test]
fn resource_embeds_stay_in_owning_modules() {
    let mut violations = Vec::new();

    for file in rust_files_under(Path::new("src")) {
        let source = read_source_file(&file);
        for (line_number, line) in source.lines().enumerate() {
            if !line.contains("include_str!") || !line.contains("resources/") {
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
    let root = Path::new("resources/skills/bundled");
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
        .chain(MCP_TRANSITIONAL_LEGACY_IMPORTS.iter())
        .chain(HOOKS_TRANSITIONAL_LEGACY_IMPORTS.iter())
        .chain(TUI_TRANSITIONAL_LEGACY_IMPORTS.iter())
        .find_map(|(allowed_file, roots)| (file == Path::new(allowed_file)).then_some(*roots))
}

fn assert_reexports(path: &Path, expected_roots: &[&str]) {
    let source = read_source_file(path);
    let actual = crate_reexports(&source);
    let expected = expected_roots
        .iter()
        .map(|root| root.to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        actual,
        expected,
        "{} must expose exactly the current transitional re-export set",
        path.display()
    );

    assert_no_public_module_items(path, &source);
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
            line.strip_prefix("pub mod ")
                .and_then(|module| module.strip_suffix(';'))
                .map(str::to_string)
        })
        .collect()
}

fn assert_no_public_module_items(path: &Path, source: &str) {
    let has_public_module = source.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("pub mod ")
            || line.starts_with("pub(crate) mod ")
            || line.starts_with("pub(super) mod ")
            || line.starts_with("pub(crate) use ")
            || line.starts_with("pub(super) use ")
            || line.starts_with("pub(in ")
    });

    assert!(
        !has_public_module,
        "{} must expose only the current transitional re-exports",
        path.display()
    );
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

fn source_without_pub_mod_statements(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("pub mod "))
        .collect::<Vec<_>>()
        .join("\n")
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

fn is_allowed_compatibility_reexport(file: &Path, line: &str) -> bool {
    INTERFACE_COMPATIBILITY_REEXPORTS
        .iter()
        .any(|(allowed_file, root)| {
            file == Path::new(allowed_file) && import_line_reexports_root_only(line, root)
        })
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
        protected_interface_import("use crate::task as legacy_task;"),
        Some("legacy crate::task root import".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::{task as legacy_task};"),
        Some("legacy crate::task root import".to_string())
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
        protected_interface_import("use super::super::task::template::TaskRecord;"),
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
            "fn render() { super::super::task::template::render_task_body(); }"
        ),
        Some("relative deep path under src/interfaces".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::operations::metrics::summary::MetricsSummary;"),
        Some("operations::metrics::summary".to_string())
    );
    assert_eq!(
        protected_interface_import("use crate::operations::metrics::{self as metrics_ops};"),
        Some("operations::metrics root alias".to_string())
    );
    assert_eq!(
        protected_interface_path_reference(
            "fn render() { crate::operations::metrics::summary::summarize(); }"
        ),
        Some("operations::metrics::summary".to_string())
    );
    assert_eq!(
        protected_interface_path_reference("fn run() { feature::run(args); }"),
        None
    );
    assert_eq!(
        protected_interface_path_reference("fn run() { crate::feature::query::load(); }"),
        Some("legacy crate::feature::".to_string())
    );
    assert_eq!(
        protected_interface_path_reference(
            "fn render() { super::super::super::super::task::template::render_task_body(); }"
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
    assert!(!is_allowed_compatibility_reexport(
        Path::new("src/interfaces/mod.rs"),
        "pub use crate::hooks;"
    ));
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
