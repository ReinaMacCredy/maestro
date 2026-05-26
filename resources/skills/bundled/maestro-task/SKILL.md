---
name: maestro-task
description: Feature and task workflow layer for operating the Maestro harness.
---

# Maestro Task

Use this skill when creating, claiming, updating, blocking, or completing Maestro tasks.

Start by reading `.maestro/harness/HARNESS.md`, then inspect the relevant task and feature
artifacts before changing state. Prefer Maestro CLI verbs for durable updates, preserve evidence,
and keep task status transitions explicit.

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-task`, and `activation_mode` set to `agent_selected`.
