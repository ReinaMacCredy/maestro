use serde_json::{Map, Value};

use crate::core::error::MaestroError;

const JSON_MANAGED_KEYS: &str = "_maestro_managed_keys";

/// Supported text managed-block marker styles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedBlockFormat {
    /// Markdown comments used for `CLAUDE.md` and `AGENTS.md`.
    Markdown,
    /// Hash comments used for `.gitignore` and TOML config files.
    HashComment,
}

impl ManagedBlockFormat {
    fn markers(self) -> (&'static str, &'static str) {
        match self {
            Self::Markdown => ("<!-- maestro:start -->", "<!-- maestro:end -->"),
            Self::HashComment => ("# >>> maestro >>>", "# <<< maestro <<<"),
        }
    }
}

/// Insert or replace Maestro-owned text while preserving user-owned content.
pub fn upsert_managed_block(
    existing: Option<&str>,
    format: ManagedBlockFormat,
    body: &str,
) -> String {
    let existing = existing.unwrap_or("");
    let (start, end) = format.markers();
    let block = render_block(start, end, body);

    match find_block(existing, start, end) {
        Some((block_start, block_end)) => {
            let mut output = String::new();
            output.push_str(&existing[..block_start]);
            output.push_str(&block);
            output.push_str(&existing[block_end..]);
            output
        }
        None if existing.trim().is_empty() => block,
        None => {
            let mut output = existing.trim_end_matches('\n').to_string();
            output.push_str("\n\n");
            output.push_str(&block);
            output
        }
    }
}

/// Remove Maestro-owned text while preserving user-owned content.
pub fn remove_managed_block(existing: &str, format: ManagedBlockFormat) -> String {
    let (start, end) = format.markers();

    match find_block(existing, start, end) {
        Some((block_start, block_end)) => {
            join_after_block_removal(&existing[..block_start], &existing[block_end..])
        }
        None => existing.to_string(),
    }
}

/// Merge Maestro-owned JSON keys into an existing top-level JSON object.
pub fn upsert_managed_json_keys(
    existing: Option<&str>,
    managed_keys: Map<String, Value>,
) -> Result<String, MaestroError> {
    let mut object = parse_json_object(existing)?;

    let mut key_names = Vec::new();
    for (key, value) in managed_keys {
        object.insert(key.clone(), value);
        key_names.push(Value::String(key));
    }
    object.insert(JSON_MANAGED_KEYS.to_string(), Value::Array(key_names));

    Ok(format_json_object(object))
}

/// Remove all Maestro-owned top-level JSON keys from an existing JSON object.
pub fn remove_managed_json_keys(
    existing: &str,
    expected_managed_keys: &[&str],
) -> Result<String, MaestroError> {
    let mut object = parse_json_object(Some(existing))?;

    for key in previous_managed_keys(&object) {
        if expected_managed_keys
            .iter()
            .any(|expected_key| *expected_key == key)
        {
            object.remove(&key);
        }
    }
    object.remove(JSON_MANAGED_KEYS);

    Ok(format_json_object(object))
}

fn render_block(start: &str, end: &str, body: &str) -> String {
    let mut block = String::new();
    block.push_str(start);
    block.push('\n');
    block.push_str(body.trim_matches('\n'));
    block.push('\n');
    block.push_str(end);
    block.push('\n');
    block
}

fn find_block(existing: &str, start: &str, end: &str) -> Option<(usize, usize)> {
    let block_start = existing.find(start)?;
    let end_marker_start = existing[block_start..].find(end)? + block_start;
    let mut block_end = end_marker_start + end.len();

    if existing[block_end..].starts_with('\n') {
        block_end += 1;
    }

    Some((block_start, block_end))
}

fn join_after_block_removal(before: &str, after: &str) -> String {
    if before.is_empty() {
        return after.to_string();
    }
    if after.is_empty() {
        return before
            .strip_suffix("\n\n")
            .map(|trimmed| format!("{trimmed}\n"))
            .unwrap_or_else(|| before.to_string());
    }

    let before_without_boundary = before.trim_end_matches('\n');
    let after_without_boundary = after.trim_start_matches('\n');
    let before_newlines = before.len() - before_without_boundary.len();
    let after_newlines = after.len() - after_without_boundary.len();

    if before_newlines == 0 || after_newlines == 0 {
        let mut output = String::with_capacity(before.len() + after.len());
        output.push_str(before);
        output.push_str(after);
        return output;
    }

    let boundary_newlines = if before_newlines > 1 || after_newlines > 1 {
        "\n\n"
    } else {
        "\n"
    };
    let mut output = String::with_capacity(
        before_without_boundary.len() + boundary_newlines.len() + after_without_boundary.len(),
    );
    output.push_str(before_without_boundary);
    output.push_str(boundary_newlines);
    output.push_str(after_without_boundary);
    output
}

fn parse_json_object(existing: Option<&str>) -> Result<Map<String, Value>, MaestroError> {
    let Some(existing) = existing else {
        return Ok(Map::new());
    };

    if existing.trim().is_empty() {
        return Ok(Map::new());
    }

    match serde_json::from_str::<Value>(existing) {
        Ok(Value::Object(object)) => Ok(object),
        Ok(_) | Err(_) => Err(MaestroError::InvalidJsonMirror),
    }
}

fn previous_managed_keys(object: &Map<String, Value>) -> Vec<String> {
    object
        .get(JSON_MANAGED_KEYS)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn format_json_object(object: Map<String, Value>) -> String {
    let value = Value::Object(object);
    let mut formatted =
        serde_json::to_string_pretty(&value).expect("invariant: JSON object should serialize");
    formatted.push('\n');
    formatted
}
