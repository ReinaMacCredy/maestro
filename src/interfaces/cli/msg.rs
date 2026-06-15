//! `maestro msg`: pull-only messaging on a linked-card channel. `send` is
//! link-gated and validated; `read` consumes unread (advancing the viewer's
//! cursor); `list` shows the channel overview or one partner's full timeline.
//! The sender is always the running session's current card -- the card it last
//! touched -- never a named argument, so messaging rides on normal work.
//!
//! Visibility is the live link: a channel shows in `read`/`list` (and the
//! banner) only while the pair currently shares a `related` edge. Unlinking
//! hides it without deleting; relinking restores history and prior unread
//! (`dec-channel-visibility-hide-on-unlink-6091`).

use anyhow::{Result, anyhow, bail};

use crate::domain::card;
use crate::domain::channel::{self, Channel};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{MsgArgs, MsgCommand};

/// How many already-seen partner messages a read prints as context above the
/// unread block (`dec-msg-verbs-read-model-send-link-gated-52f6`).
const CONTEXT_LIMIT: usize = 5;

pub fn run(args: MsgArgs) -> Result<()> {
    match args.command {
        MsgCommand::Send { to, text } => send(&to, &text),
        MsgCommand::Read { card } => read(card.as_deref()),
        MsgCommand::List { card } => list(card.as_deref()),
    }
}

/// `maestro msg send <to> <text>`: append a message to the channel between the
/// running card and `<to>`. Link-gated (bidirectional, archive-aware) and
/// validated; emits no card_touch and no run event -- a send is not work state.
fn send(to: &str, text: &str) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let me = current_card(&paths)?;
    let me_card = resolve_card(&paths, &me)?;

    if !card_exists(&paths, to)? {
        let ids = card::query::scan(&paths).unwrap_or_default();
        return match card::suggest::did_you_mean(to, ids.iter().map(|card| card.id.as_str())) {
            Some(near) => Err(anyhow!("no card {to}; did you mean {near}?")),
            None => Err(anyhow!("no card {to} to message")),
        };
    }
    if !card::query::pair_linked(&paths, &me_card, to)? {
        if let Some(status) = partner_terminal_status(&paths, to)? {
            bail!("{to} is finished ({status}); no channel can be opened with a finished card");
        }
        bail!("{me} and {to} are not linked; run `maestro link add {me} {to}` before messaging");
    }

    channel::send(&paths, &me, to, &super::cli_run_id(), text)?;
    println!("sent to {to} (from {me})");
    Ok(())
}

/// `maestro msg read [card]`: print each visible channel's seen context plus all
/// unread (oldest-to-newest), then advance the viewer's cursor past what was
/// shown. No arg aggregates every visible channel; `<card>` scopes to one.
fn read(scope: Option<&str>) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    let me = current_card(&paths)?;
    let me_card = resolve_card(&paths, &me)?;
    let channels = visible_channels(&paths, &me_card)?;
    let selected: Vec<&Channel> = channels
        .iter()
        .filter(|channel| scope.is_none_or(|target| channel.partner(&me) == target))
        .collect();

    if selected.is_empty() {
        println!("{}", empty_note(scope));
        return Ok(());
    }
    for channel in selected {
        read_channel(&paths, &me, channel)?;
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
    let mut channels = visible_channels(&paths, &me_card)?;
    channels.sort_by(|a, b| a.partner(&me).cmp(b.partner(&me)));

    match scope {
        None => {
            if channels.is_empty() {
                println!("{}", empty_note(None));
                return Ok(());
            }
            for channel in &channels {
                let partner = channel.partner(&me);
                let unread = channel.unread(&me, cursor(&paths, channel, &me)?).len();
                let last_message = channel.messages.last();
                let last = last_message.map_or("-", |message| message.ts.as_str());
                // Direction of the last message -> whose turn it is to reply.
                let direction = match last_message {
                    Some(message) if message.from_card == me => "from you",
                    Some(_) => "from them",
                    None => "-",
                };
                // The partner's read-through is derived from their stored cursor,
                // omitted when absent (peer hasn't read / cross-machine no cursor).
                let peer_cursor = channel::cursor(&paths, &channel.key, partner)?;
                let read_through = match channel.read_through(peer_cursor) {
                    Some(through) => format!("  peer read through {through}"),
                    None => String::new(),
                };
                println!(
                    "{partner}  your unread: {unread}{read_through}  last {last} ({direction})"
                );
            }
        }
        Some(target) => {
            let Some(channel) = channels.iter().find(|c| c.partner(&me) == target) else {
                println!("{}", empty_note(scope));
                return Ok(());
            };
            println!("{target}:");
            for message in &channel.messages {
                let who = if message.from_card == me {
                    "you"
                } else {
                    target
                };
                println!("  {who}  {}  {}", message.ts, message.text);
            }
            channel::set_cursor(&paths, &channel.key, &me, channel.len)?;
        }
    }
    Ok(())
}

