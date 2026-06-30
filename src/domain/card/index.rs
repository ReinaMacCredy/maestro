//! The local text index behind `list --grep` (SPEC-archive-memory-2 R6).
//!
//! A zoekt-inspired trigram index over every live and archived card's grep
//! surfaces (title, body, dir-backed sidecars), kept as one JSON file under
//! `.maestro/index/`. It is a pure accelerator: [`candidates`] returns a
//! superset of the true matches (callers still confirm with the real grep), or
//! `None` whenever the index cannot answer -- missing, stale, unreadable, or a
//! term too short to trigram -- so the scan-grep fallback stays the source of
//! truth and the index can never change results.
//!
//! Freshness is passive (no daemon): a file manifest of both card trees is
//! stored alongside the postings, and an indexed read that finds it stale
//! rebuilds in place -- the rebuild costs about as much as the scan-grep it
//! replaces. `maestro index rebuild` recovers explicitly from anything else.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::card::query::{GREP_SIDECARS, body_of, scan_with_paths};
use crate::domain::card::schema::Card;
use crate::domain::card::store::is_dir_backed;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;

const TEXT_INDEX_SCHEMA_VERSION: &str = "maestro.text-index.v1";

#[derive(Debug, Deserialize, Serialize)]
struct TextIndex {
    schema_version: String,
    /// Every file under both card trees as (relative path, mtime ns, len),
    /// sorted; compared verbatim at read time, so any write, move, or hand
    /// edit -- through maestro or not -- reads as stale.
    manifest: Vec<ManifestEntry>,
    docs: Vec<DocEntry>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct ManifestEntry {
    path: String,
    mtime_ns: u64,
    len: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct DocEntry {
    id: String,
    archived: bool,
    /// Sorted, deduped trigrams of the card's lowercased grep surfaces.
    trigrams: Vec<String>,
}

/// What a rebuild wrote, for the `index rebuild` receipt.
#[derive(Debug)]
pub struct RebuildReport {
    pub live_docs: usize,
    pub archived_docs: usize,
}

/// Rebuild the index from scratch over the live and archive card trees.
pub fn rebuild(paths: &MaestroPaths) -> Result<RebuildReport> {
    let live = scan_with_paths(paths)?;
    let archived = crate::domain::card::query::scan_archived_with_paths(paths)?;

    let mut docs = Vec::with_capacity(live.len() + archived.len());
    for (card, path) in &live {
        docs.push(doc_entry(card, path, false));
    }
    for (card, path) in &archived {
        docs.push(doc_entry(card, path, true));
    }

    let index = TextIndex {
        schema_version: TEXT_INDEX_SCHEMA_VERSION.to_string(),
        manifest: manifest(paths)?,
        docs,
    };
    let contents = serde_json::to_string(&index).context("failed to serialize the text index")?;
    let file = paths.text_index_file();
    ensure_dir(paths.index_dir())?;
    write_string_atomic(&file, &contents)?;
    Ok(RebuildReport {
        live_docs: live.len(),
        archived_docs: archived.len(),
    })
}

/// The ids whose indexed text contains every trigram of `term` -- a strict
/// superset of the cards `grep_matches` would accept, live and archived alike
/// (the caller's live/archive scoping happens by which card lists it filters).
/// `None` means "index cannot answer, run the scan-grep": the term is shorter
/// than a trigram, or the index is missing/stale/unreadable and the in-place
/// rebuild could not bring it current (read-only stores stay on the fallback).
pub fn candidates(paths: &MaestroPaths, term: &str) -> Option<BTreeSet<String>> {
    let needle = trigrams(&term.to_lowercase());
    if needle.is_empty() {
        return None;
    }
    let index = match load_fresh(paths) {
        Some(index) => index,
        // Self-heal in-verb: the rebuild costs about one scan, which is what
        // the fallback grep would spend anyway. Any failure stays silent.
        None => {
            rebuild(paths).ok()?;
            load_fresh(paths)?
        }
    };
    Some(
        index
            .docs
            .into_iter()
            .filter(|doc| {
                needle
                    .iter()
                    .all(|tri| doc.trigrams.binary_search(tri).is_ok())
            })
            .map(|doc| doc.id)
            .collect(),
    )
}

/// Load the index iff it parses, carries the current schema, and its manifest
/// still matches both card trees byte-for-byte.
fn load_fresh(paths: &MaestroPaths) -> Option<TextIndex> {
    let contents = std::fs::read_to_string(paths.text_index_file()).ok()?;
    let index: TextIndex = serde_json::from_str(&contents).ok()?;
    if index.schema_version != TEXT_INDEX_SCHEMA_VERSION {
        return None;
    }
    if index.manifest != manifest(paths).ok()? {
        return None;
    }
    Some(index)
}

fn doc_entry(card: &Card, path: &Path, archived: bool) -> DocEntry {
    let mut text = card.title.clone();
    if let Some(body) = body_of(card) {
        text.push('\n');
        text.push_str(&body);
    }
    if is_dir_backed(path)
        && let Some(dir) = path.parent()
    {
        for sidecar in GREP_SIDECARS {
            if let Ok(prose) = std::fs::read_to_string(dir.join(sidecar)) {
                text.push('\n');
                text.push_str(&prose);
            }
        }
    }
    DocEntry {
        id: card.id.clone(),
        archived,
        trigrams: trigrams(&text.to_lowercase()).into_iter().collect(),
    }
}

/// All char-level trigrams of an (already lowercased) string. Any substring of
/// the string of length >= 3 has all of its trigrams in this set, which is the
/// superset guarantee `candidates` relies on.
fn trigrams(text: &str) -> BTreeSet<String> {
    let chars: Vec<char> = text.chars().collect();
    chars
        .windows(3)
        .map(|window| window.iter().collect())
        .collect()
}

/// Every file under both card trees as sorted (relative path, mtime, len).
fn manifest(paths: &MaestroPaths) -> Result<Vec<ManifestEntry>> {
    let mut entries = Vec::new();
    collect_files(&paths.cards_dir(), &paths.maestro_dir(), &mut entries)?;
    collect_one_file(
        &crate::domain::card::archive_db::archive_db_file(paths),
        &paths.maestro_dir(),
        &mut entries,
    )?;
    entries.sort();
    Ok(entries)
}

fn collect_files(dir: &Path, base: &Path, entries: &mut Vec<ManifestEntry>) -> Result<()> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        // An absent tree (no archive yet) is an empty contribution, not an error.
        return Ok(());
    };
    for entry in read_dir {
        let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("failed to stat {}", path.display()))?;
        if metadata.is_dir() {
            collect_files(&path, base, entries)?;
        } else if metadata.is_file() {
            let mtime_ns = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_nanos() as u64)
                .unwrap_or_default();
            entries.push(ManifestEntry {
                path: relative_label(&path, base),
                mtime_ns,
                len: metadata.len(),
            });
        }
        // Symlinks are neither walked nor listed: the card store refuses them.
    }
    Ok(())
}

fn collect_one_file(path: &Path, base: &Path, entries: &mut Vec<ManifestEntry>) -> Result<()> {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return Ok(());
    };
    if !metadata.is_file() {
        return Ok(());
    }
    let mtime_ns = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or_default();
    entries.push(ManifestEntry {
        path: relative_label(path, base),
        mtime_ns,
        len: metadata.len(),
    });
    Ok(())
}

fn relative_label(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigrams_cover_every_substring_window() {
        let set = trigrams("csv export");
        assert!(set.contains("csv"));
        assert!(set.contains("v e"));
        assert!(set.contains("ort"));
        assert!(!set.contains("CSV"), "input is expected pre-lowercased");
        assert!(trigrams("ab").is_empty(), "shorter than one trigram");
    }

    /// The superset guarantee: every trigram of a contained substring is in
    /// the text's trigram set, so trigram-AND can only over-approximate.
    #[test]
    fn needle_trigrams_are_a_subset_of_matching_text_trigrams() {
        let text = trigrams("the streaming writer keeps a header row");
        for needle in ["streaming", "header row", "er k"] {
            assert!(
                trigrams(needle).is_subset(&text),
                "{needle} trigrams must all appear"
            );
        }
    }
}
