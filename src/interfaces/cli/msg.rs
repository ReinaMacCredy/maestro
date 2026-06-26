//! `maestro msg`: pull-only messaging on a card channel. A send addressed to a
//! pair partner is link-gated; a send addressed to a feature card broadcasts to
//! every card under that feature, gated by membership (the parent edge) rather
//! than a link. `read` consumes unread (advancing the viewer's cursor); `list`
//! shows the channel overview or one channel's full timeline. The sender is
//! always the running session's current card -- the card it last touched -- never
//! a named argument, so messaging rides on normal work.
//!
//! Pair visibility is the live link: the channel shows in `read`/`list` (and the
//! banner) only while the pair currently shares a `related` edge. Unlinking hides
//! it without deleting; relinking restores history and prior unread
//! (`dec-channel-visibility-hide-on-unlink-6091`). A feature channel shows while
//! the running card is under that feature (membership is the live parent edge;
//! there is no reparent verb, so it ends only when the feature goes terminal).

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail};

use crate::domain::card;
use crate::domain::channel::{self, Channel, ChannelKind, Message};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{MsgArgs, MsgCommand, worktree_roots};

/// How many already-seen partner messages a read prints as context above the
/// unread block (`dec-msg-verbs-read-model-send-link-gated-52f6`).
const CONTEXT_LIMIT: usize = 5;
const TASK_ORDERING_HINT: &str = "hint: inbox is advisory; enforce ordering with explicit task blockers: maestro task block <id> --by <task-id>";

pub fn run(args: MsgArgs) -> Result<()> {
    match args.command {
        MsgCommand::Send { from, to, text } => send(from.as_deref(), &to, &text),
        MsgCommand::Read { card } => read(card.as_deref()),
        MsgCommand::List { card } => list(card.as_deref()),
    }
}

/// `maestro msg send <to> <text>`: append a message to the channel between the
/// running card and `<to>`. Link-gated (bidirectional, archive-aware) and
/// validated; emits no card_touch and no run event -- a send is not work state.
fn send(from: Option<&str>, to: &str, text: &str) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let me = current_card(&paths)?;
    if let Some(asserted) = from
        && !asserted.eq_ignore_ascii_case(&me)
    {
        bail!(
            "--from does not match current card; requested from: {asserted}; current card: {me}; touch or claim {asserted} before sending from it"
        );
    }
    let me_card = resolve_card(&paths, &me)?;

    let Some(target) = resolve_live_or_archived_card(&paths, to)? else {
        let ids = card::query::scan(&paths).unwrap_or_default();
        return match card::suggest::did_you_mean(to, ids.iter().map(|card| card.id.as_str())) {
            Some(near) => Err(anyhow!("no card {to}; did you mean {near}?")),
            None => Err(anyhow!("no card {to} to message")),
        };
    };

    if target.card().card_type == card::schema::CardType::Task {
        reject_task_inbox_endpoint(&paths, &me, &me_card, target.card())?;
    }

    // A send addressed to a live feature card is a broadcast to its members, not
    // a pairwise message: dispatch before the link gate.
    if let CardPresence::Live(target_card) = &target
        && target_card.card_type == card::schema::CardType::Feature
    {
        return send_broadcast(&paths, &me, &me_card, &target_card.id, text);
    }

    if !card::query::pair_linked(&paths, &me_card, to)? {
        if let Some(status) = target.terminal_status() {
            bail!("{to} is finished ({status}); no channel can be opened with a finished card");
        }
        bail!("{me} and {to} are not linked; run `maestro link add {me} {to}` before messaging");
    }

    channel::send(&paths, &me, to, &super::cli_run_id(), text)?;
    println!("sent to {to} (from {me})");
    Ok(())
}

/// Broadcast to a feature channel, gated by membership: the running card must be
/// the feature itself or a card under it. The non-member error names the real
/// join path (create under the feature) -- maestro has no reparent verb, so an
/// existing loose card cannot be moved in.
fn send_broadcast(
    paths: &MaestroPaths,
    me: &str,
    me_card: &card::schema::Card,
    feature_id: &str,
    text: &str,
) -> Result<()> {
    let member =
        card::query::feature_of(me_card).is_some_and(|f| f.eq_ignore_ascii_case(feature_id));
    if !member {
        bail!(
            "{me} is not in feature {feature_id}; only the feature and cards created under it can post here -- create your work with `maestro task create --feature {feature_id}` (maestro has no reparent verb to move an existing card in)"
        );
    }
    channel::send_feature(paths, feature_id, me, &super::cli_run_id(), text)?;
    println!("broadcast to feature {feature_id} (from {me})");
    Ok(())
}

