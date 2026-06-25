//! Cross-session liveness read model over the merged run-event logs.
//!
//! `maestro active` renders this; the model itself is pure. Given the managed
//! run logs and an injected `now`, it returns one row per session bucket with
//! that session's mode, bound card, last action, age, and presence label.
//! Liveness rests on the recency and kind of the last event, never on a daemon
//! (decision `dec-liveness-recency-of-last-event-activity-b77b`).

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Component, Path};

use anyhow::Result;

use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::timestamp_nanos;

use super::event::run_dir_name;
use super::reader::{RunEvent, visit_event_log, visit_managed_events};

/// Minutes within which the last event marks a session live. Tunable default
/// (the decision leaves thresholds to the implementation), not a locked value.
const LIVE_THRESHOLD_MINUTES: u64 = 5;
/// Minutes within which an idle session is still shown without `--all`.
const WINDOW_MINUTES: u64 = 30;

/// Presence label derived from the last event's age and kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Presence {
    /// Last event is a non-Stop event within the live threshold (mid-task).
    Working,
    /// Last event is a Stop within the live threshold (idling, open to coordinate).
    Waiting,
    /// Last event is older than the live threshold but within the window.
    Idle,
    /// Last event is beyond the window; hidden unless `--all`.
    Stale,
}

/// One session bucket's cross-session liveness summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionActivity {
    /// Logical session id (the run bucket).
    pub session_id: String,
    /// Agent runtime identity observed in this session, when known.
    pub agent_runtime: Option<String>,
    /// Skill from the session's latest `skill_activation`, when any.
    pub mode: Option<String>,
    /// Card id from the session's latest `card_touch`, when any.
    pub bound_card: Option<String>,
    /// Normalized event type of the session's latest event.
    pub last_action: String,
    /// Timestamp of the session's latest event.
    pub last_ts: String,
    /// Whole minutes between the latest event and `now`.
    pub age_minutes: u64,
    /// Presence label derived from `age_minutes` and `last_action`.
    pub presence: Presence,
}

/// One session bucket's most recent observations across the kinds we surface.
#[derive(Default)]
struct Accumulator {
    overall: Option<(i128, String, String)>,
    agent_runtime: Option<(i128, String)>,
    skill: Option<(i128, String)>,
    card: Option<(i128, String)>,
}

impl Accumulator {
    fn observe_overall(&mut self, ts_nanos: i128, event_type: &str, ts: &str) {
        if self
            .overall
            .as_ref()
            .is_none_or(|(seen, ..)| ts_nanos >= *seen)
        {
            self.overall = Some((ts_nanos, event_type.to_string(), ts.to_string()));
        }
    }

    fn observe_skill(&mut self, ts_nanos: i128, skill: &str) {
        if self
            .skill
            .as_ref()
            .is_none_or(|(seen, _)| ts_nanos >= *seen)
        {
            self.skill = Some((ts_nanos, skill.to_string()));
        }
    }

    fn observe_agent_runtime(&mut self, ts_nanos: i128, agent_runtime: &str) {
        if self
            .agent_runtime
            .as_ref()
            .is_none_or(|(seen, _)| ts_nanos >= *seen)
        {
            self.agent_runtime = Some((ts_nanos, agent_runtime.to_string()));
        }
    }

    fn observe_card(&mut self, ts_nanos: i128, card: &str) {
        if self.card.as_ref().is_none_or(|(seen, _)| ts_nanos >= *seen) {
            self.card = Some((ts_nanos, card.to_string()));
        }
    }
}

/// Build the cross-session liveness rows from the managed run logs as of `now`.
///
/// Returns every session bucket whose latest event has a parseable timestamp,
/// newest first, including stale rows so the caller can choose to hide them.
pub fn active_sessions(paths: &MaestroPaths, now: &str) -> Result<Vec<SessionActivity>> {
    active_sessions_union(std::slice::from_ref(paths), now)
}

/// Display/session key used by cross-worktree active rows. A single-root read
/// keeps the historical session id; a union qualifies the id with the source
/// worktree so same-named sessions never collapse into one row.
pub fn union_session_id(paths: &MaestroPaths, roots: &[MaestroPaths], session_id: &str) -> String {
    if roots.len() < 2 {
        return session_id.to_string();
    }
    format!("{session_id}@{}", root_label(paths.repo_root()))
}

