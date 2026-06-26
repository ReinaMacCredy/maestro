use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, anyhow};
use serde::Serialize;
use serde_yaml::Value;

use crate::domain::{card, feature, search, task};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::slug::slugify_ascii;
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::{
    ArchiveArgs, AssignArgs, CardPrepareArgs, ClaimArgs, CloseArgs, CreateArgs, DepArgs,
    DepCommand, LinkArgs, LinkCommand, ListArgs, NoteArgs, ReadyArgs, ShowArgs, UpdateArgs,
};
use crate::operations::feature_prepare;

const READY_JSON_SCHEMA: &str = "maestro.ready.v1";
const LIST_JSON_SCHEMA: &str = "maestro.list.v1";
const CARD_QUERY_JSON_VERSION: u8 = 1;

/// Execute `maestro ready`: workable cards with no open blockers.
pub fn ready(args: ReadyArgs) -> Result<()> {
    let paths = if args.json {
        card_paths_json()?
    } else {
        card_paths()?
    };
    let Some(paths) = paths else {
        if args.json {
            render_ready_json(&[])?;
        }
        return Ok(());
    };
    let cards = card::query::scan(&paths)?;
    let mut ready = card::query::ready(&cards);
    if let Some(feature) = args.feature.as_deref() {
        ready.retain(|c| c.parent.as_deref() == Some(feature));
    }
    if let Some(project) = args.project.as_deref() {
        ready.retain(|c| c.project.as_deref() == Some(project));
    }
    if args.json {
        render_ready_json(&ready)?;
    } else {
        render_ready(&ready);
    }
    Ok(())
}

/// Execute `maestro list`: cards filtered by parent, type, assignee, coarse
/// status, or a `--grep` substring; `--archived` extends the same query into
/// the archive tree (SPEC-archive-memory A1).
pub fn list(args: ListArgs) -> Result<()> {
    let paths = if args.json {
        card_paths_json()?
    } else {
        card_paths()?
    };
    let Some(paths) = paths else {
        if args.json {
            render_list_json(&[])?;
        }
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
    let grep = args.grep.as_deref();
    // Search indexes narrow grep to a superset of possible matches; `None` --
    // term too short or indexes unavailable -- falls back to the plain scan.
    let candidates = grep.and_then(|term| grep_candidates(&paths, term));
    let candidates = candidates.as_ref();
    let live = card::query::scan_with_paths(&paths)?;
    let archived = if args.archived {
        card::query::scan_dir_with_paths(&paths.archive_cards_dir())?
    } else {
        Vec::new()
    };
    let mut rows: Vec<(&card::schema::Card, bool)> =
        card::query::query_scanned(&live, &filter, grep, candidates)
            .into_iter()
            .map(|c| (c, false))
            .chain(
                card::query::query_scanned(&archived, &filter, grep, candidates)
                    .into_iter()
                    .map(|c| (c, true)),
            )
            .collect();
    // Bare list (no --all and no explicit narrowing filter) bounds its default to
    // the live slice: drop non-archived coarse-Closed rows so an agent's routine
    // `list` doesn't re-ingest the whole terminal history every orientation. Any
    // explicit filter or --all governs the result as-is; --archived stays
    // orthogonal (it keeps its archived rows here). An unrecognized status maps to
    // coarse None and is kept, never silently hidden.
    let apply_live_slice = !args.all
        && args.parent.is_none()
        && args.card_type.is_none()
        && args.assignee.is_none()
        && args.status.is_none()
        && args.grep.is_none();
    let mut hidden = 0usize;
    if apply_live_slice {
        rows.retain(|(c, archived)| {
            let drop = !*archived
                && card::query::coarse_of(&c.status) == Some(card::query::Coarse::Closed);
            if drop {
                hidden += 1;
            }
            !drop
        });
    }
    if let Some(project) = args.project.as_deref() {
        rows.retain(|(c, _)| c.project.as_deref() == Some(project));
    }
    if args.json {
        render_list_json(&rows)?;
    } else {
        render_list(&rows, hidden, args.archived);
    }
    Ok(())
}

fn grep_candidates(paths: &MaestroPaths, term: &str) -> Option<BTreeSet<String>> {
    let mut candidates = search::card_list_grep_candidates(paths, term);
    if let Some(text_candidates) = card::index::candidates(paths, term) {
        match &mut candidates {
            Some(candidates) => candidates.extend(text_candidates),
            None => candidates = Some(text_candidates),
        }
    }
    candidates
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
        DepCommand::Remove { child, parent } => {
            let removed =
                card::edit::remove_blocks_dep(&paths, &child, &parent, &utc_now_timestamp())?;
            if removed {
                println!("{child} is no longer blocked by {parent}");
            } else {
                println!("{child} is not blocked by {parent}");
            }
        }
    }
    Ok(())
}

