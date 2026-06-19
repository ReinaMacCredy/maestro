//! `maestro conflict`: assert (or `--clear`) a link-free "I am taking this
//! ground, hold off" notice aimed at a peer card, and the ambient `[CONFLICT]`
//! banner that surfaces it on the peer's next command.
//!
//! The verb never writes a link edge and never runs git -- a notice is a
//! transient coordination signal, persisted by the `conflict` store and read
//! cross-worktree via its union (`domain::conflict`). Visibility is
//! liveness-scoped: the banner shows a notice only while its asserter is a
//! non-stale session in the active union and the asserting card is not terminal
//! (`dec-conflict-notice-lifetime-scoped-to-c5eb`). The peer never has to erase
//! someone else's notice; it fades when the asserter clears it, finishes its
//! card, or falls out of the live union.

use std::collections::HashMap;

use anyhow::{Result, anyhow};

use crate::domain::card;
use crate::domain::conflict::{self, Notice};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{ConflictArgs, worktree_roots};

pub fn run(args: ConflictArgs) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let peer = resolve_peer_id(&paths, &args.peer)?;
    if args.clear {
        return clear(&paths, &peer);
    }
    let reason = args.reason.as_deref().filter(|reason| !reason.trim().is_empty());
    let Some(reason) = reason else {
        return Err(anyhow!(
            "a conflict needs a reason: maestro conflict {peer} \"<why you are taking it>\" (or --clear to retract)"
        ));
    };
    assert(&paths, &peer, reason)
}

/// Assert a conflict notice from the running session's current card against
/// `peer`. Requires a current card so the notice reads "my card -> taking ->
/// peer card"; the session id is what the liveness gate tracks.
fn assert(paths: &MaestroPaths, peer: &str, reason: &str) -> Result<()> {
    let me = current_card(paths)?;
    conflict::assert(paths, &super::cli_run_id(), &me, peer, reason)?;
    println!(
        "conflict noted: holding {peer} (from {me}). retract with `maestro conflict --clear {peer}`"
    );
    Ok(())
}

/// Retract the notice this session asserted against `peer`. Only the asserter
/// clears its own notice; a peer never erases someone else's.
fn clear(paths: &MaestroPaths, peer: &str) -> Result<()> {
    conflict::clear(paths, &super::cli_run_id(), peer)?;
    println!("conflict cleared: released {peer}");
    Ok(())
}

/// The ambient `[CONFLICT]` banner: a one-line STDERR reminder, before any
/// command, for each live notice aimed at the running card. Silent unless in a
/// repo with a current card that some live peer has asserted against. STDERR
/// only, so JSON stdout stays clean. Best-effort: the caller ignores any error.
pub(super) fn conflict_banner() -> Result<()> {
    let Ok(root) = discover_repo_root() else {
        return Ok(());
    };
    let paths = MaestroPaths::new(root);
    let Some(me) = super::current_card(&paths) else {
        return Ok(());
    };
    let now = utc_now_timestamp();
    let roots = worktree_roots(&paths);

    let notices = conflict::active_notices(&roots, &now)?;
    let mine: Vec<&Notice> = notices.iter().filter(|notice| notice.peer_card == me).collect();
    if mine.is_empty() {
        return Ok(());
    }

    // Local card scan for the terminal gate only. The asserter's card may be
    // absent here -- uncommitted in its own worktree -- so absent reads as
    // not-terminal and the liveness gate (which crosses worktrees) governs.
    let cards = if paths.cards_dir().is_dir() {
        card::query::scan(&paths).unwrap_or_default()
    } else {
        Vec::new()
    };
    let by_id: HashMap<&str, &card::schema::Card> =
        cards.iter().map(|card| (card.id.as_str(), card)).collect();

    let mut printed = false;
    for notice in mine {
        if asserter_card_terminal(&notice.asserter_card, &by_id) {
            continue;
        }
        let who = if notice.asserter_card.is_empty() {
            notice.asserter_session.as_str()
        } else {
            notice.asserter_card.as_str()
        };
        eprintln!("[CONFLICT] {who} holds {me}: {}", notice.reason);
        printed = true;
    }
    if printed {
        eprintln!("           -> hold off the shared file until the notice clears");
    }
    Ok(())
}

/// Whether the asserter's card is terminal (coarse-Closed) in the viewer's local
/// scan, so a finished assertion stops surfacing (AC-2). An empty card id or a
/// card absent from the scan is NOT terminal: card state does not cross the
/// worktree boundary the way run events do, so a missing card leaves the
/// liveness gate in charge (advisor blocker: never hide on a not-found card).
fn asserter_card_terminal(asserter_card: &str, by_id: &HashMap<&str, &card::schema::Card>) -> bool {
    if asserter_card.is_empty() {
        return false;
    }
    by_id.get(asserter_card).is_some_and(|card| {
        card::query::coarse_of(&card.status) == Some(card::query::Coarse::Closed)
    })
}

/// Resolve `peer` to its canonical card id, validating it exists (trust
/// boundary). Storing the canonical id is what lets the banner match a notice's
/// `peer_card` against the viewer's bound card exactly.
fn resolve_peer_id(paths: &MaestroPaths, peer: &str) -> Result<String> {
    if let Some(resolved) = card::store::resolve(paths, peer)? {
        return Ok(resolved.card.id);
    }
    let ids = card::query::scan(paths).unwrap_or_default();
    match card::suggest::did_you_mean(peer, ids.iter().map(|card| card.id.as_str())) {
        Some(near) => Err(anyhow!("no card {peer}; did you mean {near}?")),
        None => Err(anyhow!("no card {peer} to flag a conflict against")),
    }
}

/// The running session's current card, or an actionable error: a conflict is
/// asserted on behalf of the card you are working, so one must be bound.
fn current_card(paths: &MaestroPaths) -> Result<String> {
    super::current_card(paths).ok_or_else(|| {
        anyhow!(
            "no current card in this session; claim or touch a card first, then run `maestro conflict`"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use card::schema::{Card, CardType};

    fn closed(id: &str) -> Card {
        Card::new(id, CardType::Task, id, "closed", "t0")
    }

    fn open(id: &str) -> Card {
        Card::new(id, CardType::Task, id, "in_progress", "t0")
    }

    #[test]
    fn terminal_gate_hides_only_a_found_closed_asserter_card() {
        let closed_card = closed("card-done");
        let open_card = open("card-live");
        let by_id: HashMap<&str, &Card> = [
            (closed_card.id.as_str(), &closed_card),
            (open_card.id.as_str(), &open_card),
        ]
        .into_iter()
        .collect();

        // A found, closed asserter card is terminal -> its notice is suppressed.
        assert!(asserter_card_terminal("card-done", &by_id));
        // A found, open asserter card is not terminal -> notice stays.
        assert!(!asserter_card_terminal("card-live", &by_id));
        // Absent from the local scan (uncommitted in the asserter's worktree) is
        // NOT terminal -- liveness governs, not a missing card.
        assert!(!asserter_card_terminal("card-elsewhere", &by_id));
        // No asserter card recorded -> not terminal.
        assert!(!asserter_card_terminal("", &by_id));
    }
}
