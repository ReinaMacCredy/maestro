use anyhow::{Result, anyhow};

use crate::domain::card;
use crate::domain::feature;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::slug::slugify_ascii;
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{
    ArchiveArgs, ClaimArgs, CloseArgs, CreateArgs, DepArgs, DepCommand, ListArgs, NoteArgs,
    ReadyArgs, ShowArgs, UpdateArgs,
};

/// Execute `maestro ready`: workable cards with no open blockers.
pub fn ready(args: ReadyArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
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
    let Some(paths) = card_paths()? else {
        return Ok(());
    };

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
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
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
/// `parent=<feature>` children to the archive sibling tree (SPEC E4/D5). The
/// flat verb drives the same `feature::archive_feature` cascade as `maestro
/// feature archive`, so the typed terminal gate, sweep re-run, and no-clobber
/// pre-flight hold on both spellings.
pub fn archive(args: ArchiveArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    let report = feature::archive_feature(&paths, &args.feature, false)?;
    println!("{}", report.note);
    Ok(())
}

/// Execute `maestro claim <id>`: take a workable card for this session, stamping
/// the `<agent>#<session>` identity and moving it to `in_progress` (SPEC E6/DN8).
pub fn claim(args: ClaimArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
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
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    let created = card::edit::append_note(&paths, &args.id, &args.text, &utc_now_timestamp())?;
    if created {
        println!("noted {} (notes.md created)", args.id);
    } else {
        println!("noted {}", args.id);
    }
    Ok(())
}

/// Execute `maestro create -t <type> <title>`: mint a new card (DN9). Non-feature
/// cards get a typed slug id `<type>-<slug>-<hex4>` (SPEC-card-slug-ids D1/D1b);
/// feature cards keep an immutable creation slug (SPEC E2). The initial status is
/// the uniform coarse-open word `open`, so a workable card is immediately `ready`
/// once it has no open blocker.
pub fn create(args: CreateArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    let card_type = parse_card_type(&args.card_type)?;
    let now = utc_now_timestamp();
    let id = match card_type {
        card::schema::CardType::Feature => slugify_ascii(&args.title),
        _ => card::store::mint_card_id(&paths, card_type, &args.title),
    };
    let mut new_card = card::schema::Card::new(&id, card_type, &args.title, "open", &now);
    if let Some(parent) = args.parent {
        // SPEC G1/E1: `parent` docks a card under a feature container. Features
        // are roots, and a dangling parent ref would poison the display alias
        // and parent-filtered queries, so the dock is validated at the door.
        if card_type == card::schema::CardType::Feature {
            return Err(anyhow!(
                "a feature card cannot take --parent; features are top-level containers"
            ));
        }
        card::store::validate_card_id(&parent)?;
        let parent_card = card::store::resolve(&paths, &parent)?
            .map(|resolved| resolved.card)
            .ok_or_else(|| {
                anyhow!(
                    "parent {parent} not found; create the feature first \
                     (`maestro create -t feature \"<title>\"`)"
                )
            })?;
        if parent_card.card_type != card::schema::CardType::Feature {
            return Err(anyhow!(
                "parent {parent} is a {}, not a feature; cards dock under feature parents",
                parent_card.card_type.as_str()
            ));
        }
        new_card.parent = Some(parent);
    }
    new_card.description = args.description;
    card::store::create_card(&paths, &new_card)?;
    println!("created {id} ({}): {}", card_type.as_str(), args.title);
    Ok(())
}

/// Execute `maestro show <id>`: the card's header, parent, edges, and body (DN9).
/// `--json` prints the raw card; a missing card exits 0 with a guiding line.
pub fn show(args: ShowArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    card::store::validate_card_id(&args.id)?;
    let Some(c) = card::store::resolve(&paths, &args.id)?.map(|resolved| resolved.card) else {
        println!("no card {} in the card store (.maestro/cards)", args.id);
        return Ok(());
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&c)?);
    } else {
        // The alias names same-parent siblings, so a parentless card never
        // has one -- skip the store scan that exists only to compute it.
        let alias = if c.parent.is_some() {
            card::query::display_alias(&card::query::scan(&paths)?, &c)
        } else {
            None
        };
        render_show(&c, alias.as_deref());
    }
    Ok(())
}

