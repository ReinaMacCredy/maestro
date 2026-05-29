---
name: maestro-setup
description: Initial setup and harness tuning protocol for a Maestro-enabled repository.
---

# Maestro Setup

Use this skill after `maestro init` to tune the repository harness.

Inspect the repo structure, build and test commands, existing agent instructions, and current
workflow constraints. Update harness guidance only from verified repository evidence, and keep
setup changes small enough for future agents to trust and maintain.

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-setup`, and `activation_mode` set to `agent_selected`.
