//! Read-side card queries (SPEC-beads-model P4): the single scan seam (D4), the
//! coarse status derivation (DN3), the `ready` rule (E3/E8), and the `list`
//! filter (G3). These are pure functions over the scanned card set; the CLI
//! verbs that surface them are a thin adapter layer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::load;
use crate::foundation::core::fs::child_dirs;
use crate::foundation::core::paths::MaestroPaths;

/// The coarse, board-level status every card maps to (SPEC DN3, LOCKED). The
/// real per-type status string is the single source of truth; this is derived
/// from it on demand and never stored, so the two cannot desync.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coarse {
    Open,
    InProgress,
    Closed,
}

impl Coarse {
    /// Parse a coarse word as a `--status` filter accepts it.
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "open" => Some(Self::Open),
            "in_progress" => Some(Self::InProgress),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }

    /// The `open | in_progress | closed` label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::InProgress => "in_progress",
            Self::Closed => "closed",
        }
    }
}

/// Map a real per-type status word to its coarse board status (SPEC DN3). An
/// unrecognized word returns `None`: the per-type vocab is not yet frozen (SPEC
/// O5), and an unclassifiable status must not silently read as open (which would
/// surface unready work in `ready`) nor as closed (which would satisfy a
/// blocker). Callers treat `None` conservatively.
pub fn coarse_of(status: &str) -> Option<Coarse> {
    match status {
        "proposed" | "draft" | "exploring" | "ready" | "open" => Some(Coarse::Open),
        "in_progress" | "needs_verification" | "accepted" => Some(Coarse::InProgress),
        "closed" | "verified" | "measured" | "locked" | "superseded" | "shipped" | "cancelled"
        | "rejected" | "abandoned" | "dismissed" => Some(Coarse::Closed),
        _ => None,
    }
}

/// The status words `update --status` accepts on a workable card: the task
/// fine states (SPEC DN3) plus the uniform create/close words `open`/`closed`.
/// The `update` error message prints this list, so keep the two in step.
pub const WORKABLE_STATUS_WORDS: &[&str] = &[
    "open",
    "draft",
    "exploring",
    "ready",
    "in_progress",
    "needs_verification",
    "verified",
    "rejected",
    "abandoned",
    "superseded",
    "closed",
];

/// The card's prose body for `show`: the top-level description, falling back to
/// the legacy record's own field inside `extra` (`description`, or a decision's
/// `context`) for a migrated card folded before the description lift existed.
pub fn body_of(card: &Card) -> Option<String> {
    card.description
        .clone()
        .or_else(|| crate::domain::card::fold::nonempty_field(&card.extra, "description"))
        .or_else(|| crate::domain::card::fold::nonempty_field(&card.extra, "context"))
}

/// The single card scan seam (SPEC D4): every card in the store, symlink-safe.
/// Built on `child_dirs` (which skips symlinked directories) and skips any
/// directory without a `card.yaml` -- the `.alloc-` id-reservation markers are
/// `card.yaml`-less by design. Returned sorted by id for deterministic output.
/// Fails loud on a malformed or schema-mismatched card; tolerant scans that need
/// to survive one bad artifact filter at their own layer.
pub fn scan(paths: &MaestroPaths) -> Result<Vec<Card>> {
    scan_dir(&paths.cards_dir())
}

/// [`scan`] over an explicit card tree root, so the archive reads
/// (`archive/cards/`) ride the same seam as the live store.
pub fn scan_dir(root: &Path) -> Result<Vec<Card>> {
    let mut cards = Vec::new();
    for (dir, _modified) in child_dirs(root)? {
        if let Some(card) = load(&dir.join("card.yaml"))? {
            cards.push(card);
        }
    }
    cards.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(cards)
}

/// One tolerant walk over the store for the card-aware doctor: every loadable
/// card paired with its `card.yaml` path, plus the cards that failed to load.
/// A failed card's type is unknowable, so failures carry no `CardType`; the
/// caller owns reporting each one exactly once.
#[derive(Debug)]
pub struct StoreScan {
    pub cards: Vec<(Card, PathBuf)>,
    pub failures: Vec<StoreScanFailure>,
}

