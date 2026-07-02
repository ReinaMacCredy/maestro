use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::domain::card::schema::CardType;
use crate::domain::card::store;
use crate::domain::task::template::{TaskRecord, TaskState};
use crate::foundation::core::paths::MaestroPaths;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TaskPlanInput {
    pub tasks: Vec<TaskPlanItem>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TaskPlanItem {
    pub alias: Option<String>,
    pub title: String,
    pub covers: Vec<String>,
    pub checks: Vec<String>,
    pub lane: Option<String>,
    pub blocked_by: Vec<String>,
    pub gate: bool,
    pub gate_kind: Option<String>,
    pub order: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedPlanTask {
    pub id: String,
    pub title: String,
    pub covers: Vec<String>,
    pub checks: Vec<String>,
    pub lane: Option<String>,
    pub blocked_by: Vec<String>,
    pub gate: bool,
    pub gate_kind: Option<String>,
    pub order: usize,
}

#[derive(Debug, Deserialize)]
struct TaskPlanFile {
    #[serde(default, alias = "schema_version")]
    schema: Option<String>,
    #[serde(default)]
    tasks: Vec<TaskPlanFileItem>,
}

#[derive(Debug, Deserialize)]
struct TaskPlanFileItem {
    #[serde(default)]
    alias: Option<String>,
    title: String,
    #[serde(default)]
    lane: Option<String>,
    #[serde(default)]
    covers: Vec<String>,
    #[serde(default, alias = "check")]
    checks: Vec<String>,
    #[serde(default, alias = "after")]
    blocked_by: Vec<String>,
    #[serde(default)]
    gate: bool,
    #[serde(default)]
    gate_kind: Option<String>,
    #[serde(default)]
    order: Option<usize>,
}

pub fn parse_plan_file(contents: &str) -> Result<TaskPlanInput> {
    let file: TaskPlanFile =
        serde_yaml::from_str(contents).context("failed to parse task plan input")?;
    let schema = file.schema.as_deref().unwrap_or("maestro.task_plan.v1");
    if schema != "maestro.task_plan.v1" {
        bail!("unsupported task plan schema {schema}; expected maestro.task_plan.v1");
    }
    let tasks = file
        .tasks
        .into_iter()
        .map(|item| TaskPlanItem {
            alias: item.alias.and_then(nonempty_owned),
            title: item.title,
            covers: item.covers,
            checks: item.checks,
            lane: item.lane.and_then(nonempty_owned),
            blocked_by: item.blocked_by,
            gate: item.gate,
            gate_kind: item.gate_kind.and_then(nonempty_owned),
            order: item.order,
        })
        .collect();
    Ok(TaskPlanInput { tasks })
}

pub fn plan_from_cli(
    task_specs: &[String],
    lane_specs: &[String],
    after_specs: &[String],
    gate_specs: &[String],
) -> Result<TaskPlanInput> {
    let mut tasks = Vec::with_capacity(task_specs.len());
    let mut alias_index = BTreeMap::new();
    for (index, spec) in task_specs.iter().enumerate() {
        let (alias, title) = parse_task_spec(spec)?;
        if let Some(alias) = alias.as_deref()
            && alias_index.insert(alias.to_string(), index).is_some()
        {
            bail!("duplicate task alias {alias:?}");
        }
        tasks.push(TaskPlanItem {
            alias,
            title,
            covers: Vec::new(),
            checks: Vec::new(),
            order: Some(index),
            ..TaskPlanItem::default()
        });
    }

    for spec in lane_specs {
        let (alias, lane) = parse_required_key_value(spec, "--lane")?;
        let task = task_by_alias_mut(&mut tasks, &alias, "--lane")?;
        task.lane = Some(lane);
    }
    for spec in after_specs {
        let (alias, refs) = parse_required_key_value(spec, "--after")?;
        let task = task_by_alias_mut(&mut tasks, &alias, "--after")?;
        task.blocked_by = refs
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        if task.blocked_by.is_empty() {
            bail!("--after {alias}=... must name at least one dependency ref");
        }
    }
    for spec in gate_specs {
        let (alias, kind) = match spec.split_once('=') {
            Some((alias, kind)) => (clean_alias(alias, "--gate")?, nonempty(kind, "--gate")?),
            None => {
                let alias = clean_alias(spec, "--gate")?;
                (alias, "custom".to_string())
            }
        };
        let task = task_by_alias_mut(&mut tasks, &alias, "--gate")?;
        task.gate = true;
        task.gate_kind = Some(kind);
    }

    Ok(TaskPlanInput { tasks })
}

pub fn normalize_new_task_plan(
    paths: &MaestroPaths,
    input: TaskPlanInput,
    existing_tasks: &[TaskRecord],
) -> Result<Vec<NormalizedPlanTask>> {
    if input.tasks.is_empty() {
        bail!("task setup requires at least one --task or task plan item");
    }
    let mut aliases = BTreeMap::new();
    let mut generated = Vec::with_capacity(input.tasks.len());
    for (index, item) in input.tasks.iter().enumerate() {
        let title = nonempty(&item.title, "task title")?;
        if let Some(alias) = item.alias.as_deref()
            && aliases.insert(alias.to_string(), index).is_some()
        {
            bail!("duplicate task alias {alias:?}");
        }
        generated.push(store::mint_card_id(paths, CardType::Task, &title));
    }

    let existing: BTreeMap<&str, &TaskRecord> = existing_tasks
        .iter()
        .map(|task| (task.id.as_str(), task))
        .collect();
    let generated_ids: BTreeSet<&str> = generated.iter().map(String::as_str).collect();
    let mut rows = Vec::with_capacity(input.tasks.len());
    for (index, item) in input.tasks.into_iter().enumerate() {
        let id = generated[index].clone();
        let mut blocked_by = Vec::with_capacity(item.blocked_by.len());
        for reference in item.blocked_by {
            let reference = nonempty(&reference, "dependency ref")?;
            let resolved = if let Some(dep_index) = aliases.get(&reference) {
                generated[*dep_index].clone()
            } else if let Some(task) = existing.get(reference.as_str()) {
                validate_existing_dependency(task)?;
                task.id.clone()
            } else {
                bail!("unknown task dependency ref {reference:?}");
            };
            if resolved == id {
                bail!(
                    "task {} cannot block on itself",
                    display_name(item.alias.as_deref(), &id)
                );
            }
            blocked_by.push(resolved);
        }
        blocked_by.sort();
        blocked_by.dedup();
        rows.push(NormalizedPlanTask {
            id,
            title: nonempty(&item.title, "task title")?,
            covers: item.covers,
            checks: item.checks,
            lane: item.lane.and_then(nonempty_owned),
            blocked_by,
            gate: item.gate,
            gate_kind: item.gate_kind.and_then(nonempty_owned),
            order: item.order.unwrap_or(index),
        });
    }
    reject_cycles(&rows, &generated_ids)?;
    Ok(rows)
}

fn validate_existing_dependency(task: &TaskRecord) -> Result<()> {
    match task.state {
        TaskState::Rejected | TaskState::Abandoned | TaskState::Superseded => bail!(
            "task dependency {} is terminal {}; use a verified task or live dependency",
            task.id,
            task.state.as_str()
        ),
        _ => Ok(()),
    }
}

fn reject_cycles(rows: &[NormalizedPlanTask], generated_ids: &BTreeSet<&str>) -> Result<()> {
    let edges: BTreeMap<&str, Vec<&str>> = rows
        .iter()
        .map(|row| {
            (
                row.id.as_str(),
                row.blocked_by
                    .iter()
                    .map(String::as_str)
                    .filter(|id| generated_ids.contains(id))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack = Vec::new();
    for row in rows {
        visit(
            row.id.as_str(),
            &edges,
            &mut visiting,
            &mut visited,
            &mut stack,
        )?;
    }
    Ok(())
}

fn visit<'a>(
    id: &'a str,
    edges: &BTreeMap<&'a str, Vec<&'a str>>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
    stack: &mut Vec<&'a str>,
) -> Result<()> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id) {
        stack.push(id);
        bail!("task dependency cycle detected: {}", stack.join(" -> "));
    }
    stack.push(id);
    for dependency in edges.get(id).into_iter().flatten() {
        visit(dependency, edges, visiting, visited, stack)?;
    }
    stack.pop();
    visiting.remove(id);
    visited.insert(id);
    Ok(())
}

fn task_by_alias_mut<'a>(
    tasks: &'a mut [TaskPlanItem],
    alias: &str,
    flag: &str,
) -> Result<&'a mut TaskPlanItem> {
    let mut matches = tasks
        .iter_mut()
        .filter(|task| task.alias.as_deref() == Some(alias));
    let Some(task) = matches.next() else {
        bail!("{flag} references unknown task alias {alias:?}");
    };
    if matches.next().is_some() {
        bail!("{flag} references ambiguous task alias {alias:?}");
    }
    Ok(task)
}

