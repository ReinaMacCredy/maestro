use anyhow::Result;
use serde::Serialize;

use crate::domain::card::store as card_store;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::{parse_utc_timestamp, utc_now_timestamp};

const SCHEMA: &str = "maestro.research_check.v1";
const RESEARCH_FILE: &str = "research.md";
const FRESH_NANOS: i128 = 7 * 86_400 * 1_000_000_000;
const SECTION_HEADINGS: &[&str] = &[
    "Research Status",
    "Hosting",
    "Problem",
    "Users / Stakeholders",
    "Current Context",
    "Constraints",
    "Unknowns",
    "Assumptions",
    "Landscape",
    "Recommended First Design Fork",
    "Stakeholder Actions",
    "Research Validity",
    "Gate",
];
const SUBSECTION_HEADINGS: &[&str] = &["Blocking", "Important but non-blocking", "Safe to defer"];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResearchCheckReport {
    pub version: u32,
    pub schema: &'static str,
    pub card: String,
    pub status: String,
    pub gate: Option<String>,
    pub fresh: bool,
    pub hosting: HostingReport,
    pub blocking_unknowns: Vec<String>,
    pub stakeholder_actions: Vec<StakeholderActionReport>,
    pub first_design_fork: Option<String>,
    pub reasons: Vec<String>,
    pub next: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HostingReport {
    pub project: Option<String>,
    pub compatible: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StakeholderActionReport {
    pub question: Option<String>,
    pub ask: Option<String>,
    pub status: String,
    pub blocks: Option<String>,
}

pub fn check(
    paths: &MaestroPaths,
    card_id: &str,
    intended_project: Option<&str>,
) -> Result<ResearchCheckReport> {
    card_store::validate_card_id(card_id)?;
    let Some(raw) = read_research_text(paths, card_id)? else {
        return Ok(missing_report(card_id));
    };
    let card_exists = card_store::resolve(paths, card_id)?.is_some();
    let receipt = Receipt::parse(&raw);
    let mut reasons = Vec::new();

    if !card_exists {
        reasons.push("card_missing".to_string());
    }

    if receipt.problem.is_none() {
        reasons.push("problem_missing".to_string());
    }

    if !receipt.fresh {
        reasons.push("stale".to_string());
    }

    let hosting_compatible = intended_project.is_none_or(|project| {
        receipt
            .hosting_project
            .as_deref()
            .is_some_and(|host| host == project)
    });
    if !hosting_compatible {
        reasons.push("hosting_mismatch".to_string());
    }

    if !receipt.blocking_unknowns.is_empty() {
        reasons.push("blocked_unknowns".to_string());
    }

    if receipt
        .stakeholder_actions
        .iter()
        .any(|action| action.status == "open")
    {
        reasons.push("stakeholder_blocked".to_string());
    }

    if receipt.gate.as_deref() == Some("READY_FOR_DESIGN") && receipt.first_design_fork.is_none() {
        reasons.push("first_design_fork_missing".to_string());
    }

    if receipt.skipped {
        reasons.push(if receipt.skip_is_valid() {
            "skip_valid".to_string()
        } else {
            "skip_risky".to_string()
        });
    }

    reasons.sort();
    reasons.dedup();

    let status = public_status(&receipt, &reasons);
    let next = next_step(&status, &reasons);
    Ok(ResearchCheckReport {
        version: 1,
        schema: SCHEMA,
        card: card_id.to_string(),
        status,
        gate: receipt.gate,
        fresh: receipt.fresh,
        hosting: HostingReport {
            project: receipt.hosting_project,
            compatible: hosting_compatible,
        },
        blocking_unknowns: receipt.blocking_unknowns,
        stakeholder_actions: receipt.stakeholder_actions,
        first_design_fork: receipt.first_design_fork,
        reasons,
        next,
    })
}

fn missing_report(card_id: &str) -> ResearchCheckReport {
    ResearchCheckReport {
        schema: SCHEMA,
        version: 1,
        card: card_id.to_string(),
        status: "missing".to_string(),
        gate: None,
        fresh: false,
        hosting: HostingReport {
            project: None,
            compatible: false,
        },
        blocking_unknowns: Vec::new(),
        stakeholder_actions: Vec::new(),
        first_design_fork: None,
        reasons: vec!["research_missing".to_string()],
        next: "run maestro-research or record an explicit skip receipt".to_string(),
    }
}

fn public_status(receipt: &Receipt, reasons: &[String]) -> String {
    if reasons.iter().any(|reason| reason == "hosting_mismatch") {
        "hosting_mismatch".to_string()
    } else if reasons.iter().any(|reason| reason == "stale") {
        "stale".to_string()
    } else if receipt.skipped && reasons.iter().any(|reason| reason == "skip_risky") {
        "risky_skipped".to_string()
    } else if receipt.skipped {
        "skipped".to_string()
    } else if reasons.is_empty() && receipt.gate.as_deref() == Some("READY_FOR_DESIGN") {
        "ready".to_string()
    } else {
        "blocked".to_string()
    }
}

fn next_step(status: &str, reasons: &[String]) -> String {
    if status == "ready" {
        "maestro-design may start".to_string()
    } else if status == "skipped" && reasons.iter().any(|reason| reason == "skip_valid") {
        "maestro-design may start from the valid skip receipt".to_string()
    } else if reasons.iter().any(|reason| reason == "research_missing") {
        "run maestro-research or record an explicit skip receipt".to_string()
    } else if reasons.iter().any(|reason| reason == "hosting_mismatch") {
        "resolve hosting before starting maestro-design here".to_string()
    } else if reasons.iter().any(|reason| reason == "stale") {
        "supersede research.md before starting maestro-design".to_string()
    } else if reasons
        .iter()
        .any(|reason| reason == "first_design_fork_missing")
    {
        "update research.md with one concrete design entry question".to_string()
    } else if reasons
        .iter()
        .any(|reason| reason == "stakeholder_blocked" || reason == "blocked_unknowns")
    {
        "resolve stakeholder/evidence blockers on the same card".to_string()
    } else if reasons.iter().any(|reason| reason == "skip_risky") {
        "route to maestro-research unless the user accepts the recorded risk".to_string()
    } else {
        "route to maestro-research".to_string()
    }
}

fn read_research_text(paths: &MaestroPaths, card_id: &str) -> Result<Option<String>> {
    card_store::read_sidecar_text(paths, card_id, RESEARCH_FILE)
}

#[derive(Clone, Debug, Default)]
struct Receipt {
    skipped: bool,
    skip_reason: Option<String>,
    skipped_by: Option<String>,
    skip_evidence: Option<String>,
    unresolved_risks: Vec<String>,
    hosting_project: Option<String>,
    problem: Option<String>,
    blocking_unknowns: Vec<String>,
    stakeholder_actions: Vec<StakeholderActionReport>,
    first_design_fork: Option<String>,
    fresh: bool,
    gate: Option<String>,
}

impl Receipt {
    fn parse(raw: &str) -> Self {
        let status = section(raw, "Research Status").unwrap_or_default();
        let hosting = section(raw, "Hosting").unwrap_or_default();
        let unknowns = section(raw, "Unknowns").unwrap_or_default();
        let stakeholders = section(raw, "Stakeholder Actions").unwrap_or_default();
        let validity = section(raw, "Research Validity").unwrap_or_default();
        Self {
            skipped: bool_field(&status, "skipped"),
            skip_reason: field(&status, "skip_reason"),
            skipped_by: field(&status, "skipped_by"),
            skip_evidence: field(&status, "evidence"),
            unresolved_risks: list_under_field(&status, "unresolved_risks"),
            hosting_project: field(&hosting, "project"),
            problem: section(raw, "Problem").and_then(|body| first_content_line(&body)),
            blocking_unknowns: subsection(&unknowns, "Blocking")
                .map(|body| content_items(&body))
                .unwrap_or_default(),
            stakeholder_actions: parse_stakeholder_actions(&stakeholders),
            first_design_fork: section(raw, "Recommended First Design Fork")
                .and_then(|body| first_content_line(&body)),
            fresh: fresh_validity(&validity),
            gate: section(raw, "Gate").and_then(|body| first_content_line(&body)),
        }
    }

    fn skip_is_valid(&self) -> bool {
        if !self.skipped {
            return false;
        }
        let reason = self.skip_reason.as_deref().unwrap_or_default();
        let by = self.skipped_by.as_deref().unwrap_or_default();
        let whitelisted = matches!(
            reason,
            "settled spec pasted"
                | "existing fresh research"
                | "small local change"
                | "clearly settled context"
        );
        whitelisted
            && by == "agent"
            && self.skip_evidence.is_some()
            && self.unresolved_risks.is_empty()
    }
}

fn section(raw: &str, heading: &str) -> Option<String> {
    let mut body = Vec::new();
    let mut active = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(current) = trimmed.strip_prefix("## ") {
            if active {
                break;
            }
            active = current.trim().eq_ignore_ascii_case(heading);
            continue;
        }
        if is_label_heading(line, heading) {
            if active {
                break;
            }
            active = true;
            continue;
        }
        if active && is_any_section_heading(line) {
            break;
        }
        if active {
            body.push(line);
        }
    }
    active.then(|| body.join("\n"))
}

fn subsection(raw: &str, heading: &str) -> Option<String> {
    let mut body = Vec::new();
    let mut active = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(current) = trimmed.strip_prefix("### ") {
            if active {
                break;
            }
            active = current.trim().eq_ignore_ascii_case(heading);
            continue;
        }
        if is_label_heading(line, heading) {
            if active {
                break;
            }
            active = true;
            continue;
        }
        if active && is_any_subsection_heading(line) {
            break;
        }
        if active {
            body.push(line);
        }
    }
    active.then(|| body.join("\n"))
}

