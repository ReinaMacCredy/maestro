use std::collections::BTreeMap;

use anyhow::{Context, Result};

use crate::domain::feature::registry;
use crate::domain::feature::schema::FeatureRecord;
use crate::foundation::core::git;
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;

const WITNESS_FILE: &str = "witness.md";
const ADVISOR_FILE: &str = "advisor.md";

pub(crate) fn close_gaps(
    paths: &MaestroPaths,
    id: &str,
    record: &FeatureRecord,
) -> Result<Vec<String>> {
    let Some(witness) = registry::read_sidecar_text(paths, id, WITNESS_FILE)? else {
        return Ok(vec![missing_witness_gap(id)]);
    };
    let fields = fields(&witness);
    if truthy(field(&fields, "skipped")) {
        return Ok(skip_gaps(id, &fields));
    }
    full_witness_gaps(paths, id, record, &witness, &fields)
}

fn full_witness_gaps(
    paths: &MaestroPaths,
    id: &str,
    record: &FeatureRecord,
    witness: &str,
    witness_fields: &BTreeMap<String, String>,
) -> Result<Vec<String>> {
    let mut gaps = Vec::new();
    let refs = current_refs(paths, id, record)?;
    require_equals(
        &mut gaps,
        "witness gate",
        field(witness_fields, "gate"),
        "APPROVED",
    );
    require_equals(
        &mut gaps,
        "witness contract_ref",
        field(witness_fields, "contract_ref"),
        &refs.contract_ref,
    );
    require_equals(
        &mut gaps,
        "witness proof_ref",
        field(witness_fields, "proof_ref"),
        &refs.proof_ref,
    );
    require_equals(
        &mut gaps,
        "witness qa_ref",
        field(witness_fields, "qa_ref"),
        &refs.qa_ref,
    );
    require_equals(
        &mut gaps,
        "witness tree_ref",
        field(witness_fields, "tree_ref"),
        &refs.tree_ref,
    );
    require_true(
        &mut gaps,
        "witness acceptance_mapping_complete",
        field(witness_fields, "acceptance_mapping_complete"),
    );
    require_true(
        &mut gaps,
        "witness proof_matrix_complete",
        field(witness_fields, "proof_matrix_complete"),
    );
    for index in 0..record.acceptance.len() {
        let ac_id = format!("ac-{}", index + 1);
        if !witness.contains(&ac_id) {
            gaps.push(format!("witness acceptance mapping missing {ac_id}"));
        }
    }

    let Some(advisor) = registry::read_sidecar_text(paths, id, ADVISOR_FILE)? else {
        gaps.push(format!(
            "advisor.md missing for witness approval\n    skill: maestro-witness\n    target: .maestro/cards/{id}/advisor.md"
        ));
        return Ok(render_gaps(id, gaps));
    };
    let advisor_fields = fields(&advisor);
    let witness_ref = format!("witness:{}", sha256_hex(witness.as_bytes()));
    gaps.extend(advisor_gaps(
        id,
        witness_fields,
        &advisor_fields,
        &witness_ref,
    ));
    Ok(render_gaps(id, gaps))
}

fn advisor_gaps(
    id: &str,
    witness_fields: &BTreeMap<String, String>,
    advisor_fields: &BTreeMap<String, String>,
    witness_ref: &str,
) -> Vec<String> {
    let mut gaps = Vec::new();
    require_equals(
        &mut gaps,
        "advisor verdict",
        field(advisor_fields, "verdict"),
        "APPROVE",
    );
    require_equals(
        &mut gaps,
        "advisor reviewed_witness_ref",
        field(advisor_fields, "reviewed_witness_ref"),
        witness_ref,
    );
    let worker_ref = field(advisor_fields, "worker_ref");
    let advisor_ref = field(advisor_fields, "advisor_ref");
    require_present(&mut gaps, "advisor worker_ref", worker_ref);
    require_present(&mut gaps, "advisor advisor_ref", advisor_ref);
    if worker_ref.is_some() && worker_ref == advisor_ref {
        gaps.push("advisor worker_ref and advisor_ref must be distinct".to_string());
    }
    require_true(
        &mut gaps,
        "advisor independent_session",
        field(advisor_fields, "independent_session"),
    );
    require_true(
        &mut gaps,
        "advisor acceptance_audit_complete",
        field(advisor_fields, "acceptance_audit_complete")
            .or_else(|| field(advisor_fields, "claim_audit_complete")),
    );
    require_equals(
        &mut gaps,
        "advisor proof_spot_check_result",
        field(advisor_fields, "proof_spot_check_result")
            .or_else(|| field(advisor_fields, "spot_checked_proof_result")),
        "pass",
    );
    risk_gaps(id, witness_fields, advisor_fields, &mut gaps);
    gaps
}

