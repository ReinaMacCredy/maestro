//! Schema-pack validator: proves the embedded packs match the Rust constants.
//!
//! The packs are the reviewable contract; this validator is what makes them
//! trustworthy. It ties every pack's current stamp to the constant the Rust
//! model actually writes, keeps read/write/legacy/retired sets coherent,
//! enforces the retired-name never-reuse rule across all families, and checks
//! every shipped fixture against its contract's field list. An integration
//! test asserts [`violations`] is empty, so a drifting pack fails CI.

use serde_yaml::Value as YamlValue;

use crate::foundation::core::schema::{
    CARD_SCHEMA_VERSION, EVENT_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION,
    GLOBAL_SKILLS_LOCK_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION, INSTALL_LOCK_SCHEMA_VERSION,
    RUN_EVIDENCE_SCHEMA_VERSION, TASK_SCHEMA_VERSION, VERIFICATION_SCHEMA_VERSION,
};

use super::catalog::{ContractSpec, SchemaPack, VersionClass, pack, packs};

/// Family -> the current stamps its contracts must carry, straight from the
/// constants the Rust models write. Both directions are enforced: a pack for
/// an unlisted family and a listed family without a pack are violations.
const EXPECTED_CURRENT: &[(&str, &[&str])] = &[
    ("backlog", &[CARD_SCHEMA_VERSION]),
    ("card", &[CARD_SCHEMA_VERSION]),
    ("decision", &[CARD_SCHEMA_VERSION]),
    ("feature", &[FEATURE_SCHEMA_VERSION]),
    ("harness", &[HARNESS_SCHEMA_VERSION]),
    (
        "install",
        &[
            INSTALL_LOCK_SCHEMA_VERSION,
            GLOBAL_SKILLS_LOCK_SCHEMA_VERSION,
        ],
    ),
    ("proof", &[VERIFICATION_SCHEMA_VERSION]),
    ("run-event", &[EVENT_SCHEMA_VERSION]),
    ("run-evidence", &[RUN_EVIDENCE_SCHEMA_VERSION]),
    ("task", &[TASK_SCHEMA_VERSION]),
];

/// Every inconsistency between the shipped packs and the Rust constants.
pub fn violations() -> Vec<String> {
    let mut violations = Vec::new();

    for (family, _) in EXPECTED_CURRENT {
        if pack(family).is_none() {
            violations.push(format!("expected family {family} has no schema pack"));
        }
    }

    for pack in packs() {
        validate_pack(pack, &mut violations);
    }

    validate_retired_names(&mut violations);
    violations
}

fn validate_pack(pack: &'static SchemaPack, violations: &mut Vec<String>) {
    let family = pack.family;
    let Some((_, expected_versions)) = EXPECTED_CURRENT
        .iter()
        .find(|(expected, _)| *expected == family)
    else {
        violations.push(format!(
            "schema pack {family} is not an artifact-matrix family"
        ));
        return;
    };

    for (label, declared) in [
        ("current.yaml", pack.current.family.as_str()),
        ("supported.yaml", pack.supported.family.as_str()),
        ("retired.yaml", pack.retired.family.as_str()),
    ] {
        if declared != family {
            violations.push(format!(
                "{family}/{label} declares family {declared}, expected {family}"
            ));
        }
    }

    let stamps: Vec<&str> = pack
        .current
        .contracts
        .iter()
        .map(|contract| contract.schema_version.as_str())
        .collect();
    for expected in *expected_versions {
        if !stamps.contains(expected) {
            violations.push(format!(
                "{family}/current.yaml is missing a contract stamped {expected}"
            ));
        }
    }
    for stamp in &stamps {
        if !expected_versions.contains(stamp) {
            violations.push(format!(
                "{family}/current.yaml stamps {stamp}, which no Rust constant declares current"
            ));
        }
    }

    for contract in &pack.current.contracts {
        if contract.fields.is_empty() {
            violations.push(format!(
                "{family}/current.yaml contract {} has no fields",
                contract.name
            ));
        }
        if contract.stamp != "envelope"
            && !contract
                .fields
                .iter()
                .any(|field| field == "schema_version")
        {
            violations.push(format!(
                "{family}/current.yaml contract {} stamps its own document but omits \
                 schema_version from its field list",
                contract.name
            ));
        }
        if !pack.supported.read.contains(&contract.schema_version) {
            violations.push(format!(
                "{family}/supported.yaml read set is missing the current stamp {}",
                contract.schema_version
            ));
        }
    }

    if !pack.supported.read.contains(&pack.supported.write) {
        violations.push(format!(
            "{family}/supported.yaml writes {} but cannot read it back",
            pack.supported.write
        ));
    }

    for legacy in &pack.supported.legacy {
        if pack.supported.read.contains(&legacy.version) {
            violations.push(format!(
                "{family}/supported.yaml lists {} as both readable and legacy",
                legacy.version
            ));
        }
        if legacy.route.trim().is_empty() {
            violations.push(format!(
                "{family}/supported.yaml legacy version {} has no migrate route",
                legacy.version
            ));
        }
    }

    validate_fixtures(pack, violations);
}

