//! Proof freshness helpers.

use serde::{Deserialize, Serialize};

/// Current proof inputs used to decide whether a stored verification is fresh.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FreshnessInputs {
    pub commit: Option<String>,
    pub task_contract_hash: String,
    pub acceptance_hash: String,
    pub checks_hash: String,
}

/// Stored proof inputs from a previous verification artifact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoredFreshness {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_commit: Option<String>,
    pub task_contract_hash: String,
    pub acceptance_hash: String,
    pub checks_hash: String,
}

/// A concrete reason a stored proof no longer matches current inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaleReason {
    pub field: &'static str,
    pub expected: String,
    pub actual: String,
}

/// Return all mismatches between current proof inputs and a stored proof.
pub fn stale_reasons(current: &FreshnessInputs, stored: &StoredFreshness) -> Vec<StaleReason> {
    let mut reasons = Vec::new();
    push_if_changed(
        &mut reasons,
        "verified_commit",
        current.commit.as_deref().unwrap_or("<none>"),
        stored.verified_commit.as_deref().unwrap_or("<none>"),
    );
    push_if_changed(
        &mut reasons,
        "task_contract_hash",
        &current.task_contract_hash,
        &stored.task_contract_hash,
    );
    push_if_changed(
        &mut reasons,
        "acceptance_hash",
        &current.acceptance_hash,
        &stored.acceptance_hash,
    );
    push_if_changed(
        &mut reasons,
        "checks_hash",
        &current.checks_hash,
        &stored.checks_hash,
    );
    reasons
}

fn push_if_changed(
    reasons: &mut Vec<StaleReason>,
    field: &'static str,
    current: &str,
    stored: &str,
) {
    if current != stored {
        reasons.push(StaleReason {
            field,
            expected: current.to_string(),
            actual: stored.to_string(),
        });
    }
}
