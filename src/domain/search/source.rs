use std::collections::BTreeMap;
use std::path::{Component, Path};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, bail};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::domain::search::memory;
use crate::domain::search::query::{self, ParsedQuery, QueryAtom, QueryExpr};
use crate::domain::search::types::{
    DiagnosticSeverity, GrepEnvelope, MatchSpan, ScoreReason, SearchCorpus, SearchDiagnostic,
    SearchHit,
};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_atomic;

const SOURCE_SHARD_SCHEMA_VERSION: &str = "source-shard.v1";
const SOURCE_SHARD_MAGIC: &[u8] = b"MAESTRO_SOURCE_SHARD_V1\n";
const MAX_SOURCE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_SOURCE_LINES: usize = 80_000;

#[derive(Debug, Serialize)]
pub struct SourceRebuildReport {
    pub indexed_files: usize,
    pub skipped_files: usize,
    pub skipped_by_reason: BTreeMap<String, usize>,
    pub representative_skips: Vec<SourceSkip>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceSkip {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct SourceShard {
    schema_version: String,
    manifest: Vec<ManifestEntry>,
    files: Vec<SourceFile>,
    postings: BTreeMap<String, Vec<SourcePosting>>,
    skipped: Vec<SourceSkip>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SourceFile {
    path: String,
    language: String,
    contents: String,
    line_offsets: Vec<LineOffset>,
    trigram_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct LineOffset {
    line: u64,
    byte_start: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct SourcePosting {
    path: String,
    line: u64,
    byte_start: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
struct ManifestEntry {
    path: String,
    mtime_ns: u64,
    len: u64,
}

#[derive(Clone, Debug)]
struct SourceMatch {
    byte_start: usize,
    byte_end: usize,
    line: u64,
    snippet: String,
    factor: &'static str,
}

pub fn grep(paths: &MaestroPaths, raw_query: &str) -> GrepEnvelope {
    let parsed = match query::parse(raw_query) {
        Ok(parsed) => parsed,
        Err(diagnostic) => return GrepEnvelope::error(raw_query, diagnostic),
    };

    if should_use_source(&parsed) {
        if parsed.filters.corpus.as_deref() == Some("memory") && has_source_filter(&parsed) {
            return GrepEnvelope::error(
                raw_query,
                SearchDiagnostic::error(
                    "invalid_filter",
                    "source filters cannot be combined with corpus:memory",
                ),
            );
        }
        grep_source_parsed(paths, raw_query, &parsed)
    } else {
        memory::grep_memory(paths, raw_query)
    }
}

fn should_use_source(parsed: &ParsedQuery) -> bool {
    parsed.filters.corpus.as_deref() == Some("source")
        || has_source_filter(parsed)
        || !parsed.regexes.is_empty()
}

fn has_source_filter(parsed: &ParsedQuery) -> bool {
    !parsed.filters.file_globs.is_empty()
        || !parsed.filters.excluded_file_globs.is_empty()
        || parsed.filters.lang.is_some()
        || parsed.filters.sym.is_some()
}

pub fn grep_source(paths: &MaestroPaths, raw_query: &str) -> GrepEnvelope {
    let parsed = match query::parse(raw_query) {
        Ok(parsed) => parsed,
        Err(diagnostic) => return GrepEnvelope::error(raw_query, diagnostic),
    };
    grep_source_parsed(paths, raw_query, &parsed)
}

pub fn rebuild_source(paths: &MaestroPaths) -> Result<SourceRebuildReport> {
    let mut skipped = Vec::new();
    let candidates = source_manifest(paths, &mut skipped)?;
    let mut files = Vec::new();
    let mut postings: BTreeMap<String, Vec<SourcePosting>> = BTreeMap::new();

    for entry in &candidates {
        let path = paths.repo_root().join(&entry.path);
        let bytes = std::fs::read(&path)
            .with_context(|| format!("failed to read source candidate {}", entry.path))?;
        let contents = match String::from_utf8(bytes) {
            Ok(contents) => contents,
            Err(_) => {
                skipped.push(SourceSkip {
                    path: entry.path.clone(),
                    reason: "binary".to_string(),
                });
                continue;
            }
        };
        let line_count = contents
            .lines()
            .count()
            .max(usize::from(!contents.is_empty()));
        if line_count > MAX_SOURCE_LINES {
            skipped.push(SourceSkip {
                path: entry.path.clone(),
                reason: "line_cap".to_string(),
            });
            continue;
        }
        let line_offsets = line_offsets(&contents);
        let mut trigram_count = 0;
        for (trigram, byte_start) in trigrams(&contents) {
            trigram_count += 1;
            postings.entry(trigram).or_default().push(SourcePosting {
                path: entry.path.clone(),
                line: line_for_byte(&line_offsets, byte_start),
                byte_start,
            });
        }
        files.push(SourceFile {
            path: entry.path.clone(),
            language: language_for_path(&entry.path).to_string(),
            contents,
            line_offsets,
            trigram_count,
        });
    }

    let report = report(files.len(), &skipped);
    let shard = SourceShard {
        schema_version: SOURCE_SHARD_SCHEMA_VERSION.to_string(),
        manifest: candidates,
        files,
        postings,
        skipped,
    };
    ensure_dir(paths.search_index_dir())?;
    write_atomic(paths.source_shard_file(), &encode_shard(&shard)?)?;
    Ok(report)
}

fn grep_source_parsed(paths: &MaestroPaths, raw_query: &str, parsed: &ParsedQuery) -> GrepEnvelope {
    if let Some(invalid) = parsed
        .filters
        .kinds
        .iter()
        .find(|kind| kind.as_str() != "file")
    {
        return GrepEnvelope::error(
            raw_query,
            SearchDiagnostic::error(
                "invalid_type",
                format!("type:{invalid} is not a source result kind in this slice"),
            ),
        );
    }
    if parsed.filters.sym.is_some() {
        return GrepEnvelope::error(
            raw_query,
            SearchDiagnostic::error(
                "symbol_index_unavailable",
                "sym: ships with the outline/symbols slice",
            ),
        );
    }

    let shard = match load_fresh(paths) {
        Ok(shard) => shard,
        Err(_) => match rebuild_source(paths).and_then(|_| load_fresh(paths)) {
            Ok(shard) => shard,
            Err(error) => {
                return GrepEnvelope::error(
                    raw_query,
                    SearchDiagnostic {
                        severity: DiagnosticSeverity::Error,
                        code: "source_shard_unavailable".to_string(),
                        message: format!("source shard unavailable: {error}"),
                        corpus: Some(SearchCorpus::Source),
                        path: Some(".maestro/index/search/source.shard".to_string()),
                        retryable: Some(true),
                    },
                );
            }
        },
    };

    let hits = match search_shard(&shard, parsed) {
        Ok(hits) => hits,
        Err(diagnostic) => return GrepEnvelope::error(raw_query, diagnostic),
    };
    GrepEnvelope::success(raw_query, hits, parsed.explicit_filter_overrides.clone())
}

fn load_fresh(paths: &MaestroPaths) -> Result<SourceShard> {
    let bytes = std::fs::read(paths.source_shard_file()).context("failed to read source shard")?;
    let shard = decode_shard(&bytes)?;
    if shard.schema_version != SOURCE_SHARD_SCHEMA_VERSION {
        bail!("source shard schema is stale");
    }
    let mut skipped = Vec::new();
    if shard.manifest != source_manifest(paths, &mut skipped)? {
        bail!("source shard manifest is stale");
    }
    Ok(shard)
}

fn search_shard(
    shard: &SourceShard,
    parsed: &ParsedQuery,
) -> Result<Vec<SearchHit>, SearchDiagnostic> {
    let case_sensitive = query::literal_case_sensitive(parsed);
    let mut hits = Vec::new();
    for file in &shard.files {
        if !source_filters_match(file, parsed)? {
            continue;
        }
        if !evaluate_expr(&parsed.expr, &file.contents, case_sensitive)? {
            continue;
        }
        let Some(first_match) = first_positive_match(file, parsed, case_sensitive)? else {
            continue;
        };
        let score = source_score(file, &first_match, parsed);
        hits.push(SearchHit {
            rank: 0,
            corpus: SearchCorpus::Source,
            kind: "file".to_string(),
            id: file.path.clone(),
            path: Some(file.path.clone()),
            line: Some(first_match.line),
            title: file.path.clone(),
            snippet: first_match.snippet,
            score,
            score_reasons: vec![
                ScoreReason {
                    factor: first_match.factor.to_string(),
                    value: 1.0,
                    detail: "confirmed against stored source contents".to_string(),
                },
                ScoreReason {
                    factor: "trigram_shard".to_string(),
                    value: file.trigram_count as f64,
                    detail: "source shard stores positional trigram postings".to_string(),
                },
            ],
            opener: Some(format!("{}:{}", file.path, first_match.line)),
            archived: false,
            feature: None,
            parent: None,
            symbol_kind: None,
            match_spans: vec![MatchSpan::Source {
                line: first_match.line,
                byte_start: first_match.byte_start,
                byte_end: first_match.byte_end,
            }],
        });
    }
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
    });
    for (idx, hit) in hits.iter_mut().enumerate() {
        hit.rank = idx + 1;
    }
    Ok(hits)
}

fn source_filters_match(file: &SourceFile, parsed: &ParsedQuery) -> Result<bool, SearchDiagnostic> {
    if let Some(lang) = &parsed.filters.lang
        && lang != &file.language
    {
        return Ok(false);
    }
    if !parsed.filters.file_globs.is_empty()
        && !parsed
            .filters
            .file_globs
            .iter()
            .any(|pattern| glob_matches(pattern, &file.path))
    {
        return Ok(false);
    }
    if parsed
        .filters
        .excluded_file_globs
        .iter()
        .any(|pattern| glob_matches(pattern, &file.path))
    {
        return Ok(false);
    }
    Ok(true)
}

fn evaluate_expr(
    expr: &QueryExpr,
    contents: &str,
    case_sensitive: bool,
) -> Result<bool, SearchDiagnostic> {
    match expr {
        QueryExpr::Atom(QueryAtom::Literal(term)) => {
            Ok(find_literal(contents, term, case_sensitive).is_some())
        }
        QueryExpr::Atom(QueryAtom::Regex(pattern)) => {
            Ok(regex_for(pattern, case_sensitive)?.find(contents).is_some())
        }
        QueryExpr::Not(inner) => Ok(!evaluate_expr(inner, contents, case_sensitive)?),
        QueryExpr::And(items) => {
            for item in items {
                if !evaluate_expr(item, contents, case_sensitive)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        QueryExpr::Or(items) => {
            for item in items {
                if evaluate_expr(item, contents, case_sensitive)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

fn first_positive_match(
    file: &SourceFile,
    parsed: &ParsedQuery,
    case_sensitive: bool,
) -> Result<Option<SourceMatch>, SearchDiagnostic> {
    for pattern in &parsed.regexes {
        let regex = regex_for(pattern, case_sensitive)?;
        if let Some(mat) = regex.find(&file.contents) {
            return Ok(Some(source_match(file, mat.start(), mat.end(), "regex")));
        }
    }
    for term in &parsed.terms {
        if let Some((start, end)) = find_literal(&file.contents, term, case_sensitive) {
            return Ok(Some(source_match(file, start, end, "lexical")));
        }
    }
    Ok(None)
}

fn source_match(
    file: &SourceFile,
    byte_start: usize,
    byte_end: usize,
    factor: &'static str,
) -> SourceMatch {
    let line = line_for_byte(&file.line_offsets, byte_start);
    SourceMatch {
        byte_start,
        byte_end,
        line,
        snippet: line_snippet(&file.contents, byte_start),
        factor,
    }
}

fn source_score(file: &SourceFile, first_match: &SourceMatch, parsed: &ParsedQuery) -> f64 {
    let mut score: f64 = 0.55;
    if parsed
        .filters
        .file_globs
        .iter()
        .any(|pattern| glob_matches(pattern, &file.path))
    {
        score += 0.15;
    }
    if parsed
        .filters
        .lang
        .as_ref()
        .is_some_and(|lang| lang == &file.language)
    {
        score += 0.15;
    }
    if first_match.factor == "regex" {
        score += 0.10;
    }
    score.min(1.0)
}

fn regex_for(pattern: &str, case_sensitive: bool) -> Result<Regex, SearchDiagnostic> {
    RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|error| SearchDiagnostic::error("parse_error", format!("invalid regex: {error}")))
}

fn find_literal(text: &str, needle: &str, case_sensitive: bool) -> Option<(usize, usize)> {
    if case_sensitive {
        return text.find(needle).map(|start| (start, start + needle.len()));
    }
    let needle_chars = lowercase_chars(needle);
    if needle_chars.is_empty() {
        return None;
    }
    let chars: Vec<(usize, String)> = text
        .char_indices()
        .map(|(idx, ch)| (idx, ch.to_lowercase().collect::<String>()))
        .collect();
    for start_idx in 0..chars.len() {
        if chars[start_idx..]
            .iter()
            .zip(needle_chars.iter())
            .all(|((_, hay), needle)| hay == needle)
        {
            let byte_start = chars[start_idx].0;
            let end_char_idx = start_idx + needle_chars.len();
            let byte_end = chars.get(end_char_idx).map_or(text.len(), |(idx, _)| *idx);
            return Some((byte_start, byte_end));
        }
    }
    None
}

fn lowercase_chars(text: &str) -> Vec<String> {
    text.chars()
        .map(|ch| ch.to_lowercase().collect::<String>())
        .collect()
}

fn line_snippet(contents: &str, byte_start: usize) -> String {
    let line_start = contents[..byte_start].rfind('\n').map_or(0, |idx| idx + 1);
    let line_end = contents[byte_start..]
        .find('\n')
        .map_or(contents.len(), |idx| byte_start + idx);
    contents[line_start..line_end].trim().to_string()
}

fn line_offsets(contents: &str) -> Vec<LineOffset> {
    let mut offsets = vec![LineOffset {
        line: 1,
        byte_start: 0,
    }];
    for (idx, ch) in contents.char_indices() {
        if ch == '\n' && idx + 1 < contents.len() {
            offsets.push(LineOffset {
                line: offsets.len() as u64 + 1,
                byte_start: idx + 1,
            });
        }
    }
    offsets
}

fn line_for_byte(offsets: &[LineOffset], byte: usize) -> u64 {
    let mut line = 1;
    for offset in offsets {
        if offset.byte_start > byte {
            break;
        }
        line = offset.line;
    }
    line
}

fn trigrams(contents: &str) -> Vec<(String, usize)> {
    let chars: Vec<(usize, char)> = contents.char_indices().collect();
    let mut result = Vec::new();
    for window in chars.windows(3) {
        let trigram = window
            .iter()
            .flat_map(|(_, ch)| ch.to_lowercase())
            .collect::<String>();
        result.push((trigram, window[0].0));
    }
    result
}

fn encode_shard(shard: &SourceShard) -> Result<Vec<u8>> {
    let mut bytes = SOURCE_SHARD_MAGIC.to_vec();
    let payload = serde_json::to_vec(shard).context("failed to serialize source shard")?;
    bytes.extend(payload);
    Ok(bytes)
}

fn decode_shard(bytes: &[u8]) -> Result<SourceShard> {
    let Some(payload) = bytes.strip_prefix(SOURCE_SHARD_MAGIC) else {
        bail!("source shard magic header is missing");
    };
    serde_json::from_slice(payload).context("failed to parse source shard")
}

fn source_manifest(
    paths: &MaestroPaths,
    skipped: &mut Vec<SourceSkip>,
) -> Result<Vec<ManifestEntry>> {
    let repo = git2::Repository::discover(paths.repo_root()).ok();
    let mut entries = Vec::new();
    collect_source_files(
        paths,
        paths.repo_root(),
        repo.as_ref(),
        skipped,
        &mut entries,
    )?;
    entries.sort();
    Ok(entries)
}

fn collect_source_files(
    paths: &MaestroPaths,
    dir: &Path,
    repo: Option<&git2::Repository>,
    skipped: &mut Vec<SourceSkip>,
    entries: &mut Vec<ManifestEntry>,
) -> Result<()> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in read_dir {
        let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("failed to stat {}", path.display()))?;
        let rel = relative_label(&path, paths.repo_root());
        if metadata.is_dir() {
            if is_system_dir(&path) {
                continue;
            }
            if is_vendor_path(&path) {
                skipped.push(SourceSkip {
                    path: rel,
                    reason: "vendor".to_string(),
                });
                continue;
            }
            if is_generated_path(&path) {
                skipped.push(SourceSkip {
                    path: rel,
                    reason: "generated".to_string(),
                });
                continue;
            }
            collect_source_files(paths, &path, repo, skipped, entries)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        if repo
            .and_then(|repo| repo.status_should_ignore(Path::new(&rel)).ok())
            .unwrap_or(false)
        {
            skipped.push(SourceSkip {
                path: rel,
                reason: "gitignored".to_string(),
            });
            continue;
        }
        if is_vendor_path(&path) {
            skipped.push(SourceSkip {
                path: rel,
                reason: "vendor".to_string(),
            });
            continue;
        }
        if is_generated_path(&path) {
            skipped.push(SourceSkip {
                path: rel,
                reason: "generated".to_string(),
            });
            continue;
        }
        if metadata.len() > MAX_SOURCE_BYTES {
            skipped.push(SourceSkip {
                path: rel,
                reason: "oversized".to_string(),
            });
            continue;
        }
        let bytes = std::fs::read(&path)
            .with_context(|| format!("failed to inspect source candidate {}", path.display()))?;
        if bytes.contains(&0) {
            skipped.push(SourceSkip {
                path: rel,
                reason: "binary".to_string(),
            });
            continue;
        }
        if std::str::from_utf8(&bytes).is_err() {
            skipped.push(SourceSkip {
                path: rel,
                reason: "binary".to_string(),
            });
            continue;
        }
        entries.push(ManifestEntry {
            path: rel,
            mtime_ns: modified_ns(&metadata),
            len: metadata.len(),
        });
    }
    Ok(())
}

fn is_system_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | ".maestro" | "target" | "node_modules" | ".direnv" | ".DS_Store"
            )
        })
}

fn is_vendor_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            Component::Normal(name)
                if matches!(
                    name.to_str(),
                    Some("vendor" | "third_party" | "node_modules" | "bower_components")
                )
        )
    })
}

