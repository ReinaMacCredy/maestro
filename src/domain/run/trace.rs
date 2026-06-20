//! Run-trace read model for `maestro query run`: reassemble a window of session
//! activity from the durable run log (`card_touch` + `task_proof` events) joined
//! to the current card store. Pure assembly over already-persisted state -- it
//! schedules nothing and starts nothing; Maestro stays the passive substrate.
//!
//! The signature signal is the crash-survival span: when one card's in-window
//! touches span more than one `session_id`, a later firing picked the card up
//! after an earlier one died, and the trace says so.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::Serialize;

use crate::domain::card;
use crate::domain::card::query::{Coarse, coarse_of};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::timestamp_nanos;

use super::reader::visit_managed_events;

/// The minimal event the fold needs, decoupled from the on-disk reader so the
/// assembly (window math, crash-span flag, TDD evidence) is testable with
/// synthetic input.
pub struct TraceEvent {
    pub kind: TraceEventKind,
    pub card_id: String,
    pub session_id: String,
    pub ts: String,
    /// Proof claims (`task_proof` only); empty otherwise.
    pub claims: Vec<String>,
    /// Proof summary line (`task_proof` `message`), when present.
    pub message: Option<String>,
}

/// The two run-log event kinds the trace reads; everything else is ignored.
#[derive(PartialEq)]
pub enum TraceEventKind {
    Touch,
    Proof,
}

/// Current store facts for a touched card, resolved by the caller so the join is
/// testable without a real store.
#[derive(Clone)]
pub struct CardFacts {
    pub title: String,
    pub status: String,
    /// Open `blocks`-edge target ids (a card whose target is not closed).
    pub blocked_by: Vec<String>,
}

/// One card's activity inside the trace window, joined to its current state.
#[derive(Debug, Serialize)]
pub struct TraceEntry {
    pub card_id: String,
    pub title: String,
    pub status: String,
    /// TDD evidence derived from proof claims, e.g. `red->green`; `None` when no
    /// claim carries a marker.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tdd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_proof: Option<String>,
    /// Distinct sessions that touched this card in-window; `>1` is the crash span.
    pub session_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<String>,
    pub last_activity: String,
}

impl TraceEntry {
    /// True when the card was worked across more than one session -- a later
    /// firing resumed it after an earlier one died mid-card.
    pub fn resumed_across_sessions(&self) -> bool {
        self.session_count > 1
    }
}

/// The assembled trace: per-card entries newest-first, plus window metadata.
#[derive(Debug, Serialize)]
pub struct RunTrace {
    /// Distinct sessions seen anywhere in the window.
    pub session_count: usize,
    pub entries: Vec<TraceEntry>,
    /// The most recent activity timestamp across all entries, when any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<String>,
}

/// Per-card accumulator while folding events.
#[derive(Default)]
struct CardActivity {
    sessions: BTreeSet<String>,
    last: Option<String>,
    claims: Vec<String>,
    /// (ts, message) of the most recent proof carrying a message.
    latest_proof: Option<(String, String)>,
}

