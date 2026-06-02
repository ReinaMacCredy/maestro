use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::BufReader;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::run::discovery::managed_run_evidence_files;
use crate::domain::run::event::run_dir_name;
use crate::domain::run::reader::{RunEvent, visit_open_event_log};
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{Compat, RUN_EVIDENCE_SCHEMA_VERSION, classify};
use crate::foundation::core::time::{ParsedTimestamp, parse_utc_timestamp};

#[derive(Debug, Serialize)]
struct RunEvidence {
    schema_version: &'static str,
    session_id: String,
    agent: Option<String>,
    task_id: Option<String>,
    start_at: Option<String>,
    end_at: Option<String>,
    start_commit: Option<String>,
    end_commit: Option<String>,
    tools_used: BTreeMap<String, u64>,
    human_interventions: u64,
    duration_seconds: Option<u64>,
}

/// Run evidence subset used by metrics and improver rules.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RunEvidenceRecord {
    pub schema_version: String,
    #[serde(default)]
    pub session_id: String,
    pub agent: Option<String>,
    pub task_id: Option<String>,
    pub duration_seconds: Option<u64>,
    #[serde(default)]
    pub human_interventions: u64,
}

/// Best-effort run evidence load result.
#[derive(Clone, Debug, PartialEq)]
pub struct RunEvidenceLoad {
    pub records: Vec<RunEvidenceRecord>,
    pub skipped: usize,
}

/// Aggregate `.maestro/runs/<session_id>/events.jsonl` into `run_evidence.yaml`.
pub fn write_evidence_for_session(paths: &MaestroPaths, session_id: &str) -> Result<()> {
    let run_dir_name = run_dir_name(session_id);
    managed_path(
        paths,
        &format!(".maestro/runs/{run_dir_name}"),
        SymlinkPolicy::RejectAllComponents,
    )?;
    let events_path = managed_path(
        paths,
        &format!(".maestro/runs/{run_dir_name}/events.jsonl"),
        SymlinkPolicy::RejectAllComponents,
    )?;
    let events_file = File::open(&events_path)
        .with_context(|| format!("failed to read {}", events_path.display()))?;
    let mut builder = RunEvidenceBuilder::new(session_id);
    visit_open_event_log(&events_path, BufReader::new(events_file), |record| {
        builder.observe(record.event());
        Ok(())
    })?;
    let evidence = builder.finish();
    let yaml = serde_yaml::to_string(&evidence).context("failed to serialize run evidence")?;
    let evidence_path = managed_path(
        paths,
        &format!(".maestro/runs/{run_dir_name}/run_evidence.yaml"),
        SymlinkPolicy::RejectAllComponents,
    )?;
    write_string_atomic(evidence_path, &yaml)
}

/// Load valid managed run evidence records.
pub fn load_run_evidence(paths: &MaestroPaths) -> Result<RunEvidenceLoad> {
    let mut records = Vec::new();
    let mut skipped = 0;
    for path in managed_run_evidence_files(paths)? {
        let Ok(raw) = fs::read_to_string(&path) else {
            skipped += 1;
            continue;
        };
        let Ok(record) = serde_yaml::from_str::<RunEvidenceRecord>(&raw) else {
            skipped += 1;
            continue;
        };
        if classify(&record.schema_version, RUN_EVIDENCE_SCHEMA_VERSION) == Compat::Exact {
            records.push(record);
        } else {
            skipped += 1;
        }
    }
    Ok(RunEvidenceLoad { records, skipped })
}

struct RunEvidenceBuilder {
    session_id: String,
    agent: Option<String>,
    task_id: Option<String>,
    start: Option<ParsedTimestamp>,
    end: Option<ParsedTimestamp>,
    start_commit: Option<String>,
    end_commit: Option<String>,
    tools_used: BTreeMap<String, u64>,
    prompts: u64,
}

impl RunEvidenceBuilder {
    fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            agent: None,
            task_id: None,
            start: None,
            end: None,
            start_commit: None,
            end_commit: None,
            tools_used: BTreeMap::new(),
            prompts: 0,
        }
    }

    fn observe(&mut self, event: &RunEvent) {
        if self.agent.is_none() {
            self.agent = event.agent().map(str::to_string);
        }
        if self.task_id.is_none() {
            self.task_id = event.task_id().map(str::to_string);
        }
        if event.is_event_type("PostToolUse")
            && let Some(tool_name) = event.tool_name()
        {
            *self.tools_used.entry(tool_name.to_string()).or_insert(0) += 1;
        }
        if event.is_event_type("UserPromptSubmit") {
            self.prompts += 1;
        }
        match event.event_type() {
            Some("SessionStart") if self.start_commit.is_none() => {
                self.start_commit = event.commit().map(str::to_string);
            }
            Some("Stop") => {
                self.end_commit = event.commit().map(str::to_string);
            }
            _ => {}
        }
        if let Some(timestamp) = event.timestamp().and_then(parse_utc_timestamp) {
            if self.start.is_none() {
                self.start = Some(ParsedTimestamp {
                    raw: timestamp.raw.clone(),
                    nanos_since_epoch: timestamp.nanos_since_epoch,
                });
            }
            self.end = Some(timestamp);
        }
    }

    fn finish(self) -> RunEvidence {
        let duration_seconds = match (&self.start, &self.end) {
            (Some(start), Some(end)) if end.nanos_since_epoch >= start.nanos_since_epoch => {
                Some(((end.nanos_since_epoch - start.nanos_since_epoch) / 1_000_000_000) as u64)
            }
            _ => None,
        };
        RunEvidence {
            schema_version: RUN_EVIDENCE_SCHEMA_VERSION,
            session_id: self.session_id,
            agent: self.agent,
            task_id: self.task_id,
            start_at: self.start.map(|timestamp| timestamp.raw),
            end_at: self.end.map(|timestamp| timestamp.raw),
            start_commit: self.start_commit,
            end_commit: self.end_commit,
            tools_used: self.tools_used,
            human_interventions: self.prompts.saturating_sub(1),
            duration_seconds,
        }
    }
}
