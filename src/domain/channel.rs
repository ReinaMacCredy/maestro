//! The chat-channel store: pull-only, file-backed messaging. A channel is either
//! a two-card pair that shares a `related` edge, or a feature-scoped broadcast
//! every card under a feature shares (no `related` edge required). This module is
//! pure persistence and queries -- the link/membership gate, visibility, and
//! rendering live in the `msg` verbs and the inbox banner. maestro never pushes;
//! a peer's message is seen only when the other agent reads it.
//!
//! On-disk shape under `.maestro/channels/` (gitignored, machine-local):
//!   `<key>.jsonl`         line 1 = `{"pair":[a,b]}` (two-card) or `{"feature":id}`
//!                         (feature broadcast) header (authoritative),
//!                         lines 2.. = `{id,ts,from_card,from_session,text}`.
//!   `<key>.cur-<cardkey>` the viewer's read-through cursor: the newest seen ts
//!                         plus exact message ids at that ts. Legacy bare-ts
//!                         cursors still read as timestamp-only cursors.
//!
//! `key` is a short hash of the sorted lowercased id pair (two-card) or of
//! `feature\n<id>` (broadcast), so every member derives the same channel;
//! `cardkey` is a short hash of the viewer's card id, so the cursor survives a
//! title rename (ids are stable, titles are not).
//!
//! A send always writes the running worktree's own channel file
//! (`open_managed_appendable` cannot escape the local repo root). The union
//! reads -- `load_union`/`channels_for_union` -- merge every worktree's file for
//! a pair by ts; because a message lives in exactly one worktree's file there is
//! nothing to dedup, only to interleave.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::domain::run::{append_jsonl_line, open_managed_appendable};
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::{parse_utc_timestamp, utc_now_timestamp};

/// Truncation length for the hashed channel and cursor keys: enough to make a
/// collision astronomically unlikely while keeping filenames short. A collision
/// is still caught by the authoritative header check, never silently merged.
const KEY_LEN: usize = 16;
const CURSOR_SCHEMA_VERSION: &str = "maestro.channel-cursor.v1";
static MESSAGE_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// One message line. Timestamps are fixed-width RFC3339 millis, so a string
/// compare on `ts` is a chronological compare -- the cursor and the union sort
/// both rely on it.
pub struct Message {
    pub id: Option<String>,
    pub ts: String,
    pub from_card: String,
    pub from_session: String,
    pub text: String,
}

/// The two channel shapes, mirroring the header line: a two-card `Pair`
/// (`related`-gated) or a `Feature` broadcast every card under the feature
/// shares. The variant drives rendering -- a pair has a partner; a feature labels
/// each line by its sender.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChannelKind {
    Pair([String; 2]),
    Feature(String),
}

/// A loaded channel: its kind (the authoritative header) and every message
/// (ts-sorted when produced by the union readers).
pub struct Channel {
    pub key: String,
    pub kind: ChannelKind,
    pub messages: Vec<Message>,
}

impl Channel {
    /// Messages `viewer` has not read: from the partner (never the viewer's own)
    /// and either newer than the cursor or, for exact cursors, missing from the
    /// cursor's same-timestamp seen set. Own messages are auto-seen and never
    /// counted.
    pub fn unread(&self, viewer: &str, through: Option<&str>) -> Vec<&Message> {
        let cursor = through.and_then(parse_cursor);
        self.messages
            .iter()
            .filter(|message| message.from_card != viewer)
            .filter(|message| cursor.as_ref().is_none_or(|seen| !seen.covers(message)))
            .collect()
    }

    /// Up to `limit` most-recent already-seen partner messages, oldest-to-newest:
    /// the context window a read prints above the unread block. Seen means a
    /// partner message at or before `through`; `None` yields no context.
    pub fn seen_context(&self, viewer: &str, through: Option<&str>, limit: usize) -> Vec<&Message> {
        let Some(cursor) = through.and_then(parse_cursor) else {
            return Vec::new();
        };
        let mut seen: Vec<&Message> = self
            .messages
            .iter()
            .filter(|message| message.from_card != viewer && cursor.covers(message))
            .collect();
        let start = seen.len().saturating_sub(limit);
        seen.split_off(start)
    }

