use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, IsTerminal, Write};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::domain::card;
use crate::domain::feature;
use crate::domain::proof;
use crate::domain::task;
use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;

/// Run the polling watch loop. The loop is the reuse unit: each tick calls
/// `render` for one frame, so `task list --watch` and `maestro watch` share it
/// while sourcing different renderers (task records vs the card graph). A
/// non-terminal stdout prints one frame and exits.
pub fn run<R>(interval_seconds: u64, render: R) -> Result<()>
where
    R: Fn() -> Result<String>,
{
    let interval = normalized_interval(interval_seconds);
    if !io::stdout().is_terminal() {
        print!("{}", render()?);
        return Ok(());
    }

    loop {
        print!("\x1b[2J\x1b[H{}", render()?);
        io::stdout()
            .flush()
            .context("failed to flush watch output")?;
        thread::sleep(Duration::from_secs(interval));
    }
}

/// The board-row state a workable card maps to, finer than coarse status: it
/// splits the OPEN/IN_PROGRESS band into the planr header buckets.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RowState {
    Done,
    Blocked,
    NeedsVerification,
    Active,
    Ready,
}

/// Classify a workable card for the board. `blocked_ids` is the set of card ids
/// with an unresolved blocker (the same `has_unresolved_blockers` predicate
/// `maestro status` counts), so a card reads blocked here exactly when status
/// counts it blocked.
fn classify(card: &card::schema::Card, blocked_ids: &std::collections::BTreeSet<String>) -> RowState {
    if card::query::coarse_of(&card.status) == Some(card::query::Coarse::Closed) {
        return RowState::Done;
    }
    if blocked_ids.contains(&card.id) {
        return RowState::Blocked;
    }
    if card.status == "needs_verification" {
        return RowState::NeedsVerification;
    }
    if card.claimed_by.is_some() || card.status == "in_progress" {
        return RowState::Active;
    }
    RowState::Ready
}

fn glyph(state: RowState) -> char {
    match state {
        RowState::Done => '\u{2713}',             // tick
        RowState::Ready => '\u{25CB}',            // open circle
        RowState::Active => '\u{25D0}',           // half circle
        RowState::NeedsVerification => '\u{25C6}', // diamond
        RowState::Blocked => '\u{00B7}',          // middle dot
    }
}

fn state_word(state: RowState) -> &'static str {
    match state {
        RowState::Done => "done",
        RowState::Ready => "ready",
        RowState::Active => "active",
        RowState::NeedsVerification => "needs_verification",
        RowState::Blocked => "blocked",
    }
}

/// Append one board row at `depth`. Depth 0 (a root, and the only depth in the
/// flat no-edges case) renders exactly as a flat row; deeper rows carry a
/// `\u{2514} ` tree connector so a blocks chain reads like planr's nested view.
fn push_row(out: &mut String, card: &card::schema::Card, state: RowState, depth: usize) {
    let connector = if depth == 0 {
        String::new()
    } else {
        format!("{}\u{2514} ", "  ".repeat(depth - 1))
    };
    out.push_str(&format!(
        "  {connector}{} {:<18} {}  {}",
        glyph(state),
        state_word(state),
        card.id,
        card.title,
    ));
    if state == RowState::Active
        && let Some(claimant) = card.claimed_by.as_deref()
    {
        out.push_str(&format!("  {claimant}"));
    }
    out.push('\n');
}

/// Depth-first placement of one blocks subtree, seeded at `root`. Each card is
/// emitted at most once (the `visited` set), so a back-edge into an
/// already-placed card is dropped: the walk is total and terminates on any
/// cycle. Done cards are counted in the header but their row is hidden.
fn place<'a>(
    root: &'a card::schema::Card,
    children_of: &BTreeMap<&'a str, Vec<&'a card::schema::Card>>,
    blocked_ids: &BTreeSet<String>,
    visited: &mut BTreeSet<&'a str>,
    out: &mut String,
) {
    let mut stack = vec![(root, 0usize)];
    while let Some((node, depth)) = stack.pop() {
        if !visited.insert(node.id.as_str()) {
            continue;
        }
        let state = classify(node, blocked_ids);
        if state != RowState::Done {
            push_row(out, node, state, depth);
        }
        if let Some(dependents) = children_of.get(node.id.as_str()) {
            // push reversed so siblings pop in id order (pre-order, id-sorted)
            for dependent in dependents.iter().rev() {
                stack.push((dependent, depth + 1));
            }
        }
    }
}

