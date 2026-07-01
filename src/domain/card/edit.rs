//! Write-side card edits: mutations that load a card,
//! change it, and persist through the CAS seam (D1). The read-side counterpart
//! is `query`; both sit above the `store` persistence seam.

use anyhow::{Context, Result, bail};

use crate::domain::card::query::{Coarse, coarse_of, has_related_to};
use crate::domain::card::schema::{Card, Dep, DepKind};
use crate::domain::card::store::{
    CARD_FILE, ResolvedCard, load, locate, resolve, save_resolved, validate_card_id,
};
use crate::foundation::core::fs::append_text_file;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::timestamp_nanos;

/// A claim older than this is stale and may be re-claimed (SPEC O2). Reuses the
/// 15-minute write-lock staleness precedent (`fs.rs` `STALE_WRITE_LOCK_AGE`); a
/// dead session must never pin a card forever (SPEC E6).
pub const STALE_CLAIM_AGE_SECONDS: u64 = 15 * 60;
const STALE_CLAIM_AGE_NANOS: i128 = STALE_CLAIM_AGE_SECONDS as i128 * 1_000_000_000;

/// Add a `blocks` edge so `child` waits on `parent` (SPEC E1/DN6: the edge is
/// stored on the dependent and gates only its `ready`). Mirrors
/// `bd dep add child parent` -- `child` is the dependent, `parent` the blocker.
///
/// Validates at the user-input boundary: a card cannot block itself, and both
/// cards must exist (a dep to a missing card would dangle, which the card-mode
/// doctor flags under E5 -- failing here keeps the bad ref from being written
/// at all). Idempotent: a second identical edge is a no-op. Returns whether a
/// new edge was written.
pub fn add_blocks_dep(paths: &MaestroPaths, child: &str, parent: &str, now: &str) -> Result<bool> {
    let resolved = resolve_dep_child(paths, child, parent, "add a dependency to")?;
    let mut card = resolved.card.clone();
    if card
        .deps
        .iter()
        .any(|dep| dep.kind == DepKind::Blocks && dep.target == parent)
    {
        return Ok(false);
    }

    card.deps.push(Dep {
        kind: DepKind::Blocks,
        target: parent.to_string(),
    });
    card.updated_at = now.to_string();
    save_resolved(&card, &resolved)?;
    Ok(true)
}

/// Remove the `blocks` edge that makes `child` wait on `parent` -- the inverse
/// of `add_blocks_dep`. The edge is directional and stored on the dependent, so
/// argument order matters here (unlike `link remove`): `remove parent child`
/// finds no edge on `parent` and is a no-op. Validates the same input boundary
/// as `add` (no self-edge; both cards exist). Idempotent: removing an absent
/// edge is a no-op. Returns whether an edge was removed.
pub fn remove_blocks_dep(
    paths: &MaestroPaths,
    child: &str,
    parent: &str,
    now: &str,
) -> Result<bool> {
    let resolved = resolve_dep_child(paths, child, parent, "remove a dependency from")?;
    let mut card = resolved.card.clone();
    let before = card.deps.len();
    card.deps
        .retain(|dep| dep.kind != DepKind::Blocks || dep.target != parent);
    if card.deps.len() == before {
        return Ok(false);
    }
    card.updated_at = now.to_string();
    save_resolved(&card, &resolved)?;
    Ok(true)
}

/// Resolve `child` for a `blocks`-dep edit after the shared input-boundary
/// checks both `add` and `remove` enforce: well-formed ids, no self-edge, and an
/// existing `parent`. `action` names the edit in the not-found message.
fn resolve_dep_child(
    paths: &MaestroPaths,
    child: &str,
    parent: &str,
    action: &str,
) -> Result<ResolvedCard> {
    validate_card_id(child)?;
    validate_card_id(parent)?;
    if child == parent {
        bail!("a card cannot block itself: {child}");
    }
    if locate(paths, parent)?.is_none() {
        bail!("no card {parent} to depend on");
    }
    match resolve(paths, child)? {
        Some(resolved) => Ok(resolved),
        None => bail!("no card {child} to {action}"),
    }
}

