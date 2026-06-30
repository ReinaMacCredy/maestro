//! Lean debt: harvest inline `// lean:` markers from the tree into a ledger and
//! mint deduped task cards from them.

use anyhow::Result;

use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store as card_store;
use crate::foundation::core::git;
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

/// The comment leaders that anchor a marker; a `lean:` token not immediately
/// preceded by one of these (after optional spaces) is not a marker.
const COMMENT_LEADERS: [&str; 3] = ["//", "#", "--"];

/// The marker token; the text after it is the debt note.
const TOKEN: &str = "lean:";

/// One harvested lean-debt marker: where it is and what it says.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Marker {
    /// Repo-relative path of the file the marker is in.
    pub file: String,
    /// 1-based line number of the marker.
    pub line: usize,
    /// The marker text after the `lean:` token, trimmed.
    pub text: String,
}

impl Marker {
    /// The ceiling: the note text before the first comma (the whole note when it
    /// has none). A lean marker reads `lean: <ceiling>, <upgrade trigger>`, where
    /// the ceiling is the simpler choice taken and the trigger says when to climb.
    pub fn ceiling(&self) -> &str {
        match self.text.split_once(',') {
            Some((ceiling, _)) => ceiling.trim(),
            None => self.text.trim(),
        }
    }

    /// The upgrade trigger: the note text after the first comma, if any. `None`
    /// when the marker names no trigger -- debt flags those as `no-trigger`.
    pub fn upgrade(&self) -> Option<&str> {
        self.text
            .split_once(',')
            .map(|(_, trigger)| trigger.trim())
            .filter(|trigger| !trigger.is_empty())
    }
}

/// The outcome of minting cards from markers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MintOutcome {
    /// Cards minted this run, each `(card_id, marker)`.
    pub minted: Vec<(String, Marker)>,
    /// Markers skipped because a card for the same file+line+text already exists.
    pub deduped: usize,
}

