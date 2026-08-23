# ADR-0004: Hook-first delivery for guidance and messages

Date: 2026-08-23
Status: accepted

## Context

Two delivery problems share one shape. (1) Guidance: the Rust maestro syncs a
~200-line HARNESS.md protocol into every repo, @-included by CLAUDE.md and
AGENTS.md — static, token-heavy every session, drifts with every behavior
change. (2) Messages: the Rust inbox is advisory and polled, and the recorded
lesson is that agents ignore it. Claude Code's native SendMessage shows the
model that works: messages land inside the receiving session's context instead
of waiting in a box — but SendMessage is Claude-only transport, and the owner
runs Claude, Codex, and Cursor.

## Decision

Adopt the native model, own the transport. The store is the neutral carrier;
harness hooks are the injection point:

- A dynamic session brief, generated from live state (held work, enabled
  policies, pending message count, next verb), injected at SessionStart via
  each harness's hook. Plugins contribute brief sections through effects.
  Emission is `hook record` stdout — harnesses inject hook stdout into
  context — and rides every wired hook event: SessionStart for the brief,
  the turn-level hook (UserPromptSubmit on Claude) so a message sent to a
  running session surfaces at its next turn, not its next session.
- Messages: a mailbox table in the shared store with a per-session read
  cursor; `msg send/read` verbs; pending messages surface through the same
  hook injection, so they arrive in-context rather than waiting to be polled.
- The repo mirror (CLAUDE.md/AGENTS.md) shrinks to a few static pointer lines
  written at install and essentially never re-synced.
- Recipes are pulled on demand (`recipe show <name>`), never pre-injected.
- Claude<->Claude native SendMessage bridging is an optional later plugin.

## Rejected

- Thick mirror files as the guidance channel: static, drifting, token-charged
  every session for text describing state the store already knows.
- Native SendMessage as the only transport: locks maestro to one harness.
- Advisory polled inbox: empirically ignored; if order matters it must be a
  blocker on work, and if attention matters it must be injected.

## Consequences

- Install must wire hooks per harness (Claude, Codex adapters; harnesses
  without hooks fall back to the thin mirror pointing at `maestro status`).
- The brief has a token budget by design — it lists state and pointers, never
  protocol prose.
- Behavior changes ship in plugins/binary, not via re-syncing repo files.