/// Build the cross-session liveness rows as of `now`, unioned over every
/// worktree root. With a single root this is the local view; with the roots of
/// every worktree (`git::worktree_roots`), sessions from sibling worktrees merge
/// in as a read-only union. Cross-worktree session ids are source-qualified so
/// fallback or reused ids do not collapse distinct work. The union is read-only:
/// it never writes outside any root and needs no flag to engage.
pub fn active_sessions_union(roots: &[MaestroPaths], now: &str) -> Result<Vec<SessionActivity>> {
    let now_nanos = timestamp_nanos(now).unwrap_or(i128::MAX);
    let mut by_session: BTreeMap<String, Accumulator> = BTreeMap::new();

    for paths in roots {
        visit_managed_events(paths, |record| {
            let session_id = union_session_id(paths, roots, record.session_id());
            let event = record.event();
            let Some(ts) = event.timestamp() else {
                return Ok(());
            };
            let Some(ts_nanos) = timestamp_nanos(ts) else {
                return Ok(());
            };
            let event_type = event
                .event_type()
                .or_else(|| event.alias_kind())
                .unwrap_or("<unknown>");
            let acc = by_session.entry(session_id).or_default();
            acc.observe_overall(ts_nanos, event_type, ts);
            if let Some(agent_runtime) = event.agent_runtime() {
                acc.observe_agent_runtime(ts_nanos, agent_runtime);
            }
            if event.is_event_type("skill_activation")
                && let Some(skill) = event.skill_name()
            {
                acc.observe_skill(ts_nanos, skill);
            }
            if event.is_event_type("card_touch")
                && let Some(card) = event.card_id()
            {
                acc.observe_card(ts_nanos, card);
            }
            Ok(())
        })?;
    }

    let mut rows: Vec<(i128, SessionActivity)> = Vec::new();
    for (session_id, acc) in by_session {
        let Some((last_nanos, last_action, last_ts)) = acc.overall else {
            continue;
        };
        let age_minutes = age_minutes_between(last_nanos, now_nanos);
        let presence = classify(age_minutes, &last_action);
        rows.push((
            last_nanos,
            SessionActivity {
                session_id,
                agent_runtime: acc.agent_runtime.map(|(_, agent_runtime)| agent_runtime),
                mode: acc.skill.map(|(_, skill)| skill),
                bound_card: acc.card.map(|(_, card)| card),
                last_action,
                last_ts,
                age_minutes,
                presence,
            },
        ));
    }

    rows.sort_by_key(|(last_nanos, _)| Reverse(*last_nanos));
    Ok(rows.into_iter().map(|(_, row)| row).collect())
}

/// The card this single session is currently bound to: the `card_id` of the
/// last `card_touch` in its OWN run log. Reads one `events.jsonl` (not the full
/// run tree like [`active_sessions`]) so the inbox banner and `msg` verbs can
/// resolve "my current card" cheaply on every command. `None` when the session
/// has touched no card or its log is unreadable.
pub fn current_bound_card(paths: &MaestroPaths, session_id: &str) -> Result<Option<String>> {
    let path = paths
        .runs_dir()
        .join(run_dir_name(session_id))
        .join("events.jsonl");
    let mut latest: Option<String> = None;
    visit_event_log(&path, |record| {
        let event = record.event();
        if event.is_event_type("card_touch")
            && let Some(card) = event.card_id()
        {
            latest = Some(card.to_string());
        }
        Ok(())
    })?;
    Ok(latest)
}

/// Tools whose edits create a same-file write conflict worth flagging. A
/// concurrent Read of a file is not contention, so the overlap signal ignores
/// it even though `file_path` is recorded for every tool that carries one.
const WARM_EDIT_TOOLS: &[&str] = &["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// One live session warm-editing a shared file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WarmEditor {
    /// The peer session's run bucket.
    pub session_id: String,
    /// The card that session is bound to (its latest `card_touch`), when any.
    pub bound_card: Option<String>,
    /// Whole minutes since that session last edited the file.
    pub age_minutes: u64,
}

/// A file two or more live sessions are warm-editing in the same worktree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileOverlap {
    /// Repo-relative or absolute path as the agent passed it to Edit/Write.
    pub file_path: String,
    /// The contending editors, ordered by session id for a stable banner.
    pub editors: Vec<WarmEditor>,
}

/// A declared path scope claimed by two or more live sessions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredScopeOverlap {
    /// The contended repo-relative path or path pair.
    pub scope_path: String,
    /// The sessions whose declared scopes overlap, ordered by session id.
    pub owners: Vec<WarmEditor>,
}

