use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use anyhow::{Result, anyhow};

use crate::domain::search::types::SearchDiagnostic;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;

pub struct SearchWriterLock {
    path: PathBuf,
}

const STALE_LOCK_AGE: Duration = Duration::from_secs(30 * 60);

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
    match try_create_lock(&path) {
        Ok(lock) => Ok(lock),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => match remove_stale_lock(&path) {
            Ok(true) => try_create_lock(&path).map_err(lock_unavailable_diagnostic),
            Ok(false) => Err(lock_contention_diagnostic()),
            Err(error) => Err(lock_unavailable_diagnostic(error)),
        },
        Err(error) => Err(lock_unavailable_diagnostic(error)),
    }
}

fn try_create_lock(path: &Path) -> std::io::Result<SearchWriterLock> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            let _ = writeln!(file, "pid={}", process::id());
            Ok(SearchWriterLock {
                path: path.to_path_buf(),
            })
        }
        Err(error) => Err(error),
    }
}

fn lock_unavailable_diagnostic(error: std::io::Error) -> SearchDiagnostic {
    SearchDiagnostic::error(
        "search_index_lock_unavailable",
        format!("failed to create search index writer lock: {error}"),
    )
    .with_path(".maestro/index/search/write.lock")
    .with_retryable(true)
}

fn remove_stale_lock(path: &Path) -> std::io::Result<bool> {
    if !lock_is_stale(path) {
        return Ok(false);
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

fn lock_is_stale(path: &Path) -> bool {
    if let Ok(contents) = std::fs::read_to_string(path)
        && let Some(pid) = parse_lock_pid(&contents)
        && let Some(running) = pid_is_running(pid)
    {
        return !running;
    }
    std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age >= STALE_LOCK_AGE)
}

fn parse_lock_pid(contents: &str) -> Option<u32> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("pid="))
        .and_then(|raw| raw.trim().parse().ok())
}

#[cfg(unix)]
fn pid_is_running(pid: u32) -> Option<bool> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Some(false);
    }
    const EPERM: i32 = 1;
    const ESRCH: i32 = 3;
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    let result = unsafe { kill(pid as i32, 0) };
    if result == 0 {
        return Some(true);
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(ESRCH) => Some(false),
        Some(EPERM) => Some(true),
        _ => None,
    }
}

#[cfg(not(unix))]
fn pid_is_running(_pid: u32) -> Option<bool> {
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "maestro-{name}-{}-{}",
            process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("invariant: temp root should be creatable");
        root
    }

    #[cfg(unix)]
    #[test]
    fn stale_dead_pid_lock_is_replaced() {
        let root = temp_root("search-lock-stale");
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.search_index_dir()).expect("invariant: search dir should be creatable");
        let mut child = std::process::Command::new("sh")
            .arg("-c")
            .arg("exit 0")
            .spawn()
            .expect("invariant: shell should spawn");
        let dead_pid = child.id();
        child.wait().expect("invariant: shell should exit");
        fs::write(paths.search_writer_lock_file(), format!("pid={dead_pid}\n"))
            .expect("invariant: stale lock should be writable");

        let guard = try_acquire_writer(&paths).expect("dead pid lock should be replaced");
        let contents = fs::read_to_string(paths.search_writer_lock_file())
            .expect("invariant: live lock should be readable");
        assert!(contents.contains(&format!("pid={}", process::id())));
        drop(guard);
        assert!(!paths.search_writer_lock_file().exists());
        fs::remove_dir_all(root).expect("invariant: temp root should clean up");
    }
}