/// `maestro msg read [card]`: print each visible channel's seen context plus all
/// unread (oldest-to-newest), then advance the viewer's cursor past what was
/// shown. No arg aggregates every visible channel; `<card>` scopes to one.
fn read(scope: Option<&str>) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let me = current_card(&paths)?;
    let me_card = resolve_card(&paths, &me)?;
    let channels = visible_channels_union(&paths, &me_card)?;
    let legacy_channels = legacy_task_channels_union(&paths, &me_card)?;
    let selected: Vec<&Channel> = channels
        .iter()
        .filter(|channel| scope.is_none_or(|target| channel.address(&me) == target))
        .collect();
    let selected_legacy: Vec<&LegacyTaskChannel> = legacy_channels
        .iter()
        .filter(|channel| scope.is_none_or(|target| channel.matches_scope(target)))
        .collect();

    if selected.is_empty() && selected_legacy.is_empty() {
        println!("{}", empty_note(scope));
        return Ok(());
    }
    for channel in selected {
        read_channel(&paths, &me, channel)?;
    }
    for channel in selected_legacy {
        read_legacy_task_channel(&paths, &me, channel)?;
    }
    Ok(())
}

/// `maestro msg list [card]`: no arg prints the channel overview (one line per
/// visible partner with its unread count and last activity); `<card>` prints
/// that partner's full timeline oldest-to-newest and marks shown unread seen.
fn list(scope: Option<&str>) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let me = current_card(&paths)?;
    let me_card = resolve_card(&paths, &me)?;
    let mut channels = visible_channels_union(&paths, &me_card)?;
    channels.sort_by(|a, b| a.address(&me).cmp(b.address(&me)));
    let mut legacy_channels = legacy_task_channels_union(&paths, &me_card)?;
    legacy_channels.sort_by(|a, b| {
        a.partner
            .cmp(&b.partner)
            .then_with(|| a.task_id.cmp(&b.task_id))
    });

    match scope {
        None => {
            if channels.is_empty() && legacy_channels.is_empty() {
                println!("{}", empty_note(None));
                return Ok(());
            }
            for channel in &channels {
                let at = cursor(&paths, channel, &me)?;
                let unread = channel.unread(&me, at.as_deref()).len();
                let last_message = channel.messages.last();
                let last = last_message.map_or("-", |message| message.ts.as_str());
                match &channel.kind {
                    ChannelKind::Pair(_) => {
                        let partner = channel
                            .pair_partner(&me)
                            .expect("pair channel has a partner");
                        // Direction of the last message -> whose turn it is to reply.
                        let direction = match last_message {
                            Some(message) if message.from_card == me => "from you",
                            Some(_) => "from them",
                            None => "-",
                        };
                        // The partner's read-through is derived from their newest
                        // cursor across worktrees, omitted when absent.
                        let peer_cursor = cursor(&paths, channel, partner)?;
                        let read_through = match channel.read_through(peer_cursor.as_deref()) {
                            Some(through) => format!("  peer read through {through}"),
                            None => String::new(),
                        };
                        println!(
                            "{partner}  your unread: {unread}{read_through}  last {last} ({direction})"
                        );
                    }
                    // N-party: no single-partner direction or peer-read-through
                    // framing -- a feature thread has many speakers, not one peer.
                    ChannelKind::Feature(feature) => {
                        println!("feature {feature}  your unread: {unread}  last {last}");
                    }
                }
            }
            for channel in &legacy_channels {
                let at = cursor(&paths, &channel.channel, &me)?;
                let unread = channel.channel.unread(&me, at.as_deref()).len();
                let last = channel
                    .channel
                    .messages
                    .last()
                    .map_or("-", |message| message.ts.as_str());
                println!(
                    "legacy task-addressed channel about {} with {}  your unread: {}  last {} (read-only; reply via parent Card {})",
                    channel.task_id, channel.partner, unread, last, me
                );
            }
        }
        Some(target) => {
            let selected: Vec<&Channel> = channels
                .iter()
                .filter(|channel| channel.address(&me) == target)
                .collect();
            let selected_legacy: Vec<&LegacyTaskChannel> = legacy_channels
                .iter()
                .filter(|channel| channel.matches_scope(target))
                .collect();
            if selected.is_empty() && selected_legacy.is_empty() {
                println!("{}", empty_note(scope));
                return Ok(());
            }
            for channel in selected {
                list_channel_timeline(&paths, &me, target, channel)?;
            }
            for channel in selected_legacy {
                list_legacy_task_channel(&paths, &me, channel)?;
            }
        }
    }
    Ok(())
}

