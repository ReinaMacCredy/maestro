---
title: Attention and brief
description: Find what needs a person in one repository or across every registered repository.
---

`attention` reads one repository's store and `brief` reads every registered
repository. Both compute their answer when you call them; neither runs in the
background, opens a pane, prompts an agent, or changes work.

## Attention

```sh
maestro attention
maestro attention --json
```

Attention reports three kinds of finding:

| Kind | When it appears |
| --- | --- |
| `HUMAN_DECISION_REQUIRED` | A draft decision marked `--needs-owner` is linked to an unfinished work item. |
| `LESSONS_PENDING` | Lessons filed with `maestro lesson file` have not been processed. |
| `REPEATED_FAILURE` | An unfinished work item has three or more notes starting `failed: ` since its latest start. |

Each finding names the smallest next command, such as
`maestro decision show <id>`. When there is nothing to report, attention says
`no attention findings`. The SessionStart brief carries the same findings.

## Brief

```sh
maestro brief
maestro brief --session
```

`maestro brief` reads the registry in `~/maestro/registry`, opens each
repository's store without changing it, and reports only what needs
attention. A missing repository is named and skipped. When every repository
is fine, the brief says so in one line. `maestro install` registers the
repository it runs in, and `maestro room forget <path>` removes one.

`maestro brief --session` prints this session's SessionStart brief, the text
the hooks and `maestro mcp serve` deliver.

## Teams

Inside an SLP team, attention is raised by the seats and the team's runtime
pane, not by these verbs. A seat records `maestro work note <id> "<what you
need>" --blocked`, which Maestro pushes one seat up, and the runtime pane
nudges a seat that sits idle while it holds work. See
[Attention](/guides/supervised-teams/#attention).