fn risk_gaps(
    id: &str,
    witness_fields: &BTreeMap<String, String>,
    advisor_fields: &BTreeMap<String, String>,
    gaps: &mut Vec<String>,
) {
    let risk = field(witness_fields, "risk_tier")
        .or_else(|| field(witness_fields, "tier"))
        .map(str::to_ascii_uppercase)
        .unwrap_or_default();
    match risk.as_str() {
        "T0" | "T1" => {}
        "T2" => {
            let demo_present = field(witness_fields, "demo_evidence").is_some();
            let waived = truthy(field(witness_fields, "demo_waived"));
            let reason_present = field(witness_fields, "demo_waiver_reason").is_some();
            if !(demo_present || waived && reason_present) {
                gaps.push(format!(
                    "witness risk tier T2 needs demo evidence or advisor waiver reason\n    gate: NEEDS_HUMAN_DEMO\n    retry: maestro feature close {id} --outcome \"<outcome>\""
                ));
            }
        }
        "T3" => {
            require_present(
                gaps,
                "witness demo_evidence for T3",
                field(witness_fields, "demo_evidence"),
            );
            let low_confidence = field(advisor_fields, "confidence")
                .map(|value| value.eq_ignore_ascii_case("L"))
                .unwrap_or(true);
            let lens_exceeded = truthy(field(advisor_fields, "advisor_lens_exceeded"));
            if (low_confidence || lens_exceeded)
                && !matches!(field(witness_fields, "expert_escalation"), Some(value) if value.eq_ignore_ascii_case("satisfied"))
            {
                gaps.push(format!(
                    "witness risk tier T3 needs expert escalation or waiver\n    gate: NEEDS_EXPERT\n    retry: maestro feature close {id} --outcome \"<outcome>\""
                ));
            }
        }
        _ => gaps
            .push("witness risk_tier missing or invalid; expected T0, T1, T2, or T3".to_string()),
    }
}

fn skip_gaps(id: &str, fields: &BTreeMap<String, String>) -> Vec<String> {
    let mut gaps = Vec::new();
    require_equals(
        &mut gaps,
        "witness skip tier",
        field(fields, "tier").or_else(|| field(fields, "risk_tier")),
        "T0",
    );
    require_equals(
        &mut gaps,
        "witness skipped_by",
        field(fields, "skipped_by"),
        "user",
    );
    require_present(
        &mut gaps,
        "witness user_authorization_ref",
        field(fields, "user_authorization_ref"),
    );
    require_present(
        &mut gaps,
        "witness skip_reason",
        field(fields, "skip_reason"),
    );
    require_present(
        &mut gaps,
        "witness changed_surface",
        field(fields, "changed_surface"),
    );
    render_gaps(id, gaps)
}

fn current_refs(paths: &MaestroPaths, id: &str, record: &FeatureRecord) -> Result<WitnessRefs> {
    let handoff = registry::read_sidecar_text(paths, id, "handoff.md")?
        .context("handoff.md missing while computing witness contract_ref")?;
    let proof_yaml =
        serde_yaml::to_string(&(&record.acceptance_evidence, &record.acceptance_sweeps))
            .context("failed to serialize feature proof anchors")?;
    let qa_ref = match registry::read_sidecar_text(paths, id, "qa.md")? {
        Some(contents) => format!("qa:{}", sha256_hex(contents.as_bytes())),
        None => {
            let qa_yaml = serde_yaml::to_string(&record.qa)
                .context("failed to serialize feature QA declaration")?;
            format!("qa-declaration:{}", sha256_hex(qa_yaml.as_bytes()))
        }
    };
    let tree_ref = git::head(paths.repo_root())
        .unwrap_or(None)
        .map(|head| format!("git:{head}"))
        .unwrap_or_else(|| "git:none".to_string());
    Ok(WitnessRefs {
        contract_ref: format!("handoff:{}", sha256_hex(handoff.as_bytes())),
        proof_ref: format!("proof:{}", sha256_hex(proof_yaml.as_bytes())),
        qa_ref,
        tree_ref,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WitnessRefs {
    contract_ref: String,
    proof_ref: String,
    qa_ref: String,
    tree_ref: String,
}

fn fields(contents: &str) -> BTreeMap<String, String> {
    contents
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_start_matches("- ").trim();
            let (key, value) = trimmed.split_once(':')?;
            let key = key.trim();
            if key.is_empty()
                || !key
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            {
                return None;
            }
            let value = value.trim().trim_matches('"').trim_matches('\'');
            (!value.is_empty()).then(|| {
                (
                    key.replace('-', "_").to_ascii_lowercase(),
                    value.to_string(),
                )
            })
        })
        .collect()
}

fn field<'a>(fields: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    fields.get(key).map(String::as_str)
}