/// Render the planr-style board: a per-feature header (`<feature>: X/Y done
/// (Z%) | ready N | active N | needs_verification N | blocked N`) followed by
/// that feature's open workable cards. Pure over its inputs so it is testable
/// without IO. Features with no workable children (design-only) or no open work
/// (finished) are omitted; closed cards are hidden but still counted in X/Y.
fn format_board(
    cards: &[card::schema::Card],
    blocked_ids: &std::collections::BTreeSet<String>,
    focus: Option<&str>,
) -> String {
    use std::collections::BTreeMap;

    let mut features: BTreeMap<&str, &card::schema::Card> = BTreeMap::new();
    let mut children: BTreeMap<&str, Vec<&card::schema::Card>> = BTreeMap::new();
    for card in cards {
        if card.card_type == card::schema::CardType::Feature {
            features.insert(card.id.as_str(), card);
        }
    }
    for card in cards {
        if !card.card_type.workable() {
            continue;
        }
        if let Some(parent) = card.parent.as_deref() {
            children.entry(parent).or_default().push(card);
        }
    }

    let mut out = String::new();
    for (fid, feature) in &features {
        // Focus renders exactly one feature and never hides it: a focused
        // design-only or finished feature still prints its header (ac-2), so
        // the overview-only omissions below are skipped in focus mode.
        if let Some(focus) = focus
            && *fid != focus
        {
            continue;
        }
        let empty: Vec<&card::schema::Card> = Vec::new();
        let kids = match children.get(fid) {
            Some(kids) => kids,
            None if focus.is_some() => &empty,
            None => continue,
        };
        let total = kids.len();
        let mut kids: Vec<&card::schema::Card> = kids.clone();
        kids.sort_by(|left, right| left.id.cmp(&right.id));

        let mut counts = [0usize; 5];
        for kid in &kids {
            counts[classify(kid, blocked_ids) as usize] += 1;
        }
        let done = counts[RowState::Done as usize];
        if done == total && focus.is_none() {
            continue;
        }
        // total == 0 (a focused feature with no workable kids) implies done == 0,
        // so the .max(1) guard yields 0% without a separate branch.
        let pct = done * 100 / total.max(1);
        out.push_str(&format!(
            "{}: {done}/{total} done ({pct}%) | ready {} | active {} | needs_verification {} | blocked {}\n",
            feature.title,
            counts[RowState::Ready as usize],
            counts[RowState::Active as usize],
            counts[RowState::NeedsVerification as usize],
            counts[RowState::Blocked as usize],
        ));
        // Lay the feature's workable children out as a forest over their
        // in-feature `blocks` edges (dec-tree-nesting-...-6905): roots are
        // children with no in-feature blocker; each dependent nests under its
        // earliest-created blocker. A multi-blocker card attaches once, under
        // that earliest blocker; remaining edges are not redrawn. Edges to
        // cross-feature or non-workable targets fall outside `by_id`, so they
        // are ignored for layout and the card renders as a root.
        let by_id: BTreeMap<&str, &card::schema::Card> =
            kids.iter().map(|&kid| (kid.id.as_str(), kid)).collect();
        let mut children_of: BTreeMap<&str, Vec<&card::schema::Card>> = BTreeMap::new();
        let mut has_blocker: BTreeSet<&str> = BTreeSet::new();
        for &kid in &kids {
            let mut blockers: Vec<&card::schema::Card> = kid
                .deps
                .iter()
                .filter(|dep| dep.kind.is_blocking())
                .filter_map(|dep| by_id.get(dep.target.as_str()).copied())
                .collect();
            if blockers.is_empty() {
                continue;
            }
            blockers.sort_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.id.cmp(&right.id))
            });
            children_of
                .entry(blockers[0].id.as_str())
                .or_default()
                .push(kid);
            has_blocker.insert(kid.id.as_str());
        }
        for dependents in children_of.values_mut() {
            dependents.sort_by(|left, right| left.id.cmp(&right.id));
        }

        let mut visited: BTreeSet<&str> = BTreeSet::new();
        for &kid in &kids {
            if !has_blocker.contains(kid.id.as_str()) {
                place(kid, &children_of, blocked_ids, &mut visited, &mut out);
            }
        }
        // Any kid still unplaced sits in a cycle (every edge is a back-edge);
        // seed from it in id order so the cycle renders once and terminates.
        for &kid in &kids {
            if !visited.contains(kid.id.as_str()) {
                place(kid, &children_of, blocked_ids, &mut visited, &mut out);
            }
        }
        out.push('\n');
    }
    out
}

