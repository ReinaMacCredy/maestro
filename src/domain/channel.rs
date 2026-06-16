//! The linked-card chat-channel store: pull-only, file-backed messaging between
//! two cards that share a `related` edge. This module is pure persistence and
//! queries -- the link gate, visibility, and rendering live in the `msg` verbs
//! and the inbox banner. maestro never pushes; a peer's message is seen only
//! when the other agent reads it.
//!
//! On-disk shape under `.maestro/channels/` (gitignored, machine-local):
//!   `<key>.jsonl`         line 1 = `{"pair":[a,b]}` header (authoritative),
//!                         lines 2.. = `{ts,from_card,from_session,text}`.
//!   `<key>.cur-<cardkey>` the viewer's read-through cursor: the ts of the
//!                         newest message they have seen (a point, not an
//!                         offset). A read re-shows only strictly-newer partner
//!                         messages, so the same cursor stays correct when the
//!                         channel is merged across several worktree files.
//!
//! `key` is a short hash of the sorted lowercased id pair, so both cards derive
//! the same channel; `cardkey` is a short hash of the viewer's card id, so the
//! cursor survives a title rename (ids are stable, titles are not).
//!
//! A send always writes the running worktree's own channel file
//! (`open_managed_appendable` cannot escape the local repo root). The union
//! reads -- `load_union`/`channels_for_union` -- merge every worktree's file for
//! a pair by ts; because a message lives in exactly one worktree's file there is
//! nothing to dedup, only to interleave.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::domain::run::{append_jsonl_line, open_managed_appendable};
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

/// Truncation length for the hashed channel and cursor keys: enough to make a
/// collision astronomically unlikely while keeping filenames short. A collision
/// is still caught by the authoritative header check, never silently merged.
const KEY_LEN: usize = 16;

/// One message line. Timestamps are fixed-width RFC3339 millis, so a string
/// compare on `ts` is a chronological compare -- the cursor and the union sort
/// both rely on it.
pub struct Message {
    pub ts: String,
    pub from_card: String,
    pub from_session: String,
    pub text: String,
}

/// A loaded channel: the authoritative header pair and every message (ts-sorted
/// when produced by the union readers).
pub struct Channel {
    pub key: String,
    pub pair: [String; 2],
    pub messages: Vec<Message>,
}

impl Channel {
    /// Messages `viewer` has not read: from the partner (never the viewer's own)
    /// and strictly newer than `through` (the ts of the newest message the
    /// viewer has already seen). `None` means nothing has been read, so every
    /// partner message is unread. Own messages are auto-seen and never counted.
    pub fn unread(&self, viewer: &str, through: Option<&str>) -> Vec<&Message> {
        self.messages
            .iter()
            .filter(|message| message.from_card != viewer)
            .filter(|message| through.is_none_or(|seen| message.ts.as_str() > seen))
            .collect()
    }

    /// Up to `limit` most-recent already-seen partner messages, oldest-to-newest:
    /// the context window a read prints above the unread block. Seen means a
    /// partner message at or before `through`; `None` yields no context.
    pub fn seen_context(&self, viewer: &str, through: Option<&str>, limit: usize) -> Vec<&Message> {
        let Some(through) = through else {
            return Vec::new();
        };
        let mut seen: Vec<&Message> = self
            .messages
            .iter()
            .filter(|message| message.from_card != viewer && message.ts.as_str() <= through)
            .collect();
        let start = seen.len().saturating_sub(limit);
        seen.split_off(start)
    }

    /// The ts the partner has read through, given their stored cursor ts: the
    /// latest message at or before it. `None` when the peer has no cursor here --
    /// which covers "peer hasn't read" and a peer on another machine or worktree
    /// whose cursor lives elsewhere (blank, never a wrong timestamp;
    /// `dec-msg-read-signal-partner-read-through-2035`).
    pub fn read_through(&self, peer_through: Option<&str>) -> Option<&str> {
        let through = peer_through?;
        self.messages
            .iter()
            .rev()
            .find(|message| message.ts.as_str() <= through)
            .map(|message| message.ts.as_str())
    }

    /// The newest message ts in the channel, or `None` when empty: the value a
    /// read stores as the viewer's cursor so a repeat read shows nothing new.
    pub fn latest_ts(&self) -> Option<&str> {
        self.messages
            .iter()
            .map(|message| message.ts.as_str())
            .max()
    }

    /// The partner card id for `viewer` (the other half of the header pair).
    pub fn partner(&self, viewer: &str) -> &str {
        if self.pair[0] == viewer {
            &self.pair[1]
        } else {
            &self.pair[0]
        }
    }
}

