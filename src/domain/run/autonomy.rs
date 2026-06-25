use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::timestamp_nanos;

use super::reader::visit_managed_events;

#[derive(Debug, Default, Serialize)]
pub struct AutonomyReport {
    pub started: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_hash: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hard_stops: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ledger_paths: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<AutonomyActionRow>,
    pub blocked_count: usize,
    pub hard_stop_count: usize,
    pub local_close_count: usize,
}

impl AutonomyReport {
    pub fn is_empty(&self) -> bool {
        !self.started && self.actions.is_empty()
    }
}

#[derive(Debug, Serialize)]
pub struct AutonomyActionRow {
    pub at: String,
    pub action: String,
    pub target_kind: String,
    pub target_id: String,
    pub authority_ref: String,
    pub before_state: String,
    pub command: String,
    pub result: String,
    pub after_state: String,
    pub ledger_path: String,
}

pub fn assemble_autonomy_report(
    paths: &MaestroPaths,
    cutoff_nanos: i128,
) -> Result<AutonomyReport> {
    let mut report = AutonomyReport::default();
    visit_managed_events(paths, |record| {
        let event = record.event();
        let Some(ts) = event.timestamp() else {
            return Ok(());
        };
        let Some(ts_nanos) = timestamp_nanos(ts) else {
            return Ok(());
        };
        if ts_nanos < cutoff_nanos {
            return Ok(());
        }

        match event.event_type() {
            Some("autonomy_start") => {
                report.started = true;
                if report.authority_ref.is_none() {
                    report.authority_ref = event.authority_ref().map(str::to_string);
                }
                if report.authority_summary.is_none() {
                    report.authority_summary = event.authority_summary().map(str::to_string);
                }
                if report.prompt_hash.is_none() {
                    report.prompt_hash = event.prompt_hash().map(str::to_string);
                }
                for hard_stop in event.hard_stops() {
                    push_unique(&mut report.hard_stops, hard_stop.to_string());
                }
                push_unique(&mut report.ledger_paths, ledger_path(paths, record.path()));
            }
            Some("autonomy_action") => {
                let row = AutonomyActionRow {
                    at: ts.to_string(),
                    action: value_or_unknown(event.autonomy_action()),
                    target_kind: value_or_unknown(event.target_kind()),
                    target_id: value_or_unknown(event.target_id()),
                    authority_ref: value_or_unknown(event.authority_ref()),
                    before_state: value_or_unknown(event.before_state()),
                    command: value_or_unknown(event.command()),
                    result: value_or_unknown(event.result()),
                    after_state: value_or_unknown(event.after_state()),
                    ledger_path: ledger_path(paths, record.path()),
                };
                if is_blocked(&row) {
                    report.blocked_count += 1;
                }
                if is_hard_stop(&row) {
                    report.hard_stop_count += 1;
                }
                if is_local_close(&row) {
                    report.local_close_count += 1;
                }
                push_unique(&mut report.ledger_paths, row.ledger_path.clone());
                report.actions.push(row);
            }
            _ => {}
        }
        Ok(())
    })?;
    report.actions.sort_by(|left, right| {
        left.at
            .cmp(&right.at)
            .then(left.target_id.cmp(&right.target_id))
            .then(left.action.cmp(&right.action))
    });
    Ok(report)
}

fn value_or_unknown(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("<unknown>")
        .to_string()
}

