use std::ffi::{c_char, CString};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

use crate::evidence::run_evidence;
use crate::foundation::core::git;
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::EVENT_SCHEMA_VERSION;
use crate::foundation::core::time::utc_now_timestamp;
use crate::hooks::event::{
    is_accepted_event, normalized_event_type, run_dir_name, string_field, UNATTRIBUTED_SESSION,
};

pub(crate) fn record_stdin(paths: &MaestroPaths) -> Result<()> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .context("failed to read hook payload from stdin")?;
    record_payload(paths, &raw)
}

fn record_payload(paths: &MaestroPaths, raw: &str) -> Result<()> {
    let payload: Value = serde_json::from_str(raw).context("failed to parse hook payload JSON")?;
    let Some(mut event) = normalize_event(&payload) else {
        return Ok(());
    };
    attach_commit_snapshot(paths, &mut event);
    append_event(paths, &event)?;
    if is_stop_event(&event) {
        let session_id = event
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or(UNATTRIBUTED_SESSION);
        run_evidence::write_for_session(paths, session_id)
            .context("failed to write run evidence")?;
    }
    Ok(())
}

fn normalize_event(payload: &Value) -> Option<Value> {
    let event_type = event_type(payload)?;
    if !is_accepted_event(&event_type) {
        return None;
    }

    let session_id = string_field(payload, "session_id").filter(|value| !value.trim().is_empty());
    let mut event = Map::new();
    event.insert("schema_version".to_string(), json!(EVENT_SCHEMA_VERSION));
    event.insert("ts".to_string(), json!(timestamp()));
    event.insert(
        "event_type".to_string(),
        json!(normalized_event_type(&event_type)),
    );
    if let Some(session_id) = &session_id {
        event.insert("session_id".to_string(), json!(session_id));
    }

    copy_string(payload, &mut event, "agent");
    copy_string(payload, &mut event, "task_id");
    copy_string(payload, &mut event, "feature_id");
    copy_string(payload, &mut event, "tool_name");
    copy_string(payload, &mut event, "status");
    copy_string(payload, &mut event, "permission_decision");
    copy_string(payload, &mut event, "skill_name");
    copy_string(payload, &mut event, "activation_mode");
    copy_number(payload, &mut event, "duration_ms");

    if let Some(tool_input) = payload.get("tool_input") {
        event.insert("tool_input_hash".to_string(), json!(hash_value(tool_input)));
    }

    Some(Value::Object(event))
}

fn attach_commit_snapshot(paths: &MaestroPaths, event: &mut Value) {
    if !matches!(
        event.get("event_type").and_then(Value::as_str),
        Some("SessionStart" | "Stop")
    ) {
        return;
    }
    let Ok(Some(head)) = git::head(paths.repo_root()) else {
        return;
    };
    if let Some(object) = event.as_object_mut() {
        object.insert("commit".to_string(), json!(head));
    }
}

fn append_event(paths: &MaestroPaths, event: &Value) -> Result<()> {
    let session_id = event
        .get("session_id")
        .and_then(Value::as_str)
        .map(run_dir_name)
        .unwrap_or_else(|| UNATTRIBUTED_SESSION.to_string());
    let relative_path = format!(".maestro/runs/{session_id}/events.jsonl");
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    let mut file = open_event_file(paths, &relative_path, &path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    ensure_opened_event_path_is_managed(paths, &relative_path, &path, &file)?;
    let line = serde_json::to_string(event).context("failed to encode normalized hook event")?;
    writeln!(file, "{line}").with_context(|| format!("failed to append {}", path.display()))
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
    open_event_file_at(paths.repo_root(), Path::new(relative_path))
}

#[cfg(not(unix))]
fn open_event_file(paths: &MaestroPaths, relative_path: &str, path: &Path) -> Result<File> {
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(file) => Ok(file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            create_event_parent_dirs(paths, relative_path)?;
            managed_path(paths, relative_path, SymlinkPolicy::RejectAllComponents)?;
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .with_context(|| format!("failed to open {}", path.display()))
        }
        Err(error) => Err(error).with_context(|| format!("failed to open {}", path.display())),
    }
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
    let flags = 0x0000_0001 | o_create_flag() | o_append_flag() | o_no_follow_flag();
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

fn event_type(payload: &Value) -> Option<String> {
    string_field(payload, "event_type")
        .or_else(|| string_field(payload, "hook_event_name"))
        .or_else(|| string_field(payload, "kind"))
        .or_else(|| string_field(payload, "event"))
        .or_else(|| string_field(payload, "type"))
}

fn is_stop_event(event: &Value) -> bool {
    event.get("event_type").and_then(Value::as_str) == Some("Stop")
}

fn copy_string(source: &Value, target: &mut Map<String, Value>, field: &str) {
    if let Some(value) = string_field(source, field) {
        target.insert(field.to_string(), json!(value));
    }
}

fn copy_number(source: &Value, target: &mut Map<String, Value>, field: &str) {
    if let Some(value) = source.get(field).and_then(Value::as_u64) {
        target.insert(field.to_string(), json!(value));
    }
}

fn hash_value(value: &Value) -> String {
    let bytes =
        serde_json::to_vec(value).expect("invariant: serde_json::Value should serialize to JSON");
    sha256_prefixed(&bytes)
}

fn timestamp() -> String {
    utc_now_timestamp()
}
