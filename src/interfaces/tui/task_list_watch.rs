use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, IsTerminal, Write};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::domain::card;
use crate::domain::feature;
use crate::domain::proof;
use crate::domain::run::{self, Presence, SessionActivity};
use crate::domain::task;
use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

// The board classifier moved to `card::query` so `maestro status` shares it
// (interface code reaches domain submodules inline, not via a deep `use`).
type RowState = card::query::RowState;

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

/// Redraw a board frame in place: home the cursor (never a full-screen clear,
/// which would strobe at the render tick), erase each overwritten line to its
/// end, then erase any rows left over from a taller previous frame.
fn paint(frame: &str) -> String {
    let body = frame.replace('\n', "\x1b[K\n");
    format!("\x1b[H{body}\x1b[J")
}

/// The live-loop footer: the reload cadence and how to exit. Only the live
/// board appends it; the snapshot / non-terminal frame stays bare.
fn live_footer(interval: u64) -> String {
    format!("(live; refreshes every {interval}s; Ctrl-C to exit)")
}

/// Max width for the bound-card title in a session line; matches `maestro
/// active`'s column so the two views truncate the same way.
const SESSION_CARD_WIDTH: usize = 28;

/// Truncate to `max` chars, ellipsizing with a trailing '.' (the same rule
/// `maestro active` uses, kept local so the board does not depend on the CLI).
fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let head: String = value.chars().take(max.saturating_sub(1)).collect();
    format!("{head}.")
}

/// The presence label for a session line, mirroring `maestro active`'s STATE
/// column so the board reads the same.
fn presence_label(presence: Presence) -> &'static str {
    match presence {
        Presence::Working => "[working]",
        Presence::Waiting => "[waiting]",
        Presence::Idle => "[idle]",
        Presence::Stale => "[stale]",
    }
}

/// The skill mode with the `maestro-` prefix stripped (`maestro-design` ->
/// `design`), matching `maestro active`'s MODE column.
fn mode_label(skill: &str) -> &str {
    skill.strip_prefix("maestro-").unwrap_or(skill)
}

/// A short, session-distinguishing who-label: the leading token of the session
/// id (before the union `@worktree` suffix and the first `-`), so concurrent
/// sessions read apart (`claude#s66637`, `f08020da`) rather than all collapsing
/// to the constant worktree name.
fn who_label(session_id: &str) -> &str {
    let core = session_id.split('@').next().unwrap_or(session_id);
    core.split('-').next().unwrap_or(core)
}

/// One compact line for the LIVE SESSIONS block: mode, bound-card title (or id,
/// or '-'), who, and presence. The card title is joined from the live scan; a
/// bound id absent from the scan renders `<id> (missing)` like `maestro active`.
fn session_line(session: &SessionActivity, by_id: &BTreeMap<&str, &card::schema::Card>) -> String {
    let mode = session.mode.as_deref().map(mode_label).unwrap_or("-");
    let card = match session.bound_card.as_deref() {
        Some(id) => match by_id.get(id) {
            Some(card) => truncate(&card.title, SESSION_CARD_WIDTH),
            None => format!("{id} (missing)"),
        },
        None => "-".to_string(),
    };
    format!(
        "  {mode}  {card}  {} {} {}m",
        who_label(&session.session_id),
        presence_label(session.presence),
        session.age_minutes,
    )
}

fn glyph(state: RowState) -> char {
    match state {
        RowState::Done => '\u{2713}',              // tick
        RowState::Ready => '\u{25CB}',             // open circle
        RowState::Active => '\u{25D0}',            // half circle
        RowState::NeedsVerification => '\u{25C6}', // diamond
        RowState::Blocked => '\u{00B7}',           // middle dot
    }
}

/// The Braille spinner cycle for active rows (dec-live-board-animates...).
const SPINNER_FRAMES: [char; 10] = [
    '\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}',
    '\u{2807}', '\u{280F}',
];

/// The spinner glyph for a render tick. Pure over the tick so the animation is
/// testable without the loop; cycles the set so `frame(n)` advances and wraps.
fn spinner_frame(tick: u64) -> char {
    SPINNER_FRAMES[(tick % SPINNER_FRAMES.len() as u64) as usize]
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
fn push_row(
    out: &mut String,
    card: &card::schema::Card,
    state: RowState,
    depth: usize,
    live_tick: Option<u64>,
) {
    let connector = if depth == 0 {
        String::new()
    } else {
        format!("{}\u{2514} ", "  ".repeat(depth - 1))
    };
    // Live active rows animate the spinner; everything else (and any snapshot /
    // non-terminal frame, where live_tick is None) keeps the static glyph.
    let row_glyph = match (state, live_tick) {
        (RowState::Active, Some(tick)) => spinner_frame(tick),
        _ => glyph(state),
    };
    // The active (in_progress) row prefers the card's present-tense `active_form`
    // over its title, mirroring the task tool's activeForm; every other row, and
    // any active card without one, shows the title. Display-only.
    let label = match (state, card.active_form.as_deref()) {
        (RowState::Active, Some(active_form)) => active_form,
        _ => card.title.as_str(),
    };
    out.push_str(&format!(
        "  {connector}{} {:<18} {}  {}",
        row_glyph,
        state_word(state),
        card.id,
        label,
    ));
    if state == RowState::Active
        && let Some(claimant) = card.claimed_by.as_deref()
    {
        out.push_str(&format!("  {claimant}"));
    }
    // An unclaimed card carrying an advisory `suggested_for` shows the routing
    // hint; a claim supersedes it (the claimant renders above instead).
    if card.claimed_by.is_none()
        && let Some(who) = card.suggested_for.as_deref()
    {
        out.push_str(&format!("  -> for {who}"));
    }
    out.push('\n');
}

fn push_group_header(out: &mut String, title: &str, counts: &card::query::RowStateCounts) {
    let total = counts.total();
    let pct = counts.done * 100 / total.max(1);
    out.push_str(&format!(
        "{title}: {}/{} done ({pct}%) | ready {} | active {} | needs_verification {} | blocked {}\n",
        counts.done, total, counts.ready, counts.active, counts.needs_verification, counts.blocked,
    ));
}

struct BoardLayout<'a> {
    /// Rendered LIVE SESSIONS lines (empty when no live session), shown above the
    /// feature groups so `maestro watch` mirrors `maestro active`'s liveness.
    sessions: Vec<String>,
    groups: Vec<BoardGroup<'a>>,
}

