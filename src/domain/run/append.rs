use std::ffi::{CString, c_char};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::domain::run::event::{UNATTRIBUTED_SESSION, run_dir_name};
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::retention::prune_child_dirs;
use crate::foundation::core::session::known_agent_runtime;

use super::activity::append_activity_for_run_event;

const OPEN_EVENT_FILE_RETRIES: usize = 8;

pub(crate) fn append_normalized_event(paths: &MaestroPaths, event: &Value) -> Result<()> {
    let session_id = event
        .get("session_id")
        .and_then(Value::as_str)
        .map(run_dir_name)
        .unwrap_or_else(|| UNATTRIBUTED_SESSION.to_string());
    append_event_to_session_dir(paths, &session_id, event)?;
    if event.get("event_type").and_then(Value::as_str) == Some("SessionStart") {
        prune_child_dirs(&paths.runs_dir(), 20)?;
    }
    Ok(())
}

/// Append a manually created event (`maestro event create`) into the managed
/// Run event log for `session_id`, applying the same symlink-hardened,
/// newline-repairing append the hook recorder uses. `session_id` is percent-
/// encoded into a single safe directory component, so a caller-supplied run id
/// can never escape `.maestro/runs/`.
pub(crate) fn append_manual_event(
    paths: &MaestroPaths,
    session_id: &str,
    event: &Value,
) -> Result<()> {
    append_event_to_session_dir(paths, &run_dir_name(session_id), event)
}

pub(crate) fn insert_agent_runtime(event: &mut Value, runtime: Option<&str>) {
    let Some(runtime) = runtime.and_then(known_agent_runtime) else {
        return;
    };
    if let Some(object) = event.as_object_mut() {
        object.insert("agent_runtime".to_string(), json!(runtime));
    }
}

fn append_event_to_session_dir(
    paths: &MaestroPaths,
    session_dir: &str,
    event: &Value,
) -> Result<()> {
    let relative_path = format!(".maestro/runs/{session_dir}/events.jsonl");
    let mut file = open_managed_appendable(paths, &relative_path)?;
    append_jsonl_line(&mut file, event)
        .with_context(|| format!("failed to append {relative_path}"))?;
    append_activity_for_run_event(paths, session_dir, event)?;
    Ok(())
}

/// Open a managed append-only file under the repo, creating parent dirs, applying
/// the same symlink-hardened `openat` path the Run event log uses (O_APPEND |
/// O_NOFOLLOW, dev/ino recheck). Shared by the Run event log and the channel
/// store so both reuse one hardened append rather than duplicating the FFI.
pub(crate) fn open_managed_appendable(paths: &MaestroPaths, relative_path: &str) -> Result<File> {
    let path = managed_path(paths, relative_path, SymlinkPolicy::RejectAllComponents)?;
    let file = open_event_file(paths, relative_path, &path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    ensure_opened_event_path_is_managed(paths, relative_path, &path, &file)?;
    Ok(file)
}

pub(crate) fn append_jsonl_line(file: &mut File, event: &Value) -> Result<()> {
    let mut line = Vec::new();
    if event_file_needs_leading_newline(file)? {
        line.push(b'\n');
    }
    line.extend(
        serde_json::to_string(event)
            .context("failed to encode normalized hook event")?
            .as_bytes(),
    );
    line.push(b'\n');
    file.write_all(&line)?;
    Ok(())
}

fn event_file_needs_leading_newline(file: &mut File) -> io::Result<bool> {
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(false);
    }

    file.seek(SeekFrom::End(-1))?;
    let mut last = [0_u8; 1];
    file.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

#[cfg(not(unix))]
fn create_event_parent_dirs(paths: &MaestroPaths, relative_path: &str) -> Result<()> {
    let relative = Path::new(relative_path);
    let Some(parent) = relative.parent() else {
        return Ok(());
    };

    let mut current = std::path::PathBuf::new();
    for component in parent.components() {
        current.push(component.as_os_str());
        let current_relative = current.to_string_lossy();
        let directory = managed_path(
            paths,
            current_relative.as_ref(),
            SymlinkPolicy::RejectAllComponents,
        )?;
        match fs::create_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to create {}", directory.display()));
            }
        }
        managed_path(
            paths,
            current_relative.as_ref(),
            SymlinkPolicy::RejectAllComponents,
        )?;
    }

    Ok(())
}

fn ensure_opened_event_path_is_managed(
    paths: &MaestroPaths,
    relative_path: &str,
    path: &Path,
    file: &File,
) -> Result<()> {
    managed_path(paths, relative_path, SymlinkPolicy::RejectAllComponents)?;
    ensure_open_file_matches_path(path, file)
        .with_context(|| format!("failed to verify {}", path.display()))
}

