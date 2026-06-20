use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Result, bail};

use crate::domain::card;
use crate::domain::decisions;
use crate::domain::feature;
use crate::domain::feature::FeatureStatus;
use crate::domain::proof;
use crate::domain::run;
use crate::domain::task;
use crate::foundation::core::git;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::table;
use crate::foundation::core::time::{timestamp_nanos, utc_now_timestamp};
use crate::interfaces::cli::{QueryArgs, QueryCommand};
use crate::operations::harness;

/// Default `query run` window when no `--since` is given (one overnight run).
const DEFAULT_WINDOW_NANOS: i128 = 12 * 60 * 60 * 1_000_000_000;
const NANOS_PER_MINUTE: i128 = 60 * 1_000_000_000;

/// Execute `maestro query`.
pub fn run(args: QueryArgs) -> Result<()> {
    match args.command {
        QueryCommand::Proof {
            task_id,
            task_id_flag,
        } => run_proof(task_id, task_id_flag),
        QueryCommand::Matrix => query_matrix(&query_paths()?),
        QueryCommand::Friction => query_friction(&query_paths()?),
        QueryCommand::Decisions { all, feature } => {
            query_decisions(&query_paths()?, all, feature.as_deref())
        }
        QueryCommand::Backlog => query_backlog(&query_paths()?),
        QueryCommand::Graph { id, dot } => run_graph(id, dot),
        QueryCommand::Run { since, json } => query_run(&query_paths()?, since.as_deref(), json),
    }
}

fn query_paths() -> Result<MaestroPaths> {
    Ok(MaestroPaths::new(discover_repo_root()?))
}

/// The proof read view, shared by the canonical `task proof` and the hidden
/// back-compat alias `query proof`.
pub fn run_proof(task_id: Option<String>, task_id_flag: Option<String>) -> Result<()> {
    let paths = query_paths()?;
    let explicit = match (task_id, task_id_flag) {
        (Some(positional), Some(flag)) if positional != flag => bail!(
            "conflicting task ids: positional `{positional}` and --task-id `{flag}`; pass just one"
        ),
        (Some(id), _) | (None, Some(id)) => Some(id),
        (None, None) => None,
    };
    // Honor MAESTRO_CURRENT_TASK like the sibling read view `task show`
    // (strict: no single-task auto-detect), and name it in the remedy.
    let task_id = match explicit {
        Some(id) => id,
        None => match std::env::var("MAESTRO_CURRENT_TASK") {
            Ok(id) if !id.trim().is_empty() => id,
            _ => bail!("task id is required or set MAESTRO_CURRENT_TASK for `maestro task proof`"),
        },
    };
    let status = proof::proof_status(&paths, &task_id)?;
    print!("{}", proof::render_proof_status(&status));
    Ok(())
}

/// The typed-edge graph view, shared by the canonical `card graph` and the
/// hidden back-compat alias `query graph`.
pub fn run_graph(id: Option<String>, dot: bool) -> Result<()> {
    query_graph(&query_paths()?, id, dot)
}

/// How many hops `query graph <id>` walks from the root (SPEC R7 preview).
/// `--dot` is the escape hatch for anything deeper: it exports the whole web.
const GRAPH_TREE_HOPS: usize = 2;

/// One directed edge as the data stores it: `from` owns the field.
struct GraphEdge {
    from: String,
    to: String,
    kind: EdgeKind,
}

/// The typed-edge taxonomy `query graph` walks. Forward and reverse labels
/// live in one exhaustive pairing so a new kind cannot ship with a silently
/// mislabeled reverse side.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum EdgeKind {
    Parent,
    BlockedBy,
    Related,
    Supersedes,
}

impl EdgeKind {
    /// The holder's perspective word, as the tree and DOT print it.
    fn label(self) -> &'static str {
        match self {
            Self::Parent => "parent",
            Self::BlockedBy => "blocked-by",
            Self::Related => "related",
            Self::Supersedes => "supersedes",
        }
    }

    /// The word the edge target's side sees: `parent` reads as `child` from
    /// the parent, `blocked-by` as `blocks`, `supersedes` as `superseded-by`;
    /// `related` is symmetric.
    fn reverse_label(self) -> &'static str {
        match self {
            Self::Parent => "child",
            Self::BlockedBy => "blocks",
            Self::Related => "related",
            Self::Supersedes => "superseded-by",
        }
    }
}

