use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{Context, Result};

use crate::domain::decisions::query::decision_entries;
use crate::domain::harness::{BacklogItem, HarnessConfig};
use crate::domain::proof;
use crate::domain::run;
use crate::domain::task::{self, TaskEntry};
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;
use crate::operations::metrics;

/// Detect rule-based harness improvement proposals without LLM calls.
pub fn detect(paths: &MaestroPaths) -> Result<Vec<BacklogItem>> {
    let task_entries = task::load_task_entries(&paths.tasks_dir())?;
    let mut proposals = Vec::new();
    proposals.extend(detect_recurring_interventions(paths)?);
    proposals.extend(detect_missing_checks(paths, &task_entries)?);
    proposals.extend(detect_recurring_blockers(&task_entries));
    proposals.extend(detect_missing_skills(&task_entries));
    proposals.extend(detect_rediscovered_decisions(paths, &task_entries)?);
    Ok(proposals)
}

fn detect_recurring_interventions(paths: &MaestroPaths) -> Result<Vec<BacklogItem>> {
    let mut corrections_by_session = BTreeMap::<String, Vec<String>>::new();
    run::visit_managed_events(paths, |record| {
        let event = record.event();
        if event.is_event_type("UserPromptSubmit") {
            let text = event.prompt_text().unwrap_or_default();
            if metrics::looks_like_correction(text) {
                corrections_by_session
                    .entry(record.session_id().to_string())
                    .or_default()
                    .push(text.to_string());
            }
        }
        Ok(())
    })?;

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

fn detect_missing_checks(paths: &MaestroPaths, entries: &[TaskEntry]) -> Result<Vec<BacklogItem>> {
    let harness_commands = harness_verify_commands(paths)?;
    let mut proposals = Vec::new();
    for entry in entries {
        let (commands, source) =
            match proof::verification_command_read_for_task(&entry.task, &entry.task_dir)? {
                proof::VerificationCommandRead::Commands { commands, source } => (commands, source),
                proof::VerificationCommandRead::SkippedMalformedReport => continue,
            };
        let missing = commands
            .into_iter()
            .filter(|command| !harness_commands.contains(command.command()))
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
                            "{} used {} outside harness.yml",
                            source.evidence_name(),
                            command.safe_summary()
                        )
                    })
                    .collect(),
            ));
        }
    }
    Ok(proposals)
}

fn detect_recurring_blockers(entries: &[TaskEntry]) -> Vec<BacklogItem> {
    let mut by_reason = BTreeMap::<String, BTreeSet<String>>::new();
    for entry in entries {
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

    by_reason
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
        .collect()
}

fn detect_missing_skills(entries: &[TaskEntry]) -> Vec<BacklogItem> {
    let durations = task::task_verification_durations(entries);
    let all_durations = durations.values().copied().collect::<Vec<_>>();
    let Some(overall_median) = median(&all_durations) else {
        return Vec::new();
    };
    if overall_median == 0 {
        return Vec::new();
    }

    let mut by_domain = BTreeMap::<String, Vec<u64>>::new();
    for entry in entries {
        if let Some(duration) = durations.get(&entry.task.id) {
            by_domain
                .entry(task_domain(entry))
                .or_default()
                .push(*duration);
        }
    }

    by_domain
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
        .collect()
}

fn detect_rediscovered_decisions(
    paths: &MaestroPaths,
    entries: &[TaskEntry],
) -> Result<Vec<BacklogItem>> {
    let decisions = decision_entries(&paths.decisions_dir())?;
    let decision_texts = decisions
        .iter()
        .filter_map(|entry| fs::read_to_string(&entry.path).ok())
        .map(|text| text.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let mut by_topic = BTreeMap::<String, BTreeSet<String>>::new();

    for entry in entries {
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
        .feature_id
        .as_ref()
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