/// The retired never-reuse rule, enforced across every family: a retired
/// version must not resurface as anyone's current/read/write/legacy version,
/// and a retired field must not return to its family's current field list.
fn validate_retired_names(violations: &mut Vec<String>) {
    for pack in packs() {
        for retired in &pack.retired.versions {
            for other in packs() {
                let mut reused = Vec::new();
                if other
                    .current
                    .contracts
                    .iter()
                    .any(|contract| contract.schema_version == *retired)
                {
                    reused.push("current");
                }
                if other
                    .supported
                    .read
                    .iter()
                    .any(|version| version == retired)
                {
                    reused.push("read");
                }
                if other.supported.write == *retired {
                    reused.push("write");
                }
                if other
                    .supported
                    .legacy
                    .iter()
                    .any(|legacy| legacy.version == *retired)
                {
                    reused.push("legacy");
                }
                if !reused.is_empty() {
                    violations.push(format!(
                        "version {retired} is retired by {} but reused by {} as {}",
                        pack.family,
                        other.family,
                        reused.join("+")
                    ));
                }
            }
        }
        for retired in &pack.retired.fields {
            for contract in &pack.current.contracts {
                if contract.fields.iter().any(|field| field == retired) {
                    violations.push(format!(
                        "field {retired} is retired by {} but still in contract {}",
                        pack.family, contract.name
                    ));
                }
            }
        }
    }
}

fn validate_fixtures(pack: &'static SchemaPack, violations: &mut Vec<String>) {
    let family = pack.family;
    if pack.fixtures.is_empty() {
        violations.push(format!("schema pack {family} ships no fixtures"));
        return;
    }

    let carriers: Vec<&str> = pack
        .current
        .contracts
        .iter()
        .map(|contract| contract.carrier.as_str())
        .collect();
    if carriers.windows(2).any(|pair| pair[0] != pair[1]) {
        violations.push(format!(
            "{family}/current.yaml mixes carriers {carriers:?}; fixtures cannot be matched"
        ));
        return;
    }

    for fixture in &pack.fixtures {
        let label = format!("{family}/{}", fixture.relative_path);
        let Some(contract) = pack.current.contracts.first() else {
            return;
        };
        match contract.carrier.as_str() {
            "file" => validate_file_fixture(pack, fixture.contents, &label, violations),
            "card-extra" => {
                validate_card_fixture(pack, fixture.contents, &label, true, violations);
            }
            "card-entry" => {
                validate_card_fixture(pack, fixture.contents, &label, false, violations);
            }
            "line" => validate_line_fixture(contract, fixture.contents, &label, violations),
            other => violations.push(format!("{label}: unknown carrier {other}")),
        }
    }

    for contract in &pack.current.contracts {
        if contract.carrier != "file" {
            continue;
        }
        let covered = pack.fixtures.iter().any(|fixture| {
            fixture_stamp(contract, fixture.contents) == Some(contract.schema_version.clone())
        });
        if !covered {
            violations.push(format!(
                "{family} contract {} has no fixture stamped {}",
                contract.name, contract.schema_version
            ));
        }
    }
}