fn is_generated_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            Component::Normal(name)
                if name
                    .to_str()
                    .is_some_and(|part| matches!(part, "generated" | "dist" | "build"))
        )
    }) || path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.contains(".generated.")
                || name.ends_with(".min.js")
                || name.ends_with(".bundle.js")
                || name.ends_with(".pb.rs")
        })
}

fn language_for_path(path: &str) -> &'static str {
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("rs") => "rust",
        Some("ts" | "tsx") => "typescript",
        Some("js" | "jsx" | "mjs" | "cjs") => "javascript",
        Some("py") => "python",
        Some("md" | "mdx") => "markdown",
        Some("toml") => "toml",
        Some("yaml" | "yml") => "yaml",
        Some("json") => "json",
        Some("sh" | "bash" | "zsh") => "shell",
        _ => "text",
    }
}

fn report(indexed_files: usize, skipped: &[SourceSkip]) -> SourceRebuildReport {
    let mut skipped_by_reason = BTreeMap::new();
    for skip in skipped {
        *skipped_by_reason.entry(skip.reason.clone()).or_insert(0) += 1;
    }
    SourceRebuildReport {
        indexed_files,
        skipped_files: skipped.len(),
        skipped_by_reason,
        representative_skips: skipped.iter().take(8).cloned().collect(),
    }
}