/// Append a message from `from_card` to its channel with `to_card`, creating the
/// channel and its authoritative header on first send. Does NOT advance the
/// sender's cursor (own messages are auto-seen). The caller owns the link gate;
/// the store only persists, reusing the run log's symlink-hardened append.
///
/// The write lands in the running worktree's `.maestro/channels/` only --
/// `open_managed_appendable` rejects any path outside the local repo root -- so
/// a peer in another worktree never sees this byte until a union read merges the
/// files (`dec-msg-send-local-read-union-cross-worktree`).
pub fn send(
    paths: &MaestroPaths,
    from_card: &str,
    to_card: &str,
    from_session: &str,
    text: &str,
) -> Result<()> {
    let (pair, key) = identity(from_card, to_card);
    let relative_path = channel_relative_path(&key);
    let mut file = open_managed_appendable(paths, &relative_path)?;
    if file
        .metadata()
        .context("failed to stat channel file")?
        .len()
        == 0
    {
        append_jsonl_line(&mut file, &json!({ "pair": pair }))
            .with_context(|| format!("failed to write header to {relative_path}"))?;
    } else {
        verify_header(paths, &key, &pair)?;
    }
    let message = json!({
        "ts": utc_now_timestamp(),
        "from_card": from_card,
        "from_session": from_session,
        "text": text,
    });
    append_jsonl_line(&mut file, &message)
        .with_context(|| format!("failed to append to {relative_path}"))
}

/// Load the channel between `a` and `b` from a single root, or `None` if no
/// message has been sent there. Local read; the union readers cover worktrees.
pub fn load(paths: &MaestroPaths, a: &str, b: &str) -> Result<Option<Channel>> {
    let (_, key) = identity(a, b);
    load_by_key(paths, &key)
}

/// Load and merge the channel between `a` and `b` across every worktree root,
/// or `None` if no message exists in any. A send is always local, so a given
/// message lives in exactly one root's file -- merging needs no dedup, only a ts
/// sort to interleave the per-worktree append streams.
pub fn load_union(roots: &[MaestroPaths], a: &str, b: &str) -> Result<Option<Channel>> {
    let (_, key) = identity(a, b);
    let mut found: Vec<Channel> = Vec::new();
    for paths in roots {
        if let Some(channel) = load_by_key(paths, &key)? {
            found.push(channel);
        }
    }
    if found.is_empty() {
        return Ok(None);
    }
    Ok(Some(merge_same_key(key, found)))
}

/// Every channel whose header pair contains `card`, loaded in full from one
/// root. Reads each header in `.maestro/channels/` -- O(total channels in the
/// repo). Visibility (the link gate) is applied by the caller, not here.
pub fn channels_for(paths: &MaestroPaths, card: &str) -> Result<Vec<Channel>> {
    let dir = paths.channels_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", dir.display()));
        }
    };
    let needle = card.to_lowercase();
    let mut channels = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(key) = name.to_str().and_then(|name| name.strip_suffix(".jsonl")) else {
            continue;
        };
        if let Some(channel) = load_by_key(paths, key)?
            && channel.pair.contains(&needle)
        {
            channels.push(channel);
        }
    }
    Ok(channels)
}

/// Every channel `card` participates in, merged across all worktree roots: one
/// `Channel` per distinct key with the per-worktree append streams interleaved
/// by ts. Visibility (the link gate) is applied by the caller.
pub fn channels_for_union(roots: &[MaestroPaths], card: &str) -> Result<Vec<Channel>> {
    let mut by_key: BTreeMap<String, Vec<Channel>> = BTreeMap::new();
    for paths in roots {
        for channel in channels_for(paths, card)? {
            by_key.entry(channel.key.clone()).or_default().push(channel);
        }
    }
    Ok(by_key
        .into_iter()
        .map(|(key, channels)| merge_same_key(key, channels))
        .collect())
}

