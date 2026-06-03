//! QA gate artifacts and predicates for the feature lifecycle (§4).
//!
//! Two agent-authored artifacts live in `.maestro/features/<id>/`:
//!
//! - `baseline.md` — the real-scenario behavior contract captured at `accept`
//!   (before edits, by construction). Optional `amend_log_position` frontmatter
//!   records which amend-log entry it was captured against; each Scenario Matrix
//!   entry carries a `[bl-NNN]` id, the **coverage unit**.
//! - `qa-slices.yaml` — append-only proven slices. A slice **counts** only when it
//!   references at least one `[bl-NNN]` scenario *and* carries non-empty evidence.
//!
//! The gates ([`baseline_present`] at `accept`, [`ship_qa_gaps`] at `ship`) are
//! pure functions over these artifacts so they unit-test in isolation; the
//! registry loads the inputs and renders the gaps.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::domain::feature::schema::{AmendEntry, AmendLog};
use crate::foundation::core::fs::read_to_string_if_exists;

/// The parsed `baseline.md`: the amend-log position it was captured against and
/// the set of `[bl-NNN]` scenario ids it declares (the ship-gate coverage units).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Baseline {
    /// Amend-log length at capture; freshness re-checks every entry at or after it.
    pub amend_log_position: usize,
    /// Normalized `bl-NNN` ids found in `[bl-NNN]` tokens, sorted and deduped.
    pub scenario_ids: BTreeSet<String>,
}

/// Append-only proven QA slices the ship gate reads (`qa-slices.yaml`). Only the
/// fields the gate consumes are modeled; any extra keys the skill documents
/// (`at`, `probes`, `result`) are ignored on parse.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct QaSliceLog {
    #[serde(default)]
    pub slices: Vec<QaSlice>,
}

/// One recorded slice. Counts toward coverage iff `scenarios` and `evidence` are
/// both non-empty.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub(crate) struct QaSlice {
    #[serde(default)]
    pub scenarios: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// Read `baseline.md`, returning `None` when absent or whitespace-only (the
/// fail-closed "no baseline" state the accept gate blocks on).
pub(crate) fn read_baseline(feature_dir: &Path) -> Result<Option<Baseline>> {
    let path = feature_dir.join("baseline.md");
    let Some(contents) = read_to_string_if_exists(&path)? else {
        return Ok(None);
    };
    if contents.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(Baseline {
        amend_log_position: parse_amend_log_position(&contents),
        scenario_ids: bracketed_bl_ids(&contents),
    }))
}

/// True when a non-empty `baseline.md` exists (the accept-gate precondition F).
pub(crate) fn baseline_present(feature_dir: &Path) -> Result<bool> {
    Ok(read_baseline(feature_dir)?.is_some())
}

/// Why a baseline is unusable, so the accept/ship gates word the remedy precisely:
/// a present-but-blank file reads `"empty"`, anything else (absent, or an IO error)
/// `"missing"` (fail-closed, matching `read_baseline`). Called only on the gate-fail
/// path, where a usable baseline never reaches it.
pub(crate) fn baseline_absence(feature_dir: &Path) -> &'static str {
    match read_to_string_if_exists(feature_dir.join("baseline.md")) {
        Ok(Some(text)) if text.trim().is_empty() => "empty",
        _ => "missing",
    }
}

/// Read `qa-slices.yaml`, returning an empty log when absent. A parse failure is
/// an actionable error naming the path and the expected shape (not an opaque bail).
pub(crate) fn read_qa_slices(feature_dir: &Path) -> Result<QaSliceLog> {
    let path = feature_dir.join("qa-slices.yaml");
    match read_to_string_if_exists(&path)? {
        None => Ok(QaSliceLog::default()),
        Some(contents) => serde_yaml::from_str(&contents).map_err(|err| {
            anyhow!(
                "failed to parse {}: {err}\n  expected shape:\n    slices:\n      - scenarios: [\"bl-001\"]\n        evidence: [\"<proof of the replayed scenario>\"]",
                path.display()
            )
        }),
    }
}

