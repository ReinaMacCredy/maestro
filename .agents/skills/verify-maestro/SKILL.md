---
name: verify-maestro
description: Verify a change in the maestro repository on the running product - install the checkout into a scratch HOME, doctor it, drive the CLI the way a user does, and capture the transcript as proof. Use before claiming a maestro change reached the installed, live, or journey layer.
---

# verify-maestro

Built from `maestro recipe show verification-skill`. Proves a change on an
installed runtime without touching the owner's `~/.maestro` or `~/.local/bin`.

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
- playbooks: 25 playbook and method recipes listed, bug-fix's steps shown,
  the retired `work` recipe refused; `~/maestro/PRINCIPLES.md` has 24
  sections; nine skills; the ten new agent renders; no PreToolUse hook.
- brief and grant: the SessionStart `method:` line routes by playbook; the
  `delivery grant:` line appears only while `policy-auto-pr` is enabled.
- bundle check: a fresh bundle's U1 skeleton is reported and exits 1.
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
| recipe | recipe catalog, playbooks | new recipe listed and shown; playbooks listed, `work` refused |
| PRINCIPLES.md, skills list, profiles, Claude hooks | playbooks | 24 sections, nine skills, ten renders, no PreToolUse |
| brief, policy-auto-pr | brief and grant | `method:` line; grant line only while enabled |
| plugin + policy gates | policy gate | blocked then passing `work done` |
| work lifecycle (add, start, note, done, show) | policy gate (partial) | `w1 done` |
| ready, status, trace, search | GAP | |
| bundle check | bundle check | U1 skeleton reported, exit 1 |
| bundle open/close, handoff | GAP | |
| decision, decide, term, lesson | GAP | |
| dispatch, handback | GAP | |
| memory, room, owner (Hub) | GAP | |
| topic, prompt, hook, brief, attention | GAP | |
| team, slp (Herdr panes) | GAP: needs a live herdr room | |
| graph, mcp | GAP | |
