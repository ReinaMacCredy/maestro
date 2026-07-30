use std::collections::BTreeMap;

use serde_json::Value;

const ACTIONS: &str =
    include_str!("../contracts/vnext/catalogs/generated/catalog-09-action-spec.json");
const CEREMONIES: &str =
    include_str!("../contracts/vnext/catalogs/generated/catalog-05-ceremony.json");
const SELECTIONS: &str =
    include_str!("../contracts/vnext/public/recipe_selection_application_vectors.v1.json");
const CLOSURE: &str = include_str!("fixtures/vnext/stage6/closure.v1.json");

fn rows(document: &Value) -> &[Value] {
    document["descriptors"].as_array().expect("descriptor rows")
}

#[test]
fn frozen_operation_catalog_closes_every_tag_and_outcome() {
    let actions: Value = serde_json::from_str(ACTIONS).expect("actions");
    let ceremonies: Value = serde_json::from_str(CEREMONIES).expect("ceremonies");
    let closure: Value = serde_json::from_str(CLOSURE).expect("closure");
    let action_rows = rows(&actions);
    let ceremony_rows = rows(&ceremonies);
    let action_range = closure["action_tag_range"].as_array().expect("range");
    let ceremony_range = closure["ceremony_tag_range"].as_array().expect("range");

    assert_eq!(action_rows.len(), 145);
    assert_eq!(ceremony_rows.len(), 11);
    assert_eq!(
        action_rows
            .iter()
            .map(|row| row["value"][0].as_u64().expect("tag"))
            .collect::<Vec<_>>(),
        (action_range[0].as_u64().unwrap()..=action_range[1].as_u64().unwrap()).collect::<Vec<_>>()
    );
    assert_eq!(
        ceremony_rows
            .iter()
            .map(|row| row["value"][0].as_u64().expect("tag"))
            .collect::<Vec<_>>(),
        (ceremony_range[0].as_u64().unwrap()..=ceremony_range[1].as_u64().unwrap())
            .collect::<Vec<_>>()
    );
    let expected_outcomes = closure["action_outcomes"].as_array().expect("outcomes");
    for row in action_rows {
        let value = row["value"].as_array().expect("action descriptor");
        let outcomes = &value[value.len() - 2];
        assert_eq!(
            outcomes
                .as_array()
                .expect("outcomes")
                .iter()
                .map(|outcome| outcome[1].clone())
                .collect::<Vec<_>>(),
            *expected_outcomes
        );
    }
    let expected_modes = closure["ceremony_request_modes"].as_array().expect("modes");
    for row in ceremony_rows {
        assert_eq!(
            row["value"][3].as_array().expect("request modes"),
            expected_modes
        );
    }
}

#[test]
fn recipe_discovery_and_provenance_close_all_30_products() {
    let selections: Value = serde_json::from_str(SELECTIONS).expect("selections");
    let closure: Value = serde_json::from_str(CLOSURE).expect("closure");
    let vectors = selections["vectors"].as_array().expect("vectors");
    assert_eq!(
        vectors.len() as u64,
        closure["selection_option_count"].as_u64().unwrap()
    );
    let mut cardinality = BTreeMap::new();
    for vector in vectors {
        let count = vector["packet_recipe_binding_fixture"]["component_provenance"]
            .as_array()
            .expect("component provenance")
            .len();
        *cardinality.entry(count.to_string()).or_insert(0usize) += 1;
    }
    assert_eq!(
        cardinality["0"] as u64,
        closure["component_provenance_cardinality"]["0"]
    );
    assert_eq!(
        cardinality["1"] as u64,
        closure["component_provenance_cardinality"]["1"]
    );
    assert_eq!(
        cardinality["2"] as u64,
        closure["component_provenance_cardinality"]["2"]
    );
}

#[test]
fn packet_and_result_unions_remain_exact() {
    let closure: Value = serde_json::from_str(CLOSURE).expect("closure");
    assert_eq!(
        closure["packet_read_outcomes"],
        serde_json::json!([
            "Packet",
            "SelectionContext",
            "NoActiveStore",
            "Unavailable",
            "Stale",
            "Incompatible"
        ])
    );
    assert_eq!(
        closure["action_outcomes"],
        serde_json::json!([
            "committed",
            "no_op",
            "rejected",
            "stale",
            "conflict",
            "unavailable",
            "in_doubt"
        ])
    );
}