/// The ship-gate QA gaps for a feature: presence, freshness (E.1), and per-scenario
/// coverage (E.2, which subsumes the ≥1-proven-slice floor). Empty vec = QA clear.
///
/// - **Presence** — no baseline blocks (and short-circuits; freshness/coverage are
///   undefined without one).
/// - **Freshness (E.1)** — *unconditional* on the amend-log: a behavioral amend
///   (added acceptance or affected area) recorded at or after the baseline's
///   position means the baseline predates real behavior and must be refreshed.
/// - **Coverage (E.2)** — every `[bl-NNN]` in the baseline needs a counting slice;
///   a baseline with zero `[bl-NNN]` declares no behavioral surface (QA C skip).
pub(crate) fn ship_qa_gaps(
    id: &str,
    baseline: Option<&Baseline>,
    absence: &str,
    slices: &QaSliceLog,
    amend_log: &AmendLog,
) -> Vec<String> {
    let mut gaps = Vec::new();

    let Some(baseline) = baseline else {
        gaps.push(format!(
              "qa-baseline {absence} (.maestro/features/{id}/baseline.md)\n    skill: qa-baseline\n    target: .maestro/features/{id}/baseline.md\n    retry: maestro feature ship {id} --outcome \"<outcome>\""
          ));
        return gaps;
    };

    // E.1 freshness — re-check every amend at or after the recorded position.
    // An out-of-range position is treated as 0 (fail-closed: re-check all).
    let len = amend_log.entries.len();
    let position = if baseline.amend_log_position > len {
        0
    } else {
        baseline.amend_log_position
    };
    let behavioral_after = amend_log.entries[position..]
        .iter()
        .filter(|entry| is_behavioral(entry))
        .count();
    if behavioral_after > 0 {
        gaps.push(format!(
              "qa-baseline stale — {behavioral_after} behavioral amend(s) since capture; set amend_log_position: {len}\n    skill: qa-baseline\n    target: .maestro/features/{id}/baseline.md\n    retry: maestro feature ship {id} --outcome \"<outcome>\""
          ));
    }

    // E.2 coverage — every behavioral scenario needs a counting slice (subsumes
    // the floor). No `[bl-NNN]` = no behavioral surface declared (C skip).
    if !baseline.scenario_ids.is_empty() {
        let covered = covered_ids(slices);
        let uncovered: Vec<&str> = baseline
            .scenario_ids
            .iter()
            .filter(|scenario| !covered.contains(*scenario))
            .map(String::as_str)
            .collect();
        if !uncovered.is_empty() {
            gaps.push(format!(
                  "qa-slice coverage incomplete — {} baseline scenario(s) without a counting slice: {}\n    skill: qa-slice\n    target: .maestro/features/{id}/qa-slices.yaml\n    retry: maestro feature ship {id} --outcome \"<outcome>\"",
                  uncovered.len(),
                  uncovered.join(", ")
              ));
        }
    }

    gaps
}

/// A behavioral amend grew the proven surface (acceptance or affected area), so a
/// baseline captured before it is stale. Non-goal / open-question amends do not.
fn is_behavioral(entry: &AmendEntry) -> bool {
    !entry.added.acceptance.is_empty() || !entry.added.affected_areas.is_empty()
}

/// Union of `bl-NNN` ids across counting slices (scenarios + evidence non-empty).
fn covered_ids(slices: &QaSliceLog) -> BTreeSet<String> {
    slices
        .slices
        .iter()
        .filter(|slice| !slice.scenarios.is_empty() && !slice.evidence.is_empty())
        .flat_map(|slice| {
            slice
                .scenarios
                .iter()
                .filter_map(|raw| normalize_bl_id(raw))
        })
        .collect()
}