fn glob_matches(pattern: &str, path: &str) -> bool {
    if pattern == path {
        return true;
    }
    if !pattern.contains(['*', '?', '[', ']']) {
        let prefix = pattern.trim_end_matches('/');
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }
    let regex = glob_regex(pattern);
    Regex::new(&regex).is_ok_and(|regex| regex.is_match(path))
}

fn glob_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut idx = 0;
    while idx < chars.len() {
        match chars[idx] {
            '*' if chars.get(idx + 1) == Some(&'*') => {
                regex.push_str(".*");
                idx += 2;
            }
            '*' => {
                regex.push_str("[^/]*");
                idx += 1;
            }
            '?' => {
                regex.push_str("[^/]");
                idx += 1;
            }
            ch => {
                regex.push_str(&regex::escape(&ch.to_string()));
                idx += 1;
            }
        }
    }
    regex.push('$');
    regex
}

fn modified_ns(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default()
}

fn relative_label(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_search_returns_original_byte_offsets() {
        let text = "fn café_server() {}\n";
        let found = find_literal(text, "CAFÉ", false).expect("unicode case-insensitive match");
        assert_eq!(&text[found.0..found.1], "café");
        assert_eq!(found.0, 3);
    }

    #[test]
    fn glob_supports_single_and_multi_segment_wildcards() {
        assert!(glob_matches("src/*.rs", "src/lib.rs"));
        assert!(!glob_matches("src/*.rs", "src/bin/main.rs"));
        assert!(glob_matches("src/**/*.rs", "src/bin/main.rs"));
    }
}