/// Print one channel's context + unread block for `read`, then advance the
/// cursor to EOF. Always shows context and a `(no new messages)` line when
/// nothing is unread, so a repeat read still surfaces the recent conversation.
fn read_channel(paths: &MaestroPaths, me: &str, channel: &Channel) -> Result<()> {
    let at = cursor(paths, channel, me)?;
    let unread = channel.unread(me, at);
    let seen = channel.seen_context(me, at, CONTEXT_LIMIT);

    println!("{}:", channel.partner(me));
    for message in &seen {
        println!("  . {}  {}", message.ts, message.text);
    }
    if unread.is_empty() {
        println!("  (no new messages)");
    } else {
        for message in &unread {
            println!("  * {}  {}", message.ts, message.text);
        }
    }
    channel::set_cursor(paths, &channel.key, me, channel.len)?;
    Ok(())
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
    if !paths.channels_dir().exists() {
        return Ok(());
    }
    let Some(me) = super::current_card(&paths) else {
        return Ok(());
    };
    let Some(me_card) = card::store::resolve(&paths, &me)? else {
        return Ok(());
    };

    let mut counts: Vec<(String, usize)> = Vec::new();
    for channel in visible_channels(&paths, &me_card.card)? {
        let unread = channel.unread(&me, cursor(&paths, &channel, &me)?).len();
        if unread > 0 {
            counts.push((channel.partner(&me).to_string(), unread));
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

/// Every channel `me` participates in that is currently visible: enumerated from
/// `.maestro/channels/` headers, kept only while the pair is still linked
/// (archive-aware, so a boxed partner stays visible until `link remove`).
fn visible_channels(paths: &MaestroPaths, me: &card::schema::Card) -> Result<Vec<Channel>> {
    let mut visible = Vec::new();
    for channel in channel::channels_for(paths, &me.id)? {
        let partner = channel.partner(&me.id).to_string();
        if card::query::pair_linked(paths, me, &partner)? {
            visible.push(channel);
        }
    }
    Ok(visible)
}

fn cursor(paths: &MaestroPaths, channel: &Channel, me: &str) -> Result<u64> {
    channel::cursor(paths, &channel.key, me)
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

/// Whether `id` names a real card, live OR archived. A send to a still-linked
/// partner that was archived after linking stays valid (link/unlink gates, not
/// lifecycle); only a genuinely unknown id earns the did-you-mean.
fn card_exists(paths: &MaestroPaths, id: &str) -> Result<bool> {
    if card::store::resolve(paths, id)?.is_some() {
        return Ok(true);
    }
    Ok(card::store::resolve_in(&paths.archive_cards_dir(), id)?.is_some())
}

/// The partner's status word when it is terminal -- coarse-Closed in the live
/// store, or present only in the archive (archived implies done). Drives msg
/// send's honest dead-end for a finished partner instead of pointing at a
/// `link add` the guard would refuse (`dec-terminal-card-link-msg-keep-the-live-5878`).
/// Only reached on the not-linked branch; an already-linked terminal partner
/// passes `pair_linked` and keeps messaging.
fn partner_terminal_status(paths: &MaestroPaths, id: &str) -> Result<Option<String>> {
    if let Some(resolved) = card::store::resolve(paths, id)? {
        if card::query::coarse_of(&resolved.card.status) == Some(card::query::Coarse::Closed) {
            return Ok(Some(resolved.card.status));
        }
        return Ok(None);
    }
    Ok(card::store::resolve_in(&paths.archive_cards_dir(), id)?
        .map(|resolved| resolved.card.status))
}