fn query_graph(paths: &MaestroPaths, id: Option<String>, dot: bool) -> Result<()> {
    let cards = card::query::scan(paths)?;
    let edges = graph_edges(&cards);
    match (id, dot) {
        (None, false) => bail!(
            "provide a card id or --dot\n  tree: maestro card graph <id>\n  whole web: maestro card graph --dot"
        ),
        (Some(id), false) => print_graph_tree(paths, &cards, &edges, &id),
        (None, true) => {
            print!("{}", render_dot(&cards, &edges, None));
            Ok(())
        }
        (Some(id), true) => {
            let root = resolve_graph_root(paths, &id)?;
            let members = component_members(&edges, &root);
            print!("{}", render_dot(&cards, &edges, Some(&members)));
            Ok(())
        }
    }
}

fn resolve_graph_root(paths: &MaestroPaths, id: &str) -> Result<String> {
    let Some(resolved) = card::store::resolve(paths, id)? else {
        bail!(
            "no card {id} in the live store\n  list ids: maestro card list\n  archived cards stay greppable: maestro card list --grep <word> --archived"
        );
    };
    Ok(resolved.card.id)
}

/// Collect every typed edge: the `parent` field, the `deps` list, and -- for
/// decision cards -- the record's `supersedes` list, which the fold keeps in
/// `extra` rather than as a dep (fold.rs writes decision cards with no deps).
fn graph_edges(cards: &[card::schema::Card]) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    let mut seen = BTreeSet::new();
    for c in cards {
        if let Some(parent) = &c.parent {
            push_edge(&mut edges, &mut seen, &c.id, parent, EdgeKind::Parent);
        }
        for dep in &c.deps {
            // A Blocks dep points at the card BLOCKING the holder (`ready`
            // waits on dep targets), so it reads "blocked-by", matching
            // `show`'s "blocked by" rendering.
            let kind = match dep.kind {
                card::schema::DepKind::Blocks => EdgeKind::BlockedBy,
                card::schema::DepKind::Related => EdgeKind::Related,
                card::schema::DepKind::Supersedes => EdgeKind::Supersedes,
            };
            push_edge(&mut edges, &mut seen, &c.id, &dep.target, kind);
        }
        if c.card_type == card::schema::CardType::Decision
            && let Some(serde_yaml::Value::Sequence(targets)) = c
                .extra
                .get(serde_yaml::Value::String("supersedes".to_string()))
        {
            for target in targets.iter().filter_map(serde_yaml::Value::as_str) {
                push_edge(&mut edges, &mut seen, &c.id, target, EdgeKind::Supersedes);
            }
        }
    }
    edges
}

fn push_edge(
    edges: &mut Vec<GraphEdge>,
    seen: &mut BTreeSet<(String, String, EdgeKind)>,
    from: &str,
    to: &str,
    kind: EdgeKind,
) {
    if seen.insert((from.to_string(), to.to_string(), kind)) {
        edges.push(GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            kind,
        });
    }
}