/// Add a non-blocking `related` edge between two live cards. The relation is
/// user-facing unordered, but storage stays one-edge: the first successful add
/// writes the edge to `from`; a later reverse add is a no-op.
pub fn add_related_link(paths: &MaestroPaths, from: &str, to: &str, now: &str) -> Result<bool> {
    validate_related_pair(from, to)?;
    let Some(from_resolved) = resolve(paths, from)? else {
        bail!("no live card {from} to link to");
    };
    let Some(to_resolved) = resolve(paths, to)? else {
        bail!("no live card {to} to link to");
    };

    guard_linkable(&from_resolved.card)?;
    guard_linkable(&to_resolved.card)?;

    let mut from_card = from_resolved.card.clone();
    if has_related_to(&from_card, to) || has_related_to(&to_resolved.card, from) {
        return Ok(false);
    }

    from_card.deps.push(Dep {
        kind: DepKind::Related,
        target: to.to_string(),
    });
    from_card.updated_at = now.to_string();
    save_resolved(&from_card, &from_resolved)?;
    Ok(true)
}

/// Remove a non-blocking `related` edge between two live cards. Because related
/// links are unordered at the CLI, either argument order removes the one stored
/// edge.
pub fn remove_related_link(paths: &MaestroPaths, from: &str, to: &str, now: &str) -> Result<bool> {
    validate_related_pair(from, to)?;
    let Some(from_resolved) = resolve(paths, from)? else {
        bail!("no live card {from} to link to");
    };
    let Some(to_resolved) = resolve(paths, to)? else {
        bail!("no live card {to} to link to");
    };

    let mut from_card = from_resolved.card.clone();
    if remove_related_edge(&mut from_card, to) {
        from_card.updated_at = now.to_string();
        save_resolved(&from_card, &from_resolved)?;
        return Ok(true);
    }

    let mut to_card = to_resolved.card.clone();
    if remove_related_edge(&mut to_card, from) {
        to_card.updated_at = now.to_string();
        save_resolved(&to_card, &to_resolved)?;
        return Ok(true);
    }

    Ok(false)
}

/// Refuse to link a terminal card. A related link is a live-coordination
/// signal -- and the seam a linked-card channel rides on -- so a closed
/// card has nothing actionable to coordinate (bl-012). Archived cards are
/// already rejected upstream: `resolve` finds only live cards, so this catches
/// the done-but-not-yet-archived case the resolver still returns.
fn guard_linkable(card: &Card) -> Result<()> {
    if coarse_of(&card.status) == Some(Coarse::Closed) {
        bail!(
            "{} is finished ({}); you can't open a new conversation with a finished card",
            card.id,
            card.status
        );
    }
    Ok(())
}

fn validate_related_pair(from: &str, to: &str) -> Result<()> {
    validate_card_id(from)?;
    validate_card_id(to)?;
    if from == to {
        bail!("a card cannot link to itself: {from}");
    }
    Ok(())
}

fn remove_related_edge(card: &mut Card, target: &str) -> bool {
    let before = card.deps.len();
    card.deps
        .retain(|dep| dep.kind != DepKind::Related || dep.target != target);
    card.deps.len() != before
}

/// What `claim` did to a card, so the CLI can phrase the right line.
#[derive(Debug, Eq, PartialEq)]
pub enum ClaimOutcome {
    /// The card was free; `claimed_by` is now the caller.
    Claimed,
    /// The caller already held it; nothing changed.
    AlreadyMine,
    /// A stale claim was taken over from `previous` (SPEC O2/E6).
    Reclaimed { previous: String },
}