#[derive(Debug)]
pub struct StoreScanFailure {
    pub id: String,
    pub path: PathBuf,
    /// Full error chain (`{error:#}`), ready for a diagnostic line.
    pub error: String,
}

/// [`scan`], but collecting per-card load failures instead of failing loud on
/// the first one. `Err` only when the store root itself cannot be walked.
pub fn scan_with_failures(paths: &MaestroPaths) -> Result<StoreScan> {
    let mut cards = Vec::new();
    let mut failures = Vec::new();
    for (dir, _modified) in child_dirs(&paths.cards_dir())? {
        let path = dir.join("card.yaml");
        match load(&path) {
            Ok(Some(card)) => cards.push((card, path)),
            Ok(None) => {}
            Err(error) => failures.push(StoreScanFailure {
                id: dir
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path,
                error: format!("{error:#}"),
            }),
        }
    }
    cards.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    failures.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(StoreScan { cards, failures })
}

/// The `ready` rule (SPEC E3/E8): a card is ready when it is a workable type,
/// its coarse status is OPEN, and every `blocks` dependency it carries points at
/// a card whose coarse status is CLOSED. `related`/`supersedes` edges and
/// `parent` never gate readiness. A `blocks` target that is missing from the
/// scanned set, or whose status is unclassifiable, leaves the card NOT ready.
pub fn ready(cards: &[Card]) -> Vec<&Card> {
    let by_id: HashMap<&str, &Card> = cards.iter().map(|c| (c.id.as_str(), c)).collect();
    cards.iter().filter(|c| is_ready(c, &by_id)).collect()
}

fn is_ready(card: &Card, by_id: &HashMap<&str, &Card>) -> bool {
    if !card.card_type.workable() {
        return false;
    }
    if coarse_of(&card.status) != Some(Coarse::Open) {
        return false;
    }
    card.deps
        .iter()
        .filter(|dep| dep.kind.is_blocking())
        .all(|dep| {
            by_id
                .get(dep.target.as_str())
                .is_some_and(|target| coarse_of(&target.status) == Some(Coarse::Closed))
        })
}

/// The `list` filter (SPEC G3): every supplied predicate must match (AND). An
/// unset field does not constrain. `assignee` matches a claim by full token or
/// agent portion (see [`claim_matches`]); `status` matches the COARSE word
/// (SPEC DN3, the `--status` filter's form).
#[derive(Clone, Debug, Default)]
pub struct ListFilter<'a> {
    pub parent: Option<&'a str>,
    pub card_type: Option<CardType>,
    pub assignee: Option<&'a str>,
    pub status: Option<Coarse>,
}

impl ListFilter<'_> {
    fn matches(&self, card: &Card) -> bool {
        self.parent
            .is_none_or(|parent| card.parent.as_deref() == Some(parent))
            && self
                .card_type
                .is_none_or(|card_type| card.card_type == card_type)
            && self.assignee.is_none_or(|assignee| {
                card.claimed_by
                    .as_deref()
                    .is_some_and(|owner| claim_matches(owner, assignee))
            })
            && self
                .status
                .is_none_or(|status| coarse_of(&card.status) == Some(status))
    }
}

/// Does claim `owner` answer to `--assignee <query>`? Claims are
/// `<agent>#<session>` (SPEC DN8), so `--assignee claude` must find every
/// `claude#...` session, while `--assignee claude#s1` still pins one session.
/// Matches the full token or the agent portion; agent-TOKEN equality (split on
/// `#`), not a raw prefix, so `claude` never bleeds into `claude-bot#s1`.
fn claim_matches(owner: &str, query: &str) -> bool {
    owner == query
        || owner
            .split_once('#')
            .is_some_and(|(agent, _)| agent == query)
}

