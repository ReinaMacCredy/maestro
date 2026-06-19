//! The conflict-notice store: an agent's explicit "I am taking this ground,
//! hold off" advisory aimed at a peer card, surfaced cross-worktree in the
//! pre-command banner. Like the channel store this is pull-only, file-backed,
//! and machine-local; maestro never pushes. A notice creates NO link edge -- it
//! is a transient coordination signal, not a relationship (the never-auto-links
//! invariant).
//!
//! Visibility is liveness-scoped (`dec-conflict-notice-lifetime-scoped-to-c5eb`):
//! a notice shows only while its asserter is still a non-stale session in the
//! active union. The asserter does not re-assert; any maestro command keeps its
//! session non-stale, so the asserter's SESSION presence -- not the notice's age
//! -- governs visibility. `--clear` is the only state write that retracts one; a
//! crashed asserter's notices fade out when its session goes stale, with no
//! background cleanup.
//!
//! On-disk shape under `.maestro/conflicts.jsonl` (gitignored, machine-local),
//! one append-only event per line:
//!   `{ts, action: "assert"|"clear", asserter_session, asserter_card, peer_card, reason}`
//! Per (asserter_session, peer_card) the LAST line in the file wins (append
//! order, not parsed ts -- assert and clear for a key always come from the same
//! session writing its own worktree file, so file order is authoritative and a
//! same-millisecond assert/clear pair is unambiguous).
//!
//! A write lands in the running worktree's own file only
//! (`open_managed_appendable` rejects any path outside the local repo root). The
//! union reader merges every worktree's file; a given (asserter_session,
//! peer_card) key is written only in the asserter's own worktree, so there is
//! nothing to dedup across roots.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::domain::run::{
    Presence, active_sessions_union, append_jsonl_line, open_managed_appendable, union_session_id,
};
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

const CONFLICTS_RELATIVE_PATH: &str = ".maestro/conflicts.jsonl";

/// A live conflict notice: the asserter is warning the holder of `peer_card`
/// that it is taking shared ground. Produced by [`active_notices`] after the
/// liveness gate; `asserter_session` is union-qualified to match the active
/// union's presence key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Notice {
    /// The asserting session, union-qualified across worktrees.
    pub asserter_session: String,
    /// The card the asserter is bound to (display + the terminal gate). Empty
    /// when the asserter had touched no card at assert time.
    pub asserter_card: String,
    /// The card whose ground is being claimed (the peer being warned).
    pub peer_card: String,
    /// The advisory text the peer sees.
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Assert,
    Clear,
}

impl Action {
    fn as_str(self) -> &'static str {
        match self {
            Action::Assert => "assert",
            Action::Clear => "clear",
        }
    }
}

/// One parsed line before union-qualification: carries the raw (unqualified)
/// session id and the action, so the reader can fold by append order and then
/// qualify against the active union.
struct RawNotice {
    action: Action,
    asserter_session: String,
    asserter_card: String,
    peer_card: String,
    reason: String,
}

/// Append an "I am taking `peer_card`'s ground" notice from `asserter_session`
/// (bound to `asserter_card`). Creates no link edge. The write is local to the
/// running worktree; a peer sees it only via [`active_notices`]' union read.
pub fn assert(
    paths: &MaestroPaths,
    asserter_session: &str,
    asserter_card: &str,
    peer_card: &str,
    reason: &str,
) -> Result<()> {
    append_event(
        paths,
        Action::Assert,
        asserter_session,
        asserter_card,
        peer_card,
        reason,
    )
}

/// Retract the notice `asserter_session` asserted against `peer_card`. The later
/// `clear` line wins over the earlier `assert` in the fold, so the notice stops
/// surfacing. A clear with no matching assert is a harmless no-op.
pub fn clear(paths: &MaestroPaths, asserter_session: &str, peer_card: &str) -> Result<()> {
    append_event(paths, Action::Clear, asserter_session, "", peer_card, "")
}

fn append_event(
    paths: &MaestroPaths,
    action: Action,
    asserter_session: &str,
    asserter_card: &str,
    peer_card: &str,
    reason: &str,
) -> Result<()> {
    let mut file = open_managed_appendable(paths, CONFLICTS_RELATIVE_PATH)?;
    // serde_json encodes every field, so a reason carrying quotes, braces, or
    // newlines round-trips intact (no hand-formatted line to corrupt).
    let line = json!({
        "ts": utc_now_timestamp(),
        "action": action.as_str(),
        "asserter_session": asserter_session,
        "asserter_card": asserter_card,
        "peer_card": peer_card,
        "reason": reason,
    });
    append_jsonl_line(&mut file, &line)
        .with_context(|| format!("failed to append to {CONFLICTS_RELATIVE_PATH}"))
}

