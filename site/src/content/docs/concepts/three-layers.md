---
title: Three layers
description: How Maestro separates mechanism, removable policy, and prompt-first method.
---

Maestro separates durable mechanism from workflow policy and working method.
That boundary keeps the kernel small while allowing repositories to choose
their gates and agents to load deeper guidance only when needed.

## Kernel

The mechanism kernel owns the SQLite store, event log, sessions, command
routing, and plugin loading. It contains mechanism only: work and decision
records, leases, events, and plugin lifecycle. It does not encode a particular
workflow policy.

## Policies and verb plugins

Plugins add verbs and optional gates. Every plugin is built in; repository
entries in `.maestro/config` turn one on or off without changing the kernel.
The default configuration enables the proof and breakdown gates and leaves
the card budget and the delivery grant off. SLP, the team layer, is a plugin
too and is off by default.

## Recipes and skills

Recipes and skills are prompt-first Markdown methods. Recipes are read on
demand:

```sh
maestro recipe list
maestro recipe show bug-fix
```

Most recipes are playbooks, the steps for one kind of task, and
`~/maestro/WORKFLOW.md` names the playbook that fits each kind of task. The
seven installed skills cover bundle routing, design, the delegation contract,
verification, councils, and the read-only coach and questionnaire
engagements. They add working depth without moving policy vocabulary into the
kernel.
