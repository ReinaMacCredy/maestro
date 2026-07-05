use std::fs;
use std::path::{Path, PathBuf};

const NATIVE_LAYER_SOURCES: &[&str] = &[
    "src/domain/intake.rs",
    "src/domain/capability.rs",
    "src/domain/maturity.rs",
    "src/interfaces/cli/intake.rs",
    "src/interfaces/cli/capability.rs",
    "src/interfaces/cli/maturity.rs",
    "src/domain/install/mod.rs",
    "src/domain/install/mirrors.rs",
];

#[test]
fn architecture_records_native_layer_as_existing_artifact_read_models() {
    let architecture = normalize_markdown(&read_source_file(Path::new("ARCHITECTURE.md")));
    for phrase in [
        "Native repository-harness layer",
        "read-model and router layer over existing Maestro artifacts",
        "`maestro intake` routes into Feature, Card, Task, and Loop gates",
        "`maestro capability` reports optional provider evidence and never grants permission",
        "`maestro maturity` joins Feature acceptance/proof, Harness backlog, and UX_GAPS",
        "no repository-harness docs tree, harness.db, daemon, scheduler, connector broker, or hidden authority store",
    ] {
        assert!(
            architecture.contains(phrase),
            "ARCHITECTURE.md must record native-layer authority boundary phrase {phrase:?}"
        );
    }
}

#[test]
fn native_layer_sources_do_not_own_hidden_authority_mechanisms() {
    for path in NATIVE_LAYER_SOURCES {
        let raw = read_source_file(Path::new(path));
        let source = production_source(&raw);
        for forbidden in [
            "harness.db",
            "repository-harness/",
            "rusqlite",
            "std::process::Command",
            "Command::new",
            "TcpListener",
            "tokio::spawn",
            "thread::spawn",
            "fs::write(",
            "std::fs::write(",
            "File::create(",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path} must not introduce hidden authority mechanism {forbidden:?}"
            );
        }
    }
}

#[test]
fn shipped_tree_does_not_include_repository_harness_authority_artifacts() {
    let mut violations = Vec::new();
    for root in ["src", "embedded"] {
        for path in files_under(Path::new(root)) {
            let path_text = path.display().to_string();
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if file_name == "harness.db"
                || path_text.contains("repository-harness/")
                || path_text.contains("repo-harness/")
            {
                violations.push(path_text);
            }
        }
    }
    assert!(
        violations.is_empty(),
        "native layer must not ship copied repository-harness authority artifacts:\n{}",
        violations.join("\n")
    );
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, &mut files);
    files
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
    {
        let entry = entry.expect("invariant: directory entry should be readable");
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .unwrap_or_else(|error| panic!("failed to stat {}: {error}", path.display()));
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            collect_files(&path, files);
        } else if metadata.is_file() {
            files.push(path);
        }
    }
}

fn production_source(source: &str) -> &str {
    source.split("\n#[cfg(test)]").next().unwrap_or(source)
}

fn read_source_file(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn normalize_markdown(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}