/// Every live conflict notice across all worktree roots as of `now`: read each
/// worktree's file, fold to the latest action per (asserter_session, peer_card),
/// keep the asserts whose asserter is still a non-stale session in the active
/// union. Pure over `(roots, now)` -- the liveness gate is computed, not
/// observed -- so the lifetime contract (`dec-c5eb`) is testable by assertion.
///
/// An asserter absent from the union (no run events at all) hides its notices:
/// liveness fails safe to hidden rather than showing a notice from a session we
/// cannot confirm is live.
pub fn active_notices(roots: &[MaestroPaths], now: &str) -> Result<Vec<Notice>> {
    // Fold the notice files first: when no session has run `maestro conflict`
    // there is no file to read, so we skip the cross-worktree active-session
    // union walk entirely. This runs in the pre-command banner on every command,
    // and the no-conflict case is overwhelmingly the common one.
    let mut folded: BTreeMap<(String, String), (Action, Notice)> = BTreeMap::new();
    for paths in roots {
        for raw in read_root_notices(paths)? {
            let asserter_session = union_session_id(paths, roots, &raw.asserter_session);
            let key = (asserter_session.clone(), raw.peer_card.clone());
            let notice = Notice {
                asserter_session,
                asserter_card: raw.asserter_card,
                peer_card: raw.peer_card,
                reason: raw.reason,
            };
            // Last line for a key wins: roots and lines are read in order, and a
            // key lives in exactly one worktree's file, so insert-overwrite is
            // the fold.
            folded.insert(key, (raw.action, notice));
        }
    }
    if folded.is_empty() {
        return Ok(Vec::new());
    }

    let presence: HashMap<String, Presence> = active_sessions_union(roots, now)?
        .into_iter()
        .map(|row| (row.session_id, row.presence))
        .collect();

    Ok(folded
        .into_values()
        .filter(|(action, notice)| {
            *action == Action::Assert
                && presence
                    .get(&notice.asserter_session)
                    .is_some_and(|presence| *presence != Presence::Stale)
        })
        .map(|(_, notice)| notice)
        .collect())
}

/// Read one worktree's conflict file into raw (unqualified) notices in file
/// order. A missing file is no notices. A malformed line is skipped, not fatal:
/// the store is a best-effort advisory, so one bad line must not blank the
/// banner.
fn read_root_notices(paths: &MaestroPaths) -> Result<Vec<RawNotice>> {
    let path = managed_path(paths, CONFLICTS_RELATIVE_PATH, SymlinkPolicy::RejectAllComponents)?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read {CONFLICTS_RELATIVE_PATH}"));
        }
    };
    Ok(text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .filter_map(|value| parse_line(&value))
        .collect())
}

