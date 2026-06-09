use anyhow::{Result, anyhow};

use crate::domain::card;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::slug::slugify_ascii;
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{
    ArchiveArgs, ClaimArgs, CloseArgs, CreateArgs, DepArgs, DepCommand, ListArgs, NoteArgs,
    ReadyArgs, ShowArgs, UpdateArgs,
};

/// Execute `maestro ready`: workable cards with no open blockers.
pub fn ready(args: ReadyArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }
    let cards = card::query::scan(&paths)?;
    let mut ready = card::query::ready(&cards);
    if let Some(feature) = args.feature.as_deref() {
        ready.retain(|c| c.parent.as_deref() == Some(feature));
    }
    render_ready(&ready);
    Ok(())
}

/// Execute `maestro list`: cards filtered by parent, type, assignee, or coarse status.
pub fn list(args: ListArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }

    let card_type = args.card_type.as_deref().map(parse_card_type).transpose()?;
    let status = args
        .status
        .as_deref()
        .map(|word| {
            card::query::Coarse::parse(word).ok_or_else(|| {
                anyhow!("unknown --status {word:?}; expected open, in_progress, or closed")
            })
        })
        .transpose()?;

    let filter = card::query::ListFilter {
        parent: args.parent.as_deref(),
        card_type,
        assignee: args.assignee.as_deref(),
        status,
    };
    let cards = card::query::scan(&paths)?;
    render_list(&card::query::query(&cards, &filter));
    Ok(())
}

/// Execute `maestro dep add <child> <parent>`: author a blocking edge so the
/// child waits on the parent (SPEC E1/DN6).
pub fn dep(args: DepArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }
    match args.command {
        DepCommand::Add { child, parent } => {
            let added = card::edit::add_blocks_dep(&paths, &child, &parent, &utc_now_timestamp())?;
            if added {
                println!("{child} is now blocked by {parent}");
            } else {
                println!("{child} is already blocked by {parent}");
            }
        }
    }
    Ok(())
}

/// Execute `maestro archive <feature>`: move the feature card and its
/// `parent=<feature>` children to the archive sibling tree (SPEC E4/D5).
pub fn archive(args: ArchiveArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }
    let report = card::archive::archive_feature(&paths, &args.feature)?;
    if report.children.is_empty() {
        println!("archived feature {} (no child cards)", report.feature);
    } else {
        println!(
            "archived feature {} + {} child card(s): {}",
            report.feature,
            report.children.len(),
            report.children.join(", ")
        );
    }
    Ok(())
}

/// Execute `maestro claim <id>`: take a workable card for this session, stamping
/// the `<agent>#<session>` identity and moving it to `in_progress` (SPEC E6/DN8).
pub fn claim(args: ClaimArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }
    let identity = claim_identity();
    let outcome = card::edit::claim(&paths, &args.id, &identity, &utc_now_timestamp())?;
    print_claim_outcome(&args.id, &identity, &outcome);
    Ok(())
}

/// Report a claim outcome, shared by `claim <id>` and `update <id> --claim`
/// (DN9 spells claim as `update --claim`; both drive the same `card::edit::claim`
/// seam, so they print identically).
fn print_claim_outcome(id: &str, identity: &str, outcome: &card::edit::ClaimOutcome) {
    match outcome {
        card::edit::ClaimOutcome::Claimed => println!("claimed {id} as {identity}"),
        card::edit::ClaimOutcome::AlreadyMine => println!("{id} is already yours ({identity})"),
        card::edit::ClaimOutcome::Reclaimed { previous } => {
            println!("reclaimed {id} from {previous} (stale) as {identity}")
        }
    }
}

/// Execute `maestro note <id> <text>`: append a dated note to the card's
/// `notes.md` sidecar (SPEC D5).
pub fn note(args: NoteArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }
    let created = card::edit::append_note(&paths, &args.id, &args.text, &utc_now_timestamp())?;
    if created {
        println!("noted {} (notes.md created)", args.id);
    } else {
        println!("noted {}", args.id);
    }
    Ok(())
}

