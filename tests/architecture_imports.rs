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

const INTERFACE_COMPATIBILITY_REEXPORTS: &[(&str, &str)] = &[
    ("src/interfaces/mod.rs", "crate::hooks"),
    ("src/interfaces/mod.rs", "crate::mcp"),
    ("src/interfaces/mod.rs", "crate::tui"),
];

const INTERFACE_SCAN_ROOTS: &[&str] = &["src/interfaces"];
const PRODUCTION_SCAN_ROOTS: &[&str] = &["src"];
const CLI_TRANSITIONAL_LEGACY_IMPORTS: &[(&str, &[&str])] = &[
    ("src/interfaces/cli/decision.rs", &["decisions"]),
    (
        "src/interfaces/cli/doctor.rs",
        &["feature", "harness", "task"],
    ),
    ("src/interfaces/cli/event.rs", &["task"]),
    ("src/interfaces/cli/feature.rs", &["feature"]),
    ("src/interfaces/cli/hook.rs", &["hooks"]),
    ("src/interfaces/cli/improve.rs", &["harness", "improver"]),
    ("src/interfaces/cli/init.rs", &["harness", "skills"]),
    ("src/interfaces/cli/install.rs", &["install"]),
    ("src/interfaces/cli/mcp.rs", &["mcp"]),
    ("src/interfaces/cli/metrics.rs", &["metrics"]),
    ("src/interfaces/cli/migrate.rs", &["migrate"]),
    (
        "src/interfaces/cli/query.rs",
        &[
            "decisions",
            "feature",
            "harness",
            "metrics",
            "task",
            "verification",
        ],
    ),
    (
        "src/interfaces/cli/task.rs",
        &["task", "tui", "verification"],
    ),
    ("src/interfaces/cli/uninstall.rs", &["install"]),
    ("src/interfaces/cli/update.rs", &["update"]),
    ("src/interfaces/cli/watch.rs", &["task", "tui"]),
];

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
    let _ = std::any::type_name::<maestro::verification::proof_status::ProofStatusKind>();
    let _ = std::any::type_name::<maestro::domain::task::template::TaskRecord>();
    let _ = std::any::type_name::<maestro::domain::proof::proof_status::ProofStatusKind>();
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
}

#[test]
fn transitional_public_surfaces_match_phase_policy() {
    assert_reexports(
        Path::new("src/domain/mod.rs"),
        &[
            "crate::decisions",
            "crate::feature",
            "crate::harness",
            "crate::install",
            "crate::skills",
            "crate::task",
            "crate::verification as proof",
        ],
    );
    assert_public_modules(Path::new("src/foundation/mod.rs"), &["core"], &[]);
    assert_public_modules(
        Path::new("src/interfaces/mod.rs"),
        &["cli", "shell"],
        &["crate::hooks", "crate::mcp", "crate::tui"],
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
        "interface code must call facades directly except explicit CLI transitional allowances:\n{}",
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
    cli_transitional_legacy_roots(file).is_some_and(|roots| {
        roots.iter().any(|root| {
            contains_legacy_root_import(line, root) || contains_legacy_deep_import(line, root)
        })
    })
}

fn is_allowed_transitional_interface_path_reference(file: &Path, path: &str) -> bool {
    cli_transitional_legacy_roots(file).is_some_and(|roots| {
        roots
            .iter()
            .any(|root| path == format!("legacy crate::{root}::"))
    })
}

fn cli_transitional_legacy_roots(file: &Path) -> Option<&'static [&'static str]> {
    CLI_TRANSITIONAL_LEGACY_IMPORTS
        .iter()
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

    source.match_indices("super::").any(|(index, _)| {
        let mut tail = &source[index..];
        while let Some(rest) = tail.strip_prefix("super::") {
            tail = rest;
        }
        tail.starts_with(&format!("{root}::"))
    })
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
    assert!(is_allowed_compatibility_reexport(
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
