---
name: verify-maestro
description: Verify a change in the maestro repository on the running product - install the checkout into a scratch HOME, doctor it, drive the CLI the way a user does, and capture the transcript as proof. Use before claiming a maestro change reached the installed, live, or journey layer.
---

# verify-maestro

Built from `maestro recipe show verify-skill`. Proves a change on an installed
runtime without touching the owner's `~/.maestro` or `~/.local/bin`.

## Launch

`scripts/journey.sh <checkout>` creates `/tmp/maestro-verify.XXXX` with its own
`home/` and an empty git repo, then runs `bun <checkout>/bin/maestro.ts install`
under `env -i` with that HOME and a PATH of the scratch shim dir, bun, and
`/usr/bin:/bin`. `MAESTRO_SESSION_ID`/`MAESTRO_SESSION_PID` are pinned so every
call is one session; without them each call is a new anonymous session and
`work done` fails with `LEASE_HELD`.

## Doctor

`maestro version` must print the checkout's commit, and `maestro doctor` must
report `healthy`, `source: ok <checkout>`, and the shim pointing into the
scratch HOME. A missing seat token and unverified Codex hooks are expected in
the scratch HOME.

## Drive

Add one block to `scripts/journey.sh` per journey the change touches, invoking
`maestro` exactly as a user types it. The script already drives:

- recipe catalog: `recipe list` shows the recipe, `recipe show` prints it.
- a policy gate: `plugin enable`, `work add|start|done` blocked, then passing.
- reinstall: a second install keeps the repository's policy choices.

## Capture

Run `scripts/journey.sh <checkout> > "$(maestro topic path verify-maestro)/journey-<n>.txt"`.
Each step prints its command output and `[exit N]`; read the exit codes, not
only the text. Cite the transcript path and the checkout commit in the proof,
tagged `(journey)`.

## Clean up

The script removes its scratch root last. The transcript in the topic folder
is the proof and stays.

## Feature map

| Feature | Journey | Observable outcome |
|---|---|---|
| install / update / uninstall | install, reinstall | doctor healthy; `.maestro/config` keeps enables |
| doctor, version | doctor | commit and source match the checkout |
| recipe | recipe catalog | new recipe listed and shown |
| plugin + policy gates | policy gate | blocked then passing `work done` |
| work lifecycle (add, start, note, done, show) | policy gate (partial) | `w1 done` |
| ready, status, trace, search | GAP | |
| bundle, handoff | GAP | |
| decision, decide, term, lesson | GAP | |
| dispatch, handback | GAP | |
| memory, room, owner (Hub) | GAP | |
| topic, prompt, hook, brief, attention | GAP | |
| team, slp (Herdr panes) | GAP: needs a live herdr room | |
| graph, mcp | GAP | |