/// Files that two or more live sessions are warm-editing in this worktree.
///
/// Reads the LOCAL run logs only -- never a cross-worktree union -- because the
/// overlap signal is about contention inside ONE shared folder
/// (`dec-default-active-flags-warm-file-overlap-51a9`). Worktree isolation, this
/// feature's own remedy, deliberately puts peers in separate folders where an
/// equal path is NOT a conflict, so unioning would false-alarm on the very
/// workflow the feature promotes. A warm edit is an Edit/Write within the live
/// threshold; warmth decays on that same clock with no acquire or release.
pub fn warm_file_overlaps(paths: &MaestroPaths, now: &str) -> Result<Vec<FileOverlap>> {
    let now_nanos = timestamp_nanos(now).unwrap_or(i128::MAX);

    let mut bound_card: BTreeMap<String, (i128, String)> = BTreeMap::new();
    // session -> file -> latest warm-edit ts.
    let mut warm: BTreeMap<String, BTreeMap<String, i128>> = BTreeMap::new();

    visit_managed_events(paths, |record| {
        let session_id = record.session_id().to_string();
        let event = record.event();
        let Some(ts) = event.timestamp() else {
            return Ok(());
        };
        let Some(ts_nanos) = timestamp_nanos(ts) else {
            return Ok(());
        };

        if event.is_event_type("card_touch")
            && let Some(card) = event.card_id()
        {
            let slot = bound_card
                .entry(session_id.clone())
                .or_insert((i128::MIN, String::new()));
            if ts_nanos >= slot.0 {
                *slot = (ts_nanos, card.to_string());
            }
        }

        if is_warm_edit(event)
            && let Some(file) = event.file_path()
        {
            let file = normalize_warm_file_path(paths, file);
            let latest = warm
                .entry(session_id)
                .or_default()
                .entry(file)
                .or_insert(i128::MIN);
            *latest = (*latest).max(ts_nanos);
        }
        Ok(())
    })?;

    // file -> editors whose latest edit is still within the live window.
    let mut by_file: BTreeMap<String, Vec<(String, u64)>> = BTreeMap::new();
    for (session, files) in &warm {
        for (file, ts_nanos) in files {
            let age = age_minutes_between(*ts_nanos, now_nanos);
            if age <= LIVE_THRESHOLD_MINUTES {
                by_file
                    .entry(file.clone())
                    .or_default()
                    .push((session.clone(), age));
            }
        }
    }

    let mut overlaps: Vec<FileOverlap> = Vec::new();
    for (file_path, mut editors) in by_file {
        if editors.len() < 2 {
            continue;
        }
        editors.sort_by(|left, right| left.0.cmp(&right.0));
        let editors = editors
            .into_iter()
            .map(|(session, age_minutes)| WarmEditor {
                bound_card: bound_card.get(&session).map(|(_, card)| card.clone()),
                session_id: session,
                age_minutes,
            })
            .collect();
        overlaps.push(FileOverlap { file_path, editors });
    }
    Ok(overlaps)
}

/// Declared path scopes that overlap across live sessions in one or more
/// worktrees. This is split-time advisory data: an orchestrator can declare the
/// intended repo-relative paths before agents edit, and the read model compares
/// those declarations without guessing from freeform feature areas.
#[cfg(test)]
pub fn declared_scope_overlaps_union(
    roots: &[MaestroPaths],
    now: &str,
) -> Result<Vec<DeclaredScopeOverlap>> {
    let active = active_sessions_union(roots, now)?;
    declared_scope_overlaps_for_active_union(roots, &active)
}