    /// The ts the partner has read through, given their stored cursor. `None`
    /// when the peer has no cursor.
    pub fn read_through(&self, peer_through: Option<&str>) -> Option<&str> {
        let through = peer_through.and_then(parse_cursor)?;
        self.messages
            .iter()
            .rev()
            .find(|message| through.covers(message))
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

    /// The partner card id for `viewer` on a pair channel (the other header half).
    /// A feature channel has no single partner, so it answers with the feature id;
    /// callers branch on `kind` before relying on this.
    pub fn partner(&self, viewer: &str) -> &str {
        match &self.kind {
            ChannelKind::Pair(pair) => {
                if pair[0] == viewer {
                    &pair[1]
                } else {
                    &pair[0]
                }
            }
            ChannelKind::Feature(id) => id,
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
    append_message(
        paths,
        &key,
        &ChannelKind::Pair(pair),
        from_card,
        from_session,
        text,
    )
}

/// Append a broadcast message from `from_card` to its feature's channel, creating
/// the channel and its `{"feature":id}` header on first send. Membership is the
/// caller's gate (the feature parent edge); the store only persists. Like
/// [`send`], the write is local to the running worktree -- a union read merges the
/// other worktrees' files.
pub fn send_feature(
    paths: &MaestroPaths,
    feature_id: &str,
    from_card: &str,
    from_session: &str,
    text: &str,
) -> Result<()> {
    let key = feature_identity(feature_id);
    append_message(
        paths,
        &key,
        &ChannelKind::Feature(feature_id.to_lowercase()),
        from_card,
        from_session,
        text,
    )
}

/// Append one message under `header`, writing the authoritative header line on
/// first send and verifying it on every later send (a key collision errors rather
/// than cross-writing into the wrong conversation). Shared by [`send`] and
/// [`send_feature`]; does NOT advance the sender's cursor (own messages are
/// auto-seen). The write lands in the running worktree's `.maestro/channels/`
/// only -- `open_managed_appendable` rejects any path outside the local repo root.
fn append_message(
    paths: &MaestroPaths,
    key: &str,
    header: &ChannelKind,
    from_card: &str,
    from_session: &str,
    text: &str,
) -> Result<()> {
    let relative_path = channel_relative_path(key);
    let mut file = open_managed_appendable(paths, &relative_path)?;
    if file
        .metadata()
        .context("failed to stat channel file")?
        .len()
        == 0
    {
        append_jsonl_line(&mut file, &header_json(header))
            .with_context(|| format!("failed to write header to {relative_path}"))?;
    } else {
        verify_header(paths, key, header)?;
    }
    let ts = utc_now_timestamp();
    let message = json!({
        "id": new_message_id(&ts, from_card, from_session, text),
        "ts": ts,
        "from_card": from_card,
        "from_session": from_session,
        "text": text,
    });
    append_jsonl_line(&mut file, &message)
        .with_context(|| format!("failed to append to {relative_path}"))
}

/// The JSON header line for a channel kind: `{"pair":[a,b]}` or `{"feature":id}`.
fn header_json(kind: &ChannelKind) -> Value {
    match kind {
        ChannelKind::Pair(pair) => json!({ "pair": pair }),
        ChannelKind::Feature(id) => json!({ "feature": id }),
    }
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
    load_union_by_key(roots, &key)
}

/// Load the feature-scoped broadcast channel for `feature_id` from a single root,
/// or `None` if nothing was broadcast there. Local read; the union reader covers
/// worktrees.
pub fn load_feature(paths: &MaestroPaths, feature_id: &str) -> Result<Option<Channel>> {
    load_by_key(paths, &feature_identity(feature_id))
}

/// Load and merge the feature-scoped broadcast channel across every worktree
/// root, ts-sorted, or `None` if nothing was broadcast in any.
pub fn load_feature_union(roots: &[MaestroPaths], feature_id: &str) -> Result<Option<Channel>> {
    load_union_by_key(roots, &feature_identity(feature_id))
}

/// Merge one channel key across worktree roots into a single ts-sorted channel,
/// or `None` if no root holds it. Shared by the pair and feature union readers; a
/// send is always local, so a message lives in exactly one root's file (no dedup).
fn load_union_by_key(roots: &[MaestroPaths], key: &str) -> Result<Option<Channel>> {
    let mut found: Vec<Channel> = Vec::new();
    for paths in roots {
        if let Some(channel) = load_by_key(paths, key)? {
            found.push(channel);
        }
    }
    if found.is_empty() {
        return Ok(None);
    }
    Ok(Some(merge_same_key(key.to_string(), found)))
}

/// Every *pair* channel whose header pair contains `card`, loaded in full from
/// one root. Reads each header in `.maestro/channels/` -- O(total channels in the
/// repo). Feature broadcast channels are not keyed by member id (the header holds
/// only the feature), so they are excluded here; a caller loads a feature channel
/// directly with [`load_feature`]. Visibility (the link gate) is applied by the
/// caller, not here.
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
            && let ChannelKind::Pair(pair) = &channel.kind
            && pair.contains(&needle)
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

/// The viewer's stored read-through cursor for channel `key`, or `None` when
/// unread. A legacy byte-offset cursor, malformed cursor, or empty file reads as
/// `None`: a benign over-show on the gitignored, ephemeral channel rather than a
/// parse error.
pub fn cursor(paths: &MaestroPaths, key: &str, viewer: &str) -> Result<Option<String>> {
    let relative_path = cursor_relative_path(key, viewer);
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    match fs::read_to_string(&path) {
        Ok(text) => {
            let trimmed = text.trim();
            if parse_cursor(trimmed).is_some() {
                Ok(Some(trimmed.to_string()))
            } else {
                Ok(None)
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {relative_path}")),
    }
}

/// Read the newest cursor for `viewer` across worktree roots, merging exact
/// same-timestamp seen ids when several roots have read to the same point.
pub fn cursor_union(roots: &[MaestroPaths], key: &str, viewer: &str) -> Result<Option<String>> {
    let mut merged: Option<CursorState> = None;
    for paths in roots {
        let Some(raw) = cursor(paths, key, viewer)? else {
            continue;
        };
        let Some(state) = parse_cursor(&raw) else {
            continue;
        };
        match &mut merged {
            None => merged = Some(state),
            Some(current) => current.merge(state),
        }
    }
    Ok(merged.map(|state| state.to_storage()))
}

/// Advance the viewer's cursor for channel `key` to `through_ts` (the newest
/// message ts shown by a read). managed_path rejects a symlinked cursor leaf.
pub fn set_cursor(paths: &MaestroPaths, key: &str, viewer: &str, through_ts: &str) -> Result<()> {
    ensure_channels_dir(paths)?;
    let relative_path = cursor_relative_path(key, viewer);
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    fs::write(&path, format!("{through_ts}\n"))
        .with_context(|| format!("failed to write {relative_path}"))
}

/// Advance the viewer's cursor to the newest loaded message, preserving exact
/// ids for every message at that timestamp so later same-ms messages remain
/// unread.
pub fn set_cursor_to_latest(paths: &MaestroPaths, channel: &Channel, viewer: &str) -> Result<()> {
    let Some(cursor) = CursorState::for_latest(channel) else {
        return Ok(());
    };
    ensure_channels_dir(paths)?;
    let relative_path = cursor_relative_path(&channel.key, viewer);
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    fs::write(&path, format!("{}\n", cursor.to_storage()))
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

/// The channel key for a feature-scoped broadcast: a short hash of `feature\n<id>`
/// (lowercased), namespaced apart from pair keys (`{a}\n{b}`) so the two kinds
/// never collide.
fn feature_identity(feature_id: &str) -> String {
    sha256_hex(format!("feature\n{}", feature_id.to_lowercase()).as_bytes())[..KEY_LEN].to_string()
}

fn channel_relative_path(key: &str) -> String {
    format!(".maestro/channels/{key}.jsonl")
}

fn cursor_relative_path(key: &str, viewer: &str) -> String {
    let card_key = &sha256_hex(viewer.to_lowercase().as_bytes())[..KEY_LEN];
    format!(".maestro/channels/{key}.cur-{card_key}")
}

fn ensure_channels_dir(paths: &MaestroPaths) -> Result<()> {
    let dir = managed_path(
        paths,
        ".maestro/channels",
        SymlinkPolicy::RejectAllComponents,
    )?;
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CursorState {
    through_ts: String,
    seen_at_through: BTreeSet<String>,
    exact: bool,
}

impl CursorState {
    fn timestamp_only(through_ts: String) -> Self {
        Self {
            through_ts,
            seen_at_through: BTreeSet::new(),
            exact: false,
        }
    }

    fn exact(through_ts: String, seen_at_through: BTreeSet<String>) -> Self {
        Self {
            through_ts,
            seen_at_through,
            exact: true,
        }
    }

    fn for_latest(channel: &Channel) -> Option<Self> {
        let through_ts = channel.latest_ts()?.to_string();
        let seen_at_through = channel
            .messages
            .iter()
            .filter(|message| message.ts == through_ts)
            .map(message_cursor_id)
            .collect();
        Some(Self::exact(through_ts, seen_at_through))
    }

    fn covers(&self, message: &Message) -> bool {
        match message.ts.as_str().cmp(self.through_ts.as_str()) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal if !self.exact => true,
            std::cmp::Ordering::Equal => self.seen_at_through.contains(&message_cursor_id(message)),
        }
    }

    fn merge(&mut self, other: Self) {
        match other.through_ts.cmp(&self.through_ts) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Greater => *self = other,
            std::cmp::Ordering::Equal if self.exact && other.exact => {
                self.seen_at_through.extend(other.seen_at_through);
            }
            std::cmp::Ordering::Equal => {
                self.exact = false;
                self.seen_at_through.clear();
            }
        }
    }

    fn to_storage(&self) -> String {
        if !self.exact {
            return self.through_ts.clone();
        }
        let seen: Vec<&str> = self.seen_at_through.iter().map(String::as_str).collect();
        json!({
            "schema_version": CURSOR_SCHEMA_VERSION,
            "through_ts": self.through_ts,
            "seen_at_through": seen,
        })
        .to_string()
    }
}

fn parse_cursor(raw: &str) -> Option<CursorState> {
    let raw = raw.trim();
    if parse_utc_timestamp(raw).is_some() {
        return Some(CursorState::timestamp_only(raw.to_string()));
    }
    let value: Value = serde_json::from_str(raw).ok()?;
    let through_ts = value.get("through_ts").and_then(Value::as_str)?;
    parse_utc_timestamp(through_ts)?;
    let seen_at_through = value
        .get("seen_at_through")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    Some(CursorState::exact(through_ts.to_string(), seen_at_through))
}

fn new_message_id(ts: &str, from_card: &str, from_session: &str, text: &str) -> String {
    let sequence = MESSAGE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    sha256_hex(
        format!("{ts}\x1f{from_card}\x1f{from_session}\x1f{text}\x1f{nonce}\x1f{sequence}")
            .as_bytes(),
    )
}

fn message_cursor_id(message: &Message) -> String {
    message.id.clone().unwrap_or_else(|| {
        sha256_hex(
            format!(
                "{}\x1f{}\x1f{}\x1f{}",
                message.ts, message.from_card, message.from_session, message.text
            )
            .as_bytes(),
        )
    })
}

/// Merge channels that share a key (the same pair, one file per worktree) into a
/// single channel whose messages are ts-sorted. Assumes a non-empty input.
fn merge_same_key(key: String, mut channels: Vec<Channel>) -> Channel {
    let kind = channels[0].kind.clone();
    let mut messages: Vec<Message> = channels
        .drain(..)
        .flat_map(|channel| channel.messages)
        .collect();
    messages.sort_by(|a, b| a.ts.cmp(&b.ts));
    Channel {
        key,
        kind,
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

/// Confirm the on-disk header matches the kind we are about to write to. A
/// mismatch means two different ids hashed to the same key (a collision) --
/// error rather than cross-write into the wrong conversation.
fn verify_header(paths: &MaestroPaths, key: &str, expected: &ChannelKind) -> Result<()> {
    let channel =
        load_by_key(paths, key)?.context("channel file vanished between open and header check")?;
    if &channel.kind != expected {
        bail!(
            "channel key {key} already holds {:?}; refusing to write {:?} (hash collision)",
            channel.kind,
            expected
        );
    }
    Ok(())
}

fn parse_channel(key: &str, bytes: &[u8]) -> Result<Channel> {
    let text = std::str::from_utf8(bytes).context("channel file is not UTF-8")?;
    let mut kind: Option<ChannelKind> = None;
    let mut messages = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed)
            .with_context(|| format!("malformed channel line in {key}"))?;
        match kind {
            None => kind = Some(parse_header(&value, key)?),
            Some(_) => messages.push(parse_message(&value)),
        }
    }
    let kind = kind.with_context(|| format!("channel {key} has no header line"))?;
    Ok(Channel {
        key: key.to_string(),
        kind,
        messages,
    })
}

/// Parse the authoritative header line into a [`ChannelKind`]: `{"pair":[a,b]}`
/// for a two-card channel, `{"feature":id}` for a feature broadcast.
fn parse_header(value: &Value, key: &str) -> Result<ChannelKind> {
    if value.get("pair").is_some() {
        let pair = value
            .get("pair")
            .and_then(Value::as_array)
            .with_context(|| format!("channel {key} header missing pair"))?;
        if pair.len() != 2 {
            bail!("channel {key} header pair must have two ids");
        }
        let first = pair[0].as_str().unwrap_or_default().to_string();
        let second = pair[1].as_str().unwrap_or_default().to_string();
        Ok(ChannelKind::Pair([first, second]))
    } else if let Some(feature) = value.get("feature").and_then(Value::as_str) {
        Ok(ChannelKind::Feature(feature.to_string()))
    } else {
        bail!("channel {key} header missing pair/feature")
    }
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
        id: value.get("id").and_then(Value::as_str).map(str::to_string),
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
            kind: ChannelKind::Pair([pair[0].to_string(), pair[1].to_string()]),
            messages: messages
                .iter()
                .map(|(ts, from, text)| Message {
                    id: Some(sha256_hex(
                        format!("{ts}\x1f{from}\x1fsess\x1f{text}").as_bytes(),
                    )),
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
        assert_eq!(
            channel.kind,
            ChannelKind::Pair(["card-a".to_string(), "card-b".to_string()])
        );
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
        let newest = channel
            .latest_ts()
            .expect("non-empty channel has a latest ts");
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

        // Exact cursors remember which same-ts messages were already shown, so
        // a later message stamped at the same millisecond still surfaces.
        let same_ts = channel_with(
            ["card-a", "card-b"],
            &[
                ("2026-06-17T00:00:01.000Z", "card-b", "seen at T1"),
                ("2026-06-17T00:00:01.000Z", "card-b", "arrived later at T1"),
            ],
        );
        let cursor = CursorState::exact(
            "2026-06-17T00:00:01.000Z".to_string(),
            BTreeSet::from([message_cursor_id(&same_ts.messages[0])]),
        )
        .to_storage();
        let unread = same_ts.unread("card-a", Some(&cursor));
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].text, "arrived later at T1");
    }

    #[test]
    fn cursor_union_merges_exact_same_timestamp_seen_sets() {
        let temp_a = TestTempDir::new("maestro-channel-cursor-union-a");
        let temp_b = TestTempDir::new("maestro-channel-cursor-union-b");
        let paths_a = MaestroPaths::new(temp_a.path());
        let paths_b = MaestroPaths::new(temp_b.path());
        let channel_a = channel_with(
            ["card-a", "card-b"],
            &[("2026-06-17T00:00:01.000Z", "card-b", "seen in A")],
        );
        let channel_b = channel_with(
            ["card-a", "card-b"],
            &[("2026-06-17T00:00:01.000Z", "card-b", "seen in B")],
        );

        set_cursor_to_latest(&paths_a, &channel_a, "card-a").expect("cursor A should write");
        set_cursor_to_latest(&paths_b, &channel_b, "card-a").expect("cursor B should write");

        let roots = [paths_a, paths_b];
        let cursor = cursor_union(&roots, &channel_a.key, "card-a")
            .expect("cursor union should read")
            .expect("merged cursor should exist");
        let merged = channel_with(
            ["card-a", "card-b"],
            &[
                ("2026-06-17T00:00:01.000Z", "card-b", "seen in A"),
                ("2026-06-17T00:00:01.000Z", "card-b", "seen in B"),
                ("2026-06-17T00:00:01.000Z", "card-b", "unseen at same ts"),
            ],
        );

        let unread = merged.unread("card-a", Some(&cursor));
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].text, "unseen at same ts");
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
    fn malformed_timestamp_cursor_reads_as_nothing_seen() {
        let temp = TestTempDir::new("maestro-channel-bad-cursor");
        let paths = MaestroPaths::new(temp.path());
        let (_, key) = identity("card-a", "card-b");

        let channels = temp.path().join(".maestro/channels");
        fs::create_dir_all(&channels).expect("channels dir should be creatable");
        let card_key = &sha256_hex("card-a".as_bytes())[..KEY_LEN];
        fs::write(
            channels.join(format!("{key}.cur-{card_key}")),
            "not-a-Time\n",
        )
        .expect("bad cursor should be writable");

        assert_eq!(
            cursor(&paths, &key, "card-a").expect("bad cursor should not error"),
            None,
            "a malformed timestamp cursor reads as nothing seen"
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
    fn feature_broadcast_reaches_members_on_one_distinct_channel_with_per_sender_lines() {
        let temp = TestTempDir::new("maestro-channel-feature-broadcast");
        let paths = MaestroPaths::new(temp.path());

        // Two different members broadcast to the same feature; no pair channel.
        send_feature(&paths, "feat-x", "card-a", "sess-a", "switching to redis")
            .expect("a's broadcast persists");
        send_feature(&paths, "feat-x", "card-b", "sess-b", "ack, migrating keys")
            .expect("b's broadcast persists");

        let channel = load_feature(&paths, "feat-x")
            .expect("load_feature ok")
            .expect("feature channel exists after a broadcast");
        assert_eq!(channel.kind, ChannelKind::Feature("feat-x".to_string()));
        assert_eq!(
            channel.messages.len(),
            2,
            "both members' lines on one channel"
        );

        // Each line carries its real sender (speaker-per-line render depends on it).
        let senders: BTreeSet<&str> = channel
            .messages
            .iter()
            .map(|message| message.from_card.as_str())
            .collect();
        assert_eq!(senders, BTreeSet::from(["card-a", "card-b"]));

        // The feature key is namespaced apart from the members' pair key.
        let (_, pair_key) = identity("card-a", "card-b");
        assert_ne!(channel.key, pair_key);
        assert!(
            load(&paths, "card-a", "card-b")
                .expect("pair load ok")
                .is_none(),
            "a broadcast must not create a pairwise channel"
        );

        // A member's unread from a never-read cursor is the OTHER member's line
        // only -- own lines are auto-seen.
        let unread_for_a = channel.unread("card-a", None);
        let texts: Vec<&str> = unread_for_a.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(texts, vec!["ack, migrating keys"]);
    }

    #[test]
    fn feature_broadcast_send_is_local_and_load_feature_union_merges_carrying_kind() {
        let root_a = TestTempDir::new("maestro-channel-feature-union-a");
        let root_b = TestTempDir::new("maestro-channel-feature-union-b");
        let paths_a = MaestroPaths::new(root_a.path());
        let paths_b = MaestroPaths::new(root_b.path());

        send_feature(&paths_a, "feat-x", "card-a", "sess-a", "from worktree A").expect("a send");
        send_feature(&paths_b, "feat-x", "card-b", "sess-b", "from worktree B").expect("b send");

        // Send is local: each worktree's file holds only its own line.
        let local_a = load_feature(&paths_a, "feat-x")
            .expect("local load ok")
            .expect("local feature channel exists");
        assert_eq!(local_a.messages.len(), 1);

        // The union merges both worktrees' files, ts-sorted, preserving the kind.
        let union = load_feature_union(&[paths_a, paths_b], "feat-x")
            .expect("union load ok")
            .expect("union feature channel exists");
        assert_eq!(union.messages.len(), 2, "both worktrees' messages merge");
        assert_eq!(
            union.kind,
            ChannelKind::Feature("feat-x".to_string()),
            "the union must carry the Feature kind, not drop it on merge"
        );
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
