//! `maestro active`: a pull-only view of what other live sessions are doing,
//! indexed by run-event liveness and enriched from the card store.
//!
//! The verb never writes and never creates a link edge -- it reads
//! `run::active_sessions`, joins each bound card's title/status/progress from a
//! single card scan, prints one row per session, and emits a copy-pasteable
//! `maestro link add` hint the agent decides whether to run
//! (`dec-link-follow-up-copy-pasteable-hint-5b33`, `dec-awareness-view-is-an-explicit-verb-not-3092`).

use std::collections::HashMap;

use anyhow::Result;

use crate::domain::card;
use crate::domain::run::{self, Presence, SessionActivity};
use crate::foundation::core::git;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::ActiveArgs;

/// Max width for the bound-card title column; longer titles truncate to keep one
/// scannable line per session (row width is a tunable detail, not locked by D5).
const CARD_WIDTH: usize = 28;

pub fn run(args: ActiveArgs) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let now = utc_now_timestamp();
    let rows = run::active_sessions_union(&worktree_roots(&paths), &now)?;

    let cards = if paths.cards_dir().is_dir() {
        card::query::scan(&paths)?
    } else {
        Vec::new()
    };
    let by_id: HashMap<&str, &card::schema::Card> =
        cards.iter().map(|card| (card.id.as_str(), card)).collect();

    let me = super::cli_run_id();
    let your_card = rows
        .iter()
        .find(|row| row.session_id == me)
        .and_then(|row| row.bound_card.as_deref());

    let shown: Vec<&SessionActivity> = rows
        .iter()
        .filter(|row| args.all || row.presence != Presence::Stale)
        .collect();
    let hidden_stale = rows.len() - shown.len();

    if shown.is_empty() {
        println!("No active sessions.");
        if hidden_stale > 0 {
            println!("({hidden_stale} stale; --all to show)");
        }
        return Ok(());
    }

    println!(
        "{} active session{}:",
        shown.len(),
        if shown.len() == 1 { "" } else { "s" }
    );
    println!();
    render_table(&shown, &by_id, &cards, &me, your_card);

    if hidden_stale > 0 {
        println!();
        println!("({hidden_stale} stale hidden; --all to show)");
    }

    render_link_hint(&shown, &by_id, &me, your_card);
    Ok(())
}

/// The worktree roots to union liveness over: every worktree the repo has, or
/// just the local root when git topology is unreadable (not a repo, bare, etc.).
/// `git::worktree_roots` returns one root for a lone repo, so the single-worktree
/// view is unchanged and no flag is needed to engage the union.
fn worktree_roots(paths: &MaestroPaths) -> Vec<MaestroPaths> {
    match git::worktree_roots(paths.repo_root()) {
        Ok(roots) if !roots.is_empty() => roots.into_iter().map(MaestroPaths::new).collect(),
        _ => vec![MaestroPaths::new(paths.repo_root().to_path_buf())],
    }
}

/// Whether the live cards `a` and `b` share a `related` edge in either
/// direction, delegating to the domain predicate so the LINK column and the
/// `msg`/banner gate read relatedness the same way. Both must be in the live
/// scan; a peer absent from it (e.g. archived) reads as not-linked here -- the
/// archive-aware check lives in `card::query::pair_linked` for the verbs.
fn related_pair(by_id: &HashMap<&str, &card::schema::Card>, a: &str, b: &str) -> bool {
    match (by_id.get(a), by_id.get(b)) {
        (Some(a_card), Some(b_card)) => card::query::cards_related(a_card, b_card),
        _ => false,
    }
}

/// Whether a peer's bound card is terminal (coarse-Closed) in the live scan, so
/// `active`'s link hint must not suggest opening a link the guard would refuse
/// (`dec-terminal-card-link-msg-keep-the-live-5878`).
fn peer_terminal(by_id: &HashMap<&str, &card::schema::Card>, peer: &str) -> bool {
    by_id.get(peer).is_some_and(|card| {
        card::query::coarse_of(&card.status) == Some(card::query::Coarse::Closed)
    })
}

/// The display cells for one session row, in column order.
struct Cells {
    session: String,
    mode: String,
    card: String,
    link: String,
    status: String,
    progress: String,
    age: String,
    state: String,
    last_action: String,
}

fn render_table(
    shown: &[&SessionActivity],
    by_id: &HashMap<&str, &card::schema::Card>,
    cards: &[card::schema::Card],
    me: &str,
    your_card: Option<&str>,
) {
    let rows: Vec<Cells> = shown
        .iter()
        .map(|row| cells_for(row, by_id, cards, me, your_card))
        .collect();

    let headers = [
        "SESSION",
        "MODE",
        "CARD",
        "LINK",
        "STATUS",
        "PROGRESS",
        "AGE",
        "STATE",
        "LAST ACTION",
    ];
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for cell in &rows {
        for (index, value) in cell.columns().iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    print_columns(&headers, &widths);
    for cell in &rows {
        let columns = cell.columns();
        println!("{}", render_columns(&columns, &widths));
    }
}

impl Cells {
    fn columns(&self) -> [&str; 9] {
        [
            &self.session,
            &self.mode,
            &self.card,
            &self.link,
            &self.status,
            &self.progress,
            &self.age,
            &self.state,
            &self.last_action,
        ]
    }
}

fn print_columns(values: &[&str; 9], widths: &[usize]) {
    println!("{}", render_columns(values, widths));
}

fn render_columns(values: &[&str], widths: &[usize]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| format!("{value:<width$}", width = widths[index]))
        .collect::<Vec<_>>()
        .join("  ")
        .trim_end()
        .to_string()
}