pub fn declared_scope_overlaps_for_active_union(
    roots: &[MaestroPaths],
    active: &[SessionActivity],
) -> Result<Vec<DeclaredScopeOverlap>> {
    let live: BTreeMap<String, WarmEditor> = active
        .iter()
        .filter(|row| row.presence != Presence::Stale)
        .map(|row| {
            (
                row.session_id.clone(),
                WarmEditor {
                    session_id: row.session_id.clone(),
                    bound_card: row.bound_card.clone(),
                    age_minutes: row.age_minutes,
                },
            )
        })
        .collect();

    let mut latest_scope: BTreeMap<String, (i128, Vec<String>)> = BTreeMap::new();
    for paths in roots {
        visit_managed_events(paths, |record| {
            let session_id = union_session_id(paths, roots, record.session_id());
            if !live.contains_key(&session_id) {
                return Ok(());
            }
            let event = record.event();
            let scopes = event.scope_paths();
            if scopes.is_empty() {
                return Ok(());
            }
            let Some(ts) = event.timestamp() else {
                return Ok(());
            };
            let Some(ts_nanos) = timestamp_nanos(ts) else {
                return Ok(());
            };
            let normalized = scopes
                .into_iter()
                .map(|scope| normalize_warm_file_path(paths, scope))
                .collect::<Vec<_>>();
            let slot = latest_scope
                .entry(session_id)
                .or_insert((i128::MIN, Vec::new()));
            if ts_nanos >= slot.0 {
                *slot = (ts_nanos, normalized);
            }
            Ok(())
        })?;
    }

    let mut by_overlap: BTreeMap<String, BTreeMap<String, WarmEditor>> = BTreeMap::new();
    let sessions: Vec<(&String, &Vec<String>)> = latest_scope
        .iter()
        .map(|(session, (_, scopes))| (session, scopes))
        .collect();
    for (left_index, (left_session, left_scopes)) in sessions.iter().enumerate() {
        for (right_session, right_scopes) in sessions.iter().skip(left_index + 1) {
            for left in left_scopes.iter() {
                for right in right_scopes.iter() {
                    if !scopes_overlap(left, right) {
                        continue;
                    }
                    let label = scope_overlap_label(left, right);
                    if let Some(owner) = live.get(*left_session) {
                        by_overlap
                            .entry(label.clone())
                            .or_default()
                            .insert((*left_session).clone(), owner.clone());
                    }
                    if let Some(owner) = live.get(*right_session) {
                        by_overlap
                            .entry(label)
                            .or_default()
                            .insert((*right_session).clone(), owner.clone());
                    }
                }
            }
        }
    }

    Ok(by_overlap
        .into_iter()
        .map(|(scope_path, owners)| DeclaredScopeOverlap {
            scope_path,
            owners: owners.into_values().collect(),
        })
        .collect())
}

fn is_warm_edit(event: &RunEvent) -> bool {
    event.is_event_type("PostToolUse")
        && event
            .tool_name()
            .is_some_and(|name| WARM_EDIT_TOOLS.contains(&name))
}

fn root_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn normalize_warm_file_path(paths: &MaestroPaths, file: &str) -> String {
    let trimmed = file.trim();
    let path = Path::new(trimmed);
    let relative = if path.is_absolute() {
        path.strip_prefix(paths.repo_root()).unwrap_or(path)
    } else {
        path
    };
    let mut parts: Vec<String> = Vec::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::ParentDir => match parts.last() {
                Some(last) if last != ".." => {
                    parts.pop();
                }
                _ => parts.push("..".to_string()),
            },
            Component::RootDir | Component::Prefix(_) => return trimmed.to_string(),
        }
    }
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn scopes_overlap(left: &str, right: &str) -> bool {
    left == right
        || left == "."
        || right == "."
        || is_path_prefix(left, right)
        || is_path_prefix(right, left)
}