/// Undirected adjacency with the label each endpoint sees.
fn adjacency(edges: &[GraphEdge]) -> BTreeMap<&str, Vec<(&'static str, &str)>> {
    let mut adj: BTreeMap<&str, Vec<(&'static str, &str)>> = BTreeMap::new();
    for edge in edges {
        adj.entry(&edge.from)
            .or_default()
            .push((edge.kind.label(), &edge.to));
        adj.entry(&edge.to)
            .or_default()
            .push((edge.kind.reverse_label(), &edge.from));
    }
    for neighbors in adj.values_mut() {
        neighbors.sort();
        neighbors.dedup();
    }
    adj
}

fn print_graph_tree(
    paths: &MaestroPaths,
    cards: &[card::schema::Card],
    edges: &[GraphEdge],
    id: &str,
) -> Result<()> {
    let root = resolve_graph_root(paths, id)?;
    let by_id: BTreeMap<&str, &card::schema::Card> =
        cards.iter().map(|c| (c.id.as_str(), c)).collect();
    let adj = adjacency(edges);

    // Pre-mark every edge target outside the live store: an archived card (the
    // lid points at it) or a genuinely dangling ref. Sources are always live --
    // edges are read off scanned cards. One archive scan classifies them all,
    // and a malformed target reads as missing instead of failing the render.
    let unknown: BTreeSet<&str> = edges
        .iter()
        .map(|edge| edge.to.as_str())
        .filter(|target| !by_id.contains_key(target))
        .collect();
    let mut dangling: BTreeMap<&str, &'static str> = BTreeMap::new();
    if !unknown.is_empty() {
        let archived: BTreeSet<String> =
            card::query::scan_dir_with_paths(&paths.archive_cards_dir())?
                .into_iter()
                .map(|(card, _)| card.id)
                .collect();
        for target in unknown {
            let mark = if archived.contains(target) {
                "[archived]"
            } else {
                "[missing]"
            };
            dangling.insert(target, mark);
        }
    }

    println!(
        "{}",
        node_line(
            by_id
                .get(root.as_str())
                .expect("resolved root is in the scan")
        )
    );
    let mut visited = BTreeSet::from([root.clone()]);
    let printed = print_tree_level(&adj, &by_id, &dangling, &mut visited, &root, "", 1);
    if printed == 0 {
        println!("no connected cards");
    }
    Ok(())
}

fn print_tree_level(
    adj: &BTreeMap<&str, Vec<(&'static str, &str)>>,
    by_id: &BTreeMap<&str, &card::schema::Card>,
    dangling: &BTreeMap<&str, &'static str>,
    visited: &mut BTreeSet<String>,
    id: &str,
    came_from: &str,
    depth: usize,
) -> usize {
    let Some(neighbors) = adj.get(id) else {
        return 0;
    };
    let indent = "  ".repeat(depth - 1);
    let mut printed = 0;
    for (label, neighbor) in neighbors {
        // The immediate back-edge is the line that brought us here; reprinting
        // it under every child is noise. Other revisits stay visible.
        if *neighbor == came_from {
            continue;
        }
        printed += 1;
        match by_id.get(neighbor) {
            Some(card) => {
                if !visited.insert((*neighbor).to_string()) {
                    println!("{indent}- {label}: {neighbor} (shown above)");
                    continue;
                }
                println!("{indent}- {label}: {}", node_line(card));
                if depth < GRAPH_TREE_HOPS {
                    print_tree_level(adj, by_id, dangling, visited, neighbor, id, depth + 1);
                }
            }
            None => {
                let mark = dangling.get(neighbor).copied().unwrap_or("[missing]");
                println!("{indent}- {label}: {neighbor} {mark}");
            }
        }
    }
    printed
}

fn node_line(card: &card::schema::Card) -> String {
    format!(
        "{} ({}, {}) {}",
        card.id,
        card.card_type.as_str(),
        card::query::canonical_status(&card.status),
        card.title
    )
}

/// Every id reachable from `root` over the undirected web, dangling targets
/// included (they render as dashed nodes).
fn component_members(edges: &[GraphEdge], root: &str) -> BTreeSet<String> {
    let adj = adjacency(edges);
    let mut members = BTreeSet::from([root.to_string()]);
    let mut queue = VecDeque::from([root.to_string()]);
    while let Some(id) = queue.pop_front() {
        let Some(neighbors) = adj.get(id.as_str()) else {
            continue;
        };
        for (_, neighbor) in neighbors {
            if members.insert((*neighbor).to_string()) {
                queue.push_back((*neighbor).to_string());
            }
        }
    }
    members
}

fn render_dot(
    cards: &[card::schema::Card],
    edges: &[GraphEdge],
    members: Option<&BTreeSet<String>>,
) -> String {
    let in_scope = |id: &str| members.is_none_or(|set| set.contains(id));
    let mut out = String::from("digraph cards {\n  rankdir=LR;\n");
    let mut declared: BTreeSet<&str> = BTreeSet::new();
    for card in cards.iter().filter(|card| in_scope(&card.id)) {
        declared.insert(&card.id);
        out.push_str(&format!(
            "  \"{}\" [label=\"{}\\n{}:{}\\n{}\"];\n",
            dot_escape(&card.id),
            dot_escape(&card.id),
            card.card_type.as_str(),
            dot_escape(&card.status),
            dot_escape(&card.title)
        ));
    }
    // Targets outside the live store render as dashed placeholders so the
    // picture stays honest about edges into the archive (or thin air).
    for edge in edges.iter().filter(|edge| in_scope(&edge.to)) {
        if in_scope(&edge.from) && !declared.contains(edge.to.as_str()) {
            declared.insert(&edge.to);
            out.push_str(&format!(
                "  \"{}\" [label=\"{}\" style=dashed];\n",
                dot_escape(&edge.to),
                dot_escape(&edge.to)
            ));
        }
    }
    for edge in edges
        .iter()
        .filter(|edge| in_scope(&edge.from) && in_scope(&edge.to))
    {
        out.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
            dot_escape(&edge.from),
            dot_escape(&edge.to),
            edge.kind.label()
        ));
    }
    out.push_str("}\n");
    out
}

