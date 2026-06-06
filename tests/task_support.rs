use std::fs;
use std::path::{Path, PathBuf};

pub fn task_roots(repo: &Path) -> Vec<PathBuf> {
    let mut roots = vec![repo.join(".maestro/tasks")];
    let features_dir = repo.join(".maestro/features");
    if let Ok(features) = fs::read_dir(features_dir) {
        for feature in features {
            let feature = feature.expect("invariant: feature entry should be readable");
            roots.push(feature.path().join("tasks"));
        }
    }
    roots
}