fn truthy(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim),
        Some("true") | Some("yes") | Some("1") | Some("TRUE") | Some("YES")
    )
}

fn require_present(gaps: &mut Vec<String>, label: &str, value: Option<&str>) {
    if value.is_none() {
        gaps.push(format!("{label} missing"));
    }
}

fn require_true(gaps: &mut Vec<String>, label: &str, value: Option<&str>) {
    if !truthy(value) {
        gaps.push(format!("{label} must be true"));
    }
}

fn require_equals(gaps: &mut Vec<String>, label: &str, actual: Option<&str>, expected: &str) {
    match actual {
        Some(actual) if actual.eq_ignore_ascii_case(expected) => {}
        Some(actual) => gaps.push(format!("{label} expected {expected}, found {actual}")),
        None => gaps.push(format!("{label} missing; expected {expected}")),
    }
}

fn missing_witness_gap(id: &str) -> String {
    format!(
        "witness.md missing for feature close\n    skill: maestro-witness\n    target: .maestro/cards/{id}/witness.md\n    retry: maestro feature close {id} --outcome \"<outcome>\""
    )
}

fn render_gaps(id: &str, gaps: Vec<String>) -> Vec<String> {
    gaps.into_iter()
        .map(|gap| {
            if gap.contains("retry:") {
                gap
            } else {
                format!(
                    "{gap}\n    skill: maestro-witness\n    retry: maestro feature close {id} --outcome \"<outcome>\""
                )
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::domain::feature::schema::{
        AcceptanceEvidenceEntry, AcceptanceEvidenceKind, AcceptanceSweepRun, FeatureRecord,
        FeatureStatus,
    };
    use crate::domain::feature::witness::{close_gaps, current_refs};
    use crate::foundation::core::fs::ensure_dir;
    use crate::foundation::core::hash::sha256_hex;
    use crate::foundation::core::paths::MaestroPaths;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn paths(label: &str) -> MaestroPaths {
        let unique = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "maestro-witness-{label}-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        ensure_dir(&root).expect("temp root");
        MaestroPaths::new(root)
    }

    fn record(id: &str) -> FeatureRecord {
        let mut record = FeatureRecord::proposed(id, "Witness Feature", "2026-07-07T00:00:00.000Z");
        record.status = FeatureStatus::InProgress;
        record.acceptance = vec!["does thing".to_string()];
        record.acceptance_evidence = vec![AcceptanceEvidenceEntry {
            ac_id: "ac-1".to_string(),
            kind: AcceptanceEvidenceKind::Explicit,
            text: "fixture proof".to_string(),
            at: "2026-07-07T00:00:01.000Z".to_string(),
        }];
        record.acceptance_sweeps = vec![AcceptanceSweepRun {
            at: "2026-07-07T00:00:02.000Z".to_string(),
            resolved: vec!["ac-1".to_string()],
            unresolved: Vec::new(),
            invalidated_by: Vec::new(),
        }];
        record
    }

    fn write_sidecar(paths: &MaestroPaths, id: &str, name: &str, contents: &str) {
        let dir = paths.cards_dir().join(id);
        ensure_dir(&dir).expect("card dir");
        fs::write(dir.join(name), contents).expect("sidecar write");
    }

    fn seed_sidecars(paths: &MaestroPaths, id: &str) {
        write_sidecar(paths, id, "handoff.md", "# Handoff\n");
        write_sidecar(paths, id, "qa.md", "# QA\n");
    }

    fn valid_witness(paths: &MaestroPaths, id: &str, record: &FeatureRecord, risk: &str) -> String {
        let refs = current_refs(paths, id, record).expect("refs");
        format!(
            "# Witness Brief\n\
             gate: APPROVED\n\
             contract_ref: {}\n\
             proof_ref: {}\n\
             qa_ref: {}\n\
             tree_ref: {}\n\
             risk_tier: {risk}\n\
             acceptance_mapping_complete: true\n\
             proof_matrix_complete: true\n\
             demo_evidence: manual pass\n\
             ac-1: PASS\n",
            refs.contract_ref, refs.proof_ref, refs.qa_ref, refs.tree_ref
        )
    }

    fn valid_advisor(witness: &str) -> String {
        format!(
            "# Advisor Review\n\
             verdict: APPROVE\n\
             reviewed_witness_ref: witness:{}\n\
             worker_ref: session:worker\n\
             advisor_ref: subagent:advisor\n\
             independent_session: true\n\
             acceptance_audit_complete: true\n\
             proof_spot_check_result: pass\n\
             confidence: H\n",
            sha256_hex(witness.as_bytes())
        )
    }

    #[test]
    fn missing_witness_blocks_close() {
        let paths = paths("missing");
        let id = "witness-feature";
        let record = record(id);
        seed_sidecars(&paths, id);

        let gaps = close_gaps(&paths, id, &record).expect("gaps");

        assert!(
            gaps.iter().any(|gap| gap.contains("witness.md missing")),
            "{gaps:?}"
        );
    }

    #[test]
    fn valid_full_approval_has_no_gaps() {
        let paths = paths("valid");
        let id = "witness-feature";
        let record = record(id);
        seed_sidecars(&paths, id);
        let witness = valid_witness(&paths, id, &record, "T1");
        let advisor = valid_advisor(&witness);
        write_sidecar(&paths, id, "witness.md", &witness);
        write_sidecar(&paths, id, "advisor.md", &advisor);

        let gaps = close_gaps(&paths, id, &record).expect("gaps");

        assert!(gaps.is_empty(), "{gaps:?}");
    }

    #[test]
    fn invalid_advisor_receipt_blocks_close() {
        let paths = paths("advisor");
        let id = "witness-feature";
        let record = record(id);
        seed_sidecars(&paths, id);
        let witness = valid_witness(&paths, id, &record, "T1");
        let advisor = valid_advisor(&witness)
            .replace(
                "advisor_ref: subagent:advisor",
                "advisor_ref: session:worker",
            )
            .replace(
                "proof_spot_check_result: pass",
                "proof_spot_check_result: fail",
            );
        write_sidecar(&paths, id, "witness.md", &witness);
        write_sidecar(&paths, id, "advisor.md", &advisor);

        let gaps = close_gaps(&paths, id, &record).expect("gaps");

        assert!(
            gaps.iter().any(|gap| gap.contains("must be distinct")),
            "{gaps:?}"
        );
        assert!(
            gaps.iter()
                .any(|gap| gap.contains("proof_spot_check_result")),
            "{gaps:?}"
        );
    }

    #[test]
    fn stale_anchor_blocks_close() {
        let paths = paths("stale");
        let id = "witness-feature";
        let record = record(id);
        seed_sidecars(&paths, id);
        let witness = valid_witness(&paths, id, &record, "T1")
            .replace("contract_ref: handoff:", "contract_ref: handoff:stale");
        let advisor = valid_advisor(&witness);
        write_sidecar(&paths, id, "witness.md", &witness);
        write_sidecar(&paths, id, "advisor.md", &advisor);

        let gaps = close_gaps(&paths, id, &record).expect("gaps");

        assert!(
            gaps.iter().any(|gap| gap.contains("contract_ref")),
            "{gaps:?}"
        );
    }

    #[test]
    fn t0_skip_requires_user_authorization() {
        let paths = paths("skip");
        let id = "witness-feature";
        let record = record(id);
        seed_sidecars(&paths, id);
        write_sidecar(
            &paths,
            id,
            "witness.md",
            "skipped: true\nrisk_tier: T0\nskipped_by: agent\nskip_reason: rename\n",
        );

        let gaps = close_gaps(&paths, id, &record).expect("gaps");

        assert!(
            gaps.iter().any(|gap| gap.contains("skipped_by")),
            "{gaps:?}"
        );

        write_sidecar(
            &paths,
            id,
            "witness.md",
            "skipped: true\nrisk_tier: T0\nskipped_by: user\nuser_authorization_ref: msg-1\nskip_reason: rename\nchanged_surface: docs only\n",
        );
        let gaps = close_gaps(&paths, id, &record).expect("gaps");
        assert!(gaps.is_empty(), "{gaps:?}");
    }

    #[test]
    fn risk_policy_blocks_missing_demo_and_expert_escalation() {
        let paths = paths("risk");
        let id = "witness-feature";
        let record = record(id);
        seed_sidecars(&paths, id);
        let t2 =
            valid_witness(&paths, id, &record, "T2").replace("demo_evidence: manual pass\n", "");
        let advisor = valid_advisor(&t2);
        write_sidecar(&paths, id, "witness.md", &t2);
        write_sidecar(&paths, id, "advisor.md", &advisor);
        let gaps = close_gaps(&paths, id, &record).expect("gaps");
        assert!(
            gaps.iter().any(|gap| gap.contains("NEEDS_HUMAN_DEMO")),
            "{gaps:?}"
        );

        let t3 = valid_witness(&paths, id, &record, "T3");
        let advisor = valid_advisor(&t3).replace("confidence: H", "confidence: L");
        write_sidecar(&paths, id, "witness.md", &t3);
        write_sidecar(&paths, id, "advisor.md", &advisor);
        let gaps = close_gaps(&paths, id, &record).expect("gaps");
        assert!(
            gaps.iter().any(|gap| gap.contains("NEEDS_EXPERT")),
            "{gaps:?}"
        );
    }
}
