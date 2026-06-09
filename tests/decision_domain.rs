mod card_support;
mod support;

use std::fs;

use card_support::{card_doc, cards_repo};
use maestro::decisions::schema::DecisionStatus;
use maestro::domain::decisions;
use maestro::foundation::core::fs::ensure_dir;
use maestro::foundation::core::paths::MaestroPaths;

#[test]
fn create_open_persists_first_global_decision() {
    let temp = cards_repo("maestro-decision-create");
    let paths = MaestroPaths::new(temp.path());

    let report = decisions::create_open(
        &paths,
        "Use single HARNESS.md",
        Some("too many files"),
        None,
    )
    .expect("invariant: create should succeed");

    assert!(
        report.record.id.starts_with("card-"),
        "card-mode decision id: {}",
        report.record.id
    );
    assert_eq!(report.record.status, DecisionStatus::Open);
    assert_eq!(report.record.context.as_deref(), Some("too many files"));
    assert!(
        card_doc(temp.path(), &report.record.id)
            .get("extra")
            .is_some(),
        "the global decision is persisted as a card with its record under extra"
    );
    let extra = card_doc(temp.path(), &report.record.id)["extra"].clone();
    assert_eq!(extra["status"], "open");
    assert_eq!(extra["context"], "too many files");
    assert!(
        !paths.decisions_file().is_file(),
        "card-mode creation must not write the legacy decisions.yaml store"
    );
}

#[test]
fn create_open_rejects_empty_slug_title() {
    let temp = cards_repo("maestro-decision-empty");
    let paths = MaestroPaths::new(temp.path());

    let err = decisions::create_open(&paths, "   ", None, None)
        .expect_err("empty-slug title must be rejected");
    assert!(
        err.to_string()
            .contains("at least one ASCII letter or digit"),
        "{err}"
    );
    assert!(
        fs::read_dir(paths.cards_dir())
            .expect("invariant: cards dir should be readable")
            .next()
            .is_none(),
        "a rejected title must not mint a card"
    );
}

#[test]
fn decision_exists_propagates_structured_store_errors() {
    let temp = cards_repo("maestro-decision-exists-error");
    let paths = MaestroPaths::new(temp.path());
    // A decision id normalizes straight to its card path, so a corrupt card.yaml
    // there is reached by the single-card load before the type check. The card
    // store rejects a wrong schema_version, so the lookup that gates supersede
    // validation and the frozen-legacy guard surfaces the error instead of
    // silently collapsing to false.
    ensure_dir(paths.cards_dir().join("decision-001"))
        .expect("invariant: card dir should be creatable");
    fs::write(
        paths.cards_dir().join("decision-001").join("card.yaml"),
        "schema_version: wrong.version\nid: decision-001\ntype: decision\ntitle: x\nstatus: open\ncreated_at: 1970-01-01T00:00:00Z\nupdated_at: 1970-01-01T00:00:00Z\n",
    )
    .expect("invariant: invalid decision card should be writable");

    let error = decisions::decision_exists(&paths, "decision-001")
        .expect_err("schema mismatch must not collapse to false");
    assert!(
        format!("{error:#}").contains("schema mismatch"),
        "{error:#}"
    );
}