fn is_label_heading(line: &str, heading: &str) -> bool {
    label_heading(line).is_some_and(|label| label.eq_ignore_ascii_case(heading))
}

fn is_any_section_heading(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("## ") {
        return true;
    }
    label_heading(line).is_some_and(|label| {
        SECTION_HEADINGS
            .iter()
            .any(|heading| label.eq_ignore_ascii_case(heading))
    })
}

fn is_any_subsection_heading(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("### ") {
        return true;
    }
    label_heading(line).is_some_and(|label| {
        SUBSECTION_HEADINGS
            .iter()
            .any(|heading| label.eq_ignore_ascii_case(heading))
    })
}

fn label_heading(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if line.starts_with(char::is_whitespace) || trimmed.starts_with('-') {
        return None;
    }
    trimmed
        .strip_suffix(':')
        .map(str::trim)
        .filter(|label| !label.is_empty())
}

fn bool_field(body: &str, key: &str) -> bool {
    field(body, key)
        .as_deref()
        .is_some_and(|value| matches!(value, "true" | "yes"))
}

fn field(body: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&prefix) {
            let value = trimmed[prefix.len()..].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
            return None;
        }
    }
    None
}

fn list_under_field(body: &str, key: &str) -> Vec<String> {
    let prefix = format!("{key}:");
    let mut active = false;
    let mut items = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&prefix) {
            active = true;
            continue;
        }
        if active && trimmed.ends_with(':') && !trimmed.starts_with('-') {
            break;
        }
        if active && let Some(item) = strip_content_marker(trimmed) {
            items.push(item);
        }
    }
    items
}

