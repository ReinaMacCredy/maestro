---
title: Install
description: Install Maestro from source, verify it, update it, or remove repository wiring.
---

## Install with one command

Run this on a machine with Git and Bun:

```sh
curl -fsSL https://maestro.maccredyreina.me/install.sh | sh
```

The script clones the `main` branch into `~/.maestro/source`, runs the installer
from that checkout, and leaves the source record that future updates follow.

### Choose a source directory or branch

Set `MAESTRO_SOURCE_DIR` to replace the default source checkout and
`MAESTRO_REF` to replace the default `main` branch. The installer refuses an
existing source directory that is not a Git checkout.

## Install from a checkout

From a Maestro checkout, run:

```sh
bun bin/maestro.ts install
```

Installation copies the runtime into `~/.maestro/runtime`, writes the shim at
`~/.local/bin/maestro`, records the source in `~/.maestro/source.json`, and
wires the current repository. It also creates the Hub room at `~/maestro`: a
folder with the shared method `WORKFLOW.md`, the registry of repositories that
`maestro brief` reads, and the seven managed skills under `~/maestro/skills`.

## Verify

Read the installed version and run the read-only diagnostic:

```sh
maestro version
maestro doctor
```

`maestro doctor` checks the shim, runtime stamp, recorded source, repository
wiring, permissions, and store access. It exits zero when the report is
healthy and names the repair command when the fix is mechanical.

## Update

```sh
maestro update
```

Update fast-forwards the recorded source checkout and resynchronizes the
runtime. It refuses a dirty, diverged, missing, detached, or unreachable source
without partially updating the runtime.

## Remove repository wiring

```sh
maestro uninstall
```

Uninstall removes Maestro's hooks and settings keys from the current
repository, Maestro's entries in `.maestro/config`, the Claude seat config
dirs under `~/.maestro/claude/`, and the Herdr plugin link. It does not delete
repository data, the machine runtime, the shim, or the Hub room. To drop a
repository from the Hub registry only, run `maestro room forget <path>`.

## Install Herdr for teams

SLP, the team layer, is an optional plugin that is off by default. You need it
only to run a Lead and Peers in their own panes. Its seats and its runtime
pane need Herdr 0.8.2 or newer. Install Herdr separately:

```sh
curl -fsSL https://herdr.dev/install.sh | sh
```

Maestro remains the durable record. Herdr owns the workspace, the panes,
agent launch, prompting, and pane closure. Continue with
[SLP setup and storage](/getting-started/slp-setup/) to enable the plugin and
start a team.