/// The viewer's stored read-through cursor (the newest-seen message ts) for
/// channel `key`, or `None` when unread. A legacy byte-offset cursor (pure
/// digits, no `T`) or an empty file also reads as `None` ("nothing seen yet"):
/// a benign over-show on the gitignored, ephemeral channel rather than a parse
/// error -- the next read re-stamps it as a ts.
pub fn cursor(paths: &MaestroPaths, key: &str, viewer: &str) -> Result<Option<String>> {
    let relative_path = cursor_relative_path(key, viewer);
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    match fs::read_to_string(&path) {
        Ok(text) => {
            let trimmed = text.trim();
            if trimmed.contains('T') {
                Ok(Some(trimmed.to_string()))
            } else {
                Ok(None)
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {relative_path}")),
    }
}

/// Advance the viewer's cursor for channel `key` to `through_ts` (the newest
/// message ts shown by a read). managed_path rejects a symlinked cursor leaf.
pub fn set_cursor(paths: &MaestroPaths, key: &str, viewer: &str, through_ts: &str) -> Result<()> {
    let relative_path = cursor_relative_path(key, viewer);
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    fs::write(&path, format!("{through_ts}\n"))
        .with_context(|| format!("failed to write {relative_path}"))
}

/// Canonical sorted lowercased pair plus the channel key derived from it. Both
/// cards derive the same key regardless of argument order or id casing.
fn identity(a: &str, b: &str) -> ([String; 2], String) {
    let mut pair = [a.to_lowercase(), b.to_lowercase()];
    pair.sort();
    let key = sha256_hex(format!("{}\n{}", pair[0], pair[1]).as_bytes())[..KEY_LEN].to_string();
    (pair, key)
}

fn channel_relative_path(key: &str) -> String {
    format!(".maestro/channels/{key}.jsonl")
}

fn cursor_relative_path(key: &str, viewer: &str) -> String {
    let card_key = &sha256_hex(viewer.to_lowercase().as_bytes())[..KEY_LEN];
    format!(".maestro/channels/{key}.cur-{card_key}")
}

/// Merge channels that share a key (the same pair, one file per worktree) into a
/// single channel whose messages are ts-sorted. Assumes a non-empty input.
fn merge_same_key(key: String, mut channels: Vec<Channel>) -> Channel {
    let pair = channels[0].pair.clone();
    let mut messages: Vec<Message> = channels
        .drain(..)
        .flat_map(|channel| channel.messages)
        .collect();
    messages.sort_by(|a, b| a.ts.cmp(&b.ts));
    Channel {
        key,
        pair,
        messages,
    }
}

fn load_by_key(paths: &MaestroPaths, key: &str) -> Result<Option<Channel>> {
    let relative_path = channel_relative_path(key);
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {relative_path}"));
        }
    };
    parse_channel(key, &bytes).map(Some)
}

/// Confirm the on-disk header pair matches the pair we are about to write to. A
/// mismatch means two different id pairs hashed to the same key (a collision) --
/// error rather than cross-write into the wrong conversation.
fn verify_header(paths: &MaestroPaths, key: &str, expected: &[String; 2]) -> Result<()> {
    let channel =
        load_by_key(paths, key)?.context("channel file vanished between open and header check")?;
    if &channel.pair != expected {
        bail!(
            "channel key {key} already holds {:?}; refusing to write {:?} (hash collision)",
            channel.pair,
            expected
        );
    }
    Ok(())
}

fn parse_channel(key: &str, bytes: &[u8]) -> Result<Channel> {
    let text = std::str::from_utf8(bytes).context("channel file is not UTF-8")?;
    let mut pair: Option<[String; 2]> = None;
    let mut messages = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed)
            .with_context(|| format!("malformed channel line in {key}"))?;
        match pair {
            None => pair = Some(parse_header_pair(&value, key)?),
            Some(_) => messages.push(parse_message(&value)),
        }
    }
    let pair = pair.with_context(|| format!("channel {key} has no header line"))?;
    Ok(Channel {
        key: key.to_string(),
        pair,
        messages,
    })
}

fn parse_header_pair(value: &Value, key: &str) -> Result<[String; 2]> {
    let pair = value
        .get("pair")
        .and_then(Value::as_array)
        .with_context(|| format!("channel {key} header missing pair"))?;
    if pair.len() != 2 {
        bail!("channel {key} header pair must have two ids");
    }
    let first = pair[0].as_str().unwrap_or_default().to_string();
    let second = pair[1].as_str().unwrap_or_default().to_string();
    Ok([first, second])
}