fn parse_stakeholder_actions(body: &str) -> Vec<StakeholderActionReport> {
    if content_items(body).is_empty() {
        return Vec::new();
    }
    let mut actions = Vec::new();
    let mut current = StakeholderActionReport {
        question: None,
        ask: None,
        status: "open".to_string(),
        blocks: None,
    };
    let mut has_current = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("- question:") {
            if has_current {
                actions.push(current);
            }
            current = StakeholderActionReport {
                question: non_empty(value),
                ask: None,
                status: "open".to_string(),
                blocks: None,
            };
            has_current = true;
        } else if let Some(value) = trimmed.strip_prefix("ask:") {
            current.ask = non_empty(value);
            has_current = true;
        } else if let Some(value) = trimmed.strip_prefix("status:") {
            current.status = non_empty(value).unwrap_or_else(|| "open".to_string());
            has_current = true;
        } else if let Some(value) = trimmed.strip_prefix("blocks:") {
            current.blocks = non_empty(value);
            has_current = true;
        }
    }
    if has_current {
        actions.push(current);
    }
    actions
}

fn content_items(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| strip_content_marker(line.trim()))
        .collect()
}

fn first_content_line(body: &str) -> Option<String> {
    body.lines()
        .find_map(|line| strip_content_marker(line.trim()))
        .map(|line| line.trim_matches('`').to_string())
}

fn strip_content_marker(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || is_none_marker(trimmed) {
        return None;
    }
    let value = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .unwrap_or(trimmed)
        .trim();
    if value.is_empty() || is_none_marker(value) {
        None
    } else {
        Some(value.to_string())
    }
}

fn is_none_marker(value: &str) -> bool {
    matches!(
        value
            .trim()
            .trim_end_matches('.')
            .to_ascii_lowercase()
            .as_str(),
        "none" | "n/a" | "na" | "not applicable"
    )
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn fresh_validity(body: &str) -> bool {
    let Some(as_of) = field(body, "as_of") else {
        return false;
    };
    if !body.contains("invalidates_when:") {
        return false;
    }
    let timestamp = if as_of.contains('T') {
        as_of
    } else {
        format!("{as_of}T00:00:00.000Z")
    };
    let Some(parsed) = parse_utc_timestamp(&timestamp) else {
        return false;
    };
    let Some(now) = parse_utc_timestamp(&utc_now_timestamp()) else {
        return false;
    };
    let age = now.nanos_since_epoch - parsed.nanos_since_epoch;
    (0..=FRESH_NANOS).contains(&age)
}
