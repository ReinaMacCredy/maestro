use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::domain::card::store as card_store;
use crate::domain::feature::{self, FeatureStatus};
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;

const STRUCTURED_ROUTE_HINT: &str = "structured route_hint frontmatter";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntakeRoute {
    DesignRequired,
    CardReady,
    WorkReady,
}

impl IntakeRoute {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DesignRequired => "design_required",
            Self::CardReady => "card_ready",
            Self::WorkReady => "work_ready",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    File,
    Stdin,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Stdin => "stdin",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceProvenance {
    pub kind: SourceKind,
    pub path: Option<String>,
    pub bytes: usize,
    pub sha256: String,
}

impl SourceProvenance {
    pub fn file(path: String, contents: &str) -> Self {
        Self {
            kind: SourceKind::File,
            path: Some(path),
            bytes: contents.len(),
            sha256: sha256_hex(contents.as_bytes()),
        }
    }

    pub fn stdin(contents: &str) -> Self {
        Self {
            kind: SourceKind::Stdin,
            path: None,
            bytes: contents.len(),
            sha256: sha256_hex(contents.as_bytes()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IntakeReport {
    pub version: u32,
    pub schema: &'static str,
    pub route: IntakeRoute,
    pub route_hint: Option<String>,
    pub owner: Option<String>,
    pub missing: Vec<String>,
    pub blocked_by: Vec<String>,
    pub writes_allowed: bool,
    pub next: String,
    pub source_provenance: SourceProvenance,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct IntakeFrontmatter {
    route_hint: Option<String>,
    owner: Option<String>,
    #[serde(default)]
    evidence: IntakeEvidence,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct IntakeEvidence {
    #[serde(default)]
    acceptance: bool,
    #[serde(default)]
    affected_areas: bool,
    #[serde(default)]
    proof_path: bool,
    #[serde(default)]
    handoff_fresh: bool,
    #[serde(default)]
    blockers_clear: bool,
}

#[derive(Clone, Debug)]
enum FrontmatterParse {
    Missing,
    Invalid(String),
    Parsed(IntakeFrontmatter),
}

pub fn classify(
    paths: &MaestroPaths,
    raw: &str,
    source_provenance: SourceProvenance,
) -> Result<IntakeReport> {
    let parsed = parse_frontmatter(raw);
    let mut missing = Vec::new();
    let mut blocked_by = Vec::new();

    let frontmatter = match parsed {
        FrontmatterParse::Parsed(frontmatter) => Some(frontmatter),
        FrontmatterParse::Missing => {
            missing.push(STRUCTURED_ROUTE_HINT.to_string());
            None
        }
        FrontmatterParse::Invalid(error) => {
            missing.push("valid YAML frontmatter".to_string());
            blocked_by.push(format!("frontmatter parse error: {error}"));
            None
        }
    };

    let route_hint = frontmatter
        .as_ref()
        .and_then(|frontmatter| normalize_route_hint(frontmatter.route_hint.as_deref()));

    if frontmatter.is_some() && route_hint.is_none() {
        missing.push(STRUCTURED_ROUTE_HINT.to_string());
    }

    let raw_owner = frontmatter
        .as_ref()
        .and_then(|frontmatter| present(frontmatter.owner.as_deref()).map(str::to_string));
    let owner = validate_owner(paths, raw_owner.as_deref(), &mut missing, &mut blocked_by)?;
    let route = match route_hint.as_deref() {
        Some("design_required") => IntakeRoute::DesignRequired,
        Some("card_ready") => classify_card_ready(frontmatter.as_ref(), &owner, &mut missing),
        Some("work_ready") => classify_work_ready(
            paths,
            frontmatter.as_ref(),
            &owner,
            &mut missing,
            &mut blocked_by,
        ),
        Some(other) => {
            missing.push("route_hint one of design_required, card_ready, work_ready".to_string());
            blocked_by.push(format!("unknown route_hint {other}"));
            IntakeRoute::DesignRequired
        }
        None => IntakeRoute::DesignRequired,
    };

    Ok(IntakeReport {
        version: 1,
        schema: "maestro.intake.v1",
        next: next_command(&route, owner.as_deref()),
        route,
        route_hint,
        owner,
        missing,
        blocked_by,
        writes_allowed: false,
        source_provenance,
    })
}

fn classify_card_ready(
    frontmatter: Option<&IntakeFrontmatter>,
    owner: &Option<String>,
    missing: &mut Vec<String>,
) -> IntakeRoute {
    if card_ready_missing(frontmatter, owner, missing) {
        IntakeRoute::DesignRequired
    } else {
        IntakeRoute::CardReady
    }
}

fn classify_work_ready(
    paths: &MaestroPaths,
    frontmatter: Option<&IntakeFrontmatter>,
    owner: &Option<String>,
    missing: &mut Vec<String>,
    blocked_by: &mut Vec<String>,
) -> IntakeRoute {
    if card_ready_missing(frontmatter, owner, missing) {
        return IntakeRoute::DesignRequired;
    }

    let Some(frontmatter) = frontmatter else {
        return IntakeRoute::DesignRequired;
    };
    let Some(owner) = owner.as_deref() else {
        return IntakeRoute::DesignRequired;
    };

    if !frontmatter.evidence.proof_path {
        missing.push("proof_path evidence".to_string());
    }
    if !frontmatter.evidence.handoff_fresh {
        missing.push("handoff_fresh evidence".to_string());
    }
    if !frontmatter.evidence.blockers_clear {
        missing.push("blockers_clear evidence".to_string());
    }

    match feature::show(paths, owner) {
        Ok(view)
            if matches!(
                view.status,
                FeatureStatus::Ready | FeatureStatus::InProgress
            ) => {}
        Ok(view) => blocked_by.push(format!(
            "owner feature {owner} status is {}",
            view.status.as_str()
        )),
        Err(error) => blocked_by.push(format!("owner feature {owner} is not ready: {error}")),
    }

    if missing.is_empty() && blocked_by.is_empty() {
        IntakeRoute::WorkReady
    } else {
        IntakeRoute::CardReady
    }
}

fn card_ready_missing(
    frontmatter: Option<&IntakeFrontmatter>,
    owner: &Option<String>,
    missing: &mut Vec<String>,
) -> bool {
    let mut found_gap = false;
    let Some(frontmatter) = frontmatter else {
        return true;
    };
    if owner.is_none() {
        missing.push("owner".to_string());
        found_gap = true;
    }
    if !frontmatter.evidence.acceptance {
        missing.push("acceptance evidence".to_string());
        found_gap = true;
    }
    if !frontmatter.evidence.affected_areas {
        missing.push("affected_areas evidence".to_string());
        found_gap = true;
    }
    found_gap
}

fn validate_owner(
    paths: &MaestroPaths,
    owner: Option<&str>,
    missing: &mut Vec<String>,
    blocked_by: &mut Vec<String>,
) -> Result<Option<String>> {
    let Some(owner) = owner else {
        return Ok(None);
    };
    if card_store::validate_card_id(owner).is_err() {
        missing.push("valid owner".to_string());
        blocked_by.push("owner must be a single Maestro card or feature id".to_string());
        return Ok(None);
    }
    if feature::show(paths, owner).is_err() {
        missing.push("existing owner".to_string());
        blocked_by.push(format!("owner feature {owner} does not exist"));
        return Ok(Some(owner.to_string()));
    }
    Ok(Some(owner.to_string()))
}

fn next_command(route: &IntakeRoute, owner: Option<&str>) -> String {
    match (route, owner) {
        (IntakeRoute::DesignRequired, _) => {
            "maestro feature new \"<title>\" --description \"<source summary>\"".to_string()
        }
        (IntakeRoute::CardReady, Some(owner)) => {
            format!("maestro feature show {owner}")
        }
        (IntakeRoute::CardReady, None) => "maestro feature show <owner>".to_string(),
        (IntakeRoute::WorkReady, Some(owner)) => {
            format!("maestro ready {owner}")
        }
        (IntakeRoute::WorkReady, None) => "maestro ready <owner>".to_string(),
    }
}

fn parse_frontmatter(raw: &str) -> FrontmatterParse {
    let Some(rest) = raw
        .strip_prefix("---\n")
        .or_else(|| raw.strip_prefix("---\r\n"))
    else {
        return FrontmatterParse::Missing;
    };
    let mut yaml_end = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']) == "---" {
            return match serde_yaml::from_str::<IntakeFrontmatter>(&rest[..yaml_end]) {
                Ok(frontmatter) => FrontmatterParse::Parsed(frontmatter),
                Err(error) => FrontmatterParse::Invalid(error.to_string()),
            };
        }
        yaml_end += line.len();
    }
    FrontmatterParse::Invalid("missing closing --- marker".to_string())
}

fn normalize_route_hint(value: Option<&str>) -> Option<String> {
    present(value).map(|value| value.to_ascii_lowercase().replace('-', "_"))
}

fn present(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}