fn parse_message(value: &Value) -> Message {
    let field = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Message {
        ts: field("ts"),
        from_card: field("from_card"),
        from_session: field("from_session"),
        text: field("text"),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

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

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// Build an in-memory channel with controlled timestamps, so the cursor and
    /// read-through edge cases are exercised without racing the millis clock.
    fn channel_with(pair: [&str; 2], messages: &[(&str, &str, &str)]) -> Channel {
        Channel {
            key: "test-key".to_string(),
            pair: [pair[0].to_string(), pair[1].to_string()],
            messages: messages
                .iter()
                .map(|(ts, from, text)| Message {
                    ts: (*ts).to_string(),
                    from_card: (*from).to_string(),
                    from_session: "sess".to_string(),
                    text: (*text).to_string(),
                })
                .collect(),
        }
    }

    #[test]
    fn send_then_load_round_trips_with_canonical_pair_and_unread_for_partner_only() {
        let temp = TestTempDir::new("maestro-channel-roundtrip");
        let paths = MaestroPaths::new(temp.path());

        // Order and casing do not change the channel: B->A reaches the same file.
        send(&paths, "card-a", "card-b", "sess-a", "one").expect("a's send should persist");
        send(&paths, "CARD-B", "card-a", "sess-b", "two").expect("b's send should persist");
        send(&paths, "card-a", "card-b", "sess-a", "three").expect("a's third send");

        let channel = load(&paths, "card-b", "card-a")
            .expect("load should succeed")
            .expect("channel should exist after sends");
        assert_eq!(channel.pair, ["card-a".to_string(), "card-b".to_string()]);
        assert_eq!(
            channel.messages.len(),
            3,
            "three sends, three message lines"
        );

        // A's unread from a never-read cursor is only B's message, not its own.
        let unread_for_a = channel.unread("card-a", None);
        assert_eq!(
            unread_for_a.len(),
            1,
            "only the partner's message is unread"
        );
        assert_eq!(unread_for_a[0].text, "two");

        // Reading stores the newest ts as A's cursor; nothing unread remains.
        let newest = channel.latest_ts().expect("non-empty channel has a latest ts");
        set_cursor(&paths, &channel.key, "card-a", newest).expect("cursor write");
        let after = cursor(&paths, &channel.key, "card-a").expect("cursor read");
        assert_eq!(after.as_deref(), Some(newest));
        assert!(
            channel.unread("card-a", after.as_deref()).is_empty(),
            "after reading to the newest ts nothing is unread"
        );
    }

    #[test]
    fn unread_reshows_strictly_newer_partner_messages_and_holds_the_same_ts_edge() {
        // Two partner messages share a ts; a third is strictly newer.
        let channel = channel_with(
            ["card-a", "card-b"],
            &[
                ("2026-06-17T00:00:00.000Z", "card-b", "first at T0"),
                ("2026-06-17T00:00:01.000Z", "card-b", "second at T1"),
            ],
        );

        // Cursor at T0 re-shows only the strictly-newer T1 message.
        let unread = channel.unread("card-a", Some("2026-06-17T00:00:00.000Z"));
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].text, "second at T1");

        // KNOWN EDGE (qa.md baseline gap): a partner message stamped at exactly
        // the stored cursor ts reads as already-seen. After reading through T1, a
        // same-ts T1 message does not re-surface -- accepted, not fixed.
        let same_ts = channel_with(
            ["card-a", "card-b"],
            &[
                ("2026-06-17T00:00:01.000Z", "card-b", "seen at T1"),
                ("2026-06-17T00:00:01.000Z", "card-b", "arrived later at T1"),
            ],
        );
        assert!(
            same_ts
                .unread("card-a", Some("2026-06-17T00:00:01.000Z"))
                .is_empty(),
            "a same-ts post-read message is treated as seen (documented edge)"
        );
    }

    #[test]
    fn read_through_maps_partner_cursor_to_the_last_passed_message_and_blanks_when_absent() {
        let channel = channel_with(
            ["card-a", "card-b"],
            &[
                ("2026-06-17T00:00:00.000Z", "card-a", "one"),
                ("2026-06-17T00:00:01.000Z", "card-a", "two"),
            ],
        );

        // No cursor (peer hasn't read, or cross-worktree/cross-machine) -> blank.
        assert_eq!(channel.read_through(None), None);
        // A cursor at the first ts has passed exactly the first message.
        assert_eq!(
            channel.read_through(Some("2026-06-17T00:00:00.000Z")),
            Some("2026-06-17T00:00:00.000Z")
        );
        // A cursor at the newest ts has passed both -> the last ts.
        assert_eq!(
            channel.read_through(Some("2026-06-17T00:00:01.000Z")),
            Some("2026-06-17T00:00:01.000Z")
        );
    }

    #[test]
    fn legacy_byte_offset_cursor_reads_as_nothing_seen_not_a_crash() {
        let temp = TestTempDir::new("maestro-channel-legacy-cursor");
        let paths = MaestroPaths::new(temp.path());
        let (_, key) = identity("card-a", "card-b");

        // Simulate a pre-migration cursor file holding a raw byte offset.
        let channels = temp.path().join(".maestro/channels");
        fs::create_dir_all(&channels).expect("channels dir should be creatable");
        let card_key = &sha256_hex("card-a".as_bytes())[..KEY_LEN];
        fs::write(channels.join(format!("{key}.cur-{card_key}")), "4096\n")
            .expect("legacy cursor should be writable");

        assert_eq!(
            cursor(&paths, &key, "card-a").expect("legacy cursor should not error"),
            None,
            "a numeric (no-'T') cursor reads as nothing seen"
        );
    }

    #[test]
    fn channels_for_returns_only_channels_containing_the_card() {
        let temp = TestTempDir::new("maestro-channel-membership");
        let paths = MaestroPaths::new(temp.path());

        send(&paths, "card-a", "card-b", "sess-a", "hi b").expect("a-b send");
        send(&paths, "card-a", "card-e", "sess-a", "hi e").expect("a-e send");
        send(&paths, "card-c", "card-d", "sess-c", "unrelated").expect("c-d send");

        let mut partners: Vec<String> = channels_for(&paths, "card-a")
            .expect("enumeration should succeed")
            .iter()
            .map(|channel| channel.partner("card-a").to_string())
            .collect();
        partners.sort();
        assert_eq!(partners, vec!["card-b".to_string(), "card-e".to_string()]);
    }

    #[test]
    fn send_is_local_and_load_union_merges_worktree_files_by_ts() {
        let root_a = TestTempDir::new("maestro-channel-union-a");
        let root_b = TestTempDir::new("maestro-channel-union-b");
        let paths_a = MaestroPaths::new(root_a.path());
        let paths_b = MaestroPaths::new(root_b.path());

        // Each worktree sends into its OWN channel file (send is local).
        send(&paths_a, "card-a", "card-b", "sess-a", "from worktree A").expect("a send");
        send(&paths_b, "card-b", "card-a", "sess-b", "from worktree B").expect("b send");

        // A local load sees only the local worktree's one message -> send-local.
        let local_a = load(&paths_a, "card-a", "card-b")
            .expect("local load ok")
            .expect("local channel exists");
        assert_eq!(
            local_a.messages.len(),
            1,
            "send wrote only worktree A's file, not a shared store"
        );
        assert_eq!(local_a.messages[0].text, "from worktree A");

        // The union merges both worktrees' files, ts-sorted.
        let union = load_union(&[paths_a, paths_b], "card-a", "card-b")
            .expect("union load ok")
            .expect("union channel exists");
        assert_eq!(union.messages.len(), 2, "both worktrees' messages merge");
        let texts: Vec<&str> = union.messages.iter().map(|m| m.text.as_str()).collect();
        assert!(texts.contains(&"from worktree A"));
        assert!(texts.contains(&"from worktree B"));
        assert!(
            union.messages.windows(2).all(|w| w[0].ts <= w[1].ts),
            "merged messages are in non-decreasing ts order"
        );
    }

    #[test]
    fn send_into_a_key_holding_a_different_pair_errors_instead_of_cross_writing() {
        let temp = TestTempDir::new("maestro-channel-collision");
        let paths = MaestroPaths::new(temp.path());

        // Forge a channel file at the key for (card-a, card-b) whose header
        // claims a different pair, simulating a hash collision.
        let (_, key) = identity("card-a", "card-b");
        let channels = temp.path().join(".maestro/channels");
        fs::create_dir_all(&channels).expect("channels dir should be creatable");
        fs::write(
            channels.join(format!("{key}.jsonl")),
            "{\"pair\":[\"card-x\",\"card-y\"]}\n",
        )
        .expect("forged header should be writable");

        let result = send(&paths, "card-a", "card-b", "sess-a", "should not land");
        assert!(
            result.is_err(),
            "a header-pair mismatch must error, not cross-write"
        );
        let contents = fs::read_to_string(channels.join(format!("{key}.jsonl")))
            .expect("forged file should still be readable");
        assert!(
            !contents.contains("should not land"),
            "the rejected message must not be appended: {contents}"
        );
    }

    #[test]
    fn send_refuses_a_symlinked_channel_file() {
        let temp = TestTempDir::new("maestro-channel-symlink");
        let paths = MaestroPaths::new(temp.path());

        let (_, key) = identity("card-a", "card-b");
        let channels = temp.path().join(".maestro/channels");
        fs::create_dir_all(&channels).expect("channels dir should be creatable");
        let outside = temp.path().join("outside.jsonl");
        fs::write(&outside, "").expect("decoy target should be writable");
        symlink(&outside, channels.join(format!("{key}.jsonl")))
            .expect("symlink fixture should be creatable");

        let result = send(&paths, "card-a", "card-b", "sess-a", "via symlink");
        assert!(result.is_err(), "a symlinked channel file must be refused");
        let leaked = fs::read_to_string(&outside).expect("decoy should remain readable");
        assert!(
            leaked.is_empty(),
            "no bytes may be written through the symlink: {leaked}"
        );
    }
}
