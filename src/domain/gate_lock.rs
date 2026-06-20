//! Cross-session serialization of the full-suite ship gate (decision
//! `dec-heavy-run-serialization-lock-only-6a17`).
//!
//! Two maestro sessions that each launch the repo-global `stack.verify` suite
//! would run two heavy test/build jobs at once and lag the user's machine. The
//! gate lock makes that suite one-at-a-time: a second run waits on an advisory
//! `flock` over a single lockfile in the Git common dir -- shared across every
//! worktree -- then proceeds the moment the holder finishes. The lock lives on an
//! open file descriptor, so the kernel releases it if the holder crashes; there
//! is deliberately no heartbeat and no stale-reclaim path. The holder writes its
//! session id into the file so a waiting run, `maestro active`, and the
//! pre-command banner can name who is busy.
//!
//! Only the ship-gate full suite is serialized. The per-task narrow falsifier is
//! not -- concurrent slice verifications must stay independent.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::PathBuf;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
use anyhow::Context;
use anyhow::Result;

use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;

/// Name of the advisory lockfile placed in the Git common dir (or, with no Git,
/// under `.maestro/`).
const LOCK_FILE_NAME: &str = "maestro-gate.lock";

/// How often the wait loop wakes to retry the lock. Decoupled from the print
/// cadence: a tight poll keeps the wait Ctrl-C-responsive without flooding the
/// (agent-consumed) output with a line per second.
#[cfg(unix)]
const POLL: Duration = Duration::from_secs(1);

/// How often, while waiting, to print a coarse elapsed update.
#[cfg(unix)]
const REPORT_EVERY_SECS: u64 = 30;

#[cfg(unix)]
mod sys {
    pub const LOCK_SH: i32 = 1;
    pub const LOCK_EX: i32 = 2;
    pub const LOCK_NB: i32 = 4;
    pub const LOCK_UN: i32 = 8;

    unsafe extern "C" {
        pub fn flock(fd: i32, operation: i32) -> i32;
    }
}

/// Held for the lifetime of one serialized gate run. Dropping it closes the
/// descriptor, which is what releases the advisory lock. `None` means the run is
/// proceeding unserialized (non-Unix, or the lockfile could not be opened) -- a
/// best-effort degrade so a transient IO problem never blocks a legitimate ship.
pub struct GateGuard {
    _file: Option<File>,
}

