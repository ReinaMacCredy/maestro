use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::process;

use anyhow::{Result, anyhow};

use crate::domain::search::types::SearchDiagnostic;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;

pub struct SearchWriterLock {
    path: PathBuf,
}

pub fn try_acquire_writer(paths: &MaestroPaths) -> Result<SearchWriterLock, SearchDiagnostic> {
    ensure_dir(paths.search_index_dir()).map_err(|error| {
        SearchDiagnostic::error(
            "search_index_lock_unavailable",
            format!("failed to prepare search index lock directory: {error}"),
        )
        .with_path(".maestro/index/search")
        .with_retryable(true)
    })?;

    let path = paths.search_writer_lock_file();
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            let _ = writeln!(file, "pid={}", process::id());
            Ok(SearchWriterLock { path })
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Err(lock_contention_diagnostic()),
        Err(error) => Err(SearchDiagnostic::error(
            "search_index_lock_unavailable",
            format!("failed to create search index writer lock: {error}"),
        )
        .with_path(".maestro/index/search/write.lock")
        .with_retryable(true)),
    }
}

pub fn acquire_writer(paths: &MaestroPaths) -> Result<SearchWriterLock> {
    try_acquire_writer(paths)
        .map_err(|diagnostic| anyhow!("{}: {}", diagnostic.code, diagnostic.message))
}

pub fn lock_contention_diagnostic() -> SearchDiagnostic {
    SearchDiagnostic::error(
        "search_index_locked",
        "search index writer lock is held; retry after the current repair or rebuild finishes",
    )
    .with_path(".maestro/index/search/write.lock")
    .with_retryable(true)
}

pub fn is_lock_contention(envelope: &crate::domain::search::types::GrepEnvelope) -> bool {
    envelope
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "search_index_locked")
}

impl Drop for SearchWriterLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
