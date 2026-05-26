use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::domain::decisions::query::decision_entries;
use crate::domain::harness::schema::{BacklogItem, HarnessConfig};
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;
use crate::metrics::friction::{event_kind, event_text, looks_like_correction};
use crate::metrics::summary::task_verification_durations;
use crate::task::doctor::{load_task_entries, TaskEntry};
use crate::verification::events::managed_event_files;

/// Detect rule-based harness improvement proposals without LLM calls.
pub fn detect(paths: &MaestroPaths) -> Result<Vec<BacklogItem>> {
    let mut proposals = Vec::new();
    proposals.extend(detect_recurring_interventions(paths)?);
    proposals.extend(detect_missing_checks(paths)?);
    proposals.extend(detect_recurring_blockers(paths)?);
    proposals.extend(detect_missing_skills(paths)?);
    proposals.extend(detect_rediscovered_decisions(paths)?);
    Ok(proposals)
}

fn detect_recurring_interventions(paths: &MaestroPaths) -> Result<Vec<BacklogItem>> {
    let mut corrections_by_session = BTreeMap::<String, Vec<String>>::new();
    for path in managed_event_files(paths)? {
        let session = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>")
            .to_string();
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        for line in raw.lines() {
            let Ok(event) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if event_kind(&event) == "UserPromptSubmit" {
                let text = event_text(&event).unwrap_or_default();
                if looks_like_correction(&text) {
                    corrections_by_session
                        .entry(session.clone())
                        .or_default()
                        .push(text);
                }
            }
        }
    }

    let proposals = corrections_by_session
        .into_iter()
        .filter(|(_, corrections)| corrections.len() >= 3)
        .map(|(session, corrections)| {
            proposal(
                session.clone(),
                "recurring_intervention",
                "Reduce repeated correction prompts",
                vec![format!(
                    "{session} had {} correction-like user prompts",
                    corrections.len()
                )],
            )
        })
        .collect();
    Ok(proposals)
}

fn detect_missing_checks(paths: &MaestroPaths) -> Result<Vec<BacklogItem>> {
    let harness_commands = harness_verify_commands(paths)?;
    let mut proposals = Vec::new();
    for entry in load_task_entries(&paths.tasks_dir())? {
        let path = entry.task_dir.join("verification.json");
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(report) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let missing = report
            .get("commands")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(command_value)
            .filter(|command| !harness_commands.contains(*command))
            .map(str::to_string)
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            proposals.push(proposal(
                entry.task.id.clone(),
                "missing_verification",
                format!("Add reusable verification for {}", entry.task.title),
                missing
                    .into_iter()
                    .map(|command| {
                        format!(
                            "verification.json used `{}` outside harness.yml",
                            redact_command(&command)
                        )
                    })
                    .collect(),
            ));
        }
    }
    Ok(proposals)
}

fn command_value(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.get("cmd").and_then(Value::as_str))
}

fn detect_recurring_blockers(paths: &MaestroPaths) -> Result<Vec<BacklogItem>> {
    let mut by_reason = BTreeMap::<String, BTreeSet<String>>::new();
    for entry in load_task_entries(&paths.tasks_dir())? {
        for blocker in &entry.task.blockers {
            let key = normalize_topic(&blocker.reason);
            if !key.is_empty() {
                by_reason
                    .entry(key)
                    .or_default()
                    .insert(entry.task.id.clone());
            }
        }
    }

    let proposals = by_reason
        .into_iter()
        .filter(|(_, tasks)| tasks.len() >= 2)
        .map(|(reason, tasks)| {
            proposal(
                "blockers".to_string(),
                "recurring_blocker",
                format!("Reduce recurring blocker: {reason}"),
                vec![format!(
                    "same blocker pattern appeared in {} tasks: {}",
                    tasks.len(),
                    tasks.into_iter().collect::<Vec<_>>().join(", ")
                )],
            )
        })
        .collect();
    Ok(proposals)
}