fn parse_line(value: &Value) -> Option<RawNotice> {
    let action = match value.get("action").and_then(Value::as_str)? {
        "assert" => Action::Assert,
        "clear" => Action::Clear,
        _ => return None,
    };
    let asserter_session = value.get("asserter_session").and_then(Value::as_str)?;
    let peer_card = value.get("peer_card").and_then(Value::as_str)?;
    if asserter_session.is_empty() || peer_card.is_empty() {
        return None;
    }
    let optional = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Some(RawNotice {
        action,
        asserter_session: asserter_session.to_string(),
        asserter_card: optional("asserter_card"),
        peer_card: peer_card.to_string(),
        reason: optional("reason"),
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

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

    /// Seed one run-event log so a session has a known presence under the active
    /// union: a single `card_touch` at `ts`, which classifies as live when `now`
    /// is within the live threshold and stale when it is past the window.
    fn seed_session(root: &Path, session: &str, card: &str, ts: &str) {
        let run_dir = root.join(".maestro/runs").join(session);
        fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
        let line = json!({
            "event_type": "card_touch",
            "session_id": session,
            "card_id": card,
            "ts": ts,
        });
        fs::write(run_dir.join("events.jsonl"), format!("{line}\n"))
            .expect("invariant: event log fixture should be writable");
    }

    #[test]
    fn notice_shows_while_asserter_is_live_and_hides_once_it_goes_stale() {
        let temp = TestTempDir::new("maestro-conflict-liveness");
        let paths = MaestroPaths::new(temp.path());
        let roots = std::slice::from_ref(&paths);

        // The asserter's last activity fixes its session presence relative to now.
        seed_session(
            temp.path(),
            "sess-a",
            "card-mine",
            "2026-06-14T12:00:00.000Z",
        );
        assert(&paths, "sess-a", "card-mine", "card-yours", "taking login.rs")
            .expect("assert should persist");

        // 1m later the asserter is live -> the notice surfaces.
        let live = active_notices(roots, "2026-06-14T12:01:00.000Z").expect("read should succeed");
        assert_eq!(live.len(), 1, "a live asserter's notice shows");
        assert_eq!(live[0].peer_card, "card-yours");
        assert_eq!(live[0].asserter_card, "card-mine");
        assert_eq!(live[0].reason, "taking login.rs");

        // 31m later the asserter is past the window (stale) -> the notice hides,
        // with no re-assert and no cleanup write (dec-c5eb).
        let stale =
            active_notices(roots, "2026-06-14T12:31:00.000Z").expect("read should succeed");
        assert!(stale.is_empty(), "a stale asserter's notice auto-hides");
    }

    #[test]
    fn notices_union_across_worktrees() {
        let root_a = TestTempDir::new("maestro-conflict-union-a");
        let root_b = TestTempDir::new("maestro-conflict-union-b");
        let paths_a = MaestroPaths::new(root_a.path());
        let paths_b = MaestroPaths::new(root_b.path());

        // The asserter lives in worktree A and asserts there (write is local).
        seed_session(
            root_a.path(),
            "sess-a",
            "card-mine",
            "2026-06-14T12:00:00.000Z",
        );
        assert(&paths_a, "sess-a", "card-mine", "card-yours", "worktreeing it")
            .expect("assert should persist in A");

        // A read scoped to worktree B alone sees nothing (the notice is in A).
        let only_b = active_notices(std::slice::from_ref(&paths_b), "2026-06-14T12:01:00.000Z")
            .expect("read B");
        assert!(only_b.is_empty(), "the notice lives only in worktree A's file");

        // The union over both roots surfaces A's notice to B.
        let union = active_notices(&[paths_a, paths_b], "2026-06-14T12:01:00.000Z")
            .expect("union read should succeed");
        assert_eq!(union.len(), 1, "the union merges A's notice");
        assert_eq!(union[0].peer_card, "card-yours");
    }

    #[test]
    fn clear_retracts_the_notice() {
        let temp = TestTempDir::new("maestro-conflict-clear");
        let paths = MaestroPaths::new(temp.path());
        let roots = std::slice::from_ref(&paths);
        seed_session(
            temp.path(),
            "sess-a",
            "card-mine",
            "2026-06-14T12:00:00.000Z",
        );

        assert(&paths, "sess-a", "card-mine", "card-yours", "taking it").expect("assert");
        let before =
            active_notices(roots, "2026-06-14T12:01:00.000Z").expect("read after assert");
        assert_eq!(before.len(), 1, "the assert surfaces");

        clear(&paths, "sess-a", "card-yours").expect("clear");
        let after = active_notices(roots, "2026-06-14T12:01:00.000Z").expect("read after clear");
        assert!(after.is_empty(), "the later clear retracts the notice");
    }

    #[test]
    fn an_asserter_with_no_run_events_is_not_shown() {
        // Liveness fails safe to hidden: a notice from a session we cannot
        // confirm in the active union never surfaces.
        let temp = TestTempDir::new("maestro-conflict-no-session");
        let paths = MaestroPaths::new(temp.path());
        let roots = std::slice::from_ref(&paths);

        assert(&paths, "ghost", "card-mine", "card-yours", "no events").expect("assert");
        let notices = active_notices(roots, "2026-06-14T12:01:00.000Z").expect("read");
        assert!(
            notices.is_empty(),
            "an asserter absent from the active union is hidden, not shown"
        );
    }

    #[test]
    fn a_reason_with_quotes_and_braces_round_trips() {
        let temp = TestTempDir::new("maestro-conflict-roundtrip");
        let paths = MaestroPaths::new(temp.path());
        let roots = std::slice::from_ref(&paths);
        seed_session(
            temp.path(),
            "sess-a",
            "card-mine",
            "2026-06-14T12:00:00.000Z",
        );

        let tricky = r#"editing {"login.rs"} -- "hold off" please"#;
        assert(&paths, "sess-a", "card-mine", "card-yours", tricky).expect("assert");
        let notices = active_notices(roots, "2026-06-14T12:01:00.000Z").expect("read");
        assert_eq!(notices.len(), 1);
        assert_eq!(
            notices[0].reason, tricky,
            "a reason with JSON metacharacters survives the round trip"
        );
    }
}
