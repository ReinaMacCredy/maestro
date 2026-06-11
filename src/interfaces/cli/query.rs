use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Result, bail};

use crate::decisions;
use crate::domain::card;
use crate::domain::feature;
use crate::domain::proof;
use crate::domain::run;
use crate::domain::task;
use crate::foundation::core::git;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::table;
use crate::interfaces::cli::{QueryArgs, QueryCommand};
use crate::operations::harness;

/// Execute `maestro query`.
pub fn run(args: QueryArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        QueryCommand::Proof {
            task_id,
            task_id_flag,
        } => {
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
                    _ => bail!(
                        "task id is required or set MAESTRO_CURRENT_TASK for `maestro query proof`"
                    ),
                },
            };
            let status = proof::proof_status(&paths, &task_id)?;
            print!("{}", proof::render_proof_status(&status));
            Ok(())
        }
        QueryCommand::Matrix => query_matrix(&paths),
        QueryCommand::Friction => query_friction(&paths),
        QueryCommand::Decisions => query_decisions(&paths),
        QueryCommand::Backlog => query_backlog(&paths),
        QueryCommand::Graph { id, dot } => query_graph(&paths, id, dot),
    }
}

/// How many hops `query graph <id>` walks from the root (SPEC R7 preview).
/// `--dot` is the escape hatch for anything deeper: it exports the whole web.
const GRAPH_TREE_HOPS: usize = 2;

/// One directed edge as the data stores it: `from` owns the field, `kind` is
/// the from-card's perspective word.
struct GraphEdge {
    from: String,
    to: String,
    kind: &'static str,
}

fn query_graph(paths: &MaestroPaths, id: Option<String>, dot: bool) -> Result<()> {
    let cards = card::query::scan(paths)?;
    let edges = graph_edges(&cards);
    match (id, dot) {
        (None, false) => bail!(
            "provide a card id or --dot\n  tree: maestro query graph <id>\n  whole web: maestro query graph --dot"
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
            "no card {id} in the live store\n  list ids: maestro list\n  archived cards stay greppable: maestro list --grep <word> --archived"
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
            push_edge(&mut edges, &mut seen, &c.id, parent, "parent");
        }
        for dep in &c.deps {
            // The holder's perspective word: a Blocks dep points at the card
            // BLOCKING the holder (`ready` waits on dep targets), so the edge
            // reads "blocked-by", matching `show`'s "blocked by" rendering.
            let kind = match dep.kind {
                card::schema::DepKind::Blocks => "blocked-by",
                card::schema::DepKind::Related => "related",
                card::schema::DepKind::Supersedes => "supersedes",
            };
            push_edge(&mut edges, &mut seen, &c.id, &dep.target, kind);
        }
        if c.card_type == card::schema::CardType::Decision
            && let Some(serde_yaml::Value::Sequence(targets)) = c
                .extra
                .get(serde_yaml::Value::String("supersedes".to_string()))
        {
            for target in targets.iter().filter_map(serde_yaml::Value::as_str) {
                push_edge(&mut edges, &mut seen, &c.id, target, "supersedes");
            }
        }
    }
    edges
}

fn push_edge(
    edges: &mut Vec<GraphEdge>,
    seen: &mut BTreeSet<(String, String, &'static str)>,
    from: &str,
    to: &str,
    kind: &'static str,
) {
    if seen.insert((from.to_string(), to.to_string(), kind)) {
        edges.push(GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            kind,
        });
    }
}

/// Undirected adjacency with the label each endpoint sees: `parent` reads as
/// `child` from the parent's side, `blocked-by` as `blocks`, `supersedes` as
/// `superseded-by`; `related` is symmetric.
fn adjacency(edges: &[GraphEdge]) -> BTreeMap<&str, Vec<(&'static str, &str)>> {
    let mut adj: BTreeMap<&str, Vec<(&'static str, &str)>> = BTreeMap::new();
    for edge in edges {
        let reverse = match edge.kind {
            "parent" => "child",
            "blocked-by" => "blocks",
            "supersedes" => "superseded-by",
            _ => "related",
        };
        adj.entry(&edge.from)
            .or_default()
            .push((edge.kind, &edge.to));
        adj.entry(&edge.to).or_default().push((reverse, &edge.from));
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
    // edges are read off scanned cards.
    let mut dangling: BTreeMap<&str, &'static str> = BTreeMap::new();
    for edge in edges {
        let target = edge.to.as_str();
        if !by_id.contains_key(target) && !dangling.contains_key(target) {
            let found = card::store::resolve_in(&paths.archive_cards_dir(), target)?.is_some();
            dangling.insert(target, if found { "[archived]" } else { "[missing]" });
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
        card.status,
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
            edge.kind
        ));
    }
    out.push_str("}\n");
    out
}

fn dot_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn query_matrix(paths: &MaestroPaths) -> Result<()> {
    let features = feature::list(paths)?;
    let current_commit = git::head(paths.repo_root()).unwrap_or(None);
    let entries = task::load_task_entries(&paths.tasks_dir())?;
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
    let sessions = proof::managed_event_files(paths)?.len();
    let mut events = 0_usize;
    let mut user_prompts = 0_usize;
    let mut corrections = 0_usize;
    let mut kinds = BTreeMap::<String, usize>::new();

    run::visit_managed_events(paths, |record| {
        let event = record.event();
        events += 1;
        let kind = event
            .event_type()
            .or_else(|| event.alias_kind())
            .unwrap_or("<unknown>")
            .to_string();
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

fn query_decisions(paths: &MaestroPaths) -> Result<()> {
    let entries = decisions::list(paths)?;
    if entries.is_empty() {
        println!("no decisions found");
        return Ok(());
    }

    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|entry| {
            vec![
                entry.id.clone(),
                entry.status.clone(),
                decision_home(&entry.source),
                entry.title.clone(),
            ]
        })
        .collect();
    print!(
        "{}",
        table::render_table(&["ID", "STATUS", "HOME", "TITLE"], &rows)
    );
    Ok(())
}

fn decision_home(source: &decisions::DecisionSource) -> String {
    match source {
        decisions::DecisionSource::Global => "global".to_string(),
        decisions::DecisionSource::Feature { feature_id } => format!("feature:{feature_id}"),
        decisions::DecisionSource::Legacy => "legacy-md".to_string(),
    }
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
