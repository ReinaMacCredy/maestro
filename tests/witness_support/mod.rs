use maestro::domain::card::schema::Card;
use maestro::domain::card::{self};
use maestro::domain::feature::schema::{FeatureRecord, FeatureStatus};
use maestro::domain::feature::{self};
use maestro::foundation::core::git;
use maestro::foundation::core::hash::sha256_hex;
use maestro::foundation::core::paths::MaestroPaths;

pub fn write_valid_witness(paths: &MaestroPaths, id: &str) {
    let record = read_feature_record(paths, id);
    let handoff = feature::read_sidecar_text(paths, id, "handoff.md")
        .expect("invariant: handoff should be readable")
        .expect("invariant: handoff should exist");
    let qa_ref = match feature::read_sidecar_text(paths, id, "qa.md")
        .expect("invariant: qa should be readable")
    {
        Some(qa) => format!("qa:{}", sha256_hex(qa.as_bytes())),
        None => {
            let qa_yaml =
                serde_yaml::to_string(&record.qa).expect("invariant: qa declaration serializes");
            format!("qa-declaration:{}", sha256_hex(qa_yaml.as_bytes()))
        }
    };
    let proof_yaml =
        serde_yaml::to_string(&(&record.acceptance_evidence, &record.acceptance_sweeps))
            .expect("invariant: proof anchors should serialize");
    let tree = git::head(paths.repo_root())
        .unwrap_or(None)
        .unwrap_or_else(|| "none".to_string());
    let acceptance_rows = record
        .acceptance
        .iter()
        .enumerate()
        .map(|(index, _)| format!("ac-{}: PASS\n", index + 1))
        .collect::<String>();
    let witness = format!(
        "# Witness Brief\n\
         gate: APPROVED\n\
         contract_ref: handoff:{}\n\
         proof_ref: proof:{}\n\
         qa_ref: {qa_ref}\n\
         tree_ref: git:{tree}\n\
         risk_tier: T1\n\
         acceptance_mapping_complete: true\n\
         proof_matrix_complete: true\n\
         {acceptance_rows}",
        sha256_hex(handoff.as_bytes()),
        sha256_hex(proof_yaml.as_bytes()),
    );
    feature::write_sidecar_text(paths, id, "witness.md", &witness)
        .expect("invariant: witness.md should be writable");
    let witness_ref = sha256_hex(witness.as_bytes());
    let advisor = format!(
        "# Advisor Receipt\n\
         verdict: APPROVE\n\
         reviewed_witness_ref: witness:{witness_ref}\n\
         worker_ref: worker:test-worker\n\
         advisor_ref: advisor:test-advisor\n\
         independent_session: true\n\
         acceptance_audit_complete: true\n\
         proof_spot_check_result: pass\n\
         confidence: H\n"
    );
    feature::write_sidecar_text(paths, id, "advisor.md", &advisor)
        .expect("invariant: advisor.md should be writable");
}

fn read_feature_record(paths: &MaestroPaths, id: &str) -> FeatureRecord {
    let resolved = card::store::resolve(paths, id)
        .expect("invariant: feature card should be resolvable")
        .expect("invariant: feature card should exist");
    let card = resolved.card;
    if card.extra.is_empty() {
        let mut record = FeatureRecord::proposed(&card.id, &card.title, &card.created_at);
        record.updated_at = card.updated_at;
        record.description = card.description;
        record.status = FeatureStatus::parse(&card.status).unwrap_or(FeatureStatus::Proposed);
        return record;
    }
    let Card {
        id,
        title,
        status,
        description,
        created_at,
        updated_at,
        mut extra,
        ..
    } = card;
    seed_yaml_string(&mut extra, "id", &id);
    seed_yaml_string(&mut extra, "title", &title);
    let record_status = FeatureStatus::parse(&status).unwrap_or(FeatureStatus::Proposed);
    seed_yaml_string(&mut extra, "status", record_status.as_str());
    if let Some(description) = description.as_deref() {
        seed_yaml_string(&mut extra, "description", description);
    }
    seed_yaml_string(&mut extra, "created_at", &created_at);
    seed_yaml_string(&mut extra, "updated_at", &updated_at);
    let mut record: FeatureRecord = serde_yaml::from_value(serde_yaml::Value::Mapping(extra))
        .expect("invariant: feature extra should parse");
    record.id = id;
    record.title = title;
    if let Some(mapped) = FeatureStatus::parse(&status) {
        record.status = mapped;
    }
    record.description = description;
    record
}

fn seed_yaml_string(map: &mut serde_yaml::Mapping, key: &str, value: &str) {
    let key = serde_yaml::Value::String(key.to_string());
    if !map.contains_key(&key) {
        map.insert(key, serde_yaml::Value::String(value.to_string()));
    }
}