/// Assemble a run trace from a stream of trace events and a card-facts resolver.
/// Pure: callers supply the events and the store lookup, so the window cutoff,
/// the crash-span flag, and the TDD-evidence derivation are testable off disk.
/// Events with an unparseable timestamp, or older than `cutoff_nanos`, are
/// dropped.
pub fn assemble(
    events: impl IntoIterator<Item = TraceEvent>,
    resolve: impl Fn(&str) -> Option<CardFacts>,
    cutoff_nanos: i128,
) -> RunTrace {
    let mut activity: BTreeMap<String, CardActivity> = BTreeMap::new();
    let mut sessions = BTreeSet::new();

    for event in events {
        let Some(ts_nanos) = timestamp_nanos(&event.ts) else {
            continue;
        };
        if ts_nanos < cutoff_nanos {
            continue;
        }
        sessions.insert(event.session_id.clone());
        let entry = activity.entry(event.card_id.clone()).or_default();
        entry.sessions.insert(event.session_id.clone());
        if entry.last.as_deref().is_none_or(|last| event.ts.as_str() > last) {
            entry.last = Some(event.ts.clone());
        }
        if event.kind == TraceEventKind::Proof {
            entry.claims.extend(event.claims);
            if let Some(message) = event.message {
                let newer = entry
                    .latest_proof
                    .as_ref()
                    .is_none_or(|(ts, _)| event.ts.as_str() > ts.as_str());
                if newer {
                    entry.latest_proof = Some((event.ts.clone(), message));
                }
            }
        }
    }

    let mut entries: Vec<TraceEntry> = activity
        .into_iter()
        .map(|(card_id, act)| {
            let facts = resolve(&card_id);
            let last_activity = act.last.unwrap_or_default();
            TraceEntry {
                title: facts
                    .as_ref()
                    .map(|f| f.title.clone())
                    .unwrap_or_else(|| "(not in store)".to_string()),
                status: facts
                    .as_ref()
                    .map(|f| f.status.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                tdd: tdd_evidence(&act.claims),
                latest_proof: act.latest_proof.map(|(_, message)| message),
                session_count: act.sessions.len(),
                blocked_by: facts.map(|f| f.blocked_by).unwrap_or_default(),
                last_activity,
                card_id,
            }
        })
        .collect();

    // Newest activity first; the id breaks ties so the order is deterministic.
    entries.sort_by(|a, b| {
        b.last_activity
            .cmp(&a.last_activity)
            .then(a.card_id.cmp(&b.card_id))
    });

    let last_activity = entries.first().map(|e| e.last_activity.clone());
    RunTrace {
        session_count: sessions.len(),
        entries,
        last_activity,
    }
}

/// Read the durable run log and the card store, returning the assembled trace
/// for every card touched at or after `cutoff_nanos`.
pub fn assemble_trace(paths: &MaestroPaths, cutoff_nanos: i128) -> Result<RunTrace> {
    let mut events: Vec<TraceEvent> = Vec::new();
    visit_managed_events(paths, |record| {
        let session_id = record.session_id().to_string();
        let event = record.event();
        let kind = event.event_type().or_else(|| event.alias_kind());
        let trace_kind = match kind {
            Some("card_touch") => TraceEventKind::Touch,
            Some("task_proof") => TraceEventKind::Proof,
            _ => return Ok(()),
        };
        let card_id = match event.card_id().or_else(|| event.task_id()) {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => return Ok(()),
        };
        let Some(ts) = event.timestamp() else {
            return Ok(());
        };
        events.push(TraceEvent {
            kind: trace_kind,
            card_id,
            session_id,
            ts: ts.to_string(),
            claims: event.claims(),
            message: event.message().map(str::to_string),
        });
        Ok(())
    })?;

    let scan = card::query::scan_with_failures(paths)?;
    let by_id: std::collections::HashMap<&str, &card::schema::Card> =
        scan.cards.iter().map(|(c, _)| (c.id.as_str(), c)).collect();

    let resolve = |id: &str| -> Option<CardFacts> {
        if let Some(card) = by_id.get(id) {
            return Some(facts_from_card(card, &by_id));
        }
        // A card archived mid-window still rendered: resolve it by id from the
        // archive tree (no blocker computation -- a terminal card blocks nothing).
        card::store::resolve_in(&paths.archive_cards_dir(), id)
            .ok()
            .flatten()
            .map(|resolved| CardFacts {
                title: resolved.card.title.clone(),
                status: resolved.card.status.clone(),
                blocked_by: Vec::new(),
            })
    };

    Ok(assemble(events, resolve, cutoff_nanos))
}

/// Build current facts for a touched card: a `blocks` edge is open when its
/// target is missing or not yet closed (the readiness rule's failing case).
fn facts_from_card(
    card: &card::schema::Card,
    by_id: &std::collections::HashMap<&str, &card::schema::Card>,
) -> CardFacts {
    let blocked_by = card
        .deps
        .iter()
        .filter(|dep| dep.kind.is_blocking())
        .filter(|dep| {
            !by_id
                .get(dep.target.as_str())
                .is_some_and(|target| coarse_of(&target.status) == Some(Coarse::Closed))
        })
        .map(|dep| dep.target.clone())
        .collect();
    CardFacts {
        title: card.title.clone(),
        status: card.status.clone(),
        blocked_by,
    }
}

/// TDD evidence from proof claims: the `RED`/`GREEN` markers the test-first flow
/// records, matched as standalone uppercase tokens so ordinary prose
/// ("greenfield") never triggers a false positive.
fn tdd_evidence(claims: &[String]) -> Option<String> {
    let red = claims_carry_marker(claims, "RED");
    let green = claims_carry_marker(claims, "GREEN");
    match (red, green) {
        (true, true) => Some("red->green".to_string()),
        (false, true) => Some("green".to_string()),
        _ if !claims.is_empty() => Some("proof recorded".to_string()),
        _ => None,
    }
}

fn claims_carry_marker(claims: &[String], marker: &str) -> bool {
    claims.iter().any(|claim| {
        claim
            .split(|c: char| !c.is_ascii_uppercase())
            .any(|token| token == marker)
    })
}

/// Activity older than this (a loop firing cadence) with workable cards still
/// open reads as an interruption, not a clean stop.
const STALE_AFTER_MINUTES: i64 = 15;

/// The honest run-status: current backlog state at trace time plus an
/// interruption inference. It deliberately never reports a graceful "why
/// stopped" reason -- the run log carries no terminal-reason event and a crash
/// fires no clean Stop, so asserting "backlog drained / done" while workable
/// cards remain would be the trust layer lying in exactly the failure case this
/// feature exists for. "drained" is reported only when `ready` is genuinely 0.
#[derive(Debug, Serialize)]
pub struct RunStatus {
    pub ready: usize,
    pub accepted_without_tasks: usize,
    pub proposed: usize,
    /// Minutes since the last in-window activity, when any activity exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes_since_last: Option<i64>,
}

impl RunStatus {
    /// The current-state-derived verdict. Keyed on the real `ready` count, never
    /// on an inferred stop reason.
    pub fn verdict(&self) -> String {
        let age = match self.minutes_since_last {
            Some(minutes) => format!("last activity {minutes}m ago"),
            None => "no activity in window".to_string(),
        };
        if self.ready > 0 {
            let stale = self
                .minutes_since_last
                .is_some_and(|minutes| minutes >= STALE_AFTER_MINUTES);
            if stale {
                return format!(
                    "{age}, {} ready card(s) remain -> likely interrupted",
                    self.ready
                );
            }
            return format!("{age}, {} ready card(s) remain", self.ready);
        }
        if self.accepted_without_tasks > 0 {
            return format!(
                "{age}, {} accepted feature(s) await prepare -> preparable work remains",
                self.accepted_without_tasks
            );
        }
        format!("{age}, no workable cards and nothing to prepare -> backlog drained")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(card: &str, session: &str, ts: &str) -> TraceEvent {
        TraceEvent {
            kind: TraceEventKind::Touch,
            card_id: card.to_string(),
            session_id: session.to_string(),
            ts: ts.to_string(),
            claims: Vec::new(),
            message: None,
        }
    }

    fn proof(card: &str, session: &str, ts: &str, claims: &[&str], message: &str) -> TraceEvent {
        TraceEvent {
            kind: TraceEventKind::Proof,
            card_id: card.to_string(),
            session_id: session.to_string(),
            ts: ts.to_string(),
            claims: claims.iter().map(|c| c.to_string()).collect(),
            message: Some(message.to_string()),
        }
    }

    fn facts(title: &str, status: &str) -> CardFacts {
        CardFacts {
            title: title.to_string(),
            status: status.to_string(),
            blocked_by: Vec::new(),
        }
    }

    #[test]
    fn one_card_touched_by_two_sessions_is_a_crash_span() {
        // The signature behavior dev sessions never exercise: one card, two
        // session ids spanning a mid-card death and resume.
        let events = vec![
            touch("card-a", "sess-B", "2026-06-20T01:00:00Z"),
            proof(
                "card-a",
                "sess-C",
                "2026-06-20T02:00:00Z",
                &["RED->GREEN reproduce then fix"],
                "cargo test => 12 passed",
            ),
        ];
        let trace = assemble(events, |_| Some(facts("Fix the bug", "verified")), 0);

        assert_eq!(trace.entries.len(), 1);
        let entry = &trace.entries[0];
        assert_eq!(entry.session_count, 2);
        assert!(entry.resumed_across_sessions(), "two sessions = crash span");
        assert_eq!(entry.status, "verified");
        assert_eq!(entry.tdd.as_deref(), Some("red->green"));
        assert_eq!(entry.latest_proof.as_deref(), Some("cargo test => 12 passed"));
        assert_eq!(trace.session_count, 2);
    }

    #[test]
    fn single_session_card_is_not_a_span() {
        let events = vec![touch("card-a", "sess-A", "2026-06-20T01:00:00Z")];
        let trace = assemble(events, |_| Some(facts("t", "in_progress")), 0);
        assert_eq!(trace.entries[0].session_count, 1);
        assert!(!trace.entries[0].resumed_across_sessions());
    }

    #[test]
    fn events_before_the_cutoff_are_dropped() {
        let cutoff = timestamp_nanos("2026-06-20T00:00:00Z").expect("parse cutoff");
        let events = vec![
            touch("old", "s", "2026-06-19T23:00:00Z"),
            touch("new", "s", "2026-06-20T05:00:00Z"),
        ];
        let trace = assemble(events, |_| Some(facts("t", "open")), cutoff);
        assert_eq!(trace.entries.len(), 1);
        assert_eq!(trace.entries[0].card_id, "new");
    }

    #[test]
    fn entries_are_newest_activity_first() {
        let events = vec![
            touch("early", "s", "2026-06-20T01:00:00Z"),
            touch("late", "s", "2026-06-20T09:00:00Z"),
            touch("mid", "s", "2026-06-20T05:00:00Z"),
        ];
        let trace = assemble(events, |_| Some(facts("t", "open")), 0);
        let order: Vec<&str> = trace.entries.iter().map(|e| e.card_id.as_str()).collect();
        assert_eq!(order, ["late", "mid", "early"]);
        assert_eq!(trace.last_activity.as_deref(), Some("2026-06-20T09:00:00Z"));
    }

    #[test]
    fn a_card_missing_from_the_store_renders_honestly() {
        let events = vec![touch("gone", "s", "2026-06-20T01:00:00Z")];
        let trace = assemble(events, |_| None, 0);
        assert_eq!(trace.entries[0].status, "unknown");
        assert_eq!(trace.entries[0].title, "(not in store)");
    }

    #[test]
    fn tdd_evidence_does_not_false_positive_on_prose() {
        assert_eq!(tdd_evidence(&["greenfield rollout, tested".to_string()]), Some("proof recorded".to_string()));
        assert_eq!(tdd_evidence(&["GREEN: suite passes".to_string()]), Some("green".to_string()));
        assert_eq!(tdd_evidence(&[]), None);
    }

    #[test]
    fn status_with_ready_cards_and_stale_activity_says_interrupted_not_drained() {
        // The trust-layer invariant: workable cards remain + activity went quiet
        // => interruption inference, NEVER a fabricated "backlog dry / done".
        let status = RunStatus {
            ready: 1,
            accepted_without_tasks: 0,
            proposed: 8,
            minutes_since_last: Some(40),
        };
        let verdict = status.verdict();
        assert!(verdict.contains("likely interrupted"), "{verdict}");
        assert!(!verdict.contains("drained"), "{verdict}");
    }

    #[test]
    fn status_with_ready_cards_but_recent_activity_omits_the_interruption_inference() {
        let status = RunStatus {
            ready: 2,
            accepted_without_tasks: 0,
            proposed: 0,
            minutes_since_last: Some(3),
        };
        let verdict = status.verdict();
        assert!(verdict.contains("2 ready card(s) remain"), "{verdict}");
        assert!(!verdict.contains("interrupted"), "{verdict}");
        assert!(!verdict.contains("drained"), "{verdict}");
    }

    #[test]
    fn status_reports_drained_only_when_ready_is_zero() {
        let drained = RunStatus {
            ready: 0,
            accepted_without_tasks: 0,
            proposed: 8,
            minutes_since_last: Some(120),
        };
        assert!(drained.verdict().contains("backlog drained"), "{}", drained.verdict());

        let preparable = RunStatus {
            ready: 0,
            accepted_without_tasks: 2,
            proposed: 0,
            minutes_since_last: Some(10),
        };
        let verdict = preparable.verdict();
        assert!(verdict.contains("await prepare"), "{verdict}");
        assert!(!verdict.contains("drained"), "{verdict}");
    }
}
