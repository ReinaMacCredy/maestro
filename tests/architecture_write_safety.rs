use std::fs;
use std::path::Path;

const CARD_STORE_CAS_BYPASS_ALLOWLIST: &[(&str, &[usize])] = &[];

#[test]
fn card_store_mutations_route_through_snapshot_cas() {
    assert!(
        CARD_STORE_CAS_BYPASS_ALLOWLIST.is_empty(),
        "the card-store CAS bypass allowlist must start empty"
    );

    let store = read_source_file(Path::new("src/domain/card/store.rs"));
    let production = production_source(&store);
    let bypasses = cas_bypass_lines(production);
    assert!(
        bypasses.is_empty(),
        "card-store production code must not bypass snapshot/CAS helpers:\n{}",
        bypasses.join("\n")
    );

    assert!(
        section(production, "pub fn save_with_snapshot").contains("write_string_if_unchanged("),
        "dir-backed card saves must write through write_string_if_unchanged"
    );
    assert!(
        section(production, "pub fn save_entries").contains("write_string_if_unchanged("),
        "entry-backed card saves must write through write_string_if_unchanged"
    );
    let remove_resolved = section(production, "pub fn remove_resolved");
    assert!(
        remove_resolved.contains("remove_dir_if_file_unchanged("),
        "dir-backed card deletion must compare the read snapshot before removal"
    );
    assert!(
        remove_resolved.contains("save_entries("),
        "entry-backed card deletion must rewrite the entry file through whole-file CAS"
    );
}

fn cas_bypass_lines(source: &str) -> Vec<String> {
    let forbidden = [
        "write_string_atomic(",
        "write_atomic(",
        "fs::write(",
        "std::fs::write(",
        "fs::remove_dir_all(",
        "std::fs::remove_dir_all(",
    ];
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if forbidden.iter().any(|needle| trimmed.contains(needle)) {
                Some(format!("{}: {}", index + 1, trimmed))
            } else {
                None
            }
        })
        .collect()
}

fn production_source(source: &str) -> &str {
    source.split("\n#[cfg(test)]").next().unwrap_or(source)
}

fn section<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing section signature: {signature}"));
    let rest = &source[start..];
    let end = rest
        .find("\n}\n\n")
        .map(|index| index + 3)
        .unwrap_or(rest.len());
    &rest[..end]
}

fn read_source_file(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
