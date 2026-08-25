# ADR-0006: Opt-in supervisor daemon for attention coverage

Date: 2026-08-25
Status: accepted

## Context

Maestro has been passive by rule since 2026-05: CLI verbs only, no cron, no
daemon, no background process; scheduling stays external. Hook-first delivery
(ADR-0004) covers every case where an agent produces an event: the brief and
the mailbox ride SessionStart and UserPromptSubmit.

The SLP orchestration doctrine ("Orchestrating AI Teams", Paseo Foundation)
names the case hooks cannot see: an agent that hangs, or a session that holds
a lease and goes silent, produces no event at all. An event-only watcher is
blind exactly when a Supervisor is needed. The doctrine's answer is a
heartbeat/deadline timer. The owner wants that layer to live in maestro
itself rather than in Paseo, because maestro must work standalone.

## Decision

Open the no-daemon rule for exactly one component, opt-in and bounded:

- `maestro supervisor start|stop|status`: a per-store daemon that runs the
  same detector scan `maestro attention` runs, on a timer, and delivers
  attention packets through the existing mailbox (`msg`) so they ride the
  hook channel. Pid file + provenance readback; `status` reports a killed
  daemon as stale, never as running.
- `maestro attention` stays a pure synchronous verb that works with no
  daemon; the daemon adds only the timer.
- PostToolUse hook wiring so mailbox deliveries surface between tool calls,
  not only at the next prompt.
- The supervisor never mutates work, decisions, or leases. It observes and
  asks (intervention ladder levels 1-2); judgment stays with the lease
  holder.

Never auto-started, never auto-restarted, never installed as a login item.
Stopping it returns maestro to the passive baseline.

## Rejected

- Hooks only: cannot observe silence; the hung-agent case is the reason for
  the layer.
- Riding Paseo's daemon (heartbeats, schedules): correct mechanism, wrong
  dependency; maestro must stand alone.
- Maestro spawning or restarting agents (T3): rebuilds an orchestrator core
  and competes with the harness's own agent tools.
- A daemon with mutating verbs (freeze, reassign): authority theater on a
  local CLI; leases remain the only authority mechanism.

## Consequences

- One long-running process per store, visible in `supervisor status` and
  killable with `supervisor stop`; the pid file is the only state.
- Detector thresholds are flags with defaults, not hidden constants.
- The memory entry that recorded the no-daemon rule is partially reversed in
  exactly this scope; everything outside it keeps the old rule.