/// Execute `maestro link add/remove <from> <to>`: author or remove a non-blocking
/// related edge. The user-facing relation is unordered; storage remains one edge.
pub fn link(args: LinkArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    match args.command {
        LinkCommand::Add { from, to } => {
            let added = card::edit::add_related_link(&paths, &from, &to, &utc_now_timestamp())?;
            if added {
                println!("{from} and {to} are now linked (messaging works both ways)");
            } else {
                println!("{from} is already related to {to}");
            }
        }
        LinkCommand::Remove { from, to } => {
            let removed =
                card::edit::remove_related_link(&paths, &from, &to, &utc_now_timestamp())?;
            if removed {
                println!("removed related link between {from} and {to}");
            } else {
                println!("{from} is not related to {to}");
            }
        }
    }
    Ok(())
}

/// Execute `maestro archive <feature>`: move the feature card and its
/// `parent=<feature>` children to the archive sibling tree. The flat verb
/// drives the same `feature::archive_feature` cascade as `maestro feature
/// archive`, so the typed terminal gate, sweep re-run, and no-clobber pre-flight
/// hold on both spellings. `--loose` sweeps terminal parentless cards instead
/// (SPEC-archive-memory-2 R2).
pub fn archive(args: ArchiveArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    if args.loose {
        let report = feature::archive_loose(&paths)?;
        if report.swept.is_empty() && report.kept_rules.is_empty() {
            println!("nothing loose to archive");
        }
        for id in &report.swept {
            println!("boxed: {id}");
        }
        for id in &report.kept_rules {
            println!("kept:  {id} (rule)");
        }
        return Ok(());
    }
    let feature_id = args
        .feature
        .as_deref()
        .expect("clap requires <feature> unless --loose");
    let report = feature::archive_feature(&paths, feature_id, false)?;
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
    super::emit_work_touch(&paths, &args.id);
    print_claim_outcome(&args.id, &identity, &outcome);
    nudge_if_holding_other_in_progress(&paths, &identity, &args.id);
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

/// After a claim persists, nudge a session that now holds another card
/// in_progress: the focus discipline is one in-flight card per session at a time.
/// Advisory only -- it prints to STDERR (so it never pollutes `--id-only`/scripted
/// stdout), names the already-active card, and never blocks or auto-releases. The
/// pure query keys on the per-session claim id, so a different session holding its
/// own card produces no note. Shared by `claim <id>` and `update <id> --claim`.
fn nudge_if_holding_other_in_progress(paths: &MaestroPaths, identity: &str, just_claimed: &str) {
    let Ok(cards) = card::query::scan(paths) else {
        return;
    };
    let others = card::query::in_progress_held_by(&cards, identity, just_claimed);
    if let Some(other) = others.first() {
        let extra = if others.len() > 1 {
            format!(" (and {} more)", others.len() - 1)
        } else {
            String::new()
        };
        eprintln!(
            "note: you now hold {} cards in_progress -- {} is also active{extra}. \
             Focus is one card at a time; close or pause it when you switch.",
            others.len() + 1,
            other.id,
        );
    }
}

/// Execute `maestro assign <card> <who>`: set or clear a card's advisory
/// `suggested_for` routing hint. Advisory only -- it changes no status, never
/// blocks a claim by any session (agent-teams routing decision), and emits no
/// `card_touch`: routing a hint is not work state, so it must not re-bind the
/// assigner's `active`/`msg` current card. `none` or `--clear` clears the hint.
pub fn assign(args: AssignArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    let who = if args.clear {
        None
    } else {
        match args.who.as_deref().map(str::trim) {
            None | Some("") => {
                return Err(anyhow!(
                    "specify who to suggest {} for, or pass `none` / --clear to clear the hint",
                    args.id
                ));
            }
            Some("none") => None,
            Some(name) => Some(name.to_string()),
        }
    };
    let changed = card::edit::assign(&paths, &args.id, who.as_deref(), &utc_now_timestamp())?;
    match (changed, &who) {
        (true, Some(who)) => println!("{} suggested for {who} (advisory; not a claim)", args.id),
        (false, Some(who)) => println!("{} already suggested for {who}", args.id),
        (true, None) => println!("cleared the assignee hint on {}", args.id),
        (false, None) => println!("{} had no assignee hint to clear", args.id),
    }
    Ok(())
}

/// Execute `maestro note <id> <text>`: append a dated note to the card's
/// `notes.md` sidecar (SPEC D5).
pub fn note(args: NoteArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    let created = card::edit::append_note(&paths, &args.id, &args.text, &utc_now_timestamp())?;
    super::emit_work_touch(&paths, &args.id);
    if created {
        println!("noted {} (notes.md created)", args.id);
    } else {
        println!("noted {}", args.id);
    }
    Ok(())
}

/// Execute `maestro create -t <type> <title>...`: mint one card per title (DN9).
/// A single title is the original one-card behavior; two or more batch-mint that
/// many cards in one invocation. Non-feature cards get a typed slug id
/// `<type>-<slug>-<hex4>` (SPEC-card-slug-ids D1/D1b); feature cards keep an
/// immutable creation slug (SPEC E2). The initial status is the uniform
/// coarse-open word `open`, so a workable card is immediately `ready` once it has
/// no open blocker. `--parent`/`--type` apply to every card; the per-card text
/// fields (`--description`, `--active-form`) are refused in batch mode -- they
/// belong on `card update` once the cards exist.
pub fn create(args: CreateArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    let card_type = parse_card_type(&args.card_type)?;
    let custom_kind = match (card_type, args.kind.as_deref()) {
        (card::schema::CardType::Custom, Some(kind)) if !kind.trim().is_empty() => {
            Some(kind.trim().to_string())
        }
        (card::schema::CardType::Custom, _) => {
            return Err(anyhow!("custom cards require --kind <kind>"));
        }
        (_, Some(_)) => {
            return Err(anyhow!("--kind is only valid with --type custom"));
        }
        _ => None,
    };
    let batch = args.titles.len() > 1;
    // Refuse per-card text in batch mode before minting anything, so a batch
    // create is all-or-nothing: a single shared description/active-form cannot be
    // smeared across many cards, and there is no batch syntax for distinct ones.
    if batch {
        if args.description.is_some() {
            return Err(anyhow!(
                "--description is one card's text; it cannot apply to {} cards at once. \
                 Create them, then set each with `maestro card update <id> --description \"...\"`",
                args.titles.len()
            ));
        }
        if args.active_form.is_some() {
            return Err(anyhow!(
                "--active-form is one card's text; it cannot apply to {} cards at once. \
                 Create them, then set each with `maestro card update <id> --active-form \"...\"`",
                args.titles.len()
            ));
        }
    }
    let now = utc_now_timestamp();
    // Resolve the shared parent and project once, above the mint loop, so a
    // dangling --parent fails before any card is written (batch stays atomic).
    let parent = match args.parent {
        Some(parent) => {
            // SPEC G1/E1: `parent` docks a card under a feature container.
            // Features are roots, and a dangling parent ref would poison the
            // display alias and parent-filtered queries, so the dock is
            // validated at the door.
            if card_type == card::schema::CardType::Feature {
                return Err(anyhow!(
                    "a feature card cannot take --parent; features are top-level containers"
                ));
            }
            if card_type == card::schema::CardType::Custom {
                return Err(anyhow!(
                    "a custom card cannot take --parent; custom cards are top-level containers"
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
            if !parent_card.card_type.owns_task_container() {
                return Err(anyhow!(
                    "parent {parent} is a {}, not a card container; child cards dock under feature, bug, chore, or custom parents",
                    parent_card.card_type.as_str()
                ));
            }
            Some(parent)
        }
        None => None,
    };
    let project = super::resolve_project(args.project, &paths)?;

    for title in &args.titles {
        let id = match card_type {
            card::schema::CardType::Feature => slugify_ascii(title),
            _ => card::store::mint_card_id(&paths, card_type, title),
        };
        let initial_status = match card_type {
            card::schema::CardType::Bug | card::schema::CardType::Custom => "proposed",
            _ => "open",
        };
        let mut new_card = card::schema::Card::new(&id, card_type, title, initial_status, &now);
        new_card.parent = parent.clone();
        new_card.description = args.description.clone();
        new_card.active_form = args.active_form.clone();
        new_card.project = project.clone();
        if let Some(kind) = custom_kind.as_deref() {
            new_card.extra.insert(
                Value::String("kind".to_string()),
                Value::String(kind.to_string()),
            );
        }
        card::store::create_card(&paths, &new_card)?;
        super::emit_work_touch(&paths, &id);
        if args.id_only {
            println!("{id}");
        } else {
            println!("created {id} ({}): {}", card_type.as_str(), title);
        }
    }
    Ok(())
}

pub fn prepare(args: CardPrepareArgs) -> Result<()> {
    let Some(paths) = card_paths()? else {
        return Ok(());
    };
    let has_inline_tasks = !args.task.is_empty();
    match (args.from.as_deref(), args.draft, has_inline_tasks) {
        (Some(_), true, _) => Err(anyhow!(
            "use either --from <plan-file> or --draft, not both"
        )),
        (Some(_), _, true) => Err(anyhow!("use either --from <plan-file> or --task, not both")),
        (_, true, true) => Err(anyhow!("use either --draft or --task, not both")),
        (None, false, false) => Err(anyhow!(
            "card prepare requires --from <plan-file>, --draft, or --task\n  maestro card prepare {} --draft\n  maestro card prepare {} --from <plan-file>\n  maestro card prepare {} --task \"T1: <title>\" --check \"<observable result>\"",
            args.id,
            args.id,
            args.id
        )),
        (None, true, false) => {
            let report = feature_prepare::write_card_draft(&paths, &args.id)?;
            if report.written {
                println!("wrote {}", report.path.display());
            } else {
                println!("draft exists: {}", report.path.display());
            }
            println!("review and run:");
            println!(
                "  maestro card prepare {} --from {}",
                args.id,
                report.path.display()
            );
            Ok(())
        }
        (Some(plan_file), false, false) => {
            let actor = super::actor();
            let report =
                feature_prepare::prepare_card_from_file(&paths, &args.id, plan_file, &actor)?;
            super::emit_work_touch(&paths, &args.id);
            print_card_prepare_report(&report);
            Ok(())
        }
        (None, false, true) => {
            if args.check.iter().any(|check| check.trim().is_empty()) {
                return Err(anyhow!("--check must not be empty"));
            }
            if args.check.is_empty() {
                return Err(anyhow!("--task requires at least one --check"));
            }
            let plan = inline_prepare_plan(
                &args.task,
                &args.check,
                &args.covers,
                &args.blocker,
                &args.after,
            )?;
            let path = paths.cards_dir().join(&args.id).join("prepare-inline.md");
            write_string_atomic(&path, &plan)?;
            let actor = super::actor();
            let report = feature_prepare::prepare_card_from_file(&paths, &args.id, &path, &actor)?;
            super::emit_work_touch(&paths, &args.id);
            print_card_prepare_report(&report);
            Ok(())
        }
    }
}

fn inline_prepare_plan(
    tasks: &[String],
    checks: &[String],
    covers: &[String],
    blockers: &[String],
    after: &[String],
) -> Result<String> {
    let mut plan = String::new();
    for task in tasks {
        let task = task.trim();
        if task.is_empty() {
            return Err(anyhow!("--task must not be empty"));
        }
        plan.push_str("## Task ");
        plan.push_str(task);
        plan.push('\n');
        if !covers.is_empty() {
            plan.push_str("covers: ");
            plan.push_str(&covers.join(", "));
            plan.push('\n');
        }
        for check in checks {
            plan.push_str("check: ");
            plan.push_str(check);
            plan.push('\n');
        }
        for blocker in blockers {
            plan.push_str("blocker: ");
            plan.push_str(blocker);
            plan.push('\n');
        }
        if !after.is_empty() {
            plan.push_str("after: ");
            plan.push_str(&after.join(", "));
            plan.push('\n');
        }
        plan.push('\n');
    }
    Ok(plan)
}

fn print_card_prepare_report(report: &feature_prepare::PrepareReport) {
    println!("prepared {} task(s)", report.task_count);
    if report.started {
        println!("started {} -> in_progress", report.feature_id);
    } else if report.remained_ready {
        println!("card remains ready");
    }
    println!("prepared:");
    for task in &report.prepared {
        let status = if task.blocked { "blocked" } else { "ready" };
        println!("  {}  {:<7} {}", task.id, status, task.title);
    }
    if !report.blockers.is_empty() {
        println!("blockers:");
        for blocker in &report.blockers {
            println!(
                "  {} -> {} ({})",
                blocker.task_id, blocker.blocker_id, blocker.reason
            );
        }
    }
}

/// Execute `maestro show <id>`: the card's header, parent, edges, and body (DN9).
/// `--json` prints the raw card; a missing card exits 0 with a guiding line.
pub fn show(args: ShowArgs) -> Result<()> {
    let paths = if args.compact_json {
        card_paths_json()?
    } else {
        card_paths()?
    };
    let Some(paths) = paths else {
        if args.compact_json {
            println!("null");
        }
        return Ok(());
    };
    card::store::validate_card_id(&args.id)?;
    // Not in the live store -> read-only archive fallback, so a lid line or
    // old reference never dead-ends (SPEC-archive-memory-2 R2; same seam the
    // task verbs use).
    let live = card::store::resolve(&paths, &args.id)?.map(|resolved| resolved.card);
    let archived = if live.is_none() {
        card::store::resolve_in(&paths.archive_cards_dir(), &args.id)?.map(|resolved| resolved.card)
    } else {
        None
    };
    let from_archive = archived.is_some();
    let Some(c) = live.or(archived) else {
        if args.compact_json {
            eprintln!("no card {} in the card store (.maestro/cards)", args.id);
            println!("null");
        } else {
            println!("no card {} in the card store (.maestro/cards)", args.id);
        }
        return Ok(());
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&c)?);
    } else if args.compact_json {
        render_compact_card_json(&c)?;
    } else {
        let live_cards = if from_archive {
            None
        } else {
            Some(card::query::scan(&paths)?)
        };
        let alias = live_cards.as_ref().and_then(|cards| {
            // The alias names same-parent siblings, so a parentless card
            // never has one.
            c.parent
                .as_ref()
                .and_then(|_| card::query::display_alias(cards, &c))
        });
        let related_by = live_cards
            .as_ref()
            .map(|cards| incoming_related(cards, &c.id))
            .unwrap_or_default();
        render_show(&c, alias.as_deref(), &related_by);
        if from_archive {
            println!("archived: read-only (lives in .maestro/archive/cards/)");
        }
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
    let paths = if args.json {
        card_paths_json()?
    } else {
        card_paths()?
    };
    let Some(paths) = paths else {
        if args.json {
            render_update_json(&[])?;
        }
        return Ok(());
    };
    let Some(id) = args.id.as_deref() else {
        if args.json {
            eprintln!(
                "usage: maestro card update <id> [--status S] [--title T] [--description D] [--active-form F] [--claim] [--json]"
            );
            render_update_json(&[])?;
        } else {
            println!(
                "usage: maestro card update <id> [--status S] [--title T] [--description D] [--active-form F] [--claim] [--json]"
            );
        }
        return Ok(());
    };
    let has_fields = args.status.is_some()
        || args.title.is_some()
        || args.description.is_some()
        || args.active_form.is_some();
    if !has_fields && !args.claim {
        if args.json {
            eprintln!(
                "nothing to update for {id}; pass --status, --title, --description, --active-form, or --claim"
            );
            render_update_json(&[])?;
        } else {
            println!(
                "nothing to update for {id}; pass --status, --title, --description, --active-form, or --claim"
            );
        }
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
        if args.json {
            eprintln!("no card {id} in the card store (.maestro/cards)");
            render_update_json(&[])?;
        } else {
            println!("no card {id} in the card store (.maestro/cards)");
        }
        return Ok(());
    };
    let mut c = resolved.card.clone();
    if let Some(status) = args.status.as_deref() {
        // SPEC E3: feature/idea/decision keep their per-type lifecycle
        // verbs; a generic status write would bypass their gates (close/QA,
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
        if matches!(status, "needs_verification" | "verified") {
            let remedy = match status {
                "needs_verification" => format!(
                    "run `maestro task complete {id} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\"`"
                ),
                "verified" => format!("run `maestro task verify {id}`"),
                _ => unreachable!("invariant: guarded status already matched"),
            };
            return Err(anyhow!(
                "cannot set {id} to {status} with generic card update; {remedy}"
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
    if let Some(active_form) = args.active_form.as_deref() {
        c.active_form = Some(active_form.to_string());
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
    emit_update_liveness(&paths, id, args.status.as_deref());
    // `update --claim` shares the claim seam, so it shares the focus nudge. The
    // advisory is STDERR-only, so it is safe to emit before the JSON return.
    if let Some((identity, _)) = claim_outcome.as_ref() {
        nudge_if_holding_other_in_progress(&paths, identity, id);
    }
    if args.json {
        render_update_json(&[&c])?;
        return Ok(());
    }
    if has_fields {
        println!("updated {id}");
    }
    if let Some((identity, outcome)) = claim_outcome {
        print_claim_outcome(id, &identity, &outcome);
    }
    Ok(())
}

fn emit_update_liveness(paths: &MaestroPaths, id: &str, status: Option<&str>) {
    match status.and_then(card::query::coarse_of) {
        Some(card::query::Coarse::Open) => super::emit_ownership_release(
            paths,
            id,
            super::OwnershipReleaseStatus::Released,
            Some("card update status"),
        ),
        Some(card::query::Coarse::Closed) => super::emit_ownership_release(
            paths,
            id,
            super::OwnershipReleaseStatus::Done,
            Some("card update status"),
        ),
        _ => super::emit_work_touch(paths, id),
    }
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
    if c.card_type == card::schema::CardType::Custom
        || ((c.card_type == card::schema::CardType::Bug
            || c.card_type == card::schema::CardType::Chore)
            && card_has_owned_tasks(&paths, &c.id)?)
    {
        return close_task_container_card(&paths, &resolved, &mut c);
    }
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
    super::emit_ownership_release(
        &paths,
        &args.id,
        super::OwnershipReleaseStatus::Done,
        Some("card close"),
    );
    println!("closed {}", args.id);
    Ok(())
}

fn card_has_owned_tasks(paths: &MaestroPaths, id: &str) -> Result<bool> {
    Ok(task::load_task_records(&paths.tasks_dir())?
        .into_iter()
        .any(|task| task.feature_id.as_deref() == Some(id)))
}

fn close_task_container_card(
    paths: &MaestroPaths,
    resolved: &card::store::ResolvedCard,
    c: &mut card::schema::Card,
) -> Result<()> {
    if card::query::coarse_of(&c.status) == Some(card::query::Coarse::Closed) {
        println!("{} is already closed (status: {})", c.id, c.status);
        return Ok(());
    }
    let owned: Vec<task::TaskRecord> = task::load_task_records(&paths.tasks_dir())?
        .into_iter()
        .filter(|task| task.feature_id.as_deref() == Some(c.id.as_str()))
        .collect();
    if owned.is_empty() {
        return Err(anyhow!(
            "cannot close {} -- a {} card closes through owned tasks; add or prepare tasks first",
            c.id,
            c.card_type.as_str()
        ));
    }
    let unfinished: Vec<String> = owned
        .iter()
        .filter(|task| task.state != task::TaskState::Verified)
        .map(|task| format!("{} ({})", task.id, task.state.as_str()))
        .collect();
    if !unfinished.is_empty() {
        return Err(anyhow!(
            "cannot close {} -- owned task(s) are not verified: {}",
            c.id,
            unfinished.join(", ")
        ));
    }
    c.status = "closed".to_string();
    c.updated_at = utc_now_timestamp();
    card::store::save_resolved(c, resolved)?;
    super::emit_ownership_release(
        paths,
        &c.id,
        super::OwnershipReleaseStatus::Done,
        Some("card close"),
    );
    println!("closed {}", c.id);
    Ok(())
}

/// Where to send a non-workable card's lifecycle instead of `close`/`update
/// --status` (SPEC E3: feature/idea/decision keep per-type terminal verbs).
fn per_type_verbs_hint(card_type: card::schema::CardType) -> &'static str {
    match card_type {
        card::schema::CardType::Feature => {
            "use `maestro feature close` or `maestro feature cancel`"
        }
        card::schema::CardType::Custom => {
            "prepare it into tasks, then close it through its container pipeline"
        }
        card::schema::CardType::Progress => {
            "finish its progress tasks before closing the progress card"
        }
        card::schema::CardType::Decision => "use `maestro decision lock`",
        card::schema::CardType::Idea => "use `maestro harness apply/dismiss/measure`",
        card::schema::CardType::Task
        | card::schema::CardType::Bug
        | card::schema::CardType::Chore => "use `maestro card close`",
    }
}

/// Parse a `--type`/`-t` word into a [`card::schema::CardType`], shared by
/// `create` and `list`.
fn parse_card_type(word: &str) -> Result<card::schema::CardType> {
    card::schema::CardType::parse(word).ok_or_else(|| {
        anyhow!(
            "unknown --type {word:?}; expected feature, custom, progress, task, bug, chore, idea, or decision"
        )
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

/// [`card_paths`] for the agent-facing JSON contract: the guiding line moves
/// to stderr so stdout stays parseable JSON even without a card store.
fn card_paths_json() -> Result<Option<MaestroPaths>> {
    let paths = repo_paths()?;
    if !paths.cards_dir().is_dir() {
        eprintln!("{LEGACY_NOTICE}");
        return Ok(None);
    }
    Ok(Some(paths))
}

const LEGACY_NOTICE: &str = "this repo has no card store yet (.maestro/cards/); the card verbs apply once it is migrated to the card model";

/// The card verbs read `.maestro/cards/`; an unmigrated repo has none. Exit 0
/// with one guiding line rather than a dead-end error: no cards is a state.
fn legacy_notice() {
    println!("{LEGACY_NOTICE}");
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
            "  {rank}. [P{rank}] {:<id_width$}  {:<type_width$}  {:<title_width$}  {}{}{}",
            c.id,
            c.card_type.as_str(),
            c.title,
            claim_label(c),
            project_badge(c),
            assignee_hint(c),
        );
    }
}

fn render_ready_json(cards: &[&card::schema::Card]) -> Result<()> {
    let cards = cards
        .iter()
        .enumerate()
        .map(|(index, card)| ReadyCardJson::new(index + 1, card))
        .collect();
    let report = ReadyJson {
        version: CARD_QUERY_JSON_VERSION,
        schema: READY_JSON_SCHEMA,
        cards,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

/// Render `list` in the beads structure (SPEC DN9): a count header plus numbered
/// rows carrying the real per-type status and parent, emoji-free. Archived rows
/// (SPEC-archive-memory A1) carry a trailing `(archived: <parent>)` marker.
fn render_list(rows: &[(&card::schema::Card, bool)], hidden: usize, archived: bool) {
    if rows.is_empty() {
        println!("no cards match");
        return;
    }
    if hidden > 0 && !archived {
        println!("{} live ({hidden} closed hidden; --all)", rows.len());
    } else {
        println!("{} {}:", rows.len(), plural(rows.len()));
    }
    let id_width = rows.iter().map(|(c, _)| c.id.len()).max().unwrap_or(0);
    let type_width = rows
        .iter()
        .map(|(c, _)| c.card_type.as_str().len())
        .max()
        .unwrap_or(0);
    let status_width = rows
        .iter()
        .map(|(c, _)| card::query::canonical_status(&c.status).len())
        .max()
        .unwrap_or(0);
    let parent_width = rows
        .iter()
        .map(|(c, _)| c.parent.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(0);
    // Column widths are global so alignment stays consistent whether the list is
    // flat or grouped; only headers are added inside groups.
    let print_row = |rank: usize, c: &card::schema::Card, archived: bool| {
        let marker = match (archived, c.parent.as_deref()) {
            (true, Some(parent)) => format!("  (archived: {parent})"),
            (true, None) => "  (archived)".to_string(),
            (false, _) => String::new(),
        };
        println!(
            "  {rank}. {:<id_width$}  {:<type_width$}  {:<status_width$}  {:<parent_width$}  {}{}{}{marker}",
            c.id,
            c.card_type.as_str(),
            card::query::canonical_status(&c.status),
            c.parent.as_deref().unwrap_or("-"),
            c.title,
            project_badge(c),
            assignee_hint(c),
        );
    };

    let mut project_groups: BTreeMap<&str, Vec<(&card::schema::Card, bool)>> = BTreeMap::new();
    let mut unassigned = Vec::new();
    for (card, archived) in rows {
        match card.project.as_deref() {
            Some(project) => project_groups
                .entry(project)
                .or_default()
                .push((*card, *archived)),
            None => unassigned.push((*card, *archived)),
        }
    }
    // Group under project headers only when >=2 distinct projects appear among the
    // shown rows (ac-6); with 0 or 1 distinct project the list is flat and, for the
    // zero case, byte-identical to today's badge-free output (ac-3).
    if project_groups.len() < 2 {
        for (i, (c, archived)) in rows.iter().enumerate() {
            print_row(i + 1, c, *archived);
        }
        return;
    }
    let mut rank = 0usize;
    for (project, group) in project_groups {
        println!("{project}:");
        for (c, archived) in group {
            rank += 1;
            print_row(rank, c, archived);
        }
    }
    if !unassigned.is_empty() {
        println!("unassigned:");
    }
    for (c, archived) in unassigned {
        rank += 1;
        print_row(rank, c, archived);
    }
}

fn render_list_json(rows: &[(&card::schema::Card, bool)]) -> Result<()> {
    let cards = rows
        .iter()
        .map(|(card, archived)| ListCardJson::new(card, *archived))
        .collect();
    let report = ListJson {
        version: CARD_QUERY_JSON_VERSION,
        schema: LIST_JSON_SCHEMA,
        cards,
    };
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

fn render_update_json(cards: &[&card::schema::Card]) -> Result<()> {
    let cards: Vec<CompactCardJson<'_>> = cards
        .iter()
        .map(|card| CompactCardJson::new(card))
        .collect();
    println!("{}", serde_json::to_string_pretty(&cards)?);
    Ok(())
}

fn render_compact_card_json(card: &card::schema::Card) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&CompactCardJson::new(card))?
    );
    Ok(())
}

/// Render `show <id>` (SPEC DN9): header line + parent + edges grouped by kind +
/// body (timestamps and description). Emoji-free.
fn render_show(c: &card::schema::Card, alias: Option<&str>, related_by: &[String]) {
    println!(
        "{}  {}  {}  {}  {}",
        c.id,
        c.card_type.as_str(),
        c.title,
        card::query::canonical_status(&c.status),
        claim_label(c),
    );
    if let Some(parent) = &c.parent {
        println!("parent: {parent}");
    }
    // show is the full-detail view: reveal the raw advisory hint even once the
    // card is claimed (the work-board renders suppress it; show does not).
    if let Some(who) = &c.suggested_for {
        println!("suggested for: {who}");
    }
    // SPEC E2: the dotted alias is render-time only -- never a ref, never
    // accepted as an address, so it is labeled to discourage `claim <alias>`.
    if let Some(alias) = alias {
        println!("alias: {alias} (display only)");
    }
    render_edges(c, card::schema::DepKind::Blocks, "blocked by");
    render_edges(c, card::schema::DepKind::Related, "related");
    render_edge_ids(related_by, "related by");
    render_edges(c, card::schema::DepKind::Supersedes, "supersedes");
    println!("created: {}  updated: {}", c.created_at, c.updated_at);
    if let Some(description) = card::query::body_of(c) {
        println!();
        println!("{description}");
    }
}

fn incoming_related(cards: &[card::schema::Card], id: &str) -> Vec<String> {
    let mut sources: Vec<String> = cards
        .iter()
        .filter(|candidate| candidate.id != id)
        .filter(|candidate| {
            candidate
                .deps
                .iter()
                .any(|dep| dep.kind == card::schema::DepKind::Related && dep.target == id)
        })
        .map(|candidate| candidate.id.clone())
        .collect();
    sources.sort();
    sources.dedup();
    sources
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

fn render_edge_ids(ids: &[String], label: &str) {
    if !ids.is_empty() {
        println!("{label}: {}", ids.join(", "));
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

/// Trailing ` [<project>]` token for a row whose card carries a project scope, or
/// the empty string otherwise. A project-less row stays byte-identical to today.
fn project_badge(c: &card::schema::Card) -> String {
    match c.project.as_deref() {
        Some(project) => format!("  [{project}]"),
        None => String::new(),
    }
}

/// Trailing `  -> for <who>` token for an UNCLAIMED card carrying an advisory
/// `suggested_for` hint, or the empty string otherwise. A claim supersedes the
/// hint in the work-board render (the claimant shows instead), so a claimed card
/// renders no hint here even when the field is still set.
fn assignee_hint(c: &card::schema::Card) -> String {
    match (&c.claimed_by, c.suggested_for.as_deref()) {
        (None, Some(who)) => format!("  -> for {who}"),
        _ => String::new(),
    }
}

#[derive(Serialize)]
struct ReadyJson<'a> {
    version: u8,
    schema: &'static str,
    cards: Vec<ReadyCardJson<'a>>,
}

#[derive(Serialize)]
struct ReadyCardJson<'a> {
    rank: usize,
    id: &'a str,
    #[serde(rename = "type")]
    card_type: &'static str,
    title: &'a str,
    status: &'a str,
    parent: Option<&'a str>,
    claimed_by: Option<&'a str>,
    suggested_for: Option<&'a str>,
    project: Option<&'a str>,
}

impl<'a> ReadyCardJson<'a> {
    fn new(rank: usize, card: &'a card::schema::Card) -> Self {
        Self {
            rank,
            id: &card.id,
            card_type: card.card_type.as_str(),
            title: &card.title,
            status: &card.status,
            parent: card.parent.as_deref(),
            claimed_by: card.claimed_by.as_deref(),
            suggested_for: card.suggested_for.as_deref(),
            project: card.project.as_deref(),
        }
    }
}

#[derive(Serialize)]
struct ListJson<'a> {
    version: u8,
    schema: &'static str,
    cards: Vec<ListCardJson<'a>>,
}

#[derive(Serialize)]
struct ListCardJson<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    card_type: &'static str,
    title: &'a str,
    status: &'a str,
    parent: Option<&'a str>,
    claimed_by: Option<&'a str>,
    claimed_at: Option<&'a str>,
    suggested_for: Option<&'a str>,
    project: Option<&'a str>,
    archived: bool,
}

impl<'a> ListCardJson<'a> {
    fn new(card: &'a card::schema::Card, archived: bool) -> Self {
        Self {
            id: &card.id,
            card_type: card.card_type.as_str(),
            title: &card.title,
            status: &card.status,
            parent: card.parent.as_deref(),
            claimed_by: card.claimed_by.as_deref(),
            claimed_at: card.claimed_at.as_deref(),
            suggested_for: card.suggested_for.as_deref(),
            project: card.project.as_deref(),
            archived,
        }
    }
}

#[derive(Serialize)]
struct CompactCardJson<'a> {
    id: &'a str,
    title: &'a str,
    status: &'a str,
    #[serde(rename = "type")]
    card_type: &'static str,
    parent: Option<&'a str>,
    claimed_by: Option<&'a str>,
    claimed_at: Option<&'a str>,
}

impl<'a> CompactCardJson<'a> {
    fn new(card: &'a card::schema::Card) -> Self {
        Self {
            id: &card.id,
            title: &card.title,
            status: &card.status,
            card_type: card.card_type.as_str(),
            parent: card.parent.as_deref(),
            claimed_by: card.claimed_by.as_deref(),
            claimed_at: card.claimed_at.as_deref(),
        }
    }
}
