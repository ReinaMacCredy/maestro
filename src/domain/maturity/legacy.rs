use std::fs;
use std::io::ErrorKind;

use anyhow::Result;
use serde::Serialize;

use crate::domain::feature;
use crate::domain::feature::AcceptanceProof;
use crate::domain::harness;
use crate::foundation::core::paths::MaestroPaths;

const MATURITY_SCHEMA: &str = "maestro.maturity.v1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MaturityReport {
    pub version: u32,
    pub schema: &'static str,
    pub target: Option<String>,
    pub context: Vec<ContextReadout>,
    pub proof: ProofReadout,
    pub friction: FrictionReadout,
    pub maturity: MaturityLevelReadout,
    pub next_owner: NextOwnerReadout,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContextReadout {
    pub name: String,
    pub required: bool,
    pub status: ContextStatus,
    pub evidence: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextStatus {
    Present,
    Missing,
    Unverified,
}

impl ContextStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::Unverified => "unverified",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProofReadout {
    pub total: usize,
    pub complete: usize,
    pub partial: usize,
    pub incomplete: usize,
    pub gaps: Vec<ProofGap>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProofGap {
    pub ac_id: String,
    pub text: String,
    pub owner_surface: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FrictionReadout {
    pub ux_gap_entries: usize,
    pub harness_backlog_items: usize,
    pub recurring_items: Vec<FrictionItem>,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FrictionItem {
    pub id: String,
    pub title: String,
    pub occurrences: usize,
    pub sessions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MaturityLevelReadout {
    pub level: String,
    pub rationale: String,
    pub blocked_from_next_level: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NextOwnerReadout {
    pub surface: String,
    pub command: String,
}

pub fn report(paths: &MaestroPaths, target: Option<&str>) -> Result<MaturityReport> {
    let (feature_readout, proof) = target_readout(paths, target)?;
    let friction = friction_readout(paths);
    let context = context_readout(paths, target, &feature_readout, &proof);
    let maturity = maturity_level(&context, &proof, &friction);
    let next_owner = next_owner(&context, &proof, &friction, target);

    Ok(MaturityReport {
        version: 1,
        schema: MATURITY_SCHEMA,
        target: target.map(str::to_string),
        context,
        proof,
        friction,
        maturity,
        next_owner,
    })
}

enum FeatureReadout {
    NotRequested,
    Present { id: String, acceptance_count: usize },
    Missing { id: String, error: String },
}

fn target_readout(
    paths: &MaestroPaths,
    target: Option<&str>,
) -> Result<(FeatureReadout, ProofReadout)> {
    let Some(id) = target else {
        return Ok((FeatureReadout::NotRequested, empty_proof_readout()));
    };
    let sweep = match feature::current_acceptance_sweep(paths, id) {
        Ok(sweep) => sweep,
        Err(error) => {
            if feature::ensure_exists(paths, id).is_err() {
                return Ok((
                    FeatureReadout::Missing {
                        id: id.to_string(),
                        error: error.to_string(),
                    },
                    empty_proof_readout(),
                ));
            }
            return Err(error);
        }
    };
    let proof = proof_readout_from_sweep(&sweep);
    Ok((
        FeatureReadout::Present {
            id: id.to_string(),
            acceptance_count: sweep.items.len(),
        },
        proof,
    ))
}

fn empty_proof_readout() -> ProofReadout {
    ProofReadout {
        total: 0,
        complete: 0,
        partial: 0,
        incomplete: 0,
        gaps: Vec::new(),
    }
}

fn proof_readout_from_sweep(sweep: &feature::AcceptanceSweepReport) -> ProofReadout {
    let gaps = sweep
        .items
        .iter()
        .filter(|item| matches!(item.proof, AcceptanceProof::Missing))
        .map(|item| ProofGap {
            ac_id: item.ac_id.clone(),
            text: item.text.clone(),
            owner_surface: "feature_proof".to_string(),
        })
        .collect::<Vec<_>>();
    let total = sweep.items.len();
    let incomplete = gaps.len();
    ProofReadout {
        total,
        complete: total.saturating_sub(incomplete),
        partial: 0,
        incomplete,
        gaps,
    }
}

fn friction_readout(paths: &MaestroPaths) -> FrictionReadout {
    let mut diagnostics = Vec::new();
    let ux_gap_entries = match fs::read_to_string(paths.repo_root().join("UX_GAPS.md")) {
        Ok(raw) => raw
            .lines()
            .filter(|line| line.trim_start().starts_with("- Surface:"))
            .count(),
        Err(error) if error.kind() == ErrorKind::NotFound => 0,
        Err(error) => {
            diagnostics.push(format!("UX_GAPS.md unreadable: {error}"));
            0
        }
    };
    let items = match harness::backlog::load(paths) {
        Ok(backlog) => backlog.items,
        Err(error) => {
            diagnostics.push(format!("harness backlog unreadable: {error}"));
            Vec::new()
        }
    };
    let recurring_items = items
        .iter()
        .filter(|item| item.occurrences > 1 || item.sessions_hit.len() > 1)
        .map(|item| FrictionItem {
            id: item.id.clone(),
            title: item.title.clone(),
            occurrences: item.occurrences,
            sessions: item.sessions_hit.len(),
        })
        .collect();

    FrictionReadout {
        ux_gap_entries,
        harness_backlog_items: items.len(),
        recurring_items,
        diagnostics,
    }
}

fn context_readout(
    paths: &MaestroPaths,
    target: Option<&str>,
    feature: &FeatureReadout,
    proof: &ProofReadout,
) -> Vec<ContextReadout> {
    let mut context = vec![ContextReadout {
        name: "harness".to_string(),
        required: true,
        status: if paths.harness_dir().join("HARNESS.md").is_file() {
            ContextStatus::Present
        } else {
            ContextStatus::Missing
        },
        evidence: paths.harness_dir().join("HARNESS.md").display().to_string(),
    }];

    match feature {
        FeatureReadout::Present {
            id,
            acceptance_count,
        } => {
            context.push(ContextReadout {
                name: "feature".to_string(),
                required: true,
                status: ContextStatus::Present,
                evidence: id.clone(),
            });
            context.push(ContextReadout {
                name: "acceptance".to_string(),
                required: true,
                status: if *acceptance_count == 0 {
                    ContextStatus::Missing
                } else {
                    ContextStatus::Present
                },
                evidence: format!("{acceptance_count} acceptance item(s)"),
            });
        }
        FeatureReadout::Missing { id, error } => {
            context.push(ContextReadout {
                name: "feature".to_string(),
                required: target.is_some(),
                status: ContextStatus::Missing,
                evidence: format!("{id}: {error}"),
            });
            context.push(ContextReadout {
                name: "acceptance".to_string(),
                required: target.is_some(),
                status: ContextStatus::Unverified,
                evidence: "feature unavailable".to_string(),
            });
        }
        FeatureReadout::NotRequested => {
            context.push(ContextReadout {
                name: "feature".to_string(),
                required: false,
                status: ContextStatus::Unverified,
                evidence: "no feature target supplied".to_string(),
            });
            context.push(ContextReadout {
                name: "acceptance".to_string(),
                required: false,
                status: ContextStatus::Unverified,
                evidence: "no feature target supplied".to_string(),
            });
        }
    }

    context.push(ContextReadout {
        name: "proof".to_string(),
        required: target.is_some(),
        status: if proof.total == 0 {
            ContextStatus::Unverified
        } else if proof.incomplete == 0 {
            ContextStatus::Present
        } else {
            ContextStatus::Missing
        },
        evidence: format!(
            "{} complete, {} incomplete, {} total",
            proof.complete, proof.incomplete, proof.total
        ),
    });

    context
}

fn maturity_level(
    context: &[ContextReadout],
    proof: &ProofReadout,
    friction: &FrictionReadout,
) -> MaturityLevelReadout {
    let mut blocked = Vec::new();
    for item in context {
        if item.required && item.status != ContextStatus::Present {
            blocked.push(format!("context:{}", item.name));
        }
    }
    if proof.incomplete > 0 {
        blocked.push("proof_gaps".to_string());
    }
    if friction.ux_gap_entries > 0
        || !friction.recurring_items.is_empty()
        || !friction.diagnostics.is_empty()
    {
        blocked.push("friction".to_string());
    }

    let missing_non_proof_context = context
        .iter()
        .any(|item| item.name != "proof" && item.required && item.status == ContextStatus::Missing);
    let (level, rationale) = if missing_non_proof_context {
        (
            "L0 draft",
            "required context is missing, so the readout cannot claim a stable report",
        )
    } else if proof.incomplete > 0
        || friction.ux_gap_entries > 0
        || !friction.recurring_items.is_empty()
        || !friction.diagnostics.is_empty()
    {
        (
            "L1 report",
            "context is inspectable, but proof gaps or friction still block assisted readiness",
        )
    } else {
        (
            "L2 assisted",
            "context and proof are complete enough for assisted routing",
        )
    };

    MaturityLevelReadout {
        level: level.to_string(),
        rationale: rationale.to_string(),
        blocked_from_next_level: blocked,
    }
}

fn next_owner(
    context: &[ContextReadout],
    proof: &ProofReadout,
    friction: &FrictionReadout,
    target: Option<&str>,
) -> NextOwnerReadout {
    if context
        .iter()
        .any(|item| item.name == "harness" && item.status == ContextStatus::Missing)
    {
        return NextOwnerReadout {
            surface: "harness".to_string(),
            command: "maestro init --yes".to_string(),
        };
    }
    if proof.incomplete > 0
        && let Some(target) = target
    {
        return NextOwnerReadout {
            surface: "feature_proof".to_string(),
            command: format!("maestro feature prepare {target} --draft"),
        };
    }
    if friction.ux_gap_entries > 0
        || !friction.recurring_items.is_empty()
        || !friction.diagnostics.is_empty()
    {
        return NextOwnerReadout {
            surface: "harness_backlog".to_string(),
            command: "maestro harness list".to_string(),
        };
    }
    NextOwnerReadout {
        surface: "status".to_string(),
        command: "maestro status".to_string(),
    }
}