/// The marker text on `line` if it carries an anchored `lean:` debt marker (a
/// `//`, `#`, or `--` comment leader immediately before `lean:`), else `None`.
/// A bare `clean:` or `boolean:` is not a marker: the char before `lean` is a
/// word char, so the trimmed prefix does not end in a comment leader. A trailing
/// marker after code (`x = 1; // lean: ...`) is caught. An empty note is not a
/// marker.
pub fn marker_text(line: &str) -> Option<String> {
    for (idx, _) in line.match_indices(TOKEN) {
        let before = line[..idx].trim_end();
        if COMMENT_LEADERS
            .iter()
            .any(|leader| before.ends_with(leader))
        {
            let text = line[idx + TOKEN.len()..].trim();
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

/// Every marker in one file's `content`, tagged with `file` and a 1-based line.
fn markers_in(file: &str, content: &str) -> Vec<Marker> {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            marker_text(line).map(|text| Marker {
                file: file.to_string(),
                line: index + 1,
                text,
            })
        })
        .collect()
}

/// Harvest every anchored marker across the repo's non-ignored files. Binary or
/// non-UTF-8 files are skipped (a debt note is text), never an error.
pub fn harvest(paths: &MaestroPaths) -> Result<Vec<Marker>> {
    let mut markers = Vec::new();
    for relative in git::repo_files(paths.repo_root())? {
        let absolute = paths.repo_root().join(&relative);
        let Ok(bytes) = std::fs::read(&absolute) else {
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };
        markers.extend(markers_in(&relative.to_string_lossy(), &content));
    }
    Ok(markers)
}

/// The dedup id for a marker: `lean-debt-<hash>` over file+line+text. A re-run
/// over the same markers resolves to the same ids, so the existence check below
/// skips them -- no duplicates.
fn card_id(marker: &Marker) -> String {
    let key = format!("{}:{}\t{}", marker.file, marker.line, marker.text);
    format!("lean-debt-{}", &sha256_hex(key.as_bytes())[..12])
}

/// Mint one task card per marker, skipping any whose file+line+text already has
/// a live card (dedup by deterministic id).
pub fn mint_cards(paths: &MaestroPaths, markers: &[Marker]) -> Result<MintOutcome> {
    let now = utc_now_timestamp();
    let mut outcome = MintOutcome::default();
    for marker in markers {
        let id = card_id(marker);
        // Dedup against the archive too, not just the live store: create_card keeps
        // archived ids reserved, so a marker whose card was minted then archived must
        // skip here -- falling through to create_card would bail and `?` the whole run.
        let already_minted = card_store::resolve(paths, &id)?.is_some()
            || crate::domain::card::archive_db::contains_card_id(paths, &id)?;
        if already_minted {
            outcome.deduped += 1;
            continue;
        }
        let mut card = Card::new(&id, CardType::Task, &marker.text, "open", &now);
        card.description = Some(format!(
            "lean debt marker at {}:{}",
            marker.file, marker.line
        ));
        card_store::create_card(paths, &card)?;
        outcome.minted.push((id, marker.clone()));
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::card::store as card_store;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: system clock should be after the Unix epoch")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("invariant: temp dir should be creatable");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn accepts_anchored_markers_after_each_comment_leader() {
        assert_eq!(
            marker_text("// lean: cache this"),
            Some("cache this".to_string())
        );
        assert_eq!(marker_text("# lean: dedupe"), Some("dedupe".to_string()));
        assert_eq!(
            marker_text("-- lean: index this"),
            Some("index this".to_string())
        );
        assert_eq!(marker_text("//lean:nospace"), Some("nospace".to_string()));
    }

    #[test]
    fn accepts_a_trailing_marker_after_code() {
        assert_eq!(
            marker_text("    let x = 1; // lean: extract a helper"),
            Some("extract a helper".to_string())
        );
    }

    #[test]
    fn rejects_clean_and_boolean_and_unanchored_lean() {
        assert_eq!(marker_text("let clean: bool = true;"), None);
        assert_eq!(marker_text("fn parse() -> boolean: {}"), None);
        assert_eq!(
            marker_text("let lean: i32 = 0;"),
            None,
            "a var named lean is not a marker"
        );
        assert_eq!(marker_text("// nothing to see here"), None);
        assert_eq!(
            marker_text("// lean:"),
            None,
            "an empty marker is not actionable"
        );
    }

    fn marker(text: &str) -> Marker {
        Marker {
            file: "src/x.rs".to_string(),
            line: 1,
            text: text.to_string(),
        }
    }

    #[test]
    fn splits_ceiling_from_upgrade_trigger_on_the_first_comma() {
        let m = marker("global lock, per-account locks if throughput matters");
        assert_eq!(m.ceiling(), "global lock");
        assert_eq!(m.upgrade(), Some("per-account locks if throughput matters"));
    }

    #[test]
    fn a_note_with_no_comma_is_all_ceiling_and_has_no_trigger() {
        let m = marker("hand-rolled retry loop");
        assert_eq!(m.ceiling(), "hand-rolled retry loop");
        assert_eq!(m.upgrade(), None, "no comma means no upgrade trigger");
    }

    #[test]
    fn an_empty_trigger_after_the_comma_counts_as_no_trigger() {
        let m = marker("in-memory cache,   ");
        assert_eq!(m.ceiling(), "in-memory cache");
        assert_eq!(m.upgrade(), None, "a blank trigger is no trigger");
    }

    #[test]
    fn mint_dedupes_by_file_line_text_across_reruns() {
        let dir = TestTempDir::new("maestro-lean-debt-mint");
        let paths = MaestroPaths::new(dir.path());
        let markers = vec![
            Marker {
                file: "src/a.rs".to_string(),
                line: 10,
                text: "cache once".to_string(),
            },
            Marker {
                file: "src/b.rs".to_string(),
                line: 20,
                text: "use serde".to_string(),
            },
        ];

        let first = mint_cards(&paths, &markers).unwrap();
        assert_eq!(first.minted.len(), 2, "first run mints one card per marker");
        assert_eq!(first.deduped, 0);
        for (id, _) in &first.minted {
            assert!(
                card_store::resolve(&paths, id).unwrap().is_some(),
                "minted card {id} is live in the store"
            );
        }

        let second = mint_cards(&paths, &markers).unwrap();
        assert_eq!(second.minted.len(), 0, "a re-run mints nothing");
        assert_eq!(
            second.deduped, 2,
            "both markers dedupe against the existing cards"
        );
    }

    #[test]
    fn mint_dedupes_against_the_archive_and_does_not_abort_the_run() {
        let dir = TestTempDir::new("maestro-lean-debt-archived");
        let paths = MaestroPaths::new(dir.path());
        let archived = Marker {
            file: "src/a.rs".to_string(),
            line: 10,
            text: "cache once".to_string(),
        };

        // Mint the marker, then relocate its card into the archive (minted -> worked
        // -> archived) while the marker stays in the code.
        let first = mint_cards(&paths, std::slice::from_ref(&archived)).unwrap();
        assert_eq!(first.minted.len(), 1);
        let id = card_id(&archived);
        let yaml = match card_store::locate(&paths, &id)
            .unwrap()
            .expect("the minted card is locatable")
        {
            card_store::CardHome::Dir(yaml) => yaml,
            card_store::CardHome::Entry(file) => file,
            card_store::CardHome::Db(path) => path,
        };
        let card_dir = yaml.parent().expect("the card dir");
        let rel = card_dir
            .strip_prefix(paths.cards_dir())
            .expect("a minted card lives under the live store");
        crate::domain::card::archive_db::archive_directory(&paths, &id, card_dir, rel).unwrap();
        fs::remove_dir_all(card_dir).unwrap();
        assert!(
            card_store::resolve(&paths, &id).unwrap().is_none(),
            "the archived card no longer lives in the live store"
        );

        // The archived marker (still in the code) plus a fresh one: the archived
        // marker dedups against the archive instead of bailing, and the fresh marker
        // after it still mints -- before the fix the archive collision `?`-aborted
        // the whole run, dropping the fresh card.
        let fresh = Marker {
            file: "src/b.rs".to_string(),
            line: 20,
            text: "use serde".to_string(),
        };
        let second = mint_cards(&paths, &[archived, fresh]).unwrap();
        assert_eq!(
            second.deduped, 1,
            "the archived marker dedups against the archive"
        );
        assert_eq!(
            second.minted.len(),
            1,
            "the fresh marker still mints after the archived collision"
        );
    }
}
