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
- the proof gate and the check: `plugin enable policy-proof`; `work add --start`
  creates and claims a card that `work list` shows as held; `work done` without
  proof is refused; `work done --check "true"` closes it and `work show` prints
  `proof kind: check` and the `check:` line; a second card closed with
  `--evidence` prints `proof kind: text`.
- decisions: `decision add --rationale --work` records and locks in one
  command, and `decision list` shows it.
- playbooks: 18 playbook recipes listed, bug-fix's steps shown, the unknown
  `work` recipe refused; `~/maestro/PRINCIPLES.md` has 24 sections; seven
  skills; no `maestro-*` agent files; no PreToolUse hook.
- brief and grant: the SessionStart brief opens with the `maestro:` line; the
  `delivery grant:` line appears only while `policy-auto-pr` is enabled.
- bundles: `bundle check` passes a fresh bundle and `bundle check --final`
  exits 1 on its empty VERIFY result; `work done` on the bundle's only card
  archives it, and `bundle list --all` shows it archived.
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
| PRINCIPLES.md, skills list, agent files, Claude hooks | playbooks | 24 sections, seven skills, zero `maestro-*` agent files, no PreToolUse |
| brief, policy-auto-pr | brief and grant | `maestro:` line; grant line only while enabled |
| plugin + policy gates | proof gate | blocked then passing `work done` |
| work lifecycle (add --start, list, done --check, done --evidence, show) | proof gate | `proof kind: check` and `proof kind: text` |
| decision add, decision list | decisions | `d1 [locked]` |
| bundle open, check, auto-archive | bundles | check exit 0, `--final` exit 1, `probe [archived]` |
| search, attention, handoff, bundle close | GAP | |
| lesson, term, topic, hook, mcp | GAP | |
| memory, room (Hub) | GAP | |
| SLP plugin: team, slp, decide, status (Herdr panes) | GAP: needs a live herdr room | |