/// Load the card store and render one board snapshot. The blocked-id set comes
/// from the same task-record predicate `maestro status` uses, so the header's
/// blocked count matches status per card.
pub fn render_board(paths: &MaestroPaths, focus: Option<&str>) -> Result<String> {
    let cards = card::query::scan(paths)?;
    if let Some(focus) = focus {
        let known = cards.iter().any(|card| {
            card.card_type == card::schema::CardType::Feature && card.id == focus
        });
        if !known {
            anyhow::bail!(
                "no feature '{focus}'; run `maestro list --type feature` to see ids"
            );
        }
    }
    let tasks = task::load_task_records(&paths.tasks_dir())?;
    let blocked_ids: std::collections::BTreeSet<String> = tasks
        .iter()
        .filter(|task| task::has_unresolved_blockers(task))
        .map(|task| task.id.clone())
        .collect();
    Ok(format_board(&cards, &blocked_ids, focus))
}

/// Render one sandcastle-style task status snapshot.
pub fn render_snapshot(paths: &MaestroPaths, tasks: &[task::TaskRecord]) -> Result<String> {
    let features = feature::titles(paths);
    let current_commit = git::head(paths.repo_root()).unwrap_or(None);
    let active_agents = active_agents(tasks);
    let mut groups = BTreeMap::<String, Vec<&task::TaskRecord>>::new();
    for task in tasks {
        let group = task
            .feature_id
            .as_ref()
            .and_then(|id| features.get(id).cloned().or_else(|| Some(id.clone())))
            .unwrap_or_else(|| "unassigned".to_string());
        groups.entry(group).or_default().push(task);
    }

    let mut out = String::new();
    out.push_str(&format!(
        "scheduler: {} agents active\n\n",
        active_agents.len()
    ));
    if groups.is_empty() {
        out.push_str("unassigned\n  . no tasks\n");
        return Ok(out);
    }

    for (group, mut group_tasks) in groups {
        group_tasks.sort_by(|left, right| left.id.cmp(&right.id));
        out.push_str(&format!("{group}\n"));
        for task in group_tasks {
            out.push_str(&format!("  {} {}\n", task_icon(task), task.title));
            out.push_str(&format!(
                "    {}\n",
                task_substatus(task, current_commit.clone())?
            ));
        }
        out.push('\n');
    }
    Ok(out)
}

fn active_agents(tasks: &[task::TaskRecord]) -> BTreeSet<String> {
    tasks
        .iter()
        .filter(|task| task.state == task::TaskState::InProgress)
        .filter_map(|task| task.claimed_by.clone())
        .collect()
}

fn task_icon(task: &task::TaskRecord) -> &'static str {
    if task::has_unresolved_blockers(task) {
        return "!";
    }
    match task.state {
        task::TaskState::InProgress => "~",
        task::TaskState::NeedsVerification => "?",
        task::TaskState::Verified => "+",
        task::TaskState::Draft | task::TaskState::Exploring | task::TaskState::Ready => ".",
        task::TaskState::Rejected | task::TaskState::Abandoned | task::TaskState::Superseded => "x",
    }
}

fn task_substatus(task: &task::TaskRecord, current_commit: Option<String>) -> Result<String> {
    if let Some(blocker) = task
        .blockers
        .iter()
        .find(|blocker| blocker.resolved_at.is_none())
    {
        let blocker_label = blocker
            .blocked_ref
            .as_ref()
            .map(|blocked_ref| blocked_ref.id.as_str())
            .unwrap_or(blocker.title.as_str());
        return Ok(format!("blocked by {blocker_label}"));
    }
    if task.state == task::TaskState::InProgress {
        return Ok(format!(
            "in-progress ({})",
            task.claimed_by.as_deref().unwrap_or("unclaimed")
        ));
    }
    if task.state == task::TaskState::NeedsVerification {
        return needs_verification_substatus(task);
    }
    if task.state == task::TaskState::Verified {
        return verified_substatus(task, current_commit);
    }
    Ok(task.state.as_str().to_string())
}

fn needs_verification_substatus(task: &task::TaskRecord) -> Result<String> {
    let kind = proof::needs_verification_proof_status_kind_for_task(task)?;
    match kind {
        proof::ProofStatusKind::Failed => Ok("needs_verification (last verify failed)".to_string()),
        proof::ProofStatusKind::Missing
        | proof::ProofStatusKind::Accepted
        | proof::ProofStatusKind::Stale => Ok("needs_verification".to_string()),
    }
}

