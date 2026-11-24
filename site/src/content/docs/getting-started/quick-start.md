---
title: Quick start
description: Install Maestro and complete one real work item with a check Maestro runs.
---

## 1. Install Maestro

In the repository you want Maestro to wire and register, run:

```sh
curl -fsSL https://maestro.maccredyreina.me/install.sh | sh
```

## 2. See what is open

```sh
maestro work list
```

Work list prints open and active work items with their holder, blockers and
gates, then one line that counts done and cancelled items. In a new
repository it is empty.

## 3. Add a task and claim it

```sh
maestro work add "Document the install path" --acceptance "README links to the installer" --start
```

The command prints the new work ID and claims the item for your session. In a
fresh repository the ID is `w1`; use the ID printed in your repository in the
next commands. Without `--start` the item is created open with no holder, and
`maestro work start <id>` claims it later.

Make the change.

## 4. Close it with a check

```sh
maestro work done w1 --check "rg -n 'install.sh' README.md"
```

Maestro runs the command in the repository root and records its exit code,
duration, commit, and whether the tree was clean. A failing check is recorded
and the item still closes, so read the result line it prints. Read the record
back with:

```sh
maestro work show w1
```

`--evidence "<text>"` closes the item with plain text instead, recorded as
`proof kind: text`. The `policy-proof` gate, on by default, refuses
`work done` with no check, evidence, or paired `--claim` and `--proof`.

## 5. Record a decision

```sh
maestro decision add "Link the installer from the README" --rationale "first thing a new reader looks for" --work w1
```

`decision add` records the choice and locks it in one command.

## Run a team instead

This page shows Maestro in one session. To run a Lead and Peers in their own
Herdr panes, enable the SLP plugin and continue with
[SLP setup and storage](/getting-started/slp-setup/).
