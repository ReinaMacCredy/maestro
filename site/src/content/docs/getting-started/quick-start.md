---
title: Quick start
description: Install Maestro and complete one real work item with a claim and proof.
---

## 1. Install Maestro

In the repository you want Maestro to wire and register, run:

```sh
curl -fsSL https://maestro.maccredyreina.me/install.sh | sh
```

## 2. Read the current session

```sh
maestro status
```

Status shows live sessions and dead sessions that still hold work or an open
dispatch. Use `maestro status --all` to include every recorded dead session.

## 3. Record one bounded task

```sh
maestro work add "Document the install path" --kind task --atomic-reason "One bounded documentation change" --acceptance "README links to the installer"
```

The command prints the new work ID. In a fresh repository it is `w1`; use the
ID printed in your repository in the next commands.

## 4. Take the lease

```sh
maestro work start w1
```

Make the accepted change and run the check that could falsify it.

## 5. Complete with paired evidence

```sh
maestro work done w1 --claim "docs: README links to the installer" --proof "source: rg -n 'install.sh' README.md"
```

The claim states the observable result. The proof names the evidence layer and
the falsifier. Enabled development policies can require a pair, a child
breakdown, or an independent acceptance step before completion.

## 6. Read what is next

```sh
maestro ready
```

Ready lists work that can start and names the gates blocking other items.

## Start a supervised team instead

This page demonstrates Maestro's development workflow in one session. To open
an SLP v2 team with Team Supervisor, Lead and Peers, continue with
[SLP setup and storage](/getting-started/slp-setup/).
