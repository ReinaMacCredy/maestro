//! The linked-card chat-channel store: pull-only, file-backed messaging between
//! two cards that share a `related` edge. This module is pure persistence and
//! queries -- the link gate, visibility, and rendering live in the `msg` verbs
//! and the inbox banner. maestro never pushes; a peer's message is seen only
//! when the other agent reads it.
//!
//! On-disk shape under `.maestro/channels/` (gitignored, machine-local):
//!   `<key>.jsonl`         line 1 = `{"pair":[a,b]}` header (authoritative),
//!                         lines 2.. = `{ts,from_card,from_session,text}`.
//!   `<key>.cur-<cardkey>` a single decimal byte offset: the viewer's cursor.
//!
//! `key` is a short hash of the sorted lowercased id pair, so both cards derive
//! the same channel; `cardkey` is a short hash of the viewer's card id, so the
//! cursor survives a title rename (ids are stable, titles are not).

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

/// One message line, tagged with its start byte offset so a reader can split
/// seen-from-unread against a stored cursor without per-message bookkeeping.
pub struct Message {
    pub ts: String,
    pub from_card: String,
    pub from_session: String,
    pub text: String,
    pub offset: u64,
}

/// A loaded channel: the authoritative header pair, every message, and the total
/// byte length (the offset a reader advances its cursor to once all is seen).
pub struct Channel {
    pub key: String,
    pub pair: [String; 2],
    pub messages: Vec<Message>,
    pub len: u64,
}

impl Channel {
    /// Messages `viewer` has not read: from the partner (never the viewer's own)
    /// and at or beyond `cursor`. Own messages are auto-seen, so they never count
    /// as unread regardless of cursor.
    pub fn unread(&self, viewer: &str, cursor: u64) -> Vec<&Message> {
        self.messages
            .iter()
            .filter(|message| message.offset >= cursor && message.from_card != viewer)
            .collect()
    }

    /// Up to `limit` most-recent already-seen partner messages, oldest-to-newest:
    /// the context window a read prints above the unread block.
    pub fn seen_context(&self, viewer: &str, cursor: u64, limit: usize) -> Vec<&Message> {
        let mut seen: Vec<&Message> = self
            .messages
            .iter()
            .filter(|message| message.offset < cursor && message.from_card != viewer)
            .collect();
        let start = seen.len().saturating_sub(limit);
        seen.split_off(start)
    }

    /// The ts through which `peer` has read this channel, derived solely from the
    /// peer's stored byte-offset cursor: the latest message the cursor has passed.
    /// `None` when the cursor is at the start -- which covers both "peer hasn't
    /// read" and a missing cursor file. Channels are gitignored/machine-local, so
    /// a peer on another machine has no cursor here and reads as `None` (blank),
    /// never a wrong timestamp (`dec-msg-read-signal-partner-read-through-2035`).
    pub fn read_through(&self, peer_cursor: u64) -> Option<&str> {
        if peer_cursor == 0 {
            return None;
        }
        self.messages
            .iter()
            .rev()
            .find(|message| message.offset < peer_cursor)
            .map(|message| message.ts.as_str())
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
    if file.metadata().context("failed to stat channel file")?.len() == 0 {
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

/// Load the channel between `a` and `b`, or `None` if no message has been sent.
pub fn load(paths: &MaestroPaths, a: &str, b: &str) -> Result<Option<Channel>> {
    let (_, key) = identity(a, b);
    load_by_key(paths, &key)
}

/// Every channel whose header pair contains `card`, loaded in full. Reads each
/// header in `.maestro/channels/` -- O(total channels in the repo). Visibility
/// (the link gate) is applied by the caller, not here.
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

/// The viewer's stored cursor (byte offset) for the channel `key`, 0 if none.
pub fn cursor(paths: &MaestroPaths, key: &str, viewer: &str) -> Result<u64> {
    let relative_path = cursor_relative_path(key, viewer);
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text.trim().parse().unwrap_or(0)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error).with_context(|| format!("failed to read {relative_path}")),
    }
}

/// Advance the viewer's cursor for channel `key` to `offset` (the channel byte
/// length after a read). managed_path rejects a symlinked cursor leaf.
pub fn set_cursor(paths: &MaestroPaths, key: &str, viewer: &str, offset: u64) -> Result<()> {
    let relative_path = cursor_relative_path(key, viewer);
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    fs::write(&path, format!("{offset}\n"))
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
    let channel = load_by_key(paths, key)?
        .context("channel file vanished between open and header check")?;
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
    let len = bytes.len() as u64;
    let mut offset: u64 = 0;
    let mut pair: Option<[String; 2]> = None;
    let mut messages = Vec::new();
    for line in text.split_inclusive('\n') {
        let start = offset;
        offset += line.len() as u64;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed)
            .with_context(|| format!("malformed channel line in {key}"))?;
        match pair {
            None => pair = Some(parse_header_pair(&value, key)?),
            Some(_) => messages.push(parse_message(&value, start)),
        }
    }
    let pair = pair.with_context(|| format!("channel {key} has no header line"))?;
    Ok(Channel {
        key: key.to_string(),
        pair,
        messages,
        len,
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

fn parse_message(value: &Value, offset: u64) -> Message {
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
        offset,
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
        assert_eq!(channel.messages.len(), 3, "three sends, three message lines");

        // A's unread from cursor 0 is only B's message, not A's own one/three.
        let unread_for_a = channel.unread("card-a", 0);
        assert_eq!(unread_for_a.len(), 1, "only the partner's message is unread");
        assert_eq!(unread_for_a[0].text, "two");

        // Reading advances A's cursor to EOF; nothing unread remains afterwards.
        set_cursor(&paths, &channel.key, "card-a", channel.len).expect("cursor write");
        let after = cursor(&paths, &channel.key, "card-a").expect("cursor read");
        assert_eq!(after, channel.len);
        assert!(
            channel.unread("card-a", after).is_empty(),
            "after reading to EOF nothing is unread"
        );
    }

    #[test]
    fn read_through_maps_partner_cursor_to_the_last_passed_message_and_blanks_at_zero() {
        let temp = TestTempDir::new("maestro-channel-readthrough");
        let paths = MaestroPaths::new(temp.path());

        send(&paths, "card-a", "card-b", "sess-a", "one").expect("a's first send");
        send(&paths, "card-a", "card-b", "sess-a", "two").expect("a's second send");

        let channel = load(&paths, "card-a", "card-b")
            .expect("load should succeed")
            .expect("channel should exist after sends");
        let first = &channel.messages[0];
        let second = &channel.messages[1];

        // A zero cursor (peer never read, or no cursor file -> cross-machine)
        // yields no read-through: blank, never a wrong timestamp.
        assert_eq!(channel.read_through(0), None);
        // A cursor exactly at the first message's start has passed nothing yet:
        // strict offset comparison, so still None (rules out "always last").
        assert_eq!(channel.read_through(first.offset), None);
        // A cursor at the second message's start has passed only the first.
        assert_eq!(channel.read_through(second.offset), Some(first.ts.as_str()));
        // A cursor at EOF (the value a read sets) has passed both -> the last ts.
        assert_eq!(channel.read_through(channel.len), Some(second.ts.as_str()));
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
