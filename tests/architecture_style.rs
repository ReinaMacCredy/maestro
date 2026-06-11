use std::fs;
use std::path::{Path, PathBuf};

const PRODUCTION_UNWRAP_ALLOWLIST: &[(&str, usize)] = &[];

#[test]
fn production_sources_do_not_call_unwrap() {
    let mut violations = Vec::new();
    for path in rust_files_under(Path::new("src")) {
        let source = read_source_file(&path);
        for (line_number, line) in production_lines(&source) {
            if line.contains(".unwrap()") && !is_allowlisted(&path, line_number) {
                violations.push(format!("{}:{line_number}: {line}", path.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production sources must not call .unwrap(); use Result propagation, \
         an invariant expect, or an infallible rendering pattern instead:\n{}",
        violations.join("\n")
    );
}

fn is_allowlisted(path: &Path, line: usize) -> bool {
    let path = path.to_string_lossy();
    PRODUCTION_UNWRAP_ALLOWLIST
        .iter()
        .any(|(allowed_path, allowed_line)| path == *allowed_path && line == *allowed_line)
}

fn production_lines(source: &str) -> Vec<(usize, String)> {
    let mut lines = Vec::new();
    let mut cfg_test_pending = false;
    let mut skipped_brace_depth: Option<usize> = None;

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;

        if let Some(depth) = skipped_brace_depth.as_mut() {
            *depth = next_brace_depth(*depth, line);
            if *depth == 0 {
                skipped_brace_depth = None;
            }
            continue;
        }

        let trimmed = line.trim_start();
        if is_cfg_test_attr(trimmed) {
            cfg_test_pending = true;
            continue;
        }

        if cfg_test_pending {
            if trimmed.starts_with("#[") {
                continue;
            }
            let depth = next_brace_depth(0, line);
            if depth > 0 {
                skipped_brace_depth = Some(depth);
            }
            cfg_test_pending = false;
            continue;
        }

        lines.push((line_number, line.to_string()));
    }

    lines
}

fn is_cfg_test_attr(trimmed: &str) -> bool {
    trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("#[cfg(any(test")
}

fn next_brace_depth(current: usize, line: &str) -> usize {
    let opens = line.chars().filter(|ch| *ch == '{').count();
    let closes = line.chars().filter(|ch| *ch == '}').count();
    current.saturating_add(opens).saturating_sub(closes)
}

fn rust_files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files);
    files.sort();
    files
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to scan {}: {error}", dir.display()))
    {
        let entry = entry.unwrap_or_else(|error| {
            panic!("failed to read entry under {}: {error}", dir.display())
        });
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn read_source_file(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