fn ledger_path(paths: &MaestroPaths, path: &Path) -> String {
    path.strip_prefix(paths.repo_root())
        .ok()
        .unwrap_or(path)
        .display()
        .to_string()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn is_blocked(row: &AutonomyActionRow) -> bool {
    row.result == "blocked" || row.action == "block"
}

fn is_hard_stop(row: &AutonomyActionRow) -> bool {
    row.result == "hard_stop" || row.action == "hard_stop"
}

fn is_local_close(row: &AutonomyActionRow) -> bool {
    matches!(
        row.action.as_str(),
        "feature_close" | "local_close" | "close_feature" | "close"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn report_reconstructs_autonomy_start_and_action_counts() {
        let temp_dir = TestTempDir::new("maestro-autonomy-report");
        let paths = MaestroPaths::new(temp_dir.path().to_path_buf());
        let run_dir = temp_dir.path().join(".maestro/runs/night-run");
        fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
        fs::write(
            run_dir.join("events.jsonl"),
            concat!(
                "{\"schema_version\":\"maestro.event.v1\",\"ts\":\"2026-06-26T00:00:00Z\",\"event_type\":\"autonomy_start\",\"session_id\":\"night-run\",\"authority_ref\":\"run:night-run\",\"authority_summary\":\"full local autonomy\",\"prompt_hash\":\"sha256:abc\",\"hard_stops\":[\"push\",\"archive\"]}\n",
                "{\"schema_version\":\"maestro.event.v1\",\"ts\":\"2026-06-26T00:01:00Z\",\"event_type\":\"autonomy_action\",\"session_id\":\"night-run\",\"action\":\"feature_close\",\"target_kind\":\"feature\",\"target_id\":\"grep-source-shard\",\"authority_ref\":\"run:night-run\",\"before_state\":\"in_progress\",\"command\":\"maestro feature close grep-source-shard --outcome <redacted>\",\"result\":\"closed\",\"after_state\":\"closed\"}\n",
                "{\"schema_version\":\"maestro.event.v1\",\"ts\":\"2026-06-26T00:02:00Z\",\"event_type\":\"autonomy_action\",\"session_id\":\"night-run\",\"action\":\"hard_stop\",\"target_kind\":\"task\",\"target_id\":\"task-secret\",\"authority_ref\":\"run:night-run\",\"before_state\":\"blocked\",\"command\":\"<not run>\",\"result\":\"hard_stop\",\"after_state\":\"blocked\"}\n",
            ),
        )
        .expect("invariant: event log should be writable");

        let report = assemble_autonomy_report(&paths, 0).expect("autonomy report assembles");

        assert!(report.started);
        assert_eq!(report.authority_ref.as_deref(), Some("run:night-run"));
        assert_eq!(report.hard_stops, ["push", "archive"]);
        assert_eq!(
            report.ledger_paths,
            [".maestro/runs/night-run/events.jsonl"]
        );
        assert_eq!(report.actions.len(), 2);
        assert_eq!(report.local_close_count, 1);
        assert_eq!(report.hard_stop_count, 1);
        assert_eq!(report.actions[0].target_id, "grep-source-shard");
        assert_eq!(
            report.actions[0].command,
            "maestro feature close grep-source-shard --outcome <redacted>"
        );
    }

    #[test]
    fn report_respects_the_cutoff_window() {
        let temp_dir = TestTempDir::new("maestro-autonomy-report-cutoff");
        let paths = MaestroPaths::new(temp_dir.path().to_path_buf());
        let run_dir = temp_dir.path().join(".maestro/runs/night-run");
        fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
        fs::write(
            run_dir.join("events.jsonl"),
            concat!(
                "{\"ts\":\"2026-06-25T23:00:00Z\",\"event_type\":\"autonomy_action\",\"action\":\"block\",\"target_kind\":\"task\",\"target_id\":\"old\",\"result\":\"blocked\"}\n",
                "{\"ts\":\"2026-06-26T01:00:00Z\",\"event_type\":\"autonomy_action\",\"action\":\"block\",\"target_kind\":\"task\",\"target_id\":\"new\",\"result\":\"blocked\"}\n",
            ),
        )
        .expect("invariant: event log should be writable");
        let cutoff = timestamp_nanos("2026-06-26T00:00:00Z").expect("cutoff parses");

        let report = assemble_autonomy_report(&paths, cutoff).expect("autonomy report assembles");

        assert_eq!(report.actions.len(), 1);
        assert_eq!(report.actions[0].target_id, "new");
        assert_eq!(report.blocked_count, 1);
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("invariant: temp dir should be creatable");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
