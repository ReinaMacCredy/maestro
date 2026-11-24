---
title: Recipes, skills, and plugins
description: Read prompt-first methods and turn built-in plugins on or off.
---

## Recipes

List every recipe shipped by the installed runtime:

```sh
maestro recipe list
```

Read one without copying it into the repository:

```sh
maestro recipe show bug-fix
```

Most recipes are playbooks: the numbered steps for one kind of task.
`~/maestro/WORKFLOW.md` names the playbook that fits each kind of task
(investigation, bug-fix, feature, refactoring, prototype, perf-issue,
hillclimb, forensics, eval, authoring-a-skill, visual-parity,
multi-phase-plan, opening-a-pr, babysit, shipping, figure-it-out,
no-comments, review). A playbook is guidance to read when one fits. The other
recipes are `verification-skill`, `unattended`, `worktree`, `slp`, and the
`style-<language>` guides. Recipes are source-owned Markdown and are loaded
only when requested. The playbooks and `PRINCIPLES.md` are ported from pstack
(Lauren Tan, MIT).

The `review` recipe has one reviewer check a frozen diff over several rounds
against the repository's `REVIEW.md`, which names the rounds, the tags
`important`, `nit` and `pre-existing`, and a reliability checklist.

SLP seat behavior is not assembled from recipes or skills. Its contract is
`~/maestro/SLP.md`, pinned into the project at team start.

## Skills

Install writes seven managed skills under `~/maestro/skills/` and the shared
method at `~/maestro/WORKFLOW.md`, with `PRINCIPLES.md` and `ANTI_PATTERN.md`
beside it:

- `maestro-bundle` chooses direct work or a durable SPEC/NOTES/VERIFY bundle.
- `maestro-design` settles one fork at a time and records locked decisions.
- `maestro-work` holds the contract every playbook shares: the brief a
  writer receives, its writing rules, what it hands back, and the store
  record.
- `maestro-verify` checks evidence layers and controls close and delivery
  gates.
- `maestro-council` is a short procedure a leading session (a team's Lead, or
  a plain session outside a team) runs by hand with ordinary sub-agents on a
  hard-to-reverse fork. It records one binding verdict with its dissent.
- `maestro-coach` re-pitches a fork the user did not follow, or teaches the
  concept behind a recorded decision.
- `maestro-questionnaire` turns a decision someone else owns into a Markdown
  questionnaire for that person.

The installer links these skills for Claude and Codex without overwriting a
skill Maestro does not manage. Delegation uses the harness's own agent types
with a written brief and the model named in the call; Maestro ships no agent
definitions of its own.

## Plugins

```sh
maestro plugin list
maestro plugin enable <name>
maestro plugin disable <name>
```

Every plugin ships with Maestro. `plugin list` shows each one and whether it
is active. The policy plugins add gates:

| Plugin | Default | What it does |
| --- | --- | --- |
| `policy-proof` | on | `work done` needs `--check`, `--evidence`, or paired `--claim` and `--proof`. |
| `policy-breakdown` | on | A card whose write-like children are still open cannot be started or finished. |
| `policy-card-budget` | off | `work add` is refused while the store holds a set number (default 3) of open cards with no live holder. |
| `policy-auto-pr` | off | Grants delivery: the deliver step commits, pushes the branch and opens a PR. Merge, tag and release stay the owner's. |

SLP, the team layer, is the plugin `slp` and is also off by default. Enabling
it adds the team verbs and links the Hub room to Herdr as the plugin
`maestro`. `maestro install` renders `~/maestro/herdr-plugin.toml`, which
declares the team's `runtime` pane and the `slp restore` startup hook, and
links it over the Herdr socket; `maestro uninstall` removes the link and the
file. See [SLP setup and storage](/getting-started/slp-setup/).

Use `maestro plugin enable <name>` or `maestro plugin disable <name>` rather
than editing `.maestro/config` by hand, so the plugin owns its setup and
teardown.

## Repository configuration

`.maestro/config` is JSON. Each entry names a plugin and sets its disabled
state. `maestro install` writes the two default gates:

```json
{
  "plugins": [
    { "name": "policy-proof", "disabled": false },
    { "name": "policy-breakdown", "disabled": false }
  ]
}
```