/// A whole-document fixture: keys must stay inside the field list of the
/// contract its stamp selects, and the stamp must be in the read set.
fn validate_file_fixture(
    pack: &'static SchemaPack,
    contents: &[u8],
    label: &str,
    violations: &mut Vec<String>,
) {
    let Some(keys_and_stamp) = parse_document(pack, contents, label, violations) else {
        return;
    };
    let (keys, stamp) = keys_and_stamp;
    let Some(stamp) = stamp else {
        violations.push(format!("{label}: missing schema_version stamp"));
        return;
    };
    let Some(contract) = pack.contract_for_version(&stamp) else {
        violations.push(format!(
            "{label}: stamp {stamp} matches no current contract"
        ));
        return;
    };
    check_keys(&keys, contract, label, violations);
}

/// A card-carried fixture: the envelope obeys the card contract, the payload
/// under `extra` obeys this family's contract. A payload-stamped family
/// (`with_payload_stamp`) must classify as Supported or Legacy, never Unknown.
fn validate_card_fixture(
    pack: &'static SchemaPack,
    contents: &[u8],
    label: &str,
    with_payload_stamp: bool,
    violations: &mut Vec<String>,
) {
    let raw = match std::str::from_utf8(contents) {
        Ok(raw) => raw,
        Err(error) => {
            violations.push(format!("{label}: not UTF-8: {error}"));
            return;
        }
    };
    let doc: YamlValue = match serde_yaml::from_str(raw) {
        Ok(doc) => doc,
        Err(error) => {
            violations.push(format!("{label}: does not parse as YAML: {error}"));
            return;
        }
    };
    let Some(mapping) = doc.as_mapping() else {
        violations.push(format!("{label}: not a YAML mapping"));
        return;
    };

    let Some(card_pack) = super::catalog::pack("card") else {
        violations.push(format!(
            "{label}: no card pack to check the envelope against"
        ));
        return;
    };
    let Some(card_contract) = card_pack.current.contracts.first() else {
        violations.push(format!("{label}: card pack has no contract"));
        return;
    };
    let envelope_keys = mapping_keys(mapping, label, violations);
    check_keys(&envelope_keys, card_contract, label, violations);
    match string_entry(mapping, "schema_version") {
        Some(stamp) if card_pack.classify(&stamp) == VersionClass::Supported => {}
        Some(stamp) => violations.push(format!("{label}: envelope stamp {stamp} not supported")),
        None => violations.push(format!("{label}: missing envelope schema_version")),
    }

    let Some(contract) = pack.current.contracts.first() else {
        return;
    };
    let Some(extra) = mapping
        .get(YamlValue::String("extra".to_string()))
        .and_then(YamlValue::as_mapping)
    else {
        violations.push(format!("{label}: missing extra payload mapping"));
        return;
    };
    let payload_keys = mapping_keys(extra, label, violations);
    check_keys(&payload_keys, contract, label, violations);

    if with_payload_stamp {
        match string_entry(extra, "schema_version") {
            Some(stamp) if pack.classify(&stamp) != VersionClass::Unknown => {}
            Some(stamp) => violations.push(format!(
                "{label}: payload stamp {stamp} is neither supported nor a routed legacy version"
            )),
            None => violations.push(format!("{label}: missing payload schema_version")),
        }
    }
}