/// Execute `maestro create -t <type> <title>`: mint a new card (DN9). Non-feature
/// cards get a content-hash `card-<hash>` id; feature cards keep an immutable
/// creation slug (SPEC E2). The initial status is the uniform coarse-open word
/// `open`, so a workable card is immediately `ready` once it has no open blocker.
pub fn create(args: CreateArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }
    let card_type = parse_card_type(&args.card_type)?;
    let now = utc_now_timestamp();
    let id = match card_type {
        card::schema::CardType::Feature => slugify_ascii(&args.title),
        _ => card::store::mint_card_id(&paths, &args.title),
    };
    let path = card::store::card_path(&paths, &id);
    let snapshot = card::store::load_with_snapshot(&path)?;
    if snapshot.card.is_some() {
        return Err(anyhow!("card {id} already exists"));
    }
    let mut new_card = card::schema::Card::new(&id, card_type, &args.title, "open", &now);
    new_card.parent = args.parent;
    new_card.description = args.description;
    card::store::save_with_snapshot(&path, &new_card, &snapshot)?;
    println!("created {id} ({}): {}", card_type.as_str(), args.title);
    Ok(())
}

/// Execute `maestro show <id>`: the card's header, parent, edges, and body (DN9).
/// `--json` prints the raw card; a missing card exits 0 with a guiding line.
pub fn show(args: ShowArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }
    let path = card::store::card_path(&paths, &args.id);
    let Some(c) = card::store::load(&path)? else {
        println!(
            "no card {} (.maestro/cards/{}/card.yaml not found)",
            args.id, args.id
        );
        return Ok(());
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&c)?);
    } else {
        let alias = card::query::display_alias(&card::query::scan(&paths)?, &c);
        render_show(&c, alias.as_deref());
    }
    Ok(())
}

/// Execute `maestro update <id>`: a generic field mutation (DN9). `--status`,
/// `--title`, and `--description` write through the D1 CAS seam; `--claim`
/// delegates to the same `card::edit::claim` the standalone `claim` verb uses.
/// A bare `update` (no id) or an update with no flags exits 0 with usage.
pub fn update(args: UpdateArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }
    let Some(id) = args.id.as_deref() else {
        println!("usage: maestro update <id> [--status S] [--title T] [--description D] [--claim]");
        return Ok(());
    };
    let has_fields = args.status.is_some() || args.title.is_some() || args.description.is_some();
    if !has_fields && !args.claim {
        println!("nothing to update for {id}; pass --status, --title, --description, or --claim");
        return Ok(());
    }
    let now = utc_now_timestamp();
    if has_fields {
        let path = card::store::card_path(&paths, id);
        let snapshot = card::store::load_with_snapshot(&path)?;
        let Some(mut c) = snapshot.card.clone() else {
            println!("no card {id} (.maestro/cards/{id}/card.yaml not found)");
            return Ok(());
        };
        if let Some(status) = args.status.as_deref() {
            c.status = status.to_string();
        }
        if let Some(title) = args.title.as_deref() {
            c.title = title.to_string();
        }
        if let Some(description) = args.description.as_deref() {
            c.description = Some(description.to_string());
        }
        c.updated_at = now.clone();
        card::store::save_with_snapshot(&path, &c, &snapshot)?;
        println!("updated {id}");
    }
    if args.claim {
        let identity = claim_identity();
        let outcome = card::edit::claim(&paths, id, &identity, &now)?;
        print_claim_outcome(id, &identity, &outcome);
    }
    Ok(())
}

/// Execute `maestro close <id>`: move the card to the uniform terminal status
/// `closed` (coarse Closed) through the D1 CAS seam (DN9). Already-closed and
/// missing cards exit 0 with a guiding line.
pub fn close(args: CloseArgs) -> Result<()> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(());
    }
    let path = card::store::card_path(&paths, &args.id);
    let snapshot = card::store::load_with_snapshot(&path)?;
    let Some(mut c) = snapshot.card.clone() else {
        println!(
            "no card {} (.maestro/cards/{}/card.yaml not found)",
            args.id, args.id
        );
        return Ok(());
    };
    if card::query::coarse_of(&c.status) == Some(card::query::Coarse::Closed) {
        println!("{} is already closed (status: {})", args.id, c.status);
        return Ok(());
    }
    c.status = "closed".to_string();
    c.updated_at = utc_now_timestamp();
    card::store::save_with_snapshot(&path, &c, &snapshot)?;
    println!("closed {}", args.id);
    Ok(())
}

/// Parse a `--type`/`-t` word into a [`card::schema::CardType`], shared by
/// `create` and `list`.
fn parse_card_type(word: &str) -> Result<card::schema::CardType> {
    card::schema::CardType::parse(word).ok_or_else(|| {
        anyhow!("unknown --type {word:?}; expected feature, task, bug, chore, idea, or decision")
    })
}