/// Filter the scanned card set (SPEC G3 `list`). Order is preserved from input.
pub fn query<'a>(cards: &'a [Card], filter: &ListFilter) -> Vec<&'a Card> {
    cards.iter().filter(|card| filter.matches(card)).collect()
}

/// The CLI-only dotted display alias (SPEC E2): `<parent>.<N>`, where N is the
/// card's 1-based position among its id-sorted siblings (cards sharing its
/// `parent`). Computed at render time and never stored or parsed back -- the
/// ordinal shifts when siblings come and go, so only the stable id may be used
/// as a ref. `None` for a parentless card, or when the card is absent from the
/// scanned set (e.g. archived).
pub fn display_alias(cards: &[Card], card: &Card) -> Option<String> {
    let parent = card.parent.as_deref()?;
    let mut siblings: Vec<&str> = cards
        .iter()
        .filter(|sibling| sibling.parent.as_deref() == Some(parent))
        .map(|sibling| sibling.id.as_str())
        .collect();
    siblings.sort_unstable();
    let position = siblings.iter().position(|id| *id == card.id)?;
    Some(format!("{parent}.{}", position + 1))
}

#[cfg(test)]
mod tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::domain::card::schema::{Dep, DepKind};
    use crate::domain::card::store::{card_path, load_with_snapshot, save_with_snapshot};
    use crate::foundation::core::fs::ensure_dir;

    fn card(id: &str, card_type: CardType, status: &str) -> Card {
        Card::new(id, card_type, id, status, "2026-06-09T00:00:00Z")
    }

    #[test]
    fn workable_is_exactly_task_bug_chore() {
        assert!(CardType::Task.workable());
        assert!(CardType::Bug.workable());
        assert!(CardType::Chore.workable());
        assert!(!CardType::Feature.workable());
        assert!(!CardType::Idea.workable());
        assert!(!CardType::Decision.workable());
    }

    #[test]
    fn coarse_maps_the_dn3_sets() {
        for word in ["proposed", "draft", "exploring", "ready", "open"] {
            assert_eq!(coarse_of(word), Some(Coarse::Open), "{word} is OPEN");
        }
        for word in ["in_progress", "needs_verification", "accepted"] {
            assert_eq!(
                coarse_of(word),
                Some(Coarse::InProgress),
                "{word} is in_progress"
            );
        }
        for word in [
            "closed",
            "verified",
            "measured",
            "locked",
            "superseded",
            "shipped",
            "cancelled",
            "rejected",
            "abandoned",
            "dismissed",
        ] {
            assert_eq!(coarse_of(word), Some(Coarse::Closed), "{word} is CLOSED");
        }
        assert_eq!(
            coarse_of("not_a_real_status"),
            None,
            "an unfrozen/unknown word is unclassifiable, not silently open or closed"
        );
    }

    #[test]
    fn ready_requires_workable_open_and_satisfied_blockers() {
        let mut blocked = card("task-001", CardType::Task, "ready");
        blocked.deps = vec![Dep {
            kind: DepKind::Blocks,
            target: "task-002".to_string(),
        }];
        let open_blocker = card("task-002", CardType::Task, "in_progress");
        let cards = vec![blocked, open_blocker];
        assert!(
            ready(&cards).is_empty(),
            "a blocks dep on a non-closed card holds the card back"
        );
    }

    #[test]
    fn ready_clears_when_blocker_is_closed() {
        let mut blocked = card("task-001", CardType::Task, "ready");
        blocked.deps = vec![Dep {
            kind: DepKind::Blocks,
            target: "task-002".to_string(),
        }];
        let closed_blocker = card("task-002", CardType::Task, "verified");
        let cards = vec![blocked, closed_blocker];
        let r = ready(&cards);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "task-001");
    }

    #[test]
    fn ready_ignores_non_blocking_edges() {
        let mut t = card("task-001", CardType::Task, "ready");
        t.deps = vec![
            Dep {
                kind: DepKind::Related,
                target: "task-002".to_string(),
            },
            Dep {
                kind: DepKind::Supersedes,
                target: "task-003".to_string(),
            },
        ];
        // non-closed targets that, were the edges blocking, would hold task-001 back
        let cards = vec![
            t,
            card("task-002", CardType::Task, "in_progress"),
            card("task-003", CardType::Task, "in_progress"),
        ];
        let ready_ids: Vec<&str> = ready(&cards).iter().map(|c| c.id.as_str()).collect();
        assert!(
            ready_ids.contains(&"task-001"),
            "related/supersedes edges never gate ready, even to non-closed targets"
        );
    }

    #[test]
    fn ready_is_conservative_on_a_missing_blocker() {
        let mut blocked = card("task-001", CardType::Task, "ready");
        blocked.deps = vec![Dep {
            kind: DepKind::Blocks,
            target: "task-404".to_string(),
        }];
        let cards = vec![blocked];
        assert!(
            ready(&cards).is_empty(),
            "a blocks dep on a target absent from the store is unsatisfied"
        );
    }

    #[test]
    fn ready_excludes_non_workable_and_non_open() {
        let cards = vec![
            card("agent-cli-ux", CardType::Feature, "ready"),
            card("decision-001", CardType::Decision, "open"),
            card("idea-001", CardType::Idea, "proposed"),
            card("task-001", CardType::Task, "in_progress"),
        ];
        assert!(
            ready(&cards).is_empty(),
            "features/ideas/decisions are not workable; an in_progress task is not coarse-open"
        );
    }

    #[test]
    fn display_alias_is_parent_dot_ordinal_among_id_sorted_siblings() {
        let mut early = card("card-aaa111", CardType::Task, "open");
        early.parent = Some("csv-export".to_string());
        let mut late = card("card-zzz999", CardType::Task, "open");
        late.parent = Some("csv-export".to_string());
        let mut foreign = card("card-bbb222", CardType::Task, "open");
        foreign.parent = Some("other-feature".to_string());
        let unparented = card("card-ccc333", CardType::Task, "open");
        // Deliberately unsorted input: the ordinal must come from the id sort,
        // not the caller's ordering.
        let cards = vec![
            late.clone(),
            card("csv-export", CardType::Feature, "proposed"),
            foreign,
            early.clone(),
            unparented.clone(),
        ];

        assert_eq!(
            display_alias(&cards, &early).as_deref(),
            Some("csv-export.1")
        );
        assert_eq!(
            display_alias(&cards, &late).as_deref(),
            Some("csv-export.2")
        );
        assert_eq!(display_alias(&cards, &unparented), None);
    }

    #[test]
    fn list_filters_compose() {
        let mut claimed = card("task-001", CardType::Task, "in_progress");
        claimed.parent = Some("agent-cli-ux".to_string());
        claimed.claimed_by = Some("claude#s1".to_string());
        let mut other = card("task-002", CardType::Task, "ready");
        other.parent = Some("agent-cli-ux".to_string());
        let bug = card("bug-001", CardType::Bug, "ready");
        let cards = vec![claimed, other, bug];

        let by_parent = query(
            &cards,
            &ListFilter {
                parent: Some("agent-cli-ux"),
                ..Default::default()
            },
        );
        assert_eq!(by_parent.len(), 2);

        let by_type = query(
            &cards,
            &ListFilter {
                card_type: Some(CardType::Bug),
                ..Default::default()
            },
        );
        assert_eq!(by_type.len(), 1);
        assert_eq!(by_type[0].id, "bug-001");

        let by_assignee = query(
            &cards,
            &ListFilter {
                assignee: Some("claude#s1"),
                ..Default::default()
            },
        );
        assert_eq!(by_assignee.len(), 1);
        assert_eq!(by_assignee[0].id, "task-001");

        let open = query(
            &cards,
            &ListFilter {
                status: Some(Coarse::Open),
                ..Default::default()
            },
        );
        assert_eq!(open.len(), 2, "task-002 + bug-001 are coarse-open");

        let combined = query(
            &cards,
            &ListFilter {
                parent: Some("agent-cli-ux"),
                status: Some(Coarse::Open),
                ..Default::default()
            },
        );
        assert_eq!(combined.len(), 1);
        assert_eq!(combined[0].id, "task-002");
    }

    #[test]
    fn assignee_matches_full_token_and_agent_portion() {
        let mut claude = card("task-001", CardType::Task, "in_progress");
        claude.claimed_by = Some("claude#s1".to_string());
        let mut codex = card("task-002", CardType::Task, "in_progress");
        codex.claimed_by = Some("codex#s9".to_string());
        // a similarly-prefixed agent must NOT match the bare `claude` query
        let mut claude_bot = card("task-003", CardType::Task, "in_progress");
        claude_bot.claimed_by = Some("claude-bot#s1".to_string());
        let unclaimed = card("task-004", CardType::Task, "ready");
        let cards = vec![claude, codex, claude_bot, unclaimed];

        let by_agent = |q: &str| -> Vec<&str> {
            query(
                &cards,
                &ListFilter {
                    assignee: Some(q),
                    ..Default::default()
                },
            )
            .iter()
            .map(|c| c.id.as_str())
            .collect()
        };

        assert_eq!(
            by_agent("claude"),
            vec!["task-001"],
            "agent portion matches one session and does not bleed into claude-bot"
        );
        assert_eq!(
            by_agent("claude#s1"),
            vec!["task-001"],
            "the full token still pins exactly one session"
        );
        assert_eq!(
            by_agent("claude#s2"),
            Vec::<&str>::new(),
            "a non-matching session is empty even for the right agent"
        );
        assert_eq!(by_agent("codex"), vec!["task-002"]);
        assert_eq!(
            by_agent("nobody"),
            Vec::<&str>::new(),
            "no claim and no card matches; unclaimed cards never answer an assignee filter"
        );
    }

    #[test]
    fn scan_returns_every_card_sorted_and_skips_marker_dirs() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("maestro-scan-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.cards_dir()).expect("create cards dir");

        for id in ["task-002", "agent-cli-ux", "decision-001"] {
            let c = card(id, CardType::Task, "ready");
            let path = card_path(&paths, id);
            let snap = load_with_snapshot(&path).expect("absent loads None");
            save_with_snapshot(&path, &c, &snap).expect("save card");
        }
        // a card.yaml-less directory, like an `.alloc-` reservation marker
        ensure_dir(paths.cards_dir().join(".alloc-task-003")).expect("create marker dir");

        let scanned = scan(&paths).expect("scan");
        let ids: Vec<&str> = scanned.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["agent-cli-ux", "decision-001", "task-002"],
            "every card.yaml-bearing dir, sorted by id; the marker dir is skipped"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_with_failures_collects_the_bad_card_and_keeps_the_rest() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("maestro-scan-fail-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);
        ensure_dir(paths.cards_dir()).expect("create cards dir");

        for id in ["task-001", "task-002"] {
            let c = card(id, CardType::Task, "ready");
            let path = card_path(&paths, id);
            let snap = load_with_snapshot(&path).expect("absent loads None");
            save_with_snapshot(&path, &c, &snap).expect("save card");
        }
        let broken_dir = paths.cards_dir().join("broken");
        ensure_dir(&broken_dir).expect("create broken card dir");
        std::fs::write(broken_dir.join("card.yaml"), "type: [").expect("write broken card");

        let scan = scan_with_failures(&paths).expect("walkable store");
        let ids: Vec<&str> = scan.cards.iter().map(|(c, _)| c.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["task-001", "task-002"],
            "healthy cards survive a corrupt sibling"
        );
        assert!(
            scan.cards
                .iter()
                .all(|(_, path)| path.ends_with("card.yaml")),
            "each card carries its card.yaml path"
        );
        assert_eq!(scan.failures.len(), 1, "one failure for the corrupt card");
        assert_eq!(scan.failures[0].id, "broken");
        assert!(
            scan.failures[0].error.contains("failed to parse"),
            "failure carries the full load error chain: {}",
            scan.failures[0].error
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