#[cfg(unix)]
fn open_event_file(paths: &MaestroPaths, relative_path: &str, _path: &Path) -> io::Result<File> {
    retry_open_event_file_at(paths.repo_root(), Path::new(relative_path))
}

#[cfg(not(unix))]
fn open_event_file(paths: &MaestroPaths, relative_path: &str, path: &Path) -> Result<File> {
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(path)
    {
        Ok(file) => Ok(file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            create_event_parent_dirs(paths, relative_path)?;
            managed_path(paths, relative_path, SymlinkPolicy::RejectAllComponents)?;
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .read(true)
                .open(path)
                .with_context(|| format!("failed to open {}", path.display()))
        }
        Err(error) => Err(error).with_context(|| format!("failed to open {}", path.display())),
    }
}

#[cfg(unix)]
fn retry_open_event_file_at(repo_root: &Path, relative_path: &Path) -> io::Result<File> {
    let mut last_error = None;
    for attempt in 0..=OPEN_EVENT_FILE_RETRIES {
        match open_event_file_at(repo_root, relative_path) {
            Ok(file) => return Ok(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                last_error = Some(error);
                if attempt < OPEN_EVENT_FILE_RETRIES {
                    thread::sleep(Duration::from_millis(1 << attempt.min(4)));
                    continue;
                }
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| io::Error::new(io::ErrorKind::NotFound, "event path not found")))
}

#[cfg(unix)]
fn open_event_file_at(repo_root: &Path, relative_path: &Path) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};

    let parent = relative_path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "event path must have a parent")
    })?;
    let file_name = relative_path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "event path must have a file name",
        )
    })?;

    let mut dir = File::open(repo_root)?;
    for component in parent.components() {
        let name = component_cstring(component.as_os_str())?;
        dir = match open_dir_at(dir.as_raw_fd(), &name) {
            Ok(child) => child,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                mkdir_at(dir.as_raw_fd(), &name)?;
                open_dir_at(dir.as_raw_fd(), &name)?
            }
            Err(error) => return Err(error),
        };
    }

    let name = component_cstring(file_name)?;
    let fd = open_file_at(dir.as_raw_fd(), &name)?;
    // SAFETY: `open_file_at` returned a fresh owned descriptor on success.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(unix)]
fn component_cstring(name: &std::ffi::OsStr) -> io::Result<CString> {
    use std::os::unix::ffi::OsStrExt;

    CString::new(name.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path component contains NUL"))
}

#[cfg(target_os = "macos")]
fn o_no_follow_flag() -> i32 {
    0x0000_0100
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "m68k"
    )
))]
fn o_no_follow_flag() -> i32 {
    0x0000_8000
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    target_arch = "riscv64",
    target_os = "android"
))]
fn o_no_follow_flag() -> i32 {
    0x0040_0000
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    not(any(
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "m68k"
    )),
    not(all(target_arch = "riscv64", target_os = "android"))
))]
fn o_no_follow_flag() -> i32 {
    0x0002_0000
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "linux", target_os = "android"))
))]
fn o_no_follow_flag() -> i32 {
    0
}

#[cfg(target_os = "macos")]
fn o_directory_flag() -> i32 {
    0x0010_0000
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "m68k"
    )
))]
fn o_directory_flag() -> i32 {
    0x0000_4000
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    target_arch = "riscv64",
    target_os = "android"
))]
fn o_directory_flag() -> i32 {
    0x0020_0000
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    not(any(
        target_arch = "arm",
        target_arch = "aarch64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "m68k"
    )),
    not(all(target_arch = "riscv64", target_os = "android"))
))]
fn o_directory_flag() -> i32 {
    0x0001_0000
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "linux", target_os = "android"))
))]
fn o_directory_flag() -> i32 {
    0
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn o_create_flag() -> i32 {
    0x0000_0200
}

#[cfg(all(any(target_os = "linux", target_os = "android"), target_arch = "mips"))]
fn o_create_flag() -> i32 {
    0x0000_0100
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    target_arch = "mips64"
))]
fn o_create_flag() -> i32 {
    0x0000_0100
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(target_arch = "sparc", target_arch = "sparc64")
))]
fn o_create_flag() -> i32 {
    0x0000_0200
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    not(any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc",
        target_arch = "sparc64"
    ))
))]
fn o_create_flag() -> i32 {
    0x0000_0040
}

#[cfg(target_os = "macos")]
fn o_append_flag() -> i32 {
    0x0000_0008
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc",
        target_arch = "sparc64"
    )
))]
fn o_append_flag() -> i32 {
    0x0000_0008
}

#[cfg(all(
    any(target_os = "linux", target_os = "android"),
    not(any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc",
        target_arch = "sparc64"
    ))
))]
fn o_append_flag() -> i32 {
    0x0000_0400
}

