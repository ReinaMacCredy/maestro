---
title: Recipes, skills, and plugins
description: Load prompt-first methods and manage repository policy plugins.
---

## Recipes

List every recipe shipped by the installed runtime:

```sh
maestro recipe list
```

Read one without copying it into the repository:

```sh
maestro recipe show slp
```

The current runtime includes design, work, audit, ship, unattended, learning,
worktree, conflict-handoff, SLP, and language/style recipes. Recipes are
source-owned Markdown and are loaded only when requested.

## Skills

Install materializes four managed method skills under `~/maestro/skills/`:

- `maestro-bundle` chooses direct work or a durable SPEC/NOTES/VERIFY bundle.
- `maestro-design` settles one fork at a time and records locked decisions.
- `maestro-work` drives one accepted implementation unit through proof.
- `maestro-verify` checks evidence layers and controls close and delivery gates.

The installer links these skills for Claude without overwriting an unmanaged
skill. Coordination remains in the Supervisor room's lane instructions rather
than a fifth global skill.

## Plugins

```sh
maestro plugin list
```

Plugins can be built in, global, or repository-local. The `plugin` verb can add,
create, enable, disable, or remove managed plugins. Policies remain plugins so
their gates can be removed without changing the mechanism kernel.

## Repository configuration

`.maestro/config` is JSON. Each entry names a plugin and sets its disabled
state:

```json
{
  "plugins": [
    { "name": "policy-proof", "disabled": false },
    { "name": "policy-breakdown", "disabled": false },
    { "name": "policy-tdd", "disabled": true },
    { "name": "policy-qa", "disabled": true },
    { "name": "policy-research", "disabled": true },
    { "name": "policy-witness", "disabled": true },
    { "name": "policy-lifecycle", "disabled": true }
  ]
}
```

Use `maestro plugin enable <name>` or `maestro plugin disable <name>` so the
plugin owns setup and teardown rather than editing generated effects by hand.
