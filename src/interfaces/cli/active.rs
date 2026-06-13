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
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::ActiveArgs;

/// Max width for the bound-card title column; longer titles truncate to keep one
/// scannable line per session (row width is a tunable detail, not locked by D5).
const CARD_WIDTH: usize = 28;

pub fn run(args: ActiveArgs) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let now = utc_now_timestamp();
    let rows = run::active_sessions(&paths, &now)?;

    let cards = if paths.cards_dir().is_dir() {
        card::query::scan(&paths)?
    } else {
        Vec::new()
    };
    let by_id: HashMap<&str, &card::schema::Card> =
        cards.iter().map(|card| (card.id.as_str(), card)).collect();

    let me = super::cli_run_id();

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
    render_table(&shown, &by_id, &cards, &me);

    if hidden_stale > 0 {
        println!();
        println!("({hidden_stale} stale hidden; --all to show)");
    }

    render_link_hint(&rows, &shown, &me);
    Ok(())
}

/// The display cells for one session row, in column order.
struct Cells {
    session: String,
    mode: String,
    card: String,
    status: String,
    progress: String,
    age: String,
    state: String,
    last_action: String,
    you: bool,
}

fn render_table(
    shown: &[&SessionActivity],
    by_id: &HashMap<&str, &card::schema::Card>,
    cards: &[card::schema::Card],
    me: &str,
) {
    let rows: Vec<Cells> = shown
        .iter()
        .map(|row| cells_for(row, by_id, cards, me))
        .collect();

    let headers = [
        "SESSION",
        "MODE",
        "CARD",
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
        let line = render_columns(&columns, &widths);
        if cell.you {
            println!("{line}  <- you");
        } else {
            println!("{line}");
        }
    }
}

impl Cells {
    fn columns(&self) -> [&str; 8] {
        [
            &self.session,
            &self.mode,
            &self.card,
            &self.status,
            &self.progress,
            &self.age,
            &self.state,
            &self.last_action,
        ]
    }
}

fn print_columns(values: &[&str; 8], widths: &[usize]) {
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

    Cells {
        session: row.session_id.clone(),
        mode: row.mode.as_deref().map(mode_label).unwrap_or_else(dash),
        card,
        status: if status.is_empty() { dash() } else { status },
        progress: if progress.is_empty() {
            dash()
        } else {
            progress
        },
        age: format!("{}m", row.age_minutes),
        state: presence_label(row.presence, row.age_minutes),
        last_action: row.last_action.clone(),
        you: row.session_id == me,
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

/// Print the copy-pasteable link hint (D7): one `maestro link add` line per shown
/// peer that has a bound card, referencing the peer's real card id. `<your-card>`
/// is filled with the running session's bound card when it has one, else stays a
/// literal placeholder (the verb run as a first step has no card yet). maestro
/// never auto-links and never guesses relatedness -- the agent judges and runs.
fn render_link_hint(all_rows: &[SessionActivity], shown: &[&SessionActivity], me: &str) {
    let peers: Vec<&str> = shown
        .iter()
        .filter(|row| row.session_id != me)
        .filter_map(|row| row.bound_card.as_deref())
        .collect();
    if peers.is_empty() {
        return;
    }

    let your_card = all_rows
        .iter()
        .find(|row| row.session_id == me)
        .and_then(|row| row.bound_card.as_deref())
        .unwrap_or("<your-card>");

    println!();
    println!("related? link your card to theirs:");
    for their_card in peers {
        println!("  maestro link add {your_card} {their_card}");
    }
}