/// Claim a workable card for `claimed_by` (`<agent>#<session>`, SPEC DN8/E6).
///
/// Only task/bug/chore are claimable (SPEC E3); claiming a feature/idea/decision
/// or a closed card is refused at this input boundary. A live claim held by
/// someone else is refused, but a claim older than `STALE_CLAIM_AGE_NANOS` is
/// taken over with a `Reclaimed` outcome so a dead session never pins a card
/// forever (SPEC E6/O2). A successful claim stamps `claimed_at`/`updated_at` and
/// moves the card to `in_progress`.
pub fn claim(paths: &MaestroPaths, id: &str, claimed_by: &str, now: &str) -> Result<ClaimOutcome> {
    validate_card_id(id)?;
    let Some(resolved) = resolve(paths, id)? else {
        bail!("no card {id} to claim");
    };
    let mut card = resolved.card.clone();
    let outcome = apply_claim(&mut card, claimed_by, now)?;
    if outcome != ClaimOutcome::AlreadyMine {
        save_resolved(&card, &resolved)?;
    }
    Ok(outcome)
}

/// The in-memory half of [`claim`]: validate claimability and stamp the claim
/// onto an already-loaded card without persisting it. `update --claim` composes
/// this with its field edits so the combined mutation lands in one CAS write --
/// two sequential saves would let the second silently clobber the first.
/// `AlreadyMine` leaves the card untouched.
pub fn apply_claim(card: &mut Card, claimed_by: &str, now: &str) -> Result<ClaimOutcome> {
    let id = card.id.as_str();
    if !card.card_type.workable() {
        bail!(
            "{id} is a {}, not a workable card; only task/bug/chore are claimable",
            card.card_type.as_str()
        );
    }
    if coarse_of(&card.status) == Some(Coarse::Closed) {
        bail!("{id} is closed ({}); nothing to claim", card.status);
    }

    let outcome = match card.claimed_by.as_deref() {
        Some(holder) if holder == claimed_by => return Ok(ClaimOutcome::AlreadyMine),
        Some(holder) => {
            let stale = card
                .claimed_at
                .as_deref()
                .is_none_or(|at| claim_is_stale(at, now));
            if !stale {
                bail!(
                    "{id} is held by {holder} (claimed {}); not stale yet -- coordinate or wait",
                    card.claimed_at.as_deref().unwrap_or("unknown")
                );
            }
            ClaimOutcome::Reclaimed {
                previous: holder.to_string(),
            }
        }
        None => ClaimOutcome::Claimed,
    };

    card.claimed_by = Some(claimed_by.to_string());
    card.claimed_at = Some(now.to_string());
    card.status = "in_progress".to_string();
    card.updated_at = now.to_string();
    Ok(outcome)
}

/// Set (`who = Some`) or clear (`who = None`) a workable card's advisory
/// `suggested_for` routing hint -- the `assign` verb. Deliberately separate from
/// [`claim`]: it changes no status and never gates claimability, so any session
/// may set or change it and a later claim by anyone still succeeds. Only
/// task/bug/chore carry a hint, since the suggestion routes claimable work. An
/// idempotent set/clear (the stored value already matches) skips the CAS write
/// and returns `false`; a real change returns `true`.
pub fn assign(paths: &MaestroPaths, id: &str, who: Option<&str>, now: &str) -> Result<bool> {
    validate_card_id(id)?;
    let Some(resolved) = resolve(paths, id)? else {
        bail!("no card {id} to assign");
    };
    if !resolved.card.card_type.workable() {
        bail!(
            "{id} is a {}, not a workable card; only task/bug/chore take an assignee hint",
            resolved.card.card_type.as_str()
        );
    }
    let next = who.map(str::to_string);
    if resolved.card.suggested_for == next {
        return Ok(false);
    }
    let mut card = resolved.card.clone();
    card.suggested_for = next;
    card.updated_at = now.to_string();
    save_resolved(&card, &resolved)?;
    Ok(true)
}

/// Whether a claim stamped at `claimed_at` is older than the stale TTL as of
/// `now`. An unparseable/missing timestamp counts as stale so it can never pin a
/// card forever (SPEC E6).
fn claim_is_stale(claimed_at: &str, now: &str) -> bool {
    match (timestamp_nanos(claimed_at), timestamp_nanos(now)) {
        (Some(then), Some(now)) => now.saturating_sub(then) > STALE_CLAIM_AGE_NANOS,
        _ => true,
    }
}