fn detect_missing_skills(paths: &MaestroPaths) -> Result<Vec<BacklogItem>> {
    let entries = load_task_entries(&paths.tasks_dir())?;
    let durations = task_verification_durations(paths)?;
    let all_durations = durations.values().copied().collect::<Vec<_>>();
    let Some(overall_median) = median(&all_durations) else {
        return Ok(Vec::new());
    };
    if overall_median == 0 {
        return Ok(Vec::new());
    }

    let mut by_domain = BTreeMap::<String, Vec<u64>>::new();
    for entry in &entries {
        if let Some(duration) = durations.get(&entry.task.id) {
            by_domain
                .entry(task_domain(entry))
                .or_default()
                .push(*duration);
        }
    }

    let proposals = by_domain
        .into_iter()
        .filter(|(_, values)| values.len() >= 2)
        .filter_map(|(domain, values)| {
            let domain_median = median(&values)?;
            if domain_median > overall_median.saturating_mul(2) {
                Some(proposal(
                    domain.clone(),
                    "missing_skill",
                    format!("Add skill support for {domain} work"),
                    vec![format!(
                        "{domain} median verification time was {} min versus {} min overall",
                        domain_median / 60,
                        overall_median / 60
                    )],
                ))
            } else {
                None
            }
        })
        .collect();
    Ok(proposals)
}

fn detect_rediscovered_decisions(paths: &MaestroPaths) -> Result<Vec<BacklogItem>> {
    let decisions = decision_entries(&paths.decisions_dir())?;
    let decision_texts = decisions
        .iter()
        .filter_map(|entry| fs::read_to_string(&entry.path).ok())
        .map(|text| text.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let mut by_topic = BTreeMap::<String, BTreeSet<String>>::new();

    for entry in load_task_entries(&paths.tasks_dir())? {
        let path = entry.task_dir.join("task.md");
        let Ok(markdown) = fs::read_to_string(&path) else {
            continue;
        };
        for topic in decision_topics(&markdown) {
            by_topic
                .entry(topic)
                .or_default()
                .insert(entry.task.id.clone());
        }
    }

    let proposals = by_topic
        .into_iter()
        .filter(|(topic, tasks)| {
            tasks.len() >= 2
                && !decision_texts
                    .iter()
                    .any(|decision| decision.contains(topic))
        })
        .map(|(topic, tasks)| {
            proposal(
                "decisions".to_string(),
                "rediscovered_decision",
                format!("Record decision about {topic}"),
                vec![format!(
                    "{topic} was discussed in {} tasks: {}",
                    tasks.len(),
                    tasks.into_iter().collect::<Vec<_>>().join(", ")
                )],
            )
        })
        .collect();
    Ok(proposals)
}

fn harness_verify_commands(paths: &MaestroPaths) -> Result<BTreeSet<String>> {
    let path = managed_path(
        paths,
        ".maestro/harness/harness.yml",
        SymlinkPolicy::RejectAllComponents,
    )?;
    let Ok(raw) = fs::read_to_string(&path) else {
        return Ok(BTreeSet::new());
    };
    let config: HarnessConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(config.stack.verify.into_iter().collect())
}

fn task_domain(entry: &TaskEntry) -> String {
    entry
        .task
        .affected_areas
        .first()
        .or(entry.task.feature_id.as_ref())
        .or(entry.task.lane.as_ref())
        .cloned()
        .unwrap_or_else(|| "general".to_string())
}

fn decision_topics(markdown: &str) -> Vec<String> {
    markdown
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("decision") || lower.contains("decide")
        })
        .filter_map(normalize_decision_topic)
        .collect()
}

fn normalize_decision_topic(line: &str) -> Option<String> {
    let topic = normalize_topic(line);
    if topic.is_empty() {
        None
    } else {
        Some(topic)
    }
}

fn normalize_topic(value: &str) -> String {
    let stop_words = [
        "about", "after", "again", "because", "blocker", "decide", "decision", "needs", "should",
        "there", "waiting",
    ];
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| word.len() > 3 && !stop_words.contains(&word.as_str()))
        .take(6)
        .collect::<Vec<_>>()
        .join(" ")
}

fn median(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort_unstable();
    Some(values[values.len() / 2])
}

fn proposal(
    source: String,
    item_type: impl Into<String>,
    title: impl Into<String>,
    evidence: Vec<String>,
) -> BacklogItem {
    BacklogItem {
        id: String::new(),
        source,
        item_type: item_type.into(),
        title: title.into(),
        priority: "medium".to_string(),
        status: "proposed".to_string(),
        evidence,
    }
}

fn redact_command(command: &str) -> String {
    command
        .split_whitespace()
        .map(redact_command_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_command_token(token: &str) -> String {
    let Some((key, _)) = token.split_once('=') else {
        return token.to_string();
    };
    let lower = key.to_ascii_lowercase();
    if lower.contains("key")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("password")
        || lower.contains("credential")
    {
        format!("{key}=<redacted>")
    } else {
        token.to_string()
    }
}