fn parse_task_spec(spec: &str) -> Result<(Option<String>, String)> {
    if let Some((alias, title)) = spec.split_once('=') {
        let alias = clean_alias(alias, "--task")?;
        let title = nonempty(title, "task title")?;
        Ok((Some(alias), title))
    } else {
        Ok((None, nonempty(spec, "task title")?))
    }
}

fn parse_required_key_value(spec: &str, flag: &str) -> Result<(String, String)> {
    let Some((key, value)) = spec.split_once('=') else {
        bail!("{flag} expects alias=value");
    };
    Ok((clean_alias(key, flag)?, nonempty(value, flag)?))
}

fn clean_alias(value: &str, flag: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{flag} alias must not be empty");
    }
    Ok(value.to_string())
}

fn nonempty_owned(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn nonempty(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} must not be empty");
    }
    Ok(value.to_string())
}

fn display_name(alias: Option<&str>, id: &str) -> String {
    alias
        .map(|alias| format!("alias {alias:?}"))
        .unwrap_or_else(|| id.to_string())
}

pub fn read_plan_file(path: &Path) -> Result<String> {
    if path == Path::new("-") {
        let mut contents = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut contents)
            .context("failed to read task plan from stdin")?;
        return Ok(contents);
    }
    std::fs::read_to_string(path)
        .with_context(|| format!("failed to read task plan {}", path.display()))
}
