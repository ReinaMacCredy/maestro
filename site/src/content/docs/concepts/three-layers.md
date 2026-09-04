---
title: Three layers
description: How Maestro separates mechanism, removable policy, and prompt-first method.
---

Maestro separates durable mechanism from workflow policy and working method.
That boundary keeps the kernel small while allowing repositories to choose
their gates and agents to load deeper guidance only when needed.

## Kernel

The mechanism kernel owns the SQLite store, event log, sessions, CLI routing,
plugin loading, and readiness projection. It contains mechanism only: work and
decision records, leases, events, and plugin lifecycle. It does not encode a
particular workflow policy.

## Policies and verb plugins

Plugins add verbs and optional gates. Repository policy entries live in
`.maestro/config`, where a plugin can be enabled or disabled without changing
the kernel. The default configuration enables proof and breakdown gates while
leaving the TDD, QA, research, witness, and lifecycle overlays disabled.

## Recipes and skills

Recipes and skills are prompt-first Markdown methods. Recipes are read on
demand:

```sh
maestro recipe list
maestro recipe show work
```

The nine installed skills cover bundle routing, design, implementation,
verification, improvement, and the read-only explore, diagnose, coach, and
questionnaire engagements. They add working depth without moving policy vocabulary into the
kernel.
