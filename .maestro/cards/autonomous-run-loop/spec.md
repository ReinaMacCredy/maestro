# autonomous run loop

## Current state

Driver surface today (verified 2026-06-12): maestro ready --json (maestro.ready.v1), maestro list --json (maestro.list.v1), maestro status --json (maestro.status.v1) all work; maestro feature list --json does NOT exist (clap error). maestro resume prints a clean-session packet; maestro watch renders task snapshots on change (passive observer, not a driver). Work verbs an external driver would call: claim, task update/complete, verify, note, close.

Prior art and constraints: (1) Standing decision 2026-04-08 - maestro stays passive, no cron/daemon/background; scheduling lives outside (external cron, hooks, agent skills). (2) Symphony eval 2026-06-11 - no build; 'daemon locked out'; parked thread was maestro-as-tracker for an external driver, blocked then on missing --json (since shipped). (3) Agent-side loop primitives already exist outside maestro: Claude Code /loop + ScheduleWakeup self-pacing, /schedule cloud routines, cron/launchd on macOS, goal-loop skill. (4) Human-gated steps in the lifecycle: feature accept freezes the contract, ship/push/tag/publish require approval per user policy.

## Problem