fn verified_substatus(task: &task::TaskRecord, current_commit: Option<String>) -> Result<String> {
    let kind = proof::proof_status_kind_for_task(task, current_commit)?;
    match kind {
        proof::ProofStatusKind::Missing | proof::ProofStatusKind::Accepted => {
            Ok("verified".to_string())
        }
        proof::ProofStatusKind::Failed => Ok("verified / failed".to_string()),
        proof::ProofStatusKind::Stale => {
            Ok("verified / stale (HEAD changed after proof)".to_string())
        }
    }
}

fn normalized_interval(seconds: u64) -> u64 {
    seconds.max(1)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{normalized_interval, render_snapshot};
    use crate::domain::task::{TaskRecord, TaskState, VerificationStatus};
    use crate::foundation::core::paths::MaestroPaths;

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn interval_clamps_below_one_second() {
        assert_eq!(normalized_interval(0), 1);
        assert_eq!(normalized_interval(1), 1);
        assert_eq!(normalized_interval(2), 2);
    }

    #[test]
    fn render_snapshot_marks_verified_task_with_missing_embedded_proof() {
        let temp = TestTempDir::new("maestro-task-list-watch-missing");
        let paths = MaestroPaths::new(temp.path().to_path_buf());
        let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        task.state = TaskState::Verified;
        let task_dir = paths.tasks_dir().join(task.directory_name());
        fs::create_dir_all(&task_dir).expect("invariant: task dir should be creatable");

        let output = render_snapshot(&paths, &[task]).expect("invariant: snapshot should render");

        assert!(output.contains("Add CSV export"));
        assert!(output.contains("verified"));
    }

    #[test]
    fn render_snapshot_marks_needs_verification_task_with_missing_embedded_proof() {
        let temp = TestTempDir::new("maestro-task-list-watch-needs-missing");
        let paths = MaestroPaths::new(temp.path().to_path_buf());
        let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        task.state = TaskState::NeedsVerification;
        let task_dir = paths.tasks_dir().join(task.directory_name());
        fs::create_dir_all(&task_dir).expect("invariant: task dir should be creatable");

        let output = render_snapshot(&paths, &[task]).expect("invariant: snapshot should render");

        assert!(output.contains("Add CSV export"));
        assert!(output.contains("needs_verification"));
    }

    #[test]
    fn render_snapshot_marks_needs_verification_task_with_applied_failed_proof() {
        let temp = TestTempDir::new("maestro-task-list-watch-failed-applied");
        let paths = MaestroPaths::new(temp.path().to_path_buf());
        let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        task.state = TaskState::NeedsVerification;
        task.verification.status = Some(VerificationStatus::Failed);
        task.verification.verified_at = Some("t1".to_string());
        task.verification.failures = vec!["missing proof".to_string()];
        let task_dir = paths.tasks_dir().join(task.directory_name());
        fs::create_dir_all(&task_dir).expect("invariant: task dir should be creatable");

        let output = render_snapshot(&paths, &[task]).expect("invariant: snapshot should render");

        assert!(output.contains("Add CSV export"));
        assert!(output.contains("needs_verification (last verify failed)"));
    }

    use super::format_board;
    use crate::domain::card;
    use std::collections::BTreeSet;

    fn feat(id: &str, title: &str) -> card::schema::Card {
        card::schema::Card::new(id, card::schema::CardType::Feature, title, "in_progress", "t0")
    }

    fn child(
        id: &str,
        parent: &str,
        ctype: card::schema::CardType,
        title: &str,
        status: &str,
    ) -> card::schema::Card {
        let mut c = card::schema::Card::new(id, ctype, title, status, "t0");
        c.parent = Some(parent.to_string());
        c
    }

    fn work(id: &str, parent: &str, title: &str, status: &str) -> card::schema::Card {
        child(id, parent, card::schema::CardType::Task, title, status)
    }

    use super::{classify, glyph, RowState};

    #[test]
    fn glyph_vocabulary_is_the_locked_set() {
        assert_eq!(glyph(RowState::Done), '\u{2713}');
        assert_eq!(glyph(RowState::Ready), '\u{25CB}');
        assert_eq!(glyph(RowState::Active), '\u{25D0}');
        assert_eq!(glyph(RowState::NeedsVerification), '\u{25C6}');
        assert_eq!(glyph(RowState::Blocked), '\u{00B7}');
    }

    #[test]
    fn board_excludes_non_workable_rows_and_hides_closed() {
        let cards = vec![
            feat("auth", "Auth"),
            work("task-1", "auth", "hash passwords", "ready"),
            work("task-2", "auth", "old login", "verified"),
            child("idea-1", "auth", card::schema::CardType::Idea, "maybe oauth", "open"),
            child("dec-1", "auth", card::schema::CardType::Decision, "pick hasher", "locked"),
        ];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(out.contains("hash passwords"), "open work row missing:\n{out}");
        assert!(!out.contains("old login"), "closed row should be hidden:\n{out}");
        assert!(!out.contains("maybe oauth"), "idea row should never appear:\n{out}");
        assert!(!out.contains("pick hasher"), "decision row should never appear:\n{out}");
        // total counts only the two workable children (one done, one ready).
        assert!(out.contains("1/2 done"), "header should count only workable kids:\n{out}");
    }

    #[test]
    fn board_renders_active_glyph_and_claimant() {
        let mut active = work("task-1", "auth", "session store", "in_progress");
        active.claimed_by = Some("claude#a4f2".to_string());
        let cards = vec![feat("auth", "Auth"), active];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            out.contains("\u{25D0} active"),
            "active glyph missing:\n{out}"
        );
        assert!(out.contains("claude#a4f2"), "claimant token missing:\n{out}");
    }

    #[test]
    fn board_omits_design_only_and_finished_features() {
        let cards = vec![
            // design-only: only a decision child, no workable cards
            feat("design", "Design only"),
            child("dec-1", "design", card::schema::CardType::Decision, "a fork", "locked"),
            // finished: every workable child closed
            feat("done-feat", "Finished"),
            work("task-9", "done-feat", "shipped work", "verified"),
        ];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(!out.contains("Design only"), "design-only feature should be omitted:\n{out}");
        assert!(!out.contains("Finished"), "finished feature should be omitted:\n{out}");
    }

    #[test]
    fn board_focus_renders_only_the_named_feature() {
        let cards = vec![
            feat("auth", "Auth"),
            work("task-1", "auth", "hash passwords", "ready"),
            feat("billing", "Billing"),
            work("task-2", "billing", "invoice export", "ready"),
        ];
        let out = format_board(&cards, &BTreeSet::new(), Some("billing"));
        assert!(out.contains("Billing"), "focused feature header missing:\n{out}");
        assert!(out.contains("invoice export"), "focused feature row missing:\n{out}");
        assert!(!out.contains("Auth"), "other feature must be excluded under focus:\n{out}");
        assert!(!out.contains("hash passwords"), "other feature's row leaked:\n{out}");
    }

    #[test]
    fn board_focus_shows_header_for_a_finished_feature() {
        // ac-2: a focused feature is never hidden, even when every child is
        // closed -- the header renders rather than producing empty output.
        let cards = vec![
            feat("done-feat", "Finished"),
            work("task-9", "done-feat", "shipped work", "verified"),
        ];
        let out = format_board(&cards, &BTreeSet::new(), Some("done-feat"));
        assert!(
            out.contains("Finished: 1/1 done (100%)"),
            "focused finished feature must still show its header:\n{out}"
        );
    }

    #[test]
    fn board_focus_shows_header_for_a_design_only_feature() {
        // ac-2: a focused feature with no workable children still renders a
        // header (0/0) rather than empty output.
        let cards = vec![
            feat("design", "Design only"),
            child("dec-1", "design", card::schema::CardType::Decision, "a fork", "locked"),
        ];
        let out = format_board(&cards, &BTreeSet::new(), Some("design"));
        assert!(
            out.contains("Design only: 0/0 done (0%)"),
            "focused design-only feature must still show its header:\n{out}"
        );
    }

    #[test]
    fn board_marks_blocked_from_predicate_set() {
        let cards = vec![
            feat("auth", "Auth"),
            work("task-1", "auth", "waits on dep", "ready"),
        ];
        let mut blocked = BTreeSet::new();
        blocked.insert("task-1".to_string());
        let out = format_board(&cards, &blocked, None);
        assert!(
            out.contains("blocked 1"),
            "blocked count should come from the predicate set:\n{out}"
        );
        assert!(out.contains("\u{00B7} blocked"), "blocked glyph row missing:\n{out}");
        assert!(out.contains("ready 0"), "a blocked card must not also read ready:\n{out}");
    }

    #[test]
    fn classify_prefers_blocked_over_ready() {
        let card = work("task-1", "auth", "x", "ready");
        let mut blocked = BTreeSet::new();
        blocked.insert("task-1".to_string());
        assert!(classify(&card, &blocked) == RowState::Blocked);
        assert!(classify(&card, &BTreeSet::new()) == RowState::Ready);
    }

    #[test]
    fn board_header_shows_done_ratio_and_counts_for_open_feature() {
        let mut active = work("task-3", "auth", "session store", "in_progress");
        active.claimed_by = Some("claude#a4f2".to_string());
        let cards = vec![
            feat("auth", "Auth"),
            work("task-1", "auth", "add login", "verified"),
            work("task-2", "auth", "hash passwords", "ready"),
            active,
        ];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            out.contains("Auth: 1/3 done (33%) | ready 1 | active 1 | needs_verification 0 | blocked 0"),
            "header line missing; got:\n{out}"
        );
    }

    fn blocks_dep(target: &str) -> card::schema::Dep {
        card::schema::Dep {
            kind: card::schema::DepKind::Blocks,
            target: target.to_string(),
        }
    }

    #[test]
    fn board_nests_a_blocked_child_under_its_blocker() {
        let mut migrations = work("task-mig", "auth", "run migrations", "ready");
        migrations.deps = vec![blocks_dep("task-db")];
        let cards = vec![
            feat("auth", "Auth"),
            work("task-db", "auth", "setup db", "in_progress"),
            migrations,
        ];
        let out = format_board(&cards, &BTreeSet::new(), None);
        let db_line = out.lines().find(|l| l.contains("setup db")).expect("blocker row");
        let mig_line = out.lines().find(|l| l.contains("run migrations")).expect("dependent row");
        assert!(!db_line.contains('\u{2514}'), "root blocker must not be indented:\n{out}");
        assert!(mig_line.contains('\u{2514}'), "dependent must carry the tree connector:\n{out}");
        let db_at = out.find("setup db").unwrap();
        let mig_at = out.find("run migrations").unwrap();
        assert!(db_at < mig_at, "blocker should render before its dependent:\n{out}");
    }

    #[test]
    fn board_renders_flat_with_no_blocks_edges() {
        let cards = vec![
            feat("auth", "Auth"),
            work("task-1", "auth", "one", "ready"),
            work("task-2", "auth", "two", "ready"),
        ];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            !out.contains('\u{2514}'),
            "no blocks edges must render a flat list, no connectors:\n{out}"
        );
    }

    #[test]
    fn board_places_a_multi_blocker_child_once_under_earliest_created_blocker() {
        // seed lists its LATE blocker first, to prove placement keys on
        // created_at, not edge order.
        let mut seed = work("task-seed", "auth", "seed data", "ready");
        seed.deps = vec![blocks_dep("task-late"), blocks_dep("task-early")];
        let mut early = work("task-early", "auth", "early blocker", "in_progress");
        early.created_at = "t1".to_string();
        let mut late = work("task-late", "auth", "late blocker", "in_progress");
        late.created_at = "t5".to_string();
        let cards = vec![feat("auth", "Auth"), early, late, seed];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert_eq!(
            out.matches("seed data").count(),
            1,
            "a multi-blocker child must render exactly once:\n{out}"
        );
        let early_at = out.find("early blocker").unwrap();
        let late_at = out.find("late blocker").unwrap();
        let seed_at = out.find("seed data").unwrap();
        assert!(
            early_at < seed_at && seed_at < late_at,
            "seed must nest under its earliest-created blocker (early), not late:\n{out}"
        );
        let seed_line = out.lines().find(|l| l.contains("seed data")).unwrap();
        assert!(seed_line.contains('\u{2514}'), "nested child must be indented:\n{out}");
    }

    #[test]
    fn board_breaks_a_blocks_cycle_without_hanging_or_duplicating() {
        let mut a = work("task-a", "auth", "card a", "in_progress");
        a.deps = vec![blocks_dep("task-b")];
        let mut b = work("task-b", "auth", "card b", "in_progress");
        b.deps = vec![blocks_dep("task-a")];
        let cards = vec![feat("auth", "Auth"), a, b];
        // Must terminate; a naive recursion over the cycle would hang here.
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert_eq!(out.matches("card a").count(), 1, "cycle member a must render once:\n{out}");
        assert_eq!(out.matches("card b").count(), 1, "cycle member b must render once:\n{out}");
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: system clock should be after the Unix epoch")
                .as_nanos();
            let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("{prefix}-{}-{timestamp}-{counter}", process::id()));
            fs::create_dir(&path).expect("invariant: unique temp dir should be creatable");
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