/// Execute `maestro update <id>`: a generic field mutation (DN9). `--status`,
/// `--title`, and `--description` write through the D1 CAS seam; `--claim`
/// composes the same claim mutation the standalone `claim` verb applies into
/// that single write, so a partial update can never land. `--status` with
/// `--claim` is refused up front: a claim forces `in_progress`, so one would
/// silently clobber the other. A bare `update` (no id) or an update with no
/// flags exits 0 with usage.
pub fn update(args: UpdateArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    let Some(id) = args.id.as_deref() else {
        println!("usage: maestro update <id> [--status S] [--title T] [--description D] [--claim]");
        return Ok(());
    };
    let has_fields = args.status.is_some() || args.title.is_some() || args.description.is_some();
    if !has_fields && !args.claim {
        println!("nothing to update for {id}; pass --status, --title, --description, or --claim");
        return Ok(());
    }
    if args.claim && args.status.is_some() {
        return Err(anyhow!(
            "--status conflicts with --claim (a claim sets in_progress); pass one or the other"
        ));
    }
    card::store::validate_card_id(id)?;
    let now = utc_now_timestamp();
    let Some(resolved) = card::store::resolve(&paths, id)? else {
        println!("no card {id} in the card store (.maestro/cards)");
        return Ok(());
    };
    let mut c = resolved.card.clone();
    if let Some(status) = args.status.as_deref() {
        // SPEC E3: feature/idea/decision keep their per-type lifecycle
        // verbs; a generic status write would bypass their gates (ship/QA,
        // lock stamps, backlog reconciliation).
        if !c.card_type.workable() {
            return Err(anyhow!(
                "cannot set --status on {id} -- a {} keeps its own lifecycle verbs; {}",
                c.card_type.as_str(),
                per_type_verbs_hint(c.card_type)
            ));
        }
        if !card::query::WORKABLE_STATUS_WORDS.contains(&status) {
            return Err(anyhow!(
                "unknown --status {status:?}; expected one of: {}",
                card::query::WORKABLE_STATUS_WORDS.join(", ")
            ));
        }
        c.status = status.to_string();
    }
    if let Some(title) = args.title.as_deref() {
        c.title = title.to_string();
    }
    if let Some(description) = args.description.as_deref() {
        c.description = Some(description.to_string());
    }
    if has_fields {
        c.updated_at = now.clone();
    }
    let claim_outcome = if args.claim {
        let identity = claim_identity();
        let outcome = card::edit::apply_claim(&mut c, &identity, &now)?;
        Some((identity, outcome))
    } else {
        None
    };
    if c != resolved.card {
        card::store::save_resolved(&c, &resolved)?;
    }
    if has_fields {
        println!("updated {id}");
    }
    if let Some((identity, outcome)) = claim_outcome {
        print_claim_outcome(id, &identity, &outcome);
    }
    Ok(())
}

/// Execute `maestro close <id>`: move the card to the uniform terminal status
/// `closed` (coarse Closed) through the D1 CAS seam (DN9). Already-closed and
/// missing cards exit 0 with a guiding line.
pub fn close(args: CloseArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    card::store::validate_card_id(&args.id)?;
    let Some(resolved) = card::store::resolve(&paths, &args.id)? else {
        println!("no card {} in the card store (.maestro/cards)", args.id);
        return Ok(());
    };
    let mut c = resolved.card.clone();
    // SPEC E3: only task/bug/chore are closeable; feature/idea/decision keep
    // their per-type terminal verbs (and their gates).
    if !c.card_type.workable() {
        return Err(anyhow!(
            "cannot close {} -- a {} keeps its own terminal verbs; {}",
            args.id,
            c.card_type.as_str(),
            per_type_verbs_hint(c.card_type)
        ));
    }
    if card::query::coarse_of(&c.status) == Some(card::query::Coarse::Closed) {
        println!("{} is already closed (status: {})", args.id, c.status);
        return Ok(());
    }
    c.status = "closed".to_string();
    c.updated_at = utc_now_timestamp();
    card::store::save_resolved(&c, &resolved)?;
    println!("closed {}", args.id);
    Ok(())
}

/// Where to send a non-workable card's lifecycle instead of `close`/`update
/// --status` (SPEC E3: feature/idea/decision keep per-type terminal verbs).
fn per_type_verbs_hint(card_type: card::schema::CardType) -> &'static str {
    match card_type {
        card::schema::CardType::Feature => "use `maestro feature ship` or `maestro feature cancel`",
        card::schema::CardType::Decision => "use `maestro decision lock`",
        card::schema::CardType::Idea => "use `maestro harness apply/dismiss/measure`",
        card::schema::CardType::Task
        | card::schema::CardType::Bug
        | card::schema::CardType::Chore => "use `maestro close`",
    }
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

fn card_paths() -> Result<Option<MaestroPaths>> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        legacy_notice();
        return Ok(None);
    }
    Ok(Some(paths))
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
    if let Some(description) = card::query::body_of(c) {
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