fn dot_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn query_matrix(paths: &MaestroPaths) -> Result<()> {
    let current_commit = git::head(paths.repo_root()).unwrap_or(None);
    let entries = task::load_task_entries(&paths.tasks_dir())?;
    let features = feature::list_with_entries(paths, &entries)?;
    let mut task_rows = entries
        .iter()
        .map(|entry| matrix_row(&entry.task, current_commit.clone()))
        .collect::<Result<Vec<_>>>()?;
    task_rows.sort_by(|left, right| {
        left.feature_id
            .cmp(&right.feature_id)
            .then(left.id.cmp(&right.id))
    });

    if task_rows.is_empty() && features.is_empty() {
        println!("no features or tasks found");
        return Ok(());
    }

    let mut features_with_tasks = std::collections::HashSet::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in &task_rows {
        if row.feature_id != "<none>" {
            features_with_tasks.insert(row.feature_id.clone());
        }
        rows.push(vec![
            row.feature_id.clone(),
            row.id.clone(),
            row.state.to_string(),
            row.proof.to_string(),
            row.title.clone(),
        ]);
    }

    for view in features
        .iter()
        .filter(|view| !features_with_tasks.contains(&view.id))
    {
        rows.push(vec![
            view.id.clone(),
            "<none>".to_string(),
            "<none>".to_string(),
            "<none>".to_string(),
            view.title.clone(),
        ]);
    }
    print!(
        "{}",
        table::render_table(&["FEATURE", "TASK", "STATE", "PROOF", "TITLE"], &rows)
    );
    Ok(())
}

fn query_friction(paths: &MaestroPaths) -> Result<()> {
    let logs = run::managed_event_logs(paths)?;
    let sessions = logs.len();
    let mut events = 0_usize;
    let mut user_prompts = 0_usize;
    let mut corrections = 0_usize;
    let mut kinds = BTreeMap::<String, usize>::new();

    run::visit_managed_event_logs(&logs, |record| {
        let event = record.event();
        let kind = event
            .event_type()
            .or_else(|| event.alias_kind())
            .unwrap_or("<unknown>")
            .to_string();
        // card_touch is the session->card binding auto-emitted for `maestro
        // active` (D3), not a session-friction signal; counting it would inflate
        // the telemetry in step with routine work, so it stays out of every tally.
        if kind == "card_touch" {
            return Ok(());
        }
        events += 1;
        *kinds.entry(kind.clone()).or_default() += 1;
        if kind == "UserPromptSubmit" {
            user_prompts += 1;
            if harness::looks_like_correction(event.prompt_text().unwrap_or_default()) {
                corrections += 1;
            }
        }
        Ok(())
    })?;

    if events == 0 {
        println!("friction: no events found");
        return Ok(());
    }

    println!("FRICTION");
    println!("sessions: {sessions}");
    println!("events: {events}");
    println!("user_prompts: {user_prompts}");
    println!("corrections: {corrections}");
    println!("event_kinds:");
    for (kind, count) in kinds {
        println!("- {kind}: {count}");
    }
    Ok(())
}

fn query_decisions(paths: &MaestroPaths, all: bool, feature: Option<&str>) -> Result<()> {
    super::decision::render_decision_list(decisions::list(paths)?, all, feature)
}

fn query_backlog(paths: &MaestroPaths) -> Result<()> {
    let backlog = harness::load_backlog(paths)?;
    if backlog.items.is_empty() {
        println!("no backlog items found");
        return Ok(());
    }

    let rows: Vec<Vec<String>> = backlog
        .items
        .iter()
        .map(|item| vec![item.id.clone(), item.title.clone()])
        .collect();
    print!("{}", table::render_table(&["ID", "TITLE"], &rows));
    Ok(())
}