/// Take the gate lock, waiting for any current holder to finish. Best-effort and
/// infallible: on a non-Unix target or an IO failure it returns an unserialized
/// guard so the suite still runs. The wait announces itself once, then prints a
/// coarse elapsed line every [`REPORT_EVERY_SECS`]; a Ctrl-C during the wait
/// terminates the process through the default SIGINT handler (no custom handler).
pub fn acquire(paths: &MaestroPaths, holder: &str) -> GateGuard {
    #[cfg(unix)]
    {
        // Resolve the lockfile once: the wait loop below polls for the holder's
        // whole (possibly multi-minute) suite, and the path never changes.
        let path = lock_path(paths);
        match try_acquire_at(&path, holder) {
            Ok(Some(guard)) => return guard,
            Ok(None) => {}
            Err(error) => return unserialized(error),
        }

        let waiting_on = read_holder(&path).unwrap_or_else(|| "another session".to_string());
        eprintln!("{}", waiting_line(&waiting_on));
        let started = Instant::now();
        let mut last_report = 0u64;
        loop {
            std::thread::sleep(POLL);
            match try_acquire_at(&path, holder) {
                Ok(Some(guard)) => return guard,
                Ok(None) => {}
                Err(error) => return unserialized(error),
            }
            let elapsed = started.elapsed().as_secs();
            if elapsed - last_report >= REPORT_EVERY_SECS {
                last_report = elapsed;
                eprintln!("{}", still_waiting_line(&waiting_on, elapsed));
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (paths, holder);
        GateGuard { _file: None }
    }
}

/// The session id currently holding the gate, or `None` if it is free. A
/// `LOCK_SH | LOCK_NB` probe: a granted shared lock means no exclusive holder, so
/// the gate is free (the probe releases and reports `None`); `WouldBlock` means a
/// holder is running the suite, so its id is read back. Shared probes do not
/// conflict with each other, so two concurrent probes never read each other as
/// busy. An empty read in the brief window between acquiring the lock and writing
/// the id reports a generic name rather than a blank one.
pub fn holder(paths: &MaestroPaths) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;

        let path = lock_path(paths);
        let file = File::open(&path).ok()?;
        match flock_nb(file.as_raw_fd(), sys::LOCK_SH) {
            Ok(true) => {
                unsafe { sys::flock(file.as_raw_fd(), sys::LOCK_UN) };
                None
            }
            Ok(false) => Some(read_holder(&path).unwrap_or_else(|| "another session".to_string())),
            Err(_) => None,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = paths;
        None
    }
}

#[cfg(unix)]
fn unserialized(error: anyhow::Error) -> GateGuard {
    eprintln!(
        "[busy] gate lock unavailable ({error}); running without cross-session serialization"
    );
    GateGuard { _file: None }
}

/// One non-blocking attempt to take the gate exclusively at a known lockfile
/// path: `Some(guard)` if free, `None` if a peer holds it. The building block
/// behind [`acquire`]'s wait loop, which resolves the path once and reuses it so
/// a multi-minute wait never re-discovers the Git common dir on every poll.
#[cfg(unix)]
pub(crate) fn try_acquire_at(path: &std::path::Path, holder: &str) -> Result<Option<GateGuard>> {
    use std::os::unix::io::AsRawFd;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        // Never truncate on open: the holder's id must survive until WE own the
        // lock and rewrite it under set_len(0). Truncating here would blank a
        // peer's id the instant we open, before acquiring.
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open gate lock {}", path.display()))?;
    if flock_nb(file.as_raw_fd(), sys::LOCK_EX)? {
        write_holder(&file, holder)?;
        Ok(Some(GateGuard { _file: Some(file) }))
    } else {
        Ok(None)
    }
}

/// `flock(fd, op | LOCK_NB)`: `Ok(true)` acquired, `Ok(false)` would block (a peer
/// holds a conflicting lock), `Err` on any other failure.
#[cfg(unix)]
fn flock_nb(fd: i32, op: i32) -> std::io::Result<bool> {
    let ret = unsafe { sys::flock(fd, op | sys::LOCK_NB) };
    if ret == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    // EAGAIN / EWOULDBLOCK both map to WouldBlock on macOS and Linux, so this
    // stays portable without a libc dependency.
    if error.kind() == std::io::ErrorKind::WouldBlock {
        Ok(false)
    } else {
        Err(error)
    }
}

#[cfg(unix)]
fn write_holder(file: &File, holder: &str) -> Result<()> {
    use std::io::Write;

    file.set_len(0)
        .context("failed to truncate gate lock before writing holder")?;
    let mut writer = file;
    writer
        .write_all(holder.as_bytes())
        .context("failed to write gate lock holder")?;
    writer.flush().ok();
    Ok(())
}

fn read_holder(path: &std::path::Path) -> Option<String> {
    let mut contents = String::new();
    File::open(path).ok()?.read_to_string(&mut contents).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// The lockfile path: the Git common dir when in a repo (shared across every
/// worktree), else `.maestro/` for a non-Git tree.
fn lock_path(paths: &MaestroPaths) -> PathBuf {
    match git::common_dir(paths.repo_root()) {
        Ok(common) => common.join(LOCK_FILE_NAME),
        Err(_) => paths.maestro_dir().join(LOCK_FILE_NAME),
    }
}

#[cfg(unix)]
fn waiting_line(holder: &str) -> String {
    format!("[busy] waiting for {holder}'s full-suite gate to finish (Ctrl-C to abort)")
}

#[cfg(unix)]
fn still_waiting_line(holder: &str, elapsed: u64) -> String {
    format!("[busy] still waiting ({elapsed}s) for {holder}'s gate")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::io::AsRawFd;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new(label: &str) -> Self {
            let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "maestro-gate-{label}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(path.join(".maestro")).expect("temp .maestro dir is creatable");
            Self { path }
        }

        fn paths(&self) -> MaestroPaths {
            MaestroPaths::new(&self.path)
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    // flock is per-open-file-description, so two open() calls in ONE process get
    // independent descriptors that still conflict -- which is exactly what lets us
    // prove mutual exclusion without spawning a second process. This test is also
    // the flock-semantics guard: if the lock ever drifted to fcntl/POSIX
    // (per-process) locks, the second try_acquire_at below would SUCCEED and this
    // would fail.
    #[test]
    fn a_second_descriptor_cannot_acquire_while_the_first_holds_the_gate() {
        let repo = TempRepo::new("mutex");
        let paths = repo.paths();
        let path = lock_path(&paths);

        let first = try_acquire_at(&path, "sessA")
            .expect("io")
            .expect("a free gate acquires");
        assert!(
            try_acquire_at(&path, "sessB").expect("io").is_none(),
            "a second descriptor must not acquire while the first holds the gate"
        );
        assert_eq!(
            holder(&paths).as_deref(),
            Some("sessA"),
            "the holder probe names the session that took the lock"
        );

        drop(first);
        assert_eq!(
            holder(&paths),
            None,
            "the gate is free once the holder's descriptor is dropped"
        );

        let second = try_acquire_at(&path, "sessB")
            .expect("io")
            .expect("the gate is acquirable again after release");
        assert_eq!(holder(&paths).as_deref(), Some("sessB"));
        drop(second);
        assert_eq!(holder(&paths), None);
    }

    // The advisory must name a real busy state even before the holder has written
    // its id (the window between taking the lock and writing the file).
    #[test]
    fn holder_reports_a_generic_name_when_the_id_is_not_yet_written() {
        let repo = TempRepo::new("empty");
        let paths = repo.paths();
        let path = lock_path(&paths);
        std::fs::create_dir_all(path.parent().expect("lock path has a parent"))
            .expect("lock parent dir is creatable");

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .expect("lock file opens");
        assert!(
            flock_nb(file.as_raw_fd(), sys::LOCK_EX).expect("flock"),
            "the manual exclusive lock is taken"
        );

        assert_eq!(
            holder(&paths).as_deref(),
            Some("another session"),
            "an exclusive holder with an empty body reads as a generic busy state"
        );
        drop(file);
    }

    #[test]
    fn wait_lines_name_the_holder_and_elapsed() {
        assert_eq!(
            waiting_line("task-7"),
            "[busy] waiting for task-7's full-suite gate to finish (Ctrl-C to abort)"
        );
        assert_eq!(
            still_waiting_line("task-7", 90),
            "[busy] still waiting (90s) for task-7's gate"
        );
    }
}