/// Append one dated line to a card's `notes.md` sidecar (SPEC D5, the second
/// half of the archive/note-append seam). A dir-backed card owns its sidecar:
/// the first write seeds the file with the card title as a header; later
/// writes add `<date>  <text>` lines -- the exact convention the legacy `task
/// note` / `feature note` verbs used, so a migrated card's notes read
/// identically. An entry-backed card (a decision/idea in a container list
/// file) has no dir of its own, so its note lands in the CONTAINER's
/// `notes.md` as `<date>  [<id>] <text>` -- the id prefix keeps shared-log
/// lines attributable, and a feature container's log is seeded with the
/// feature title so it reads the same whichever verb wrote first. The append
/// touches only the sidecar, never the record: prose is not card state, so it
/// needs no CAS write. Returns whether `notes.md` was created by this append.
pub fn append_note(paths: &MaestroPaths, id: &str, text: &str, now: &str) -> Result<bool> {
    validate_card_id(id)?;
    if text.trim().is_empty() {
        bail!("note text cannot be empty");
    }
    let Some(resolved) = resolve(paths, id)? else {
        bail!("no card {id} to note");
    };
    let record = resolved.path();
    let dir = record
        .parent()
        .with_context(|| format!("card path missing parent: {}", record.display()))?;
    let dir_backed = resolved.is_dir_backed();
    let date = now.split_once('T').map_or(now, |(date, _)| date);
    let (header, line) = if dir_backed {
        (resolved.card.title.clone(), text.trim().to_string())
    } else {
        let container_title = load(&dir.join(CARD_FILE))?
            .map(|container| container.title)
            .unwrap_or_else(|| "Notes".to_string());
        (container_title, format!("[{id}] {}", text.trim()))
    };
    let notes_path = dir.join("notes.md");
    append_text_file(
        &notes_path,
        &format!("# {header}\n\n"),
        &format!("{date}  {line}\n"),
    )
    .with_context(|| format!("failed to append card note {}", notes_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::card::schema::{Card, CardType};
    use crate::domain::card::store::{
        card_path, create_card, load_with_snapshot, save_with_snapshot,
    };
    use crate::foundation::core::fs::ensure_dir;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    const NOW: &str = "2026-06-09T00:00:00Z";

    fn repo(label: &str) -> MaestroPaths {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("maestro-{label}-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.cards_dir()).expect("create cards dir");
        paths
    }

    fn seed(paths: &MaestroPaths, id: &str) {
        let card = Card::new(id, CardType::Task, id, "ready", NOW);
        let path = card_path(paths, id);
        let snap = load_with_snapshot(&path).expect("absent loads None");
        save_with_snapshot(&path, &card, &snap).expect("seed card");
    }

    #[test]
    fn add_writes_a_blocks_edge_onto_the_child() {
        let paths = repo("edit-add");
        seed(&paths, "task-001");
        seed(&paths, "task-002");

        let added = add_blocks_dep(&paths, "task-002", "task-001", "2026-06-09T01:00:00Z")
            .expect("add succeeds");
        assert!(added, "a fresh edge is written");

        let child = load(&card_path(&paths, "task-002"))
            .expect("load")
            .expect("child exists");
        assert_eq!(child.deps.len(), 1);
        assert_eq!(child.deps[0].kind, DepKind::Blocks);
        assert_eq!(child.deps[0].target, "task-001");
        assert_eq!(
            child.updated_at, "2026-06-09T01:00:00Z",
            "mutation bumps updated_at"
        );

        // the blocker (parent) is never touched
        let parent = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("parent exists");
        assert!(
            parent.deps.is_empty(),
            "the edge lives only on the dependent"
        );
    }

    #[test]
    fn add_is_idempotent() {
        let paths = repo("edit-idem");
        seed(&paths, "task-001");
        seed(&paths, "task-002");

        assert!(add_blocks_dep(&paths, "task-002", "task-001", NOW).expect("first add"));
        assert!(
            !add_blocks_dep(&paths, "task-002", "task-001", NOW).expect("second add"),
            "a duplicate edge is a no-op"
        );
        let child = load(&card_path(&paths, "task-002"))
            .expect("load")
            .expect("child exists");
        assert_eq!(child.deps.len(), 1, "no duplicate edge appended");
    }

    #[test]
    fn add_rejects_self_block_and_missing_cards() {
        let paths = repo("edit-reject");
        seed(&paths, "task-001");

        assert!(
            add_blocks_dep(&paths, "task-001", "task-001", NOW).is_err(),
            "a card cannot block itself"
        );
        assert!(
            add_blocks_dep(&paths, "task-001", "task-404", NOW).is_err(),
            "the blocker must exist (no dangling ref)"
        );
        assert!(
            add_blocks_dep(&paths, "task-404", "task-001", NOW).is_err(),
            "the dependent must exist"
        );
    }

    #[test]
    fn remove_deletes_the_childs_blocks_edge() {
        let paths = repo("edit-remove");
        seed(&paths, "task-001");
        seed(&paths, "task-002");
        add_blocks_dep(&paths, "task-002", "task-001", NOW).expect("add");

        assert!(
            remove_blocks_dep(&paths, "task-002", "task-001", LATER).expect("remove"),
            "the stored edge is removed"
        );
        let child = load(&card_path(&paths, "task-002"))
            .expect("load")
            .expect("child exists");
        assert!(child.deps.is_empty(), "the blocks edge is gone");
        assert_eq!(child.updated_at, LATER, "removal bumps updated_at");
    }

    #[test]
    fn remove_is_directional_and_idempotent() {
        let paths = repo("edit-remove-directional");
        seed(&paths, "task-001");
        seed(&paths, "task-002");
        add_blocks_dep(&paths, "task-002", "task-001", NOW).expect("add");

        assert!(
            !remove_blocks_dep(&paths, "task-001", "task-002", LATER).expect("reverse remove"),
            "reverse argument order finds no edge on the blocker (directional)"
        );
        let child = load(&card_path(&paths, "task-002"))
            .expect("load")
            .expect("child exists");
        assert_eq!(
            child.deps.len(),
            1,
            "the real edge survives a reverse-order remove"
        );

        assert!(
            remove_blocks_dep(&paths, "task-002", "task-001", LATER).expect("forward remove"),
            "the correctly-ordered remove deletes the edge"
        );
        assert!(
            !remove_blocks_dep(&paths, "task-002", "task-001", LATER).expect("second remove"),
            "removing an absent edge is a no-op"
        );
    }

    #[test]
    fn remove_rejects_self_block_and_missing_cards() {
        let paths = repo("edit-remove-reject");
        seed(&paths, "task-001");

        assert!(
            remove_blocks_dep(&paths, "task-001", "task-001", NOW).is_err(),
            "a card cannot block itself"
        );
        assert!(
            remove_blocks_dep(&paths, "task-001", "task-404", NOW).is_err(),
            "the blocker must exist"
        );
        assert!(
            remove_blocks_dep(&paths, "task-404", "task-001", NOW).is_err(),
            "the dependent must exist"
        );
    }

    #[test]
    fn add_related_link_writes_one_edge_and_reverse_add_is_idempotent() {
        let paths = repo("edit-related-add");
        seed(&paths, "task-001");
        seed(&paths, "task-002");

        assert!(
            add_related_link(&paths, "task-001", "task-002", LATER).expect("first add"),
            "a fresh related link writes"
        );
        assert!(
            !add_related_link(&paths, "task-002", "task-001", LATER).expect("reverse add"),
            "the reverse relation is already present"
        );

        let first = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("first exists");
        assert_eq!(first.deps.len(), 1);
        assert_eq!(first.deps[0].kind, DepKind::Related);
        assert_eq!(first.deps[0].target, "task-002");
        assert_eq!(first.updated_at, LATER);

        let second = load(&card_path(&paths, "task-002"))
            .expect("load")
            .expect("second exists");
        assert!(
            second.deps.is_empty(),
            "reverse add does not write a reciprocal edge"
        );
    }

    #[test]
    fn remove_related_link_accepts_reverse_order() {
        let paths = repo("edit-related-remove");
        seed(&paths, "task-001");
        seed(&paths, "task-002");
        add_related_link(&paths, "task-001", "task-002", NOW).expect("add");

        assert!(
            remove_related_link(&paths, "task-002", "task-001", LATER).expect("reverse remove"),
            "the stored edge is removed through reverse arguments"
        );
        let first = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("first exists");
        assert!(first.deps.is_empty(), "the related edge is gone");
        assert_eq!(first.updated_at, LATER, "removal bumps the stored side");
        assert!(
            !remove_related_link(&paths, "task-001", "task-002", LATER).expect("second remove"),
            "a missing relation is a no-op"
        );
    }

    #[test]
    fn related_links_reject_self_and_missing_cards() {
        let paths = repo("edit-related-reject");
        seed(&paths, "task-001");

        assert!(
            add_related_link(&paths, "task-001", "task-001", NOW).is_err(),
            "a card cannot relate to itself"
        );
        assert!(
            remove_related_link(&paths, "task-001", "task-001", NOW).is_err(),
            "removing a self-link is also refused"
        );
        assert!(
            add_related_link(&paths, "task-001", "task-404", NOW).is_err(),
            "the target must exist"
        );
        assert!(
            remove_related_link(&paths, "task-404", "task-001", NOW).is_err(),
            "the source must exist"
        );
    }

    #[test]
    fn add_related_link_rejects_a_terminal_card_and_writes_no_edge() {
        let paths = repo("edit-related-terminal");
        seed(&paths, "task-001");
        seed_full(&paths, "task-002", CardType::Task, "verified", None, None);

        let err = add_related_link(&paths, "task-001", "task-002", LATER)
            .expect_err("a terminal partner is not linkable");
        let reason = err.to_string();
        assert!(
            reason.contains("task-002 is finished")
                && reason.contains("you can't open a new conversation"),
            "reason names the finished card honestly: {reason}"
        );
        // the live side is untouched -- no half-written edge
        let live = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("task-001 exists");
        assert!(live.deps.is_empty(), "no edge written when the gate trips");
    }

    /// Seed a card of `ty`/`status`, optionally already claimed, and save it.
    fn seed_full(
        paths: &MaestroPaths,
        id: &str,
        ty: CardType,
        status: &str,
        claimed_by: Option<&str>,
        claimed_at: Option<&str>,
    ) {
        let mut card = Card::new(id, ty, id, status, NOW);
        card.claimed_by = claimed_by.map(str::to_string);
        card.claimed_at = claimed_at.map(str::to_string);
        let path = card_path(paths, id);
        let snap = load_with_snapshot(&path).expect("absent loads None");
        save_with_snapshot(&path, &card, &snap).expect("seed card");
    }

    const LATER: &str = "2026-06-09T01:00:00Z";
    // five minutes after NOW: a real "now" that is still inside the 15-min TTL.
    const SOON: &str = "2026-06-09T00:05:00Z";

    #[test]
    fn claim_takes_a_free_card_and_moves_it_in_progress() {
        let paths = repo("claim-free");
        seed(&paths, "task-001");

        let outcome = claim(&paths, "task-001", "claude#s1", LATER).expect("claim succeeds");
        assert_eq!(outcome, ClaimOutcome::Claimed);

        let card = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("card exists");
        assert_eq!(card.claimed_by.as_deref(), Some("claude#s1"));
        assert_eq!(card.claimed_at.as_deref(), Some(LATER));
        assert_eq!(card.status, "in_progress");
        assert_eq!(card.updated_at, LATER);
    }

    #[test]
    fn claim_is_idempotent_for_the_same_holder() {
        let paths = repo("claim-mine");
        seed_full(
            &paths,
            "task-001",
            CardType::Task,
            "in_progress",
            Some("claude#s1"),
            Some(NOW),
        );

        let outcome = claim(&paths, "task-001", "claude#s1", LATER).expect("re-claim succeeds");
        assert_eq!(outcome, ClaimOutcome::AlreadyMine);

        // an idempotent re-claim does not re-stamp claimed_at
        let card = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("card exists");
        assert_eq!(card.claimed_at.as_deref(), Some(NOW));
    }

    #[test]
    fn claim_refuses_a_fresh_claim_held_by_another() {
        let paths = repo("claim-contend");
        seed_full(
            &paths,
            "task-001",
            CardType::Task,
            "in_progress",
            Some("codex#s9"),
            Some(NOW),
        );

        // five minutes later is well inside the 15-minute TTL, so the claim is live
        let err = claim(&paths, "task-001", "claude#s1", SOON).expect_err("fresh claim refused");
        assert!(err.to_string().contains("codex#s9"), "names the holder");

        let card = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("card exists");
        assert_eq!(card.claimed_by.as_deref(), Some("codex#s9"), "untouched");
    }

    #[test]
    fn claim_reclaims_a_stale_claim() {
        let paths = repo("claim-stale");
        seed_full(
            &paths,
            "task-001",
            CardType::Task,
            "in_progress",
            Some("codex#s9"),
            Some("2020-01-01T00:00:00Z"),
        );

        let outcome =
            claim(&paths, "task-001", "claude#s1", LATER).expect("stale reclaim succeeds");
        assert_eq!(
            outcome,
            ClaimOutcome::Reclaimed {
                previous: "codex#s9".to_string()
            }
        );

        let card = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("card exists");
        assert_eq!(card.claimed_by.as_deref(), Some("claude#s1"));
        assert_eq!(card.claimed_at.as_deref(), Some(LATER));
    }

    #[test]
    fn claim_refuses_a_non_workable_card() {
        let paths = repo("claim-feature");
        seed_full(&paths, "feat-001", CardType::Feature, "open", None, None);

        let err = claim(&paths, "feat-001", "claude#s1", LATER).expect_err("feature not claimable");
        assert!(err.to_string().contains("feature"), "names the type");
    }

    #[test]
    fn claim_refuses_a_closed_card() {
        let paths = repo("claim-closed");
        seed_full(&paths, "task-001", CardType::Task, "closed", None, None);

        let err = claim(&paths, "task-001", "claude#s1", LATER).expect_err("closed not claimable");
        assert!(err.to_string().contains("closed"), "says it is closed");
    }

    #[test]
    fn assign_sets_then_clears_the_advisory_hint_without_touching_status() {
        let paths = repo("assign-set-clear");
        seed(&paths, "task-001");

        assert!(
            assign(&paths, "task-001", Some("codex#s9"), LATER).expect("set"),
            "a fresh hint is written"
        );
        let card = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("card exists");
        assert_eq!(card.suggested_for.as_deref(), Some("codex#s9"));
        assert_eq!(card.status, "ready", "advisory hint changes no status");
        assert_eq!(card.claimed_by, None, "advisory hint is not a claim");
        assert_eq!(card.updated_at, LATER, "set bumps updated_at");

        assert!(
            assign(&paths, "task-001", None, SOON).expect("clear"),
            "clearing a set hint is a change"
        );
        let cleared = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("card exists");
        assert_eq!(cleared.suggested_for, None, "clear removes the hint");
        assert_eq!(cleared.status, "ready", "clear changes no status");
    }

    #[test]
    fn assign_is_idempotent() {
        let paths = repo("assign-idem");
        seed(&paths, "task-001");

        assert!(assign(&paths, "task-001", Some("codex#s9"), LATER).expect("first set"));
        assert!(
            !assign(&paths, "task-001", Some("codex#s9"), SOON).expect("second set"),
            "re-assigning the same who is a no-op"
        );
        assert!(
            assign(&paths, "task-001", None, SOON).expect("clear"),
            "clearing a set hint is a change"
        );
        // a second clear on an already-clear card is also a no-op
        assert!(!assign(&paths, "task-001", None, SOON).expect("clear-again"));
    }

    #[test]
    fn claim_does_not_clear_a_suggested_for_and_assign_never_blocks_a_claim() {
        let paths = repo("assign-no-block");
        seed(&paths, "task-001");
        assign(&paths, "task-001", Some("codex#s9"), NOW).expect("assign to a peer");

        // A different session claims the card the hint suggested for someone else.
        let outcome = claim(&paths, "task-001", "claude#s1", LATER).expect("claim succeeds");
        assert_eq!(
            outcome,
            ClaimOutcome::Claimed,
            "the hint never blocks a claim"
        );

        let card = load(&card_path(&paths, "task-001"))
            .expect("load")
            .expect("card exists");
        assert_eq!(card.claimed_by.as_deref(), Some("claude#s1"));
        assert_eq!(
            card.suggested_for.as_deref(),
            Some("codex#s9"),
            "claim does not auto-clear the advisory hint"
        );
    }

    #[test]
    fn assign_refuses_a_non_workable_card() {
        let paths = repo("assign-feature");
        seed_full(&paths, "feat-001", CardType::Feature, "open", None, None);

        let err =
            assign(&paths, "feat-001", Some("codex#s9"), NOW).expect_err("feature takes no hint");
        assert!(err.to_string().contains("feature"), "names the type");
    }

    #[test]
    fn append_note_creates_then_appends_dated_lines() {
        let paths = repo("note-append");
        seed(&paths, "task-001");

        let created = append_note(&paths, "task-001", "first finding", "2026-06-09T00:00:00Z")
            .expect("first note");
        assert!(created, "first append creates notes.md");
        let again = append_note(&paths, "task-001", "second finding", "2026-06-10T09:30:00Z")
            .expect("second note");
        assert!(!again, "a later append does not re-create the file");

        let notes = std::fs::read_to_string(
            card_path(&paths, "task-001")
                .parent()
                .unwrap()
                .join("notes.md"),
        )
        .expect("read notes.md");
        assert!(
            notes.starts_with("# task-001\n\n"),
            "title header seeded once: {notes:?}"
        );
        assert!(
            notes.contains("2026-06-09  first finding\n"),
            "first dated line"
        );
        assert!(
            notes.contains("2026-06-10  second finding\n"),
            "second dated line"
        );
        assert_eq!(
            notes.matches("# task-001").count(),
            1,
            "header written only once"
        );
    }

    #[test]
    fn append_note_on_an_entry_backed_card_lands_in_the_container_log() {
        let paths = repo("note-entry");
        let card = Card::new("card-d1", CardType::Decision, "Pick the lock", "open", NOW);
        create_card(&paths, &card).expect("create decision entry");

        let created = append_note(&paths, "card-d1", "ruling rationale", NOW).expect("note");
        assert!(created, "first shared-log append creates notes.md");
        let notes = std::fs::read_to_string(paths.cards_dir().join("notes.md"))
            .expect("read the container log");
        assert!(
            notes.starts_with("# Notes\n\n"),
            "a root container log seeds the fallback header: {notes:?}"
        );
        assert!(
            notes.contains("2026-06-09  [card-d1] ruling rationale\n"),
            "the id prefix keeps shared-log lines attributable: {notes:?}"
        );
    }

    #[test]
    fn append_note_rejects_empty_text_and_a_missing_card() {
        let paths = repo("note-bad");
        seed(&paths, "task-001");

        assert!(
            append_note(&paths, "task-001", "   ", NOW).is_err(),
            "whitespace-only note text is refused"
        );
        assert!(
            append_note(&paths, "ghost", "anything", NOW).is_err(),
            "noting a missing card fails loud"
        );
    }
}