/// A JSONL fixture: every line is a self-stamped document.
fn validate_line_fixture(
    contract: &ContractSpec,
    contents: &[u8],
    label: &str,
    violations: &mut Vec<String>,
) {
    let raw = match std::str::from_utf8(contents) {
        Ok(raw) => raw,
        Err(error) => {
            violations.push(format!("{label}: not UTF-8: {error}"));
            return;
        }
    };
    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let line_label = format!("{label}:{}", index + 1);
        let object: serde_json::Value = match serde_json::from_str(line) {
            Ok(object) => object,
            Err(error) => {
                violations.push(format!("{line_label}: does not parse as JSON: {error}"));
                continue;
            }
        };
        let Some(object) = object.as_object() else {
            violations.push(format!("{line_label}: not a JSON object"));
            continue;
        };
        let keys: Vec<String> = object.keys().cloned().collect();
        check_keys(&keys, contract, &line_label, violations);
        match object
            .get("schema_version")
            .and_then(|value| value.as_str())
        {
            Some(stamp) if stamp == contract.schema_version => {}
            Some(stamp) => {
                violations.push(format!("{line_label}: stamp {stamp} is not current"));
            }
            None => violations.push(format!("{line_label}: missing schema_version stamp")),
        }
    }
}

/// Parse a whole-document fixture into its top-level keys and stamp.
fn parse_document(
    pack: &SchemaPack,
    contents: &[u8],
    label: &str,
    violations: &mut Vec<String>,
) -> Option<(Vec<String>, Option<String>)> {
    let raw = match std::str::from_utf8(contents) {
        Ok(raw) => raw,
        Err(error) => {
            violations.push(format!("{label}: not UTF-8: {error}"));
            return None;
        }
    };
    let format = pack
        .current
        .contracts
        .first()
        .map(|contract| contract.format.clone())
        .unwrap_or_default();
    if format == "json" {
        let doc: serde_json::Value = match serde_json::from_str(raw) {
            Ok(doc) => doc,
            Err(error) => {
                violations.push(format!("{label}: does not parse as JSON: {error}"));
                return None;
            }
        };
        let Some(object) = doc.as_object() else {
            violations.push(format!("{label}: not a JSON object"));
            return None;
        };
        let keys = object.keys().cloned().collect();
        let stamp = object
            .get("schema_version")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        return Some((keys, stamp));
    }
    let doc: YamlValue = match serde_yaml::from_str(raw) {
        Ok(doc) => doc,
        Err(error) => {
            violations.push(format!("{label}: does not parse as YAML: {error}"));
            return None;
        }
    };
    let Some(mapping) = doc.as_mapping() else {
        violations.push(format!("{label}: not a YAML mapping"));
        return None;
    };
    let keys = mapping_keys(mapping, label, violations);
    let stamp = string_entry(mapping, "schema_version");
    Some((keys, stamp))
}

/// The stamp a whole-document fixture carries, parsed by the contract's format.
fn fixture_stamp(contract: &ContractSpec, contents: &[u8]) -> Option<String> {
    let raw = std::str::from_utf8(contents).ok()?;
    if contract.format == "json" {
        let doc: serde_json::Value = serde_json::from_str(raw).ok()?;
        return doc
            .get("schema_version")
            .and_then(|value| value.as_str())
            .map(str::to_string);
    }
    let doc: YamlValue = serde_yaml::from_str(raw).ok()?;
    string_entry(doc.as_mapping()?, "schema_version")
}

fn check_keys(keys: &[String], contract: &ContractSpec, label: &str, violations: &mut Vec<String>) {
    for key in keys {
        if !contract.fields.iter().any(|field| field == key) {
            violations.push(format!(
                "{label}: key {key} is not in contract {}'s field list",
                contract.name
            ));
        }
    }
}

fn mapping_keys(
    mapping: &serde_yaml::Mapping,
    label: &str,
    violations: &mut Vec<String>,
) -> Vec<String> {
    let mut keys = Vec::new();
    for key in mapping.keys() {
        match key.as_str() {
            Some(key) => keys.push(key.to_string()),
            None => violations.push(format!("{label}: non-string mapping key {key:?}")),
        }
    }
    keys
}

fn string_entry(mapping: &serde_yaml::Mapping, key: &str) -> Option<String> {
    mapping
        .get(YamlValue::String(key.to_string()))
        .and_then(YamlValue::as_str)
        .map(str::to_string)
}
