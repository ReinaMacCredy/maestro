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
maestro recipe show work
```

The current runtime includes design, work, audit, ship, unattended, learning,
worktree, conflict-handoff, and language/style recipes. Recipes are
source-owned Markdown and are loaded only when requested.

SLP role behavior is not assembled from recipes or a skills tree. Its
canonical contract is `~/maestro/SLP.md`, pinned into the project at team
start.

## Skills

Install materializes ten managed method skills under `~/maestro/skills/`
and the shared method at `~/maestro/WORKFLOW.md`:

- `maestro-bundle` chooses direct work or a durable SPEC/NOTES/VERIFY bundle.
- `maestro-design` settles one fork at a time and records locked decisions.
- `maestro-work` drives one accepted implementation unit through proof.
- `maestro-verify` checks evidence layers and controls close and delivery gates.
- `maestro-improve` turns filed lessons into the smallest doctrine edit; the
  loop around it is in [Self-improvement](/guides/self-improvement/).
- `maestro-explore` answers an evidence question read-only: research,
  disposable prototype, or behavior baseline.
- `maestro-diagnose` finds a failure's cause without changing anything.
- `maestro-coach` re-pitches a fork the user did not follow, or teaches the
  concept behind a recorded decision.
- `maestro-questionnaire` turns a decision someone else owns into a Markdown
  questionnaire for that person.
- `maestro-council` runs a Lead-only council on a hard-to-reverse fork and
  records one binding verdict with its dissent.

The installer links these skills for Claude without overwriting an unmanaged
skill. Team coordination comes from the pinned Workspace Pack and direct role
topology rather than a coordination skill.

## Plugins

```sh
maestro plugin list
```

Plugins can be built in, global, or repository-local. The `plugin` verb can add,
create, trust, enable, disable, or remove managed plugins. Policies remain
plugins so their gates can be removed without changing the mechanism kernel.

### Trust

Built-in plugins ship with Maestro and always load. A global or
repository-local plugin is code that arrived from somewhere else, so it executes
only after you vouch for it:

```sh
maestro plugin list          # untrusted plugins are named, never imported
maestro plugin trust <name>  # after reading the source it names
```

The grant lives in `~/.maestro/trust.json`, which no repository can write, and
is keyed to the plugin's location and a digest of every file in it. Editing the
plugin, pulling a change into it, or swapping one of its files revokes the grant
and Maestro stops loading it until you trust it again. So that the digest covers
everything the plugin runs, a trusted plugin may only import local paths the
digest covers; an import reaching outside it is refused and the plugin is listed
as an error instead of loading. Enabling a plugin is a
separate statement and never confers trust: a cloned repository can ship its own
`.maestro/config`, so if `enable` could vouch for code, the repository could
vouch for itself.

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