fn is_path_prefix(parent: &str, child: &str) -> bool {
    child
        .strip_prefix(parent)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn scope_overlap_label(left: &str, right: &str) -> String {
    if left == right {
        left.to_string()
    } else if is_path_prefix(left, right) || left == "." {
        format!("{left} ↔ {right}")
    } else {
        format!("{right} ↔ {left}")
    }
}

fn age_minutes_between(then_nanos: i128, now_nanos: i128) -> u64 {
    const NANOS_PER_MINUTE: i128 = 60 * 1_000_000_000;
    let elapsed = (now_nanos - then_nanos).max(0);
    (elapsed / NANOS_PER_MINUTE) as u64
}

/// Apply the activity-aware liveness rule. A recent Stop means the session just
/// finished a turn and is waiting for its human, so it stays live as `Waiting`;
/// any other recent event is `Working`. This generalizes the locked decision's
/// Pre/PostToolUse example to every non-Stop kind (card_touch, skill_activation,
/// and the rest) so a session that only touched a card does not misread as idle.
fn classify(age_minutes: u64, last_action: &str) -> Presence {
    if age_minutes <= LIVE_THRESHOLD_MINUTES {
        if last_action == "Stop" {
            Presence::Waiting
        } else {
            Presence::Working
        }
    } else if age_minutes <= WINDOW_MINUTES {
        Presence::Idle
    } else {
        Presence::Stale
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    const NOW: &str = "2026-06-14T12:00:00.000Z";

    fn seed(root: &Path, session: &str, lines: &[&str]) {
        let run_dir = root.join(".maestro/runs").join(session);
        fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
        fs::write(
            run_dir.join("events.jsonl"),
            format!("{}\n", lines.join("\n")),
        )
        .expect("invariant: event log fixture should be writable");
    }

    fn row<'a>(rows: &'a [SessionActivity], session: &str) -> &'a SessionActivity {
        rows.iter()
            .find(|row| row.session_id == session)
            .unwrap_or_else(|| panic!("expected a row for {session}"))
    }

    #[test]
    fn one_row_per_session_with_mode_bound_card_last_action_age_and_presence() {
        let dir = TestTempDir::new("maestro-active-rows");
        let root = dir.path();

        // s-working: a card_touch 1m ago -> non-Stop recent -> [working].
        seed(
            root,
            "s-working",
            &[
                r#"{"event_type":"skill_activation","session_id":"s-working","skill_name":"maestro-card","ts":"2026-06-14T11:30:00.000Z"}"#,
                r#"{"event_type":"card_touch","session_id":"s-working","card_id":"card-bar","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        // s-waiting: a Stop 2m ago -> recent Stop -> [waiting], NOT excluded.
        seed(
            root,
            "s-waiting",
            &[
                r#"{"event_type":"skill_activation","session_id":"s-waiting","skill_name":"maestro-design","ts":"2026-06-14T11:40:00.000Z"}"#,
                r#"{"event_type":"card_touch","session_id":"s-waiting","card_id":"card-foo","ts":"2026-06-14T11:55:00.000Z"}"#,
                r#"{"event_type":"Stop","session_id":"s-waiting","ts":"2026-06-14T11:58:00.000Z"}"#,
            ],
        );
        // s-idle: last event 22m ago, within the 30m window -> [idle 22m].
        seed(
            root,
            "s-idle",
            &[
                r#"{"event_type":"PostToolUse","session_id":"s-idle","ts":"2026-06-14T11:38:00.000Z"}"#,
            ],
        );
        // s-stale: last event 45m ago, beyond the window -> [stale 45m].
        seed(
            root,
            "s-stale",
            &[
                r#"{"event_type":"skill_activation","session_id":"s-stale","skill_name":"maestro-audit","ts":"2026-06-14T11:15:00.000Z"}"#,
            ],
        );

        let rows = active_sessions(&MaestroPaths::new(root.to_path_buf()), NOW)
            .expect("active_sessions should read the seeded logs");

        assert_eq!(rows.len(), 4, "one row per seeded session bucket");

        let working = row(&rows, "s-working");
        assert_eq!(working.presence, Presence::Working);
        assert_eq!(working.mode.as_deref(), Some("maestro-card"));
        assert_eq!(working.bound_card.as_deref(), Some("card-bar"));
        assert_eq!(working.last_action, "card_touch");
        assert_eq!(working.age_minutes, 1);

        let waiting = row(&rows, "s-waiting");
        assert_eq!(waiting.presence, Presence::Waiting);
        assert_eq!(waiting.mode.as_deref(), Some("maestro-design"));
        assert_eq!(waiting.bound_card.as_deref(), Some("card-foo"));
        assert_eq!(waiting.last_action, "Stop");
        assert_eq!(waiting.age_minutes, 2);

        let idle = row(&rows, "s-idle");
        assert_eq!(idle.presence, Presence::Idle);
        assert_eq!(idle.age_minutes, 22);
        assert_eq!(idle.mode, None);
        assert_eq!(idle.bound_card, None);

        let stale = row(&rows, "s-stale");
        assert_eq!(stale.presence, Presence::Stale);
        assert_eq!(stale.age_minutes, 45);

        // Newest-first ordering by last event.
        let order: Vec<&str> = rows.iter().map(|row| row.session_id.as_str()).collect();
        assert_eq!(order, ["s-working", "s-waiting", "s-idle", "s-stale"]);
    }

    #[test]
    fn a_recent_card_touch_only_session_reads_as_working_not_idle() {
        let dir = TestTempDir::new("maestro-active-cardtouch-working");
        let root = dir.path();
        seed(
            root,
            "s-touch",
            &[
                r#"{"event_type":"card_touch","session_id":"s-touch","card_id":"card-zed","ts":"2026-06-14T11:58:00.000Z"}"#,
            ],
        );

        let rows = active_sessions(&MaestroPaths::new(root.to_path_buf()), NOW)
            .expect("active_sessions should read the seeded log");

        let touch = row(&rows, "s-touch");
        assert_eq!(
            touch.presence,
            Presence::Working,
            "a recent non-Stop card_touch keeps the session live"
        );
        assert_eq!(touch.bound_card.as_deref(), Some("card-zed"));
    }

    #[test]
    fn a_session_whose_latest_event_has_no_parseable_ts_is_skipped() {
        let dir = TestTempDir::new("maestro-active-no-ts");
        let root = dir.path();
        seed(
            root,
            "s-no-ts",
            &[
                r#"{"event_type":"skill_activation","session_id":"s-no-ts","skill_name":"maestro-card"}"#,
            ],
        );

        let rows = active_sessions(&MaestroPaths::new(root.to_path_buf()), NOW)
            .expect("active_sessions should tolerate a tsless log");

        assert!(
            rows.iter().all(|row| row.session_id != "s-no-ts"),
            "a session with no parseable ts cannot have an age and is skipped"
        );
    }

    #[test]
    fn a_merged_cli_date_bucket_yields_one_row_and_ages_out_of_the_window() {
        let dir = TestTempDir::new("maestro-active-cli-date");
        let root = dir.path();
        seed(
            root,
            "cli-2026-06-14",
            &[
                r#"{"event_type":"skill_activation","session_id":"cli-2026-06-14","skill_name":"maestro-design","ts":"2026-06-14T09:00:00.000Z"}"#,
                r#"{"event_type":"skill_activation","session_id":"cli-2026-06-14","skill_name":"maestro-card","ts":"2026-06-14T10:00:00.000Z"}"#,
            ],
        );

        let rows = active_sessions(&MaestroPaths::new(root.to_path_buf()), NOW)
            .expect("active_sessions should read the merged bucket");

        let merged = row(&rows, "cli-2026-06-14");
        assert_eq!(
            merged.mode.as_deref(),
            Some("maestro-card"),
            "latest skill wins"
        );
        assert_eq!(
            merged.presence,
            Presence::Stale,
            "120m old ages out of the window"
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row.session_id == "cli-2026-06-14")
                .count(),
            1,
            "the merged bucket collapses to exactly one row"
        );
    }

    #[test]
    fn union_merges_session_rows_from_every_worktree_root() {
        let dir_a = TestTempDir::new("maestro-active-union-a");
        let dir_b = TestTempDir::new("maestro-active-union-b");
        seed(
            dir_a.path(),
            "s-main",
            &[
                r#"{"event_type":"card_touch","session_id":"s-main","card_id":"card-main","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        seed(
            dir_b.path(),
            "s-oauth",
            &[
                r#"{"event_type":"card_touch","session_id":"s-oauth","card_id":"card-oauth","ts":"2026-06-14T11:58:00.000Z"}"#,
            ],
        );

        let roots = [
            MaestroPaths::new(dir_a.path().to_path_buf()),
            MaestroPaths::new(dir_b.path().to_path_buf()),
        ];
        let union = active_sessions_union(&roots, NOW).expect("union reads every root");
        let ids: Vec<&str> = union.iter().map(|row| row.session_id.as_str()).collect();
        assert!(
            ids.iter().any(|id| id.starts_with("s-main@")),
            "main worktree session is root-qualified in a union: {ids:?}"
        );
        assert!(
            ids.iter().any(|id| id.starts_with("s-oauth@")),
            "linked worktree session is root-qualified in a union: {ids:?}"
        );

        let local = active_sessions(&roots[0], NOW).expect("single root reads only itself");
        let local_ids: Vec<&str> = local.iter().map(|row| row.session_id.as_str()).collect();
        assert_eq!(
            local_ids,
            ["s-main"],
            "one worktree shows only local sessions"
        );
    }

    #[test]
    fn union_keeps_same_named_sessions_from_different_worktrees_distinct() {
        let dir_a = TestTempDir::new("maestro-active-union-same-a");
        let dir_b = TestTempDir::new("maestro-active-union-same-b");
        seed(
            dir_a.path(),
            "cli-2026-06-14",
            &[
                r#"{"event_type":"card_touch","session_id":"cli-2026-06-14","card_id":"card-a","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        seed(
            dir_b.path(),
            "cli-2026-06-14",
            &[
                r#"{"event_type":"card_touch","session_id":"cli-2026-06-14","card_id":"card-b","ts":"2026-06-14T11:58:00.000Z"}"#,
            ],
        );

        let roots = [
            MaestroPaths::new(dir_a.path().to_path_buf()),
            MaestroPaths::new(dir_b.path().to_path_buf()),
        ];
        let rows = active_sessions_union(&roots, NOW).expect("union reads every root");

        assert_eq!(
            rows.len(),
            2,
            "same session id in two worktrees must not collapse"
        );
        let cards: Vec<&str> = rows
            .iter()
            .filter_map(|row| row.bound_card.as_deref())
            .collect();
        assert!(
            cards.contains(&"card-a") && cards.contains(&"card-b"),
            "both bound cards survive the union: {cards:?}"
        );
    }

    #[test]
    fn warm_overlap_flags_two_live_sessions_editing_the_same_file() {
        let dir = TestTempDir::new("maestro-overlap-two");
        seed(
            dir.path(),
            "s-a",
            &[
                r#"{"event_type":"card_touch","session_id":"s-a","card_id":"card-a","ts":"2026-06-14T11:57:00.000Z"}"#,
                r#"{"event_type":"PostToolUse","session_id":"s-a","tool_name":"Edit","file_path":"src/auth/login.rs","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        seed(
            dir.path(),
            "s-b",
            &[
                r#"{"event_type":"card_touch","session_id":"s-b","card_id":"card-b","ts":"2026-06-14T11:56:00.000Z"}"#,
                r#"{"event_type":"PostToolUse","session_id":"s-b","tool_name":"Write","file_path":"src/auth/login.rs","ts":"2026-06-14T11:58:00.000Z"}"#,
            ],
        );

        let overlaps = warm_file_overlaps(&MaestroPaths::new(dir.path().to_path_buf()), NOW)
            .expect("overlap reads the seeded logs");
        assert_eq!(overlaps.len(), 1, "one shared file is contended");
        let overlap = &overlaps[0];
        assert_eq!(overlap.file_path, "src/auth/login.rs");
        let editors: Vec<(&str, Option<&str>)> = overlap
            .editors
            .iter()
            .map(|editor| (editor.session_id.as_str(), editor.bound_card.as_deref()))
            .collect();
        assert_eq!(
            editors,
            [("s-a", Some("card-a")), ("s-b", Some("card-b"))],
            "both editors are named with their bound cards"
        );
    }

    #[test]
    fn warm_overlap_normalizes_equivalent_local_path_spellings() {
        let dir = TestTempDir::new("maestro-overlap-normalized");
        let absolute = dir.path().join("src/auth/login.rs");
        seed(
            dir.path(),
            "s-a",
            &[
                r#"{"event_type":"PostToolUse","session_id":"s-a","tool_name":"Edit","file_path":"./src/auth/login.rs","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        let absolute_event = format!(
            r#"{{"event_type":"PostToolUse","session_id":"s-b","tool_name":"Write","file_path":"{}","ts":"2026-06-14T11:58:00.000Z"}}"#,
            absolute.display()
        );
        seed(dir.path(), "s-b", &[&absolute_event]);

        let overlaps = warm_file_overlaps(&MaestroPaths::new(dir.path().to_path_buf()), NOW)
            .expect("overlap reads the seeded logs");

        assert_eq!(overlaps.len(), 1, "equivalent paths are grouped");
        assert_eq!(overlaps[0].file_path, "src/auth/login.rs");
    }

    #[test]
    fn warm_overlap_silent_for_a_single_editor() {
        let dir = TestTempDir::new("maestro-overlap-single");
        seed(
            dir.path(),
            "s-a",
            &[
                r#"{"event_type":"PostToolUse","session_id":"s-a","tool_name":"Edit","file_path":"src/auth/login.rs","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );

        let overlaps = warm_file_overlaps(&MaestroPaths::new(dir.path().to_path_buf()), NOW)
            .expect("overlap reads the seeded log");
        assert!(overlaps.is_empty(), "one editor is not an overlap");
    }

    #[test]
    fn declared_scope_overlap_flags_live_sessions_across_worktrees() {
        let dir_a = TestTempDir::new("maestro-scope-a");
        let dir_b = TestTempDir::new("maestro-scope-b");
        seed(
            dir_a.path(),
            "s-a",
            &[
                r#"{"event_type":"scope_declaration","session_id":"s-a","scope_paths":["src/interfaces/cli/status.rs"],"ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        seed(
            dir_b.path(),
            "s-b",
            &[
                r#"{"event_type":"scope_declaration","session_id":"s-b","scope_paths":["./src/interfaces/cli/status.rs"],"ts":"2026-06-14T11:58:00.000Z"}"#,
            ],
        );

        let roots = [
            MaestroPaths::new(dir_a.path().to_path_buf()),
            MaestroPaths::new(dir_b.path().to_path_buf()),
        ];
        let overlaps = declared_scope_overlaps_union(&roots, NOW).expect("scope overlaps read");

        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].scope_path, "src/interfaces/cli/status.rs");
        let owners: Vec<String> = overlaps[0]
            .owners
            .iter()
            .map(|owner| owner.session_id.clone())
            .collect();
        assert_eq!(
            owners,
            [
                format!("s-a@{}", root_label(dir_a.path())),
                format!("s-b@{}", root_label(dir_b.path())),
            ]
        );
    }

    #[test]
    fn declared_scope_overlap_is_silent_for_disjoint_scopes() {
        let dir = TestTempDir::new("maestro-scope-disjoint");
        seed(
            dir.path(),
            "s-a",
            &[
                r#"{"event_type":"scope_declaration","session_id":"s-a","scope_paths":["src/interfaces/cli/status.rs"],"ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        seed(
            dir.path(),
            "s-b",
            &[
                r#"{"event_type":"scope_declaration","session_id":"s-b","scope_paths":["src/domain/run/active.rs"],"ts":"2026-06-14T11:58:00.000Z"}"#,
            ],
        );

        let overlaps =
            declared_scope_overlaps_union(&[MaestroPaths::new(dir.path().to_path_buf())], NOW)
                .expect("scope overlaps read");

        assert!(overlaps.is_empty(), "disjoint declarations stay silent");
    }

    #[test]
    fn warm_overlap_decays_when_a_peer_edit_ages_out_of_the_live_window() {
        let dir = TestTempDir::new("maestro-overlap-decay");
        seed(
            dir.path(),
            "s-a",
            &[
                r#"{"event_type":"PostToolUse","session_id":"s-a","tool_name":"Edit","file_path":"src/auth/login.rs","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        // s-b last edited the file 10m ago -> outside the live window -> no longer warm.
        seed(
            dir.path(),
            "s-b",
            &[
                r#"{"event_type":"PostToolUse","session_id":"s-b","tool_name":"Edit","file_path":"src/auth/login.rs","ts":"2026-06-14T11:50:00.000Z"}"#,
            ],
        );

        let overlaps = warm_file_overlaps(&MaestroPaths::new(dir.path().to_path_buf()), NOW)
            .expect("overlap reads the seeded logs");
        assert!(
            overlaps.is_empty(),
            "a cold peer edit decays the overlap without any release command"
        );
    }

    #[test]
    fn warm_overlap_ignores_non_write_tools() {
        let dir = TestTempDir::new("maestro-overlap-read");
        seed(
            dir.path(),
            "s-a",
            &[
                r#"{"event_type":"PostToolUse","session_id":"s-a","tool_name":"Edit","file_path":"src/auth/login.rs","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        // A concurrent Read of the same file is not a write conflict.
        seed(
            dir.path(),
            "s-b",
            &[
                r#"{"event_type":"PostToolUse","session_id":"s-b","tool_name":"Read","file_path":"src/auth/login.rs","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );

        let overlaps = warm_file_overlaps(&MaestroPaths::new(dir.path().to_path_buf()), NOW)
            .expect("overlap reads the seeded logs");
        assert!(overlaps.is_empty(), "a reader does not contend an editor");
    }

    #[test]
    fn warm_overlap_is_local_only_never_cross_worktree() {
        let dir_a = TestTempDir::new("maestro-overlap-local-a");
        let dir_b = TestTempDir::new("maestro-overlap-local-b");
        seed(
            dir_a.path(),
            "s-a",
            &[
                r#"{"event_type":"PostToolUse","session_id":"s-a","tool_name":"Edit","file_path":"src/auth/login.rs","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );
        // Same path, different worktree -> isolation, NOT contention.
        seed(
            dir_b.path(),
            "s-b",
            &[
                r#"{"event_type":"PostToolUse","session_id":"s-b","tool_name":"Edit","file_path":"src/auth/login.rs","ts":"2026-06-14T11:59:00.000Z"}"#,
            ],
        );

        let overlaps = warm_file_overlaps(&MaestroPaths::new(dir_a.path().to_path_buf()), NOW)
            .expect("overlap reads only the local root");
        assert!(
            overlaps.is_empty(),
            "equal paths in separate worktrees are isolation, not overlap"
        );
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: system clock should be after the Unix epoch")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("invariant: temp dir should be creatable");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