/// The `<agent>#<session>` claim identity (SPEC DN8): the detected agent, or
/// `maestro` when neither claude nor codex is detectable, joined to the session.
fn claim_identity() -> String {
    let agent = match super::detected_agent_hint() {
        "claude" => "claude",
        "codex" => "codex",
        _ => "maestro",
    };
    format!("{agent}#{}", super::claim_session())
}

fn repo_paths() -> Result<MaestroPaths> {
    Ok(MaestroPaths::new(discover_repo_root()?))
}

/// The card verbs read `.maestro/cards/`; an unmigrated repo has none. Exit 0
/// with one guiding line rather than a dead-end error: no cards is a state.
fn legacy_notice() {
    println!(
        "this repo has no card store yet (.maestro/cards/); the card verbs apply once it is migrated to the card model"
    );
}

/// Render `ready` in the beads structure (SPEC DN9): a count header plus numbered
/// `[P#] id type title @claim` rows, emoji-free. `[P#]` is the 1-based ready rank
/// (the card schema carries no priority field, so position is the priority).
fn render_ready(cards: &[&card::schema::Card]) {
    println!(
        "Ready work ({} {}, no blockers):",
        cards.len(),
        plural(cards.len())
    );
    let id_width = cards.iter().map(|c| c.id.len()).max().unwrap_or(0);
    let type_width = cards
        .iter()
        .map(|c| c.card_type.as_str().len())
        .max()
        .unwrap_or(0);
    let title_width = cards.iter().map(|c| c.title.len()).max().unwrap_or(0);
    for (i, c) in cards.iter().enumerate() {
        let rank = i + 1;
        println!(
            "  {rank}. [P{rank}] {:<id_width$}  {:<type_width$}  {:<title_width$}  {}",
            c.id,
            c.card_type.as_str(),
            c.title,
            claim_label(c),
        );
    }
}

/// Render `list` in the beads structure (SPEC DN9): a count header plus numbered
/// rows carrying the real per-type status and parent, emoji-free.
fn render_list(cards: &[&card::schema::Card]) {
    if cards.is_empty() {
        println!("no cards match");
        return;
    }
    println!("{} {}:", cards.len(), plural(cards.len()));
    let id_width = cards.iter().map(|c| c.id.len()).max().unwrap_or(0);
    let type_width = cards
        .iter()
        .map(|c| c.card_type.as_str().len())
        .max()
        .unwrap_or(0);
    let status_width = cards.iter().map(|c| c.status.len()).max().unwrap_or(0);
    let parent_width = cards
        .iter()
        .map(|c| c.parent.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(0);
    for (i, c) in cards.iter().enumerate() {
        println!(
            "  {}. {:<id_width$}  {:<type_width$}  {:<status_width$}  {:<parent_width$}  {}",
            i + 1,
            c.id,
            c.card_type.as_str(),
            c.status,
            c.parent.as_deref().unwrap_or("-"),
            c.title,
        );
    }
}

/// Render `show <id>` (SPEC DN9): header line + parent + edges grouped by kind +
/// body (timestamps and description). Emoji-free.
fn render_show(c: &card::schema::Card, alias: Option<&str>) {
    println!(
        "{}  {}  {}  {}  {}",
        c.id,
        c.card_type.as_str(),
        c.title,
        c.status,
        claim_label(c),
    );
    if let Some(parent) = &c.parent {
        println!("parent: {parent}");
    }
    // SPEC E2: the dotted alias is render-time only -- never a ref, never
    // accepted as an address, so it is labeled to discourage `claim <alias>`.
    if let Some(alias) = alias {
        println!("alias: {alias} (display only)");
    }
    render_edges(c, card::schema::DepKind::Blocks, "blocked by");
    render_edges(c, card::schema::DepKind::Related, "related");
    render_edges(c, card::schema::DepKind::Supersedes, "supersedes");
    println!("created: {}  updated: {}", c.created_at, c.updated_at);
    if let Some(description) = &c.description {
        println!();
        println!("{description}");
    }
}

/// Print the card's edges of one kind as a single `label: a, b, c` line, or
/// nothing when there are none.
fn render_edges(c: &card::schema::Card, kind: card::schema::DepKind, label: &str) {
    let targets: Vec<&str> = c
        .deps
        .iter()
        .filter(|dep| dep.kind == kind)
        .map(|dep| dep.target.as_str())
        .collect();
    if !targets.is_empty() {
        println!("{label}: {}", targets.join(", "));
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "card" } else { "cards" }
}

fn claim_label(c: &card::schema::Card) -> String {
    match c.claimed_by.as_deref() {
        Some(who) => format!("@{who}"),
        None => "(unclaimed)".to_string(),
    }
}