/// Parse the optional `amend_log_position` from a leading `---`-fenced YAML
/// frontmatter block; absent or malformed frontmatter yields 0 (fail-closed).
fn parse_amend_log_position(contents: &str) -> usize {
    #[derive(Default, Deserialize)]
    struct Frontmatter {
        #[serde(default)]
        amend_log_position: usize,
    }
    let Some(rest) = contents.strip_prefix("---\n") else {
        return 0;
    };
    let Some(end) = rest.find("\n---") else {
        return 0;
    };
    serde_yaml::from_str::<Frontmatter>(&rest[..end])
        .unwrap_or_default()
        .amend_log_position
}

/// Every `bl-NNN` id appearing inside square brackets (`[bl-001]` → `bl-001`).
/// Bracketing is the baseline convention, so prose mentions of `bl-` are ignored.
fn bracketed_bl_ids(contents: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for (idx, _) in contents.match_indices("[bl-") {
        let rest = &contents[idx + "[bl-".len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() && rest[digits.len()..].starts_with(']') {
            ids.insert(format!("bl-{digits}"));
        }
    }
    ids
}

/// Normalize a slice scenario reference (`bl-001` or `[bl-001]`) to its `bl-NNN`
/// id, so baseline and slice ids match regardless of bracketing. None if no id.
fn normalize_bl_id(raw: &str) -> Option<String> {
    let trimmed = raw
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let digits = trimmed.strip_prefix("bl-")?;
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| format!("bl-{digits}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::feature::schema::AmendAdditions;

    fn entry(acceptance: &[&str], areas: &[&str]) -> AmendEntry {
        AmendEntry {
            at: "t".to_string(),
            reason: "r".to_string(),
            added: AmendAdditions {
                acceptance: acceptance.iter().map(|s| s.to_string()).collect(),
                affected_areas: areas.iter().map(|s| s.to_string()).collect(),
                non_goals: Vec::new(),
                open_questions: Vec::new(),
            },
        }
    }

    fn baseline(position: usize, ids: &[&str]) -> Baseline {
        Baseline {
            amend_log_position: position,
            scenario_ids: ids.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn slice(scenarios: &[&str], evidence: &[&str]) -> QaSlice {
        QaSlice {
            scenarios: scenarios.iter().map(|s| s.to_string()).collect(),
            evidence: evidence.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn log(entries: Vec<AmendEntry>) -> AmendLog {
        AmendLog { entries }
    }

    #[test]
    fn missing_baseline_blocks_and_short_circuits() {
        let gaps = ship_qa_gaps(
            "demo",
            None,
            "missing",
            &QaSliceLog::default(),
            &log(vec![]),
        );
        assert_eq!(gaps.len(), 1);
        assert!(gaps[0].contains("qa-baseline missing"));
    }

    #[test]
    fn empty_baseline_words_the_gap_as_empty_not_missing() {
        let gaps = ship_qa_gaps("demo", None, "empty", &QaSliceLog::default(), &log(vec![]));
        assert_eq!(gaps.len(), 1);
        assert!(gaps[0].contains("qa-baseline empty"));
        assert!(!gaps[0].contains("qa-baseline missing"));
    }

    #[test]
    fn covered_behavioral_scenarios_ship() {
        let b = baseline(0, &["bl-001", "bl-002"]);
        let slices = QaSliceLog {
            slices: vec![slice(&["bl-001", "bl-002"], &["test passed"])],
        };
        assert!(ship_qa_gaps("demo", Some(&b), "missing", &slices, &log(vec![])).is_empty());
    }

    #[test]
    fn one_slice_does_not_cover_three_scenarios() {
        // The §1.3 hole: floor would pass, coverage must not.
        let b = baseline(0, &["bl-001", "bl-002", "bl-003"]);
        let slices = QaSliceLog {
            slices: vec![slice(&["bl-001"], &["proof"])],
        };
        let gaps = ship_qa_gaps("demo", Some(&b), "missing", &slices, &log(vec![]));
        assert_eq!(gaps.len(), 1);
        assert!(gaps[0].contains("bl-002"));
        assert!(gaps[0].contains("bl-003"));
        assert!(!gaps[0].contains("bl-001"));
    }

    #[test]
    fn slice_without_evidence_does_not_count() {
        let b = baseline(0, &["bl-001"]);
        let slices = QaSliceLog {
            slices: vec![slice(&["bl-001"], &[])],
        };
        let gaps = ship_qa_gaps("demo", Some(&b), "missing", &slices, &log(vec![]));
        assert!(gaps.iter().any(|g| g.contains("bl-001")));
    }

    #[test]
    fn no_behavioral_surface_ships_with_no_slices() {
        // QA C: zero [bl-NNN] declares no behavioral surface.
        let b = baseline(0, &[]);
        assert!(
            ship_qa_gaps(
                "demo",
                Some(&b),
                "missing",
                &QaSliceLog::default(),
                &log(vec![])
            )
            .is_empty()
        );
    }

    #[test]
    fn behavioral_amend_after_position_blocks_even_without_scenarios() {
        // Freshness is unconditional: a non-behavioral baseline + an area amend
        // still blocks (the baseline now needs a scenario).
        let b = baseline(0, &[]);
        let amend = log(vec![entry(&[], &["src/new.rs"])]);
        let gaps = ship_qa_gaps("demo", Some(&b), "missing", &QaSliceLog::default(), &amend);
        assert_eq!(gaps.len(), 1);
        assert!(gaps[0].contains("stale"));
    }

    #[test]
    fn non_behavioral_amend_does_not_block() {
        let b = baseline(0, &["bl-001"]);
        let slices = QaSliceLog {
            slices: vec![slice(&["bl-001"], &["proof"])],
        };
        let amend = log(vec![AmendEntry {
            at: "t".to_string(),
            reason: "r".to_string(),
            added: AmendAdditions {
                non_goals: vec!["out of scope".to_string()],
                ..Default::default()
            },
        }]);
        assert!(ship_qa_gaps("demo", Some(&b), "missing", &slices, &amend).is_empty());
    }

    #[test]
    fn amend_before_position_is_already_folded_in() {
        // position past the behavioral amend → it predates capture, no block.
        let b = baseline(1, &["bl-001"]);
        let slices = QaSliceLog {
            slices: vec![slice(&["bl-001"], &["proof"])],
        };
        let amend = log(vec![entry(&["new criterion"], &[])]);
        assert!(ship_qa_gaps("demo", Some(&b), "missing", &slices, &amend).is_empty());
    }

    #[test]
    fn out_of_range_position_re_checks_all_amends() {
        let b = baseline(99, &["bl-001"]);
        let slices = QaSliceLog {
            slices: vec![slice(&["bl-001"], &["proof"])],
        };
        let amend = log(vec![entry(&["new criterion"], &[])]);
        let gaps = ship_qa_gaps("demo", Some(&b), "missing", &slices, &amend);
        assert!(gaps.iter().any(|g| g.contains("stale")));
    }

    #[test]
    fn bracketed_ids_parse_and_prose_bl_is_ignored() {
        let body = "Scenario Matrix:\n- [bl-001] first\n- [bl-2] second\nnote: bl-999 in prose\n";
        let ids = bracketed_bl_ids(body);
        assert_eq!(
            ids,
            ["bl-001", "bl-2"].iter().map(|s| s.to_string()).collect()
        );
    }

    #[test]
    fn slice_ids_normalize_across_bracketing() {
        assert_eq!(normalize_bl_id("[bl-001]"), Some("bl-001".to_string()));
        assert_eq!(normalize_bl_id(" bl-001 "), Some("bl-001".to_string()));
        assert_eq!(normalize_bl_id("nope"), None);
        assert_eq!(normalize_bl_id("bl-"), None);
    }

    #[test]
    fn frontmatter_position_parses_and_defaults() {
        assert_eq!(
            parse_amend_log_position("---\namend_log_position: 3\n---\nbody"),
            3
        );
        assert_eq!(parse_amend_log_position("no frontmatter"), 0);
        assert_eq!(parse_amend_log_position("---\nother: 1\n---\n"), 0);
    }
}