struct BoardGroup<'a> {
    title: String,
    counts: card::query::RowStateCounts,
    rows: Vec<BoardRow<'a>>,
    /// Header note for a design-only live feature (no workable tasks): renders
    /// `<title>: <note>` instead of the 0/0 counts line.
    note: Option<String>,
}

struct BoardRow<'a> {
    card: &'a card::schema::Card,
    state: RowState,
    depth: usize,
}

impl BoardLayout<'_> {
    fn render(&self, live_tick: Option<u64>) -> String {
        let mut out = String::new();
        if !self.sessions.is_empty() {
            out.push_str(&format!("LIVE SESSIONS ({})\n", self.sessions.len()));
            for line in &self.sessions {
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(&format!("{}\n", "\u{2500}".repeat(44)));
        }
        for group in &self.groups {
            match &group.note {
                Some(note) if group.counts.total() == 0 => {
                    out.push_str(&format!("{}: {note}\n", group.title));
                }
                _ => push_group_header(&mut out, &group.title, &group.counts),
            }
            for row in &group.rows {
                push_row(&mut out, row.card, row.state, row.depth, live_tick);
            }
            out.push('\n');
        }
        if out.is_empty() {
            out.push_str(
                "No open work to show. Run `maestro card ready` or `maestro card list`.\n",
            );
        }
        out
    }
}

