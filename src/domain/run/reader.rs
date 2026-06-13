use std::fs::{self, File};
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::foundation::core::paths::MaestroPaths;

use super::discovery::{RunEventLog, managed_event_logs};
use super::event::logical_session_id_from_run_path;

/// Typed Run event read model.
#[derive(Clone, Debug, PartialEq)]
pub struct RunEvent {
    value: Value,
}

impl RunEvent {
    pub(crate) fn from_value(value: Value) -> Self {
        Self { value }
    }

    /// Event schema version, when present.
    pub fn schema_version(&self) -> Option<&str> {
        self.string("schema_version")
    }

    /// Normalized hook event type, when present.
    pub fn event_type(&self) -> Option<&str> {
        self.string("event_type")
    }

    /// Logical session id embedded in the event, when present.
    pub fn session_id(&self) -> Option<&str> {
        self.string("session_id")
    }

    /// Task id embedded in the event, when present.
    pub fn task_id(&self) -> Option<&str> {
        self.string("task_id")
    }

    /// Card id bound by a `card_touch` event, when present.
    pub fn card_id(&self) -> Option<&str> {
        self.string("card_id")
    }

    /// Skill name recorded by a `skill_activation` event, when present.
    pub fn skill_name(&self) -> Option<&str> {
        self.string("skill_name")
    }

    /// Agent name embedded in the event, when present.
    pub fn agent(&self) -> Option<&str> {
        self.string("agent")
    }

    /// Tool name embedded in the event, when present.
    pub fn tool_name(&self) -> Option<&str> {
        self.string("tool_name")
    }

    /// Tool input hash embedded in the event, when present.
    pub fn tool_input_hash(&self) -> Option<&str> {
        self.string("tool_input_hash")
    }

    /// Event status, when present.
    pub fn status(&self) -> Option<&str> {
        self.string("status")
    }

    /// Git commit snapshot embedded in the event, when present.
    pub fn commit(&self) -> Option<&str> {
        self.string("commit")
    }

    /// Event timestamp, when present.
    pub fn timestamp(&self) -> Option<&str> {
        self.string("ts")
    }

    /// Proof/event alias kind from legacy event payload fields.
    pub fn alias_kind(&self) -> Option<&str> {
        self.string("kind")
            .or_else(|| self.string("event"))
            .or_else(|| self.string("type"))
    }

    /// User-visible prompt or message text, when present.
    pub fn prompt_text(&self) -> Option<&str> {
        self.string("message")
            .or_else(|| self.string("prompt"))
            .or_else(|| self.string("text"))
    }

    /// Single proof claim, when present.
    pub fn claim(&self) -> Option<&str> {
        self.string("claim")
    }

    /// Message text, when present.
    pub fn message(&self) -> Option<&str> {
        self.string("message")
    }

    /// Explicit intervention note text, when present.
    pub fn intervention_note(&self) -> Option<&str> {
        self.string("note")
    }

    /// Normalized event topic, when present.
    pub fn topic(&self) -> Option<&str> {
        self.string("topic")
    }

    /// Proof claim list, when present.
    pub fn claims(&self) -> Vec<String> {
        self.value
            .get("claims")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    }

    /// Return true when this is the given normalized hook event type.
    pub fn is_event_type(&self, event_type: &str) -> bool {
        self.event_type() == Some(event_type)
    }

    fn string(&self, field: &str) -> Option<&str> {
        self.value.get(field).and_then(Value::as_str)
    }
}

/// Parsed event line from a managed run event log.
#[derive(Debug)]
pub struct RunEventRecord<'a> {
    path: &'a Path,
    fallback_session_id: &'a str,
    raw_line: &'a str,
    event: RunEvent,
}

impl<'a> RunEventRecord<'a> {
    /// Path to the source `events.jsonl` file.
    pub fn path(&self) -> &Path {
        self.path
    }

    /// Logical session id for this event.
    pub fn session_id(&self) -> &str {
        self.event.session_id().unwrap_or(self.fallback_session_id)
    }

    /// Typed event payload.
    pub fn event(&self) -> &RunEvent {
        &self.event
    }

    pub(crate) fn raw_line(&self) -> &str {
        self.raw_line
    }
}

/// Visit valid complete JSONL event records from managed Run event logs.
pub fn visit_managed_events<F>(paths: &MaestroPaths, mut visitor: F) -> Result<()>
where
    F: for<'a> FnMut(RunEventRecord<'a>) -> Result<()>,
{
    let logs = managed_event_logs(paths)?;
    visit_managed_event_logs(&logs, &mut visitor)
}

/// Visit valid complete JSONL event records from a pre-enumerated managed log set.
pub fn visit_managed_event_logs<F>(logs: &[RunEventLog], mut visitor: F) -> Result<()>
where
    F: for<'a> FnMut(RunEventRecord<'a>) -> Result<()>,
{
    for log in logs {
        visit_event_log(log.path(), &mut visitor)?;
    }
    Ok(())
}

/// Visit valid complete JSONL event records from one `events.jsonl` file.
///
/// Bad JSON, invalid UTF-8, missing files, symlinked files, and trailing partial
/// lines are ignored so readers can tolerate interrupted writes.
pub fn visit_event_log<F>(path: &Path, visitor: F) -> Result<()>
where
    F: for<'a> FnMut(RunEventRecord<'a>) -> Result<()>,
{
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    }

    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    visit_open_event_log(path, BufReader::new(file), visitor)
}

pub(crate) fn visit_open_event_log<R, F>(path: &Path, reader: R, visitor: F) -> Result<()>
where
    R: BufRead,
    F: for<'a> FnMut(RunEventRecord<'a>) -> Result<()>,
{
    let fallback_session_id = logical_session_id_from_run_path(path);
    visit_event_reader(path, &fallback_session_id, reader, visitor)
}

fn visit_event_reader<R, F>(
    path: &Path,
    fallback_session_id: &str,
    mut reader: R,
    mut visitor: F,
) -> Result<()>
where
    R: BufRead,
    F: for<'a> FnMut(RunEventRecord<'a>) -> Result<()>,
{
    let mut line = Vec::new();

    loop {
        line.clear();
        let bytes_read = reader
            .read_until(b'\n', &mut line)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        if !line.ends_with(b"\n") {
            break;
        }
        line.pop();
        if line.is_empty() {
            continue;
        }
        let Ok(raw_line) = std::str::from_utf8(&line) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(raw_line) else {
            continue;
        };
        let event = RunEvent::from_value(value);
        visitor(RunEventRecord {
            path,
            fallback_session_id,
            raw_line,
            event,
        })?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn visit_open_event_log_reads_from_already_open_file_after_path_disappears() {
        let temp_dir = TestTempDir::new("maestro-run-reader-open-file");
        let run_dir = temp_dir.path().join(".maestro/runs/session-open-file");
        fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
        let events_path = run_dir.join("events.jsonl");
        fs::write(
            &events_path,
            "{\"event_type\":\"PostToolUse\",\"tool_name\":\"Bash\"}\n",
        )
        .expect("invariant: event log fixture should be writable");
        let file = File::open(&events_path).expect("invariant: event log should be openable");
        fs::remove_file(&events_path)
            .expect("invariant: unix should allow unlinking an open event log");

        let mut tools = Vec::new();
        visit_open_event_log(&events_path, BufReader::new(file), |record| {
            tools.push(record.event().tool_name().map(str::to_string));
            Ok(())
        })
        .expect("open event log reader should use the existing file handle");

        assert_eq!(tools, vec![Some("Bash".to_string())]);
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