/// Print one channel's context + unread block for `read`, then advance the
/// cursor to EOF. Always shows context and a `(no new messages)` line when
/// nothing is unread, so a repeat read still surfaces the recent conversation.
fn read_channel(paths: &MaestroPaths, me: &str, channel: &Channel) -> Result<()> {
    let at = cursor(paths, channel, me)?;
    let unread = channel.unread(me, at.as_deref());
    let seen = channel.seen_context(me, at.as_deref(), CONTEXT_LIMIT);

    println!("{}:", channel.address(me));
    for message in &seen {
        println!(
            "  . {}{}  {}",
            line_speaker(channel, me, message),
            message.ts,
            message.text
        );
    }
    if unread.is_empty() {
        println!("  (no new messages)");
    } else {
        for message in &unread {
            println!(
                "  * {}{}  {}",
                line_speaker(channel, me, message),
                message.ts,
                message.text
            );
        }
        print_task_ordering_hint();
    }
    channel::set_cursor_to_latest(paths, channel, me)?;
    Ok(())
}

fn read_legacy_task_channel(
    paths: &MaestroPaths,
    me: &str,
    channel: &LegacyTaskChannel,
) -> Result<()> {
    let at = cursor(paths, &channel.channel, me)?;
    let unread = channel.channel.unread(me, at.as_deref());
    let seen = channel
        .channel
        .seen_context(me, at.as_deref(), CONTEXT_LIMIT);

    println!(
        "legacy task-addressed channel about {} with {} (read-only; reply via parent Card {}):",
        channel.task_id, channel.partner, me
    );
    for message in &seen {
        println!(
            "  . {}  {}  {}",
            legacy_line_speaker(me, message),
            message.ts,
            message.text
        );
    }
    if unread.is_empty() {
        println!("  (no new messages)");
    } else {
        for message in &unread {
            println!(
                "  * {}  {}  {}",
                legacy_line_speaker(me, message),
                message.ts,
                message.text
            );
        }
        print_task_ordering_hint();
    }
    channel::set_cursor_to_latest(paths, &channel.channel, me)?;
    Ok(())
}

fn list_channel_timeline(
    paths: &MaestroPaths,
    me: &str,
    target: &str,
    channel: &Channel,
) -> Result<()> {
    println!("{target}:");
    for message in &channel.messages {
        // A feature thread labels each non-self line with its real sender; a pair
        // thread labels the partner's lines with the partner id.
        let who: &str = if message.from_card == me {
            "you"
        } else {
            match &channel.kind {
                ChannelKind::Pair(_) => target,
                ChannelKind::Feature(_) => message.from_card.as_str(),
            }
        };
        println!("  {who}  {}  {}", message.ts, message.text);
    }
    if !channel.messages.is_empty() {
        print_task_ordering_hint();
    }
    channel::set_cursor_to_latest(paths, channel, me)?;
    Ok(())
}

fn list_legacy_task_channel(
    paths: &MaestroPaths,
    me: &str,
    channel: &LegacyTaskChannel,
) -> Result<()> {
    println!(
        "legacy task-addressed channel about {} with {} (read-only; reply via parent Card {}):",
        channel.task_id, channel.partner, me
    );
    for message in &channel.channel.messages {
        println!(
            "  {}  {}  {}",
            legacy_line_speaker(me, message),
            message.ts,
            message.text
        );
    }
    if !channel.channel.messages.is_empty() {
        print_task_ordering_hint();
    }
    channel::set_cursor_to_latest(paths, &channel.channel, me)?;
    Ok(())
}

fn legacy_line_speaker<'a>(me: &str, message: &'a Message) -> &'a str {
    if message.from_card == me {
        "you"
    } else {
        message.from_card.as_str()
    }
}

fn print_task_ordering_hint() {
    println!("  {TASK_ORDERING_HINT}");
}