/// Depth-first placement of one blocks subtree, seeded at `root`. Each card is
/// emitted at most once (the `visited` set), so a back-edge into an
/// already-placed card is dropped: the walk is total and terminates on any
/// cycle. Done cards are counted in the header but their row is hidden.
fn place_rows<'a>(
    root: &'a card::schema::Card,
    children_of: &BTreeMap<&'a str, Vec<&'a card::schema::Card>>,
    blocked_ids: &BTreeSet<String>,
    just_completed: &BTreeSet<String>,
    visited: &mut BTreeSet<&'a str>,
    rows: &mut Vec<BoardRow<'a>>,
) {
    let mut stack = vec![(root, 0usize)];
    while let Some((node, depth)) = stack.pop() {
        if !visited.insert(node.id.as_str()) {
            continue;
        }
        let Some(state) = card::query::classify(node, blocked_ids) else {
            continue;
        };
        // Done rows are hidden in steady state; a card in `just_completed`
        // (live and now Done) renders one tick frame before it drops next reload.
        if state != RowState::Done || just_completed.contains(&node.id) {
            rows.push(BoardRow {
                card: node,
                state,
                depth,
            });
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
/// Test-only session-less wrapper; production renders via `build_board_layout`.
#[cfg(test)]
fn format_board(
    cards: &[card::schema::Card],
    blocked_ids: &std::collections::BTreeSet<String>,
    focus: Option<&str>,
) -> String {
    format_board_opts(cards, blocked_ids, focus, None, &BTreeSet::new())
}

/// Render the board with the live extras: `live_tick` animates active rows
/// (None = static snapshot), and `just_completed` un-suppresses the Done rows
/// that finished this reload so they flash one tick frame (dec-completed-card).
/// Test-only session-less wrapper; production renders via `build_board_layout`.
#[cfg(test)]
fn format_board_opts(
    cards: &[card::schema::Card],
    blocked_ids: &std::collections::BTreeSet<String>,
    focus: Option<&str>,
    live_tick: Option<u64>,
    just_completed: &BTreeSet<String>,
) -> String {
    build_board_layout(cards, blocked_ids, focus, just_completed, &[]).render(live_tick)
}

fn build_board_layout<'a>(
    cards: &'a [card::schema::Card],
    blocked_ids: &BTreeSet<String>,
    focus: Option<&str>,
    just_completed: &BTreeSet<String>,
    sessions: &[SessionActivity],
) -> BoardLayout<'a> {
    let by_id: BTreeMap<&str, &card::schema::Card> =
        cards.iter().map(|card| (card.id.as_str(), card)).collect();
    // Focus is a single-feature drill-down, not the live roster: the global
    // session block is an overview concern, so it is omitted when focused. A
    // session bound to a sibling feature would otherwise leak into the view and
    // break the "focus excludes other features" contract.
    let session_lines: Vec<String> = if focus.is_none() {
        sessions
            .iter()
            .map(|session| session_line(session, &by_id))
            .collect()
    } else {
        Vec::new()
    };

    let mut features: BTreeMap<&str, &card::schema::Card> = BTreeMap::new();
    let mut children: BTreeMap<&str, Vec<&card::schema::Card>> = BTreeMap::new();
    for card in cards {
        if card.card_type == card::schema::CardType::Feature {
            features.insert(card.id.as_str(), card);
        }
    }
    // A feature bound by a live session shows even with no workable children, so
    // an in-flight design session is visible on the board. Keep the earliest
    // session's mode for that feature's header note.
    let mut live_feature_mode: BTreeMap<&str, Option<String>> = BTreeMap::new();
    for session in sessions {
        if let Some(bound) = session.bound_card.as_deref()
            && features.contains_key(bound)
        {
            live_feature_mode
                .entry(bound)
                .or_insert_with(|| session.mode.clone());
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

    let mut groups = Vec::new();
    for (fid, feature) in &features {
        // Focus renders exactly one feature and never hides it: a focused
        // design-only or finished feature still prints its header (ac-2), so
        // the overview-only omissions below are skipped in focus mode.
        if let Some(focus) = focus
            && *fid != focus
        {
            continue;
        }
        let live_mode = live_feature_mode.get(fid);
        let empty: Vec<&card::schema::Card> = Vec::new();
        let kids = match children.get(fid) {
            Some(kids) => kids,
            None if focus.is_some() || live_mode.is_some() => &empty,
            None => continue,
        };
        let mut kids: Vec<&card::schema::Card> = kids.clone();
        kids.sort_by(|left, right| left.id.cmp(&right.id));

        let counts = card::query::RowStateCounts::from_cards(kids.iter().copied(), blocked_ids);
        // A finished feature is hidden in the overview, unless one of its tasks
        // just completed this reload (it then shows its 100% header + tick row for
        // one frame), or a live session is bound to it (keep it on the board).
        let has_flash = kids.iter().any(|kid| just_completed.contains(&kid.id));
        if counts.done == counts.total() && focus.is_none() && !has_flash && live_mode.is_none() {
            continue;
        }
        // A live feature with no workable tasks gets an alt header noting its
        // session mode; one with tasks renders its normal counts line.
        let note = match live_mode {
            Some(mode) if counts.total() == 0 => Some(match mode {
                Some(mode) => format!("{} (live session)", mode_label(mode)),
                None => "(live session)".to_string(),
            }),
            _ => None,
        };
        groups.push(BoardGroup {
            title: feature.title.clone(),
            counts,
            rows: feature_rows(&kids, blocked_ids, just_completed),
            note,
        });
    }

    // Workable cards with no parent feature (or a parent that is not a feature
    // in the store) have nowhere to live on the feature-grouped board. Collect
    // the open ones under a synthetic "(no feature)" group so loose active work
    // is visible. Skipped under focus, which renders exactly one named feature.
    if focus.is_none() {
        let mut orphans: Vec<&card::schema::Card> = cards
            .iter()
            .filter(|card| card.card_type.workable())
            .filter(|card| match card.parent.as_deref() {
                None => true,
                Some(parent) => !features.contains_key(parent),
            })
            .filter(|card| {
                card::query::classify(card, blocked_ids) != Some(RowState::Done)
                    || just_completed.contains(&card.id)
            })
            .collect();
        if !orphans.is_empty() {
            orphans.sort_by(|left, right| left.id.cmp(&right.id));
            groups.push(BoardGroup {
                title: "(no feature)".to_string(),
                counts: card::query::RowStateCounts::from_cards(
                    orphans.iter().copied(),
                    blocked_ids,
                ),
                rows: orphans
                    .into_iter()
                    .filter_map(|orphan| {
                        card::query::classify(orphan, blocked_ids).map(|state| BoardRow {
                            card: orphan,
                            state,
                            depth: 0,
                        })
                    })
                    .collect(),
                note: None,
            });
        }
    }

    BoardLayout {
        sessions: session_lines,
        groups,
    }
}

fn feature_rows<'a>(
    kids: &[&'a card::schema::Card],
    blocked_ids: &BTreeSet<String>,
    just_completed: &BTreeSet<String>,
) -> Vec<BoardRow<'a>> {
    // Lay the feature's workable children out as a forest over their in-feature
    // `blocks` edges (dec-tree-nesting-...-6905): roots are children with no
    // in-feature blocker; each dependent nests under its earliest-created
    // blocker. A multi-blocker card attaches once, under that earliest blocker;
    // remaining edges are not redrawn. Edges to cross-feature or non-workable
    // targets fall outside `by_id`, so they are ignored for layout and the card
    // renders as a root.
    let by_id: BTreeMap<&str, &card::schema::Card> =
        kids.iter().map(|&kid| (kid.id.as_str(), kid)).collect();
    let mut children_of: BTreeMap<&str, Vec<&card::schema::Card>> = BTreeMap::new();
    let mut has_blocker: BTreeSet<&str> = BTreeSet::new();
    for &kid in kids {
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

    let mut rows = Vec::new();
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    for &kid in kids {
        if !has_blocker.contains(kid.id.as_str()) {
            place_rows(
                kid,
                &children_of,
                blocked_ids,
                just_completed,
                &mut visited,
                &mut rows,
            );
        }
    }
    // Any kid still unplaced sits in a cycle (every edge is a back-edge); seed
    // from it in id order so the cycle renders once and terminates.
    for &kid in kids {
        if !visited.contains(kid.id.as_str()) {
            place_rows(
                kid,
                &children_of,
                blocked_ids,
                just_completed,
                &mut visited,
                &mut rows,
            );
        }
    }
    rows
}

/// Scan the card store for the board and compute the blocked-id set. The
/// blocked set comes from the card-model `blocks` graph (`card::query::blocked`,
/// the rule `maestro ready` inverts), so a card reads blocked here exactly when
/// an unsatisfied dependency keeps it out of `ready`. An unknown focus id errors
/// with a re-list hint rather than rendering empty.
fn load_board(
    paths: &MaestroPaths,
    focus: Option<&str>,
) -> Result<(Vec<card::schema::Card>, BTreeSet<String>)> {
    // Tolerate a peer mid-writing a card.yaml: the live watch loop must skip a
    // bad/partial record and recover on the next frame, not abort on the strict
    // scan's first malformed file the way `maestro list` does.
    let cards: Vec<card::schema::Card> = card::query::scan_with_failures(paths)?
        .cards
        .into_iter()
        .map(|(card, _)| card)
        .collect();
    if let Some(focus) = focus {
        let known = cards
            .iter()
            .any(|card| card.card_type == card::schema::CardType::Feature && card.id == focus);
        if !known {
            anyhow::bail!("no feature '{focus}'; run `maestro list --type feature` to see ids");
        }
    }
    let blocked_ids: BTreeSet<String> = card::query::blocked(&cards)
        .into_iter()
        .map(|card| card.id.clone())
        .collect();
    Ok((cards, blocked_ids))
}

/// The live sessions for the board's LIVE SESSIONS block: the same cross-worktree
/// liveness `maestro active` reads (`run::active_sessions_union` over every
/// worktree root), filtered to non-stale so the block matches `active`'s default
/// view. A read failure degrades to no block rather than aborting the board.
fn load_live_sessions(paths: &MaestroPaths) -> Vec<SessionActivity> {
    let roots = crate::interfaces::cli::worktree_roots(paths);
    let now = utc_now_timestamp();
    run::active_sessions_union(&roots, &now)
        .unwrap_or_default()
        .into_iter()
        .filter(|session| session.presence != Presence::Stale)
        .collect()
}

/// Load the card store and render one static board snapshot (active = static
/// half-circle, no flash). The seam both `watch snapshot` and the live loop's
/// non-terminal path render from.
pub fn render_board(paths: &MaestroPaths, focus: Option<&str>) -> Result<String> {
    let (cards, blocked_ids) = load_board(paths, focus)?;
    let sessions = load_live_sessions(paths);
    Ok(build_board_layout(&cards, &blocked_ids, focus, &BTreeSet::new(), &sessions).render(None))
}

/// Run the live board loop. Extends the poll loop (dec-surface-under-watch):
/// a fixed ~100ms render tick advances the spinner over each `--interval` data
/// reload, and the board redraws via cursor-home so it never strobes. Between
/// reloads it diffs the live set against the previous one so a card that just
/// finished flashes the tick for that reload (dec-completed-card). A
/// non-terminal stdout prints one static frame and exits.
pub fn run_board(paths: &MaestroPaths, focus: Option<&str>, interval_seconds: u64) -> Result<()> {
    let interval = normalized_interval(interval_seconds);
    if !io::stdout().is_terminal() {
        print!("{}", render_board(paths, focus)?);
        return Ok(());
    }
    let render_ticks = interval * 10; // ~100ms render tick across the data interval
    let mut prev_live: BTreeSet<String> = BTreeSet::new();
    let mut tick: u64 = 0;
    loop {
        let (cards, blocked_ids) = load_board(paths, focus)?;
        let mut live_now: BTreeSet<String> = BTreeSet::new();
        let mut just_completed: BTreeSet<String> = BTreeSet::new();
        for card in &cards {
            let Some(state) = card::query::classify(card, &blocked_ids) else {
                continue;
            };
            if state == RowState::Done {
                if prev_live.contains(&card.id) {
                    just_completed.insert(card.id.clone());
                }
            } else {
                live_now.insert(card.id.clone());
            }
        }
        let sessions = load_live_sessions(paths);
        let layout = build_board_layout(&cards, &blocked_ids, focus, &just_completed, &sessions);
        for _ in 0..render_ticks {
            let board = layout.render(Some(tick));
            let frame = format!("{board}\n{}\n", live_footer(interval));
            print!("{}", paint(&frame));
            io::stdout()
                .flush()
                .context("failed to flush watch output")?;
            tick += 1;
            thread::sleep(Duration::from_millis(100));
        }
        prev_live = live_now;
    }
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

    #[test]
    fn load_board_skips_a_malformed_card_instead_of_aborting() {
        use super::load_board;
        let temp = TestTempDir::new("maestro-task-list-watch-malformed");
        let paths = MaestroPaths::new(temp.path().to_path_buf());

        // A malformed card.yaml a peer is mid-writing into the shared store.
        let bad_dir = paths.cards_dir().join("task-bad");
        fs::create_dir_all(&bad_dir).expect("bad card dir");
        fs::write(bad_dir.join("card.yaml"), "not a card\n").expect("plant malformed");

        // The live board's load tolerates the bad record (Ok) rather than the strict
        // scan `?`-aborting the whole watch loop on the first malformed file.
        load_board(&paths, None).expect("a malformed card must not abort the board");
    }

    use super::format_board;
    use crate::domain::card;
    use std::collections::BTreeSet;

    fn feat(id: &str, title: &str) -> card::schema::Card {
        card::schema::Card::new(
            id,
            card::schema::CardType::Feature,
            title,
            "in_progress",
            "t0",
        )
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

    fn loose(id: &str, title: &str, status: &str) -> card::schema::Card {
        card::schema::Card::new(id, card::schema::CardType::Task, title, status, "t0")
    }

    use super::{RowState, glyph};

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
            child(
                "idea-1",
                "auth",
                card::schema::CardType::Idea,
                "maybe oauth",
                "open",
            ),
            child(
                "dec-1",
                "auth",
                card::schema::CardType::Decision,
                "pick hasher",
                "locked",
            ),
        ];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            out.contains("hash passwords"),
            "open work row missing:\n{out}"
        );
        assert!(
            !out.contains("old login"),
            "closed row should be hidden:\n{out}"
        );
        assert!(
            !out.contains("maybe oauth"),
            "idea row should never appear:\n{out}"
        );
        assert!(
            !out.contains("pick hasher"),
            "decision row should never appear:\n{out}"
        );
        // total counts only the two workable children (one done, one ready).
        assert!(
            out.contains("1/2 done"),
            "header should count only workable kids:\n{out}"
        );
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
        assert!(
            out.contains("claude#a4f2"),
            "claimant token missing:\n{out}"
        );
    }

    #[test]
    fn active_row_prefers_active_form_over_title_and_falls_back() {
        let mut active = work("task-1", "auth", "session store", "in_progress");
        active.claimed_by = Some("claude#a4f2".to_string());
        active.active_form = Some("Persisting tokens".to_string());
        // A second active card with no active_form falls back to its title.
        let mut bare = work("task-2", "auth", "token refresh", "in_progress");
        bare.claimed_by = Some("claude#b1c3".to_string());
        let cards = vec![feat("auth", "Auth"), active, bare];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            out.contains("Persisting tokens"),
            "active row renders active_form:\n{out}"
        );
        assert!(
            !out.contains("session store"),
            "active row omits the title when active_form is set:\n{out}"
        );
        assert!(
            out.contains("token refresh"),
            "an active card without active_form keeps its title:\n{out}"
        );
    }

    #[test]
    fn active_form_is_ignored_on_a_non_active_row() {
        // active_form is the active row's label; a ready card shows its title.
        let mut ready = work("task-1", "auth", "session store", "ready");
        ready.active_form = Some("Should not show".to_string());
        let out = format_board(&[feat("auth", "Auth"), ready], &BTreeSet::new(), None);
        assert!(
            out.contains("session store") && !out.contains("Should not show"),
            "a ready row ignores active_form:\n{out}"
        );
    }

    #[test]
    fn board_renders_the_assignee_hint_until_a_claim_supersedes_it() {
        let mut suggested = work("task-1", "auth", "session store", "ready");
        suggested.suggested_for = Some("codex#s9".to_string());
        let cards = vec![feat("auth", "Auth"), suggested];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            out.contains("-> for codex#s9"),
            "unclaimed suggested card should show the routing hint:\n{out}"
        );

        // Once claimed, the claimant supersedes the hint on the row.
        let mut claimed = work("task-1", "auth", "session store", "in_progress");
        claimed.suggested_for = Some("codex#s9".to_string());
        claimed.claimed_by = Some("claude#a4f2".to_string());
        let out2 = format_board(&[feat("auth", "Auth"), claimed], &BTreeSet::new(), None);
        assert!(
            out2.contains("claude#a4f2") && !out2.contains("-> for"),
            "a claim supersedes the hint in the board render:\n{out2}"
        );
    }

    #[test]
    fn board_omits_design_only_and_finished_features() {
        let cards = vec![
            // design-only: only a decision child, no workable cards
            feat("design", "Design only"),
            child(
                "dec-1",
                "design",
                card::schema::CardType::Decision,
                "a fork",
                "locked",
            ),
            // finished: every workable child closed
            feat("done-feat", "Finished"),
            work("task-9", "done-feat", "shipped work", "verified"),
        ];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            !out.contains("Design only"),
            "design-only feature should be omitted:\n{out}"
        );
        assert!(
            !out.contains("Finished"),
            "finished feature should be omitted:\n{out}"
        );
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
        assert!(
            out.contains("Billing"),
            "focused feature header missing:\n{out}"
        );
        assert!(
            out.contains("invoice export"),
            "focused feature row missing:\n{out}"
        );
        assert!(
            !out.contains("Auth"),
            "other feature must be excluded under focus:\n{out}"
        );
        assert!(
            !out.contains("hash passwords"),
            "other feature's row leaked:\n{out}"
        );
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
            child(
                "dec-1",
                "design",
                card::schema::CardType::Decision,
                "a fork",
                "locked",
            ),
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
        assert!(
            out.contains("\u{00B7} blocked"),
            "blocked glyph row missing:\n{out}"
        );
        assert!(
            out.contains("ready 0"),
            "a blocked card must not also read ready:\n{out}"
        );
    }

    #[test]
    fn classify_prefers_blocked_over_ready() {
        let card = work("task-1", "auth", "x", "ready");
        let mut blocked = BTreeSet::new();
        blocked.insert("task-1".to_string());
        assert!(card::query::classify(&card, &blocked) == Some(RowState::Blocked));
        assert!(card::query::classify(&card, &BTreeSet::new()) == Some(RowState::Ready));
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
            out.contains(
                "Auth: 1/3 done (33%) | ready 1 | active 1 | needs_verification 0 | blocked 0"
            ),
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
        let db_line = out
            .lines()
            .find(|l| l.contains("setup db"))
            .expect("blocker row");
        let mig_line = out
            .lines()
            .find(|l| l.contains("run migrations"))
            .expect("dependent row");
        assert!(
            !db_line.contains('\u{2514}'),
            "root blocker must not be indented:\n{out}"
        );
        assert!(
            mig_line.contains('\u{2514}'),
            "dependent must carry the tree connector:\n{out}"
        );
        let db_at = out.find("setup db").unwrap();
        let mig_at = out.find("run migrations").unwrap();
        assert!(
            db_at < mig_at,
            "blocker should render before its dependent:\n{out}"
        );
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
        assert!(
            seed_line.contains('\u{2514}'),
            "nested child must be indented:\n{out}"
        );
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
        assert_eq!(
            out.matches("card a").count(),
            1,
            "cycle member a must render once:\n{out}"
        );
        assert_eq!(
            out.matches("card b").count(),
            1,
            "cycle member b must render once:\n{out}"
        );
    }

    use super::{SPINNER_FRAMES, format_board_opts, paint, spinner_frame};

    #[test]
    fn spinner_frame_cycles_the_locked_braille_set() {
        assert_eq!(
            SPINNER_FRAMES,
            [
                '\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}',
                '\u{2827}', '\u{2807}', '\u{280F}'
            ]
        );
        assert_ne!(
            spinner_frame(0),
            spinner_frame(1),
            "the frame must advance with the tick counter"
        );
        assert_eq!(spinner_frame(0), '\u{280B}');
        assert_eq!(
            spinner_frame(10),
            spinner_frame(0),
            "the frame cycles every 10 ticks"
        );
        assert_eq!(spinner_frame(11), spinner_frame(1));
    }

    #[test]
    fn live_board_animates_active_rows_with_the_spinner() {
        let mut active = work("task-1", "auth", "session store", "in_progress");
        active.claimed_by = Some("claude#a4f2".to_string());
        let cards = vec![feat("auth", "Auth"), active];
        let empty = BTreeSet::new();
        let f0 = format_board_opts(&cards, &BTreeSet::new(), None, Some(0), &empty);
        assert!(
            f0.contains(spinner_frame(0)),
            "active row must show spinner frame 0:\n{f0}"
        );
        assert!(
            !f0.contains('\u{25D0}'),
            "a live active row must not render the static half-circle:\n{f0}"
        );
        let f1 = format_board_opts(&cards, &BTreeSet::new(), None, Some(1), &empty);
        assert!(
            f1.contains(spinner_frame(1)),
            "active row must advance to spinner frame 1:\n{f1}"
        );
        assert!(
            f0.contains("active") && f0.contains("claude#a4f2"),
            "the state word and claimant survive the spinner glyph:\n{f0}"
        );
    }

    #[test]
    fn snapshot_active_row_stays_the_static_half_circle() {
        // ac-8: with no live tick (snapshot / non-terminal) active renders the
        // static half-circle, never an animated spinner frame.
        let active = work("task-1", "auth", "session store", "in_progress");
        let cards = vec![feat("auth", "Auth"), active];
        let out = format_board_opts(&cards, &BTreeSet::new(), None, None, &BTreeSet::new());
        assert!(
            out.contains('\u{25D0}'),
            "static active glyph missing:\n{out}"
        );
        for tick in 0..SPINNER_FRAMES.len() as u64 {
            assert!(
                !out.contains(spinner_frame(tick)),
                "snapshot must not render any spinner frame ({tick}):\n{out}"
            );
        }
    }

    #[test]
    fn live_board_flashes_a_just_completed_card_then_hides_it() {
        // A finished single-task feature: steady state hides the closed card and
        // (per the feature-skip rule) the whole feature; the completing reload
        // still flashes the card with the tick and shows the 100% header.
        let cards = vec![
            feat("auth", "Auth"),
            work("task-1", "auth", "hash passwords", "verified"),
        ];
        let hidden = format_board_opts(&cards, &BTreeSet::new(), None, Some(0), &BTreeSet::new());
        assert!(
            !hidden.contains("hash passwords"),
            "steady state hides the closed card even as a feature's only task (ac-4):\n{hidden}"
        );
        let mut just = BTreeSet::new();
        just.insert("task-1".to_string());
        let flash = format_board_opts(&cards, &BTreeSet::new(), None, Some(0), &just);
        assert!(
            flash.contains("Auth: 1/1 done (100%)"),
            "a feature whose last task just completed still shows its header on the flash reload:\n{flash}"
        );
        let row = flash
            .lines()
            .find(|l| l.contains("hash passwords"))
            .expect("just-completed card must flash one tick frame");
        assert!(
            row.contains('\u{2713}') && row.contains("done"),
            "the flash row carries the tick glyph (ac-8):\n{flash}"
        );
    }

    #[test]
    fn paint_uses_cursor_home_and_never_full_screen_clear() {
        let out = paint("Auth: 1/2 done\n  row\n");
        assert!(out.starts_with("\x1b[H"), "must home the cursor:\n{out:?}");
        assert!(
            !out.contains("\x1b[2J"),
            "must never full-screen clear (would strobe at the render tick):\n{out:?}"
        );
        assert!(
            out.contains("\x1b[K"),
            "should erase each overwritten line to end-of-line:\n{out:?}"
        );
    }

    use super::live_footer;

    #[test]
    fn board_groups_loose_workable_cards_under_no_feature() {
        // A workable card with no parent feature has nowhere to live on the
        // feature-grouped board; it must surface under a "(no feature)" group so
        // active loose work is visible instead of producing a blank screen.
        let mut active = loose("card-a1", "repeatable prove", "in_progress");
        active.claimed_by = Some("maestro".to_string());
        let waiting = loose("card-b1", "prune retired skills", "ready");
        let cards = vec![active, waiting];
        let mut blocked = BTreeSet::new();
        blocked.insert("card-b1".to_string());
        let out = format_board(&cards, &blocked, None);
        assert!(
            out.contains("(no feature):"),
            "loose cards need a (no feature) group:\n{out}"
        );
        assert!(
            out.contains("0/2 done (0%) | ready 0 | active 1 | needs_verification 0 | blocked 1"),
            "orphan header counts only open loose cards:\n{out}"
        );
        assert!(
            out.contains("repeatable prove") && out.contains("maestro"),
            "active loose row carries its title and claimant:\n{out}"
        );
        assert!(
            out.contains("prune retired skills"),
            "blocked loose row missing:\n{out}"
        );
    }

    #[test]
    fn board_hides_closed_loose_cards_from_no_feature_group() {
        // A closed loose card is not open work: it must not render nor inflate
        // the group total, and an all-closed orphan set yields no group at all.
        let cards = vec![loose("card-done", "old loose chore", "verified")];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            !out.contains("old loose chore"),
            "closed loose card must not show:\n{out}"
        );
        assert!(
            !out.contains("(no feature)"),
            "an all-closed orphan set yields no group:\n{out}"
        );
    }

    #[test]
    fn board_shows_empty_state_when_nothing_is_open() {
        // No open feature work and no open loose work: the board must still
        // render a status line, never empty output (a blank live screen reads
        // as a hang, the bug this fixes).
        let cards = vec![
            feat("design", "Design only"),
            child(
                "dec-1",
                "design",
                card::schema::CardType::Decision,
                "a fork",
                "locked",
            ),
        ];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            !out.trim().is_empty(),
            "empty board must still render a line:\n{out:?}"
        );
        assert!(
            out.contains("No open work"),
            "empty board needs an explanatory hint:\n{out}"
        );
    }

    #[test]
    fn board_focus_excludes_the_no_feature_group() {
        // Focus renders exactly one feature; loose cards are not part of it.
        let cards = vec![
            feat("auth", "Auth"),
            work("task-1", "auth", "hash passwords", "ready"),
            loose("card-loose", "loose task", "in_progress"),
        ];
        let out = format_board(&cards, &BTreeSet::new(), Some("auth"));
        assert!(
            out.contains("Auth"),
            "focused feature header missing:\n{out}"
        );
        assert!(
            !out.contains("(no feature)"),
            "focus must not show the orphan group:\n{out}"
        );
        assert!(
            !out.contains("loose task"),
            "focus must not leak loose rows:\n{out}"
        );
    }

    #[test]
    fn board_renders_a_feature_group_and_the_no_feature_group_together() {
        // The shipped per-feature overview and the loose-card group coexist: an
        // open feature renders its header and rows, and loose open work still
        // appears under "(no feature)" after it (features first).
        let cards = vec![
            feat("auth", "Auth"),
            work("task-1", "auth", "hash passwords", "ready"),
            loose("card-loose", "loose task", "in_progress"),
        ];
        let out = format_board(&cards, &BTreeSet::new(), None);
        assert!(
            out.contains("Auth:"),
            "feature group header missing:\n{out}"
        );
        assert!(
            out.contains("hash passwords"),
            "feature row missing:\n{out}"
        );
        assert!(
            out.contains("(no feature):"),
            "orphan group missing:\n{out}"
        );
        assert!(out.contains("loose task"), "loose row missing:\n{out}");
        assert!(
            out.find("Auth:").unwrap() < out.find("(no feature):").unwrap(),
            "feature groups must render before the loose group:\n{out}"
        );
    }

    #[test]
    fn live_footer_shows_the_interval_and_exit_hint() {
        assert_eq!(live_footer(2), "(live; refreshes every 2s; Ctrl-C to exit)");
        assert_eq!(live_footer(5), "(live; refreshes every 5s; Ctrl-C to exit)");
    }

    use super::{build_board_layout, mode_label, session_line, who_label};
    use crate::domain::run::{Presence, SessionActivity};
    use std::collections::BTreeMap;

    fn session(
        id: &str,
        mode: Option<&str>,
        card: Option<&str>,
        presence: Presence,
        age: u64,
    ) -> SessionActivity {
        SessionActivity {
            session_id: id.to_string(),
            mode: mode.map(str::to_string),
            bound_card: card.map(str::to_string),
            last_action: "card_touch".to_string(),
            last_ts: "t0".to_string(),
            age_minutes: age,
            presence,
        }
    }

    fn board(cards: &[card::schema::Card], sessions: &[SessionActivity]) -> String {
        build_board_layout(cards, &BTreeSet::new(), None, &BTreeSet::new(), sessions).render(None)
    }

    #[test]
    fn who_label_takes_the_session_token_not_the_worktree() {
        // Strip the union `@worktree` suffix and the trailing id, so concurrent
        // sessions read apart instead of all collapsing to the worktree name.
        assert_eq!(
            who_label("claude#s66637-1781925049@maestro"),
            "claude#s66637"
        );
        assert_eq!(who_label("f08020da-b6e9-41a8@maestro"), "f08020da");
        assert_eq!(who_label("plain"), "plain");
    }

    #[test]
    fn mode_label_strips_the_maestro_prefix() {
        assert_eq!(mode_label("maestro-design"), "design");
        assert_eq!(mode_label("design"), "design");
    }

    #[test]
    fn session_line_shows_mode_card_who_presence_and_age() {
        let cards = [feat("rcli", "Reduce CLI token bloat")];
        let by_id = cards
            .iter()
            .map(|c| (c.id.as_str(), c))
            .collect::<BTreeMap<_, _>>();
        let line = session_line(
            &session(
                "f08020da-b6e9@maestro",
                Some("maestro-design"),
                Some("rcli"),
                Presence::Working,
                2,
            ),
            &by_id,
        );
        assert_eq!(
            line, "  design  Reduce CLI token bloat  f08020da [working] 2m",
            "session line shape changed:\n{line}"
        );
    }

    #[test]
    fn session_line_marks_a_bound_card_absent_from_the_scan() {
        let by_id = BTreeMap::new();
        let line = session_line(
            &session(
                "s1",
                Some("maestro-design"),
                Some("gone"),
                Presence::Idle,
                9,
            ),
            &by_id,
        );
        assert!(
            line.contains("gone (missing)"),
            "missing card not flagged:\n{line}"
        );
        assert!(line.contains("[idle]"), "presence label wrong:\n{line}");
    }

    #[test]
    fn live_sessions_block_renders_header_and_separator() {
        let cards = vec![loose("card-loose", "loose task", "in_progress")];
        let sessions = vec![session(
            "f08020da@maestro",
            Some("maestro-design"),
            None,
            Presence::Working,
            1,
        )];
        let out = board(&cards, &sessions);
        assert!(
            out.contains("LIVE SESSIONS (1)"),
            "block header missing:\n{out}"
        );
        assert!(out.contains('\u{2500}'), "separator rule missing:\n{out}");
        assert!(
            out.find("LIVE SESSIONS").unwrap() < out.find("(no feature):").unwrap(),
            "sessions block must render above the groups:\n{out}"
        );
    }

    #[test]
    fn no_sessions_means_no_block() {
        let cards = vec![loose("card-loose", "loose task", "in_progress")];
        let out = board(&cards, &[]);
        assert!(
            !out.contains("LIVE SESSIONS"),
            "block leaked with no sessions:\n{out}"
        );
    }

    #[test]
    fn focus_mode_omits_the_live_sessions_block() {
        // Focus is a single-feature drill-down: a session bound to a sibling
        // feature must not leak into the focused view (regression: an
        // env-derived session surfaced "Other" inside `watch snapshot billing`).
        let cards = vec![feat("billing", "Billing CSV"), feat("other", "Other")];
        let sessions = vec![session(
            "7523131a@maestro",
            None,
            Some("other"),
            Presence::Working,
            0,
        )];
        let out = build_board_layout(
            &cards,
            &BTreeSet::new(),
            Some("billing"),
            &BTreeSet::new(),
            &sessions,
        )
        .render(None);
        assert!(
            !out.contains("LIVE SESSIONS"),
            "focus mode must omit the global session block:\n{out}"
        );
        assert!(
            !out.contains("Other"),
            "focus must exclude sibling features even via sessions:\n{out}"
        );
        assert!(
            out.contains("Billing CSV"),
            "focused feature still renders its header:\n{out}"
        );
    }

    #[test]
    fn live_feature_with_no_tasks_renders_alt_header() {
        // A design-only feature is hidden in the overview, but a live session
        // bound to it surfaces it with a `<title>: <mode> (live session)` header.
        let cards = vec![feat("rcli", "Reduce CLI token bloat")];
        let sessions = vec![session(
            "f08020da@maestro",
            Some("maestro-design"),
            Some("rcli"),
            Presence::Working,
            1,
        )];
        let out = board(&cards, &sessions);
        assert!(
            out.contains("Reduce CLI token bloat: design (live session)"),
            "live design-only feature header missing:\n{out}"
        );
        assert!(
            !out.contains("0/0 done"),
            "design-only live feature must not show a counts line:\n{out}"
        );
    }

    #[test]
    fn feature_with_no_session_or_tasks_stays_hidden() {
        let cards = vec![feat("idle", "Idle Feature")];
        let out = board(&cards, &[]);
        assert!(
            !out.contains("Idle Feature"),
            "idle design-only feature must stay hidden:\n{out}"
        );
    }

    #[test]
    fn live_feature_with_tasks_keeps_its_counts_header() {
        let cards = vec![
            feat("auth", "Auth"),
            work("task-1", "auth", "hash passwords", "ready"),
        ];
        let sessions = vec![session(
            "f08020da@maestro",
            Some("maestro-design"),
            Some("auth"),
            Presence::Working,
            1,
        )];
        let out = board(&cards, &sessions);
        assert!(
            out.contains("Auth: 0/1 done"),
            "counts header missing:\n{out}"
        );
        assert!(out.contains("hash passwords"), "task row missing:\n{out}");
        assert!(
            !out.contains("Auth: design (live session)"),
            "a feature with tasks must keep its counts header, not the note:\n{out}"
        );
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