#[cfg(unix)]
fn open_dir_at(parent_fd: i32, name: &CString) -> io::Result<File> {
    use std::os::fd::FromRawFd;

    let flags = o_directory_flag() | o_no_follow_flag();
    // SAFETY: `name` is a valid NUL-terminated component and `parent_fd` is an open directory fd.
    let fd = unsafe { openat(parent_fd, name.as_ptr(), flags, 0) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: `openat` returned a fresh owned descriptor on success.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

#[cfg(unix)]
fn mkdir_at(parent_fd: i32, name: &CString) -> io::Result<()> {
    // SAFETY: `name` is a valid NUL-terminated component and `parent_fd` is an open directory fd.
    let result = unsafe { mkdirat(parent_fd, name.as_ptr(), 0o755 as ModeT) };
    if result == 0 {
        Ok(())
    } else {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::AlreadyExists {
            Ok(())
        } else {
            Err(error)
        }
    }
}

#[cfg(unix)]
fn open_file_at(parent_fd: i32, name: &CString) -> io::Result<i32> {
    let flags = 0x0000_0002 | o_create_flag() | o_append_flag() | o_no_follow_flag();
    // SAFETY: `name` is a valid NUL-terminated component and `parent_fd` is an open directory fd.
    let fd = unsafe { openat(parent_fd, name.as_ptr(), flags, 0o644) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

#[cfg(unix)]
#[cfg(any(
    target_os = "macos",
    all(target_os = "android", any(target_arch = "arm", target_arch = "x86"))
))]
type ModeT = u16;

#[cfg(unix)]
#[cfg(not(any(
    target_os = "macos",
    all(target_os = "android", any(target_arch = "arm", target_arch = "x86"))
)))]
type ModeT = u32;

#[cfg(unix)]
unsafe extern "C" {
    fn mkdirat(dirfd: i32, pathname: *const c_char, mode: ModeT) -> i32;
    fn openat(dirfd: i32, pathname: *const c_char, flags: i32, ...) -> i32;
}

#[cfg(unix)]
fn ensure_open_file_matches_path(path: &Path, file: &File) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    let opened = file.metadata()?;
    let current = fs::metadata(path)?;
    if opened.dev() == current.dev() && opened.ino() == current.ino() {
        Ok(())
    } else {
        Err(io::Error::other(
            "opened event file no longer matches managed path",
        ))
    }
}

#[cfg(not(unix))]
fn ensure_open_file_matches_path(_path: &Path, _file: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;

    #[test]
    fn append_manual_event_keeps_a_traversal_session_id_inside_runs_dir() {
        let temp_dir = TestTempDir::new("maestro-run-append-manual-traversal");
        let paths = MaestroPaths::new(temp_dir.path());

        append_manual_event(&paths, "../../escape", &json!({"event": "note"}))
            .expect("a traversal-shaped run id should still write safely");

        // The encoded run id must not escape .maestro/runs/.
        assert!(
            !temp_dir.path().join("escape").exists(),
            "`../../escape` must not write a sibling of .maestro/"
        );
        assert!(
            !temp_dir
                .path()
                .parent()
                .map(|parent| parent.join("escape").exists())
                .unwrap_or(false),
            "`../../escape` must not climb above the repo root"
        );
        let children: Vec<PathBuf> = fs::read_dir(paths.runs_dir())
            .expect("runs dir should exist after a manual event")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect();
        assert_eq!(
            children.len(),
            1,
            "exactly one encoded run directory should exist, got {children:?}"
        );
        assert!(
            children[0]
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name != ".." && !name.contains('/'))
                .unwrap_or(false),
            "the run directory must be a single safe component, got {children:?}"
        );
    }

    #[test]
    fn append_manual_event_appends_newline_separated_lines() {
        let temp_dir = TestTempDir::new("maestro-run-append-manual-lines");
        let paths = MaestroPaths::new(temp_dir.path());

        append_manual_event(&paths, "sess-1", &json!({"event": "first"}))
            .expect("first manual event should write");
        append_manual_event(&paths, "sess-1", &json!({"event": "second"}))
            .expect("second manual event should append");

        let log = paths.runs_dir().join("sess-1").join("events.jsonl");
        let contents = fs::read_to_string(&log).expect("event log should be readable");
        let lines: Vec<&str> = contents.lines().filter(|line| !line.is_empty()).collect();
        assert_eq!(
            lines.len(),
            2,
            "two appends should yield two JSONL lines: {contents:?}"
        );
        assert!(lines[0].contains("\"first\""));
        assert!(lines[1].contains("\"second\""));
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: system clock should be after the Unix epoch")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("invariant: temp dir should be creatable");
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