/// The per-line speaker prefix for a read/list line. A pair channel has none (the
/// header already names the partner); a feature channel labels each line with its
/// sender (`you` for the viewer, else the sender card id) so a multi-party thread
/// is never collapsed to a single "from them".
fn line_speaker(channel: &Channel, me: &str, message: &Message) -> String {
    match &channel.kind {
        ChannelKind::Pair(_) => String::new(),
        ChannelKind::Feature(_) => {
            let who = if message.from_card == me {
                "you"
            } else {
                message.from_card.as_str()
            };
            format!("{who}  ")
        }
    }
}

/// The ambient inbox banner: a one-line STDERR summary of unread messages on
/// the running card's visible channels, printed before any command runs so a
/// waiting message is impossible to miss. Silent unless in a repo with a
/// current card that has unread on a still-linked channel
/// (`dec-channel-visibility-hide-on-unlink-6091`). STDERR only, so JSON stdout
/// stays clean. Best-effort: the caller ignores any error.
pub(super) fn inbox_banner() -> Result<()> {
    let Ok(root) = discover_repo_root() else {
        return Ok(());
    };
    let paths = MaestroPaths::new(root);
    let Some(me) = super::current_card(&paths) else {
        return Ok(());
    };
    let Some(me_card) = card::store::resolve(&paths, &me)? else {
        return Ok(());
    };

    let mut counts: Vec<(String, usize)> = Vec::new();
    for channel in visible_channels_union(&paths, &me_card.card)? {
        let at = cursor(&paths, &channel, &me)?;
        let unread = channel.unread(&me, at.as_deref()).len();
        if unread > 0 {
            counts.push((channel.address(&me).to_string(), unread));
        }
    }
    for channel in legacy_task_channels_union(&paths, &me_card.card)? {
        let at = cursor(&paths, &channel.channel, &me)?;
        let unread = channel.channel.unread(&me, at.as_deref()).len();
        if unread > 0 {
            counts.push((
                format!("legacy task {} with {}", channel.task_id, channel.partner),
                unread,
            ));
        }
    }
    if counts.is_empty() {
        return Ok(());
    }
    counts.sort();

    let total: usize = counts.iter().map(|(_, unread)| unread).sum();
    let breakdown = counts
        .iter()
        .map(|(partner, unread)| format!("{unread} {partner}"))
        .collect::<Vec<_>>()
        .join(", ");
    eprintln!("[inbox] {total} new ({breakdown}) -> maestro msg read");
    Ok(())
}

/// Visible channels merged across every worktree of the repo: a peer messaging
/// from a sibling worktree writes its own `.maestro/channels/` file (send is
/// local), so `read`/`list` union those files by ts to show the full thread
/// (`dec-msg-send-local-read-union-cross-worktree`). The link gate is checked
/// against the LOCAL card store -- relatedness is a property of my card here.
fn visible_channels_union(paths: &MaestroPaths, me: &card::schema::Card) -> Result<Vec<Channel>> {
    let roots = worktree_roots(paths);
    let mut visible = Vec::new();
    for channel in channel::channels_for_union(&roots, &me.id)? {
        let Some(partner) = channel.pair_partner(&me.id) else {
            continue;
        };
        if card::query::pair_linked(paths, me, partner)? {
            visible.push(channel);
        }
    }
    // The running card's feature broadcast channel: membership is the live parent
    // edge, checked by construction here (we load only my own feature's channel),
    // so no link is required and a non-member never reaches it.
    if let Some(feature_id) = card::query::feature_of(me)
        && let Some(channel) = channel::load_feature_union(&roots, feature_id)?
    {
        visible.push(channel);
    }
    Ok(visible)
}

struct LegacyTaskChannel {
    task_id: String,
    partner: String,
    channel: Channel,
}

impl LegacyTaskChannel {
    fn matches_scope(&self, target: &str) -> bool {
        self.partner == target || self.task_id == target
    }
}