fn cells_for(
    row: &SessionActivity,
    by_id: &HashMap<&str, &card::schema::Card>,
    cards: &[card::schema::Card],
    me: &str,
    your_card: Option<&str>,
) -> Cells {
    let (card, status, progress) = match &row.bound_card {
        Some(id) => match by_id.get(id.as_str()) {
            Some(card) => (
                truncate(&card.title, CARD_WIDTH),
                card.status.clone(),
                progress_for(&card.id, cards),
            ),
            None => (format!("{id} (missing)"), dash(), String::new()),
        },
        None => (dash(), dash(), String::new()),
    };

    let link = if row.session_id == me {
        "(you)".to_string()
    } else {
        match (your_card, row.bound_card.as_deref()) {
            (Some(mine), Some(peer)) if related_pair(by_id, mine, peer) => "linked".to_string(),
            _ => dash(),
        }
    };

    Cells {
        session: row.session_id.clone(),
        mode: row.mode.as_deref().map(mode_label).unwrap_or_else(dash),
        card,
        link,
        status: if status.is_empty() { dash() } else { status },
        progress: if progress.is_empty() {
            dash()
        } else {
            progress
        },
        age: format!("{}m", row.age_minutes),
        state: presence_label(row.presence, row.age_minutes),
        last_action: row.last_action.clone(),
    }
}

/// Type-aware progress from the bound card's children: tasks done/total when any
/// workable child exists, else the locked-decision count, else blank. Keys on
/// the children present rather than the skill mode, so a design-stage feature
/// (decisions, no tasks) and an impl-stage feature (tasks) each read correctly.
/// "done" is the `verified` terminal, matching `feature list`'s fraction.
fn progress_for(card_id: &str, cards: &[card::schema::Card]) -> String {
    let children: Vec<&card::schema::Card> = cards
        .iter()
        .filter(|card| card.parent.as_deref() == Some(card_id))
        .collect();

    let total = children.iter().filter(|c| c.card_type.workable()).count();
    if total > 0 {
        let done = children
            .iter()
            .filter(|c| c.card_type.workable() && c.status == "verified")
            .count();
        return format!("{done}/{total} tasks");
    }

    let locked = children
        .iter()
        .filter(|c| c.card_type == card::schema::CardType::Decision && c.status == "locked")
        .count();
    if locked > 0 {
        return format!("{locked} decisions");
    }
    String::new()
}

/// The skill mode with the `maestro-` prefix stripped (`maestro-design` ->
/// `design`). Derived from the real skill name; no skill->lane lookup table.
fn mode_label(skill: &str) -> String {
    skill.strip_prefix("maestro-").unwrap_or(skill).to_string()
}

fn presence_label(presence: Presence, age_minutes: u64) -> String {
    match presence {
        Presence::Working => "[working]".to_string(),
        Presence::Waiting => "[waiting]".to_string(),
        Presence::Idle => format!("[idle {age_minutes}m]"),
        Presence::Stale => format!("[stale {age_minutes}m]"),
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let head: String = value.chars().take(max.saturating_sub(1)).collect();
    format!("{head}.")
}

fn dash() -> String {
    "-".to_string()
}

/// Print the copy-pasteable addressing footer (D7), link-aware and now
/// addressing-complete (`dec-active-addressing-surface-the-peer-s-b739`): every
/// peer's full card id is reachable in a ready-to-paste command, never just the
/// truncated CARD title. Linked peers get a `maestro msg send <their-card>`
/// template (they are already messageable); unlinked live peers get the
/// `maestro link add` line followed by the same send template (link, then
/// message). `<your-card>` is filled with the running session's bound card when
/// it has one, else stays a literal placeholder (the verb run as a first step
/// has no card yet). maestro never auto-links and never guesses relatedness.
fn render_link_hint(
    shown: &[&SessionActivity],
    by_id: &HashMap<&str, &card::schema::Card>,
    me: &str,
    your_card: Option<&str>,
) {
    let peers: Vec<&str> = shown
        .iter()
        .filter(|row| row.session_id != me)
        .filter_map(|row| row.bound_card.as_deref())
        .filter(|peer| your_card != Some(*peer))
        .collect();
    if peers.is_empty() {
        return;
    }

    // Without a bound card the running session cannot be linked to anyone, so
    // every peer reads as a suggestion against the <your-card> placeholder.
    let (linked, unlinked): (Vec<&str>, Vec<&str>) = peers
        .iter()
        .copied()
        .partition(|peer| your_card.is_some_and(|mine| related_pair(by_id, mine, peer)));

    // Never suggest opening a link the guard will refuse: a peer bound to a
    // terminal (coarse-Closed) card is dropped from the suggestion list. An
    // already-linked terminal peer is unaffected -- it stays in `linked` and
    // still renders 'linked' (`dec-terminal-card-link-msg-keep-the-live-5878`).
    // A cross-worktree peer whose card is absent from this checkout cannot be
    // linked (link add resolves ids in the local store), so it gets no link
    // suggestion -- it still renders '<id> (missing)' in the table
    // (`dec-cross-worktree-active-auto-unions-read-51b9`).
    let unlinked: Vec<&str> = unlinked
        .into_iter()
        .filter(|peer| !peer_terminal(by_id, peer))
        .filter(|peer| by_id.contains_key(*peer))
        .collect();

    if !linked.is_empty() {
        println!();
        println!("linked -- message them:");
        for their_card in &linked {
            println!("  maestro msg send {their_card} \"<text>\"");
        }
    }

    if !unlinked.is_empty() {
        let your = your_card.unwrap_or("<your-card>");
        println!();
        println!("related? link, then message:");
        for their_card in &unlinked {
            println!("  maestro link add {your} {their_card}");
            println!("  maestro msg send {their_card} \"<text>\"");
        }
    }
}