/// `maestro query run`: reassemble the run trace for a window from the durable
/// run log, plus the honest current-state status. A pure read over persisted
/// state -- it schedules and starts nothing.
fn query_run(paths: &MaestroPaths, since: Option<&str>, json: bool) -> Result<()> {
    let now = utc_now_timestamp();
    let now_nanos = timestamp_nanos(&now).unwrap_or(0);
    let (cutoff_nanos, window) = match since {
        Some(raw) => match timestamp_nanos(raw) {
            Some(nanos) => (nanos, format!("since {raw}")),
            None => bail!("--since `{raw}` is not a timestamp (expected RFC3339, e.g. 2026-06-21T00:00:00Z)"),
        },
        None => (now_nanos - DEFAULT_WINDOW_NANOS, "last 12h".to_string()),
    };

    let trace = run::assemble_trace(paths, cutoff_nanos)?;
    let status = compute_run_status(paths, trace.last_activity.as_deref(), now_nanos)?;

    if json {
        let report = serde_json::json!({
            "window": window,
            "now": now,
            "trace": trace,
            "status": status,
            // The interruption inference is a method on RunStatus, not a serialized
            // field; surface it so the JSON consumer gets the same honest verdict
            // the text render prints.
            "verdict": status.verdict(),
        });
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    render_run_text(&window, &trace, &status);
    Ok(())
}

/// Current backlog state at trace time. `ready` comes from the live card scan;
/// `accepted_without_tasks`/`proposed` from the feature registry. `minutes_since_last`
/// is derived from the trace's newest activity, so the verdict can infer an
/// interruption without inventing a stop reason.
fn compute_run_status(
    paths: &MaestroPaths,
    last_activity: Option<&str>,
    now_nanos: i128,
) -> Result<run::RunStatus> {
    let cards = card::query::scan(paths)?;
    let ready = card::query::ready(&cards).len();

    let entries = task::load_task_entries(&paths.tasks_dir())?;
    let features = feature::list_with_entries(paths, &entries)?;
    let mut accepted_without_tasks = 0;
    let mut proposed = 0;
    for view in &features {
        match view.status {
            FeatureStatus::Ready if view.counts.total == 0 => accepted_without_tasks += 1,
            FeatureStatus::Proposed => proposed += 1,
            _ => {}
        }
    }

    let minutes_since_last = last_activity
        .and_then(timestamp_nanos)
        .map(|then| ((now_nanos - then) / NANOS_PER_MINUTE) as i64);

    Ok(run::RunStatus {
        ready,
        accepted_without_tasks,
        proposed,
        minutes_since_last,
    })
}

fn render_run_text(window: &str, trace: &run::RunTrace, status: &run::RunStatus) {
    println!("RUN TRACE ({window})");
    println!(
        "sessions: {}  cards touched: {}",
        trace.session_count,
        trace.entries.len()
    );
    if trace.entries.is_empty() {
        println!("  no card activity in window");
    }
    for entry in &trace.entries {
        let mut line = format!("  {}  [{}]  {}", entry.card_id, entry.status, entry.title);
        if let Some(tdd) = &entry.tdd {
            line.push_str(&format!("  ({tdd})"));
        }
        if entry.resumed_across_sessions() {
            line.push_str(&format!(
                "  resumed across {} sessions",
                entry.session_count
            ));
        }
        if !entry.blocked_by.is_empty() {
            line.push_str(&format!("  blocked by {}", entry.blocked_by.join(", ")));
        }
        println!("{line}");
        if let Some(proof) = &entry.latest_proof {
            println!("      proof: {proof}");
        }
    }

    println!();
    println!(
        "current state at trace time: ready={} accepted-without-tasks={} proposed={}",
        status.ready, status.accepted_without_tasks, status.proposed
    );
    println!("{}", status.verdict());
}

#[derive(Debug)]
struct MatrixRow {
    feature_id: String,
    id: String,
    state: &'static str,
    proof: &'static str,
    title: String,
}

fn matrix_row(task: &task::TaskRecord, current_commit: Option<String>) -> Result<MatrixRow> {
    Ok(MatrixRow {
        feature_id: task
            .feature_id
            .clone()
            .unwrap_or_else(|| "<none>".to_string()),
        id: task.id.clone(),
        state: task_state_label(&task.state, task::has_unresolved_blockers(task)),
        proof: proof_label(task, current_commit)?,
        title: task.title.clone(),
    })
}

fn proof_label(task: &task::TaskRecord, current_commit: Option<String>) -> Result<&'static str> {
    Ok(proof::proof_status_kind_for_task(task, current_commit)?.label())
}

fn task_state_label(state: &task::TaskState, blocked: bool) -> &'static str {
    if blocked {
        return "blocked";
    }
    state.as_str()
}