/// Read-only compatibility for old transitional channels that were addressed to
/// a child Task id. New sends to Task ids are rejected, but a parent Card still
/// needs a recovery surface for history that already exists on disk.
fn legacy_task_channels_union(
    paths: &MaestroPaths,
    me: &card::schema::Card,
) -> Result<Vec<LegacyTaskChannel>> {
    let roots = worktree_roots(paths);
    let mut seen = BTreeSet::new();
    let mut visible = Vec::new();
    let mut task_ids = BTreeSet::new();
    for card in card::query::scan(paths)?
        .into_iter()
        .chain(card::query::scan_dir(&paths.archive_cards_dir())?)
    {
        if card.card_type == card::schema::CardType::Task
            && card.parent.as_deref() == Some(me.id.as_str())
        {
            task_ids.insert(card.id);
        }
    }

    for task_id in task_ids {
        for channel in channel::channels_for_union(&roots, &task_id)? {
            let Some(partner) = channel
                .pair_partner(&task_id)
                .map(std::string::ToString::to_string)
            else {
                continue;
            };
            if !seen.insert((task_id.clone(), channel.key.clone())) {
                continue;
            }
            visible.push(LegacyTaskChannel {
                task_id: task_id.clone(),
                partner,
                channel,
            });
        }
    }

    Ok(visible)
}

fn cursor(paths: &MaestroPaths, channel: &Channel, me: &str) -> Result<Option<String>> {
    channel::cursor_union(&worktree_roots(paths), &channel.key, me)
}

fn empty_note(scope: Option<&str>) -> String {
    match scope {
        Some(target) => format!("no visible channel with {target}"),
        None => "no linked channels".to_string(),
    }
}

/// The running session's current card, or an actionable error naming how to get
/// one (`msg` is meaningless without a card to send from / read for).
fn current_card(paths: &MaestroPaths) -> Result<String> {
    super::current_card(paths).ok_or_else(|| {
        anyhow!(
            "no current card in this session; claim or touch a card first, then run `maestro msg`"
        )
    })
}

fn resolve_card(paths: &MaestroPaths, id: &str) -> Result<card::schema::Card> {
    Ok(card::store::resolve(paths, id)?
        .ok_or_else(|| anyhow!("current card {id} is no longer in the store"))?
        .card)
}

enum CardPresence {
    Live(card::schema::Card),
    Archived(card::schema::Card),
}

impl CardPresence {
    fn card(&self) -> &card::schema::Card {
        match self {
            Self::Live(card) | Self::Archived(card) => card,
        }
    }

    fn terminal_status(&self) -> Option<&str> {
        match self {
            Self::Live(card)
                if card::query::coarse_of(&card.status) == Some(card::query::Coarse::Closed) =>
            {
                Some(card.status.as_str())
            }
            Self::Live(_) => None,
            Self::Archived(card) => Some(card.status.as_str()),
        }
    }
}

/// Resolve `id` from the live store or archive once for `msg send`. A send to a
/// still-linked partner that was archived after linking stays valid
/// (link/unlink gates, not lifecycle); only a genuinely unknown id earns the
/// did-you-mean. Live features dispatch to broadcast before the pair link gate;
/// archived features are terminal cards, not broadcast targets.
fn resolve_live_or_archived_card(paths: &MaestroPaths, id: &str) -> Result<Option<CardPresence>> {
    if let Some(resolved) = card::store::resolve(paths, id)? {
        return Ok(Some(CardPresence::Live(resolved.card)));
    }
    Ok(card::store::resolve_in(&paths.archive_cards_dir(), id)?
        .map(|resolved| CardPresence::Archived(resolved.card)))
}

fn reject_task_inbox_endpoint(
    paths: &MaestroPaths,
    me: &str,
    me_card: &card::schema::Card,
    task: &card::schema::Card,
) -> Result<()> {
    let Some(parent) = task.parent.as_deref() else {
        bail!(
            "Task {} is not a message inbox endpoint.\nNo owning parent Card is recorded; create or choose the Card that owns this work, then message that Card.\nFor execution ordering, use an explicit Task blocker: maestro task block {} --by <dependency-task-id>",
            task.id,
            task.id
        );
    };

    let link_guidance = if card::query::pair_linked(paths, me_card, parent)? {
        String::new()
    } else {
        format!("\nFirst link the Cards:\n  maestro link add {me} {parent}")
    };
    bail!(
        "Task {} is not a message inbox endpoint.\nOwning parent Card: {}{}\nSend advisory coordination to the parent Card:\n  maestro msg send {} <text>\nFor execution ordering, use an explicit Task blocker:\n  maestro task block {} --by <dependency-task-id>",
        task.id,
        parent,
        link_guidance,
        parent,
        task.id
    );
}
