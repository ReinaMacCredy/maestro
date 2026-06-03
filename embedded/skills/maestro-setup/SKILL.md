---
name: maestro-setup
version: 1.2.0
description: Initial setup and harness tuning protocol for a Maestro-enabled repository.
---

# Maestro Setup

## Goal

Tune a Maestro-enabled repository harness from verified repository evidence.

## When To Use

Use this skill after `maestro init`, after `maestro install`, or when
`maestro doctor` says setup or local agent integration needs attention.

## Steps

1. Start with `maestro status`. If the repo is not initialized, run
   `maestro init --dry-run`, then `maestro init --yes`.
2. Run `maestro doctor`. If no agent integration is installed, run
   `maestro install --agent codex` unless the user asked for a different agent.
3. Inspect the repo structure, build and test commands, existing agent
   instructions, and current workflow constraints.
4. Update harness guidance only from verified repository evidence. Keep changes
   small enough for future agents to trust and maintain.
5. Run `maestro doctor` again, then `maestro status` to confirm the handoff.

## Done

- Setup is healthy or the remaining setup blocker is stated clearly.
- The next handoff is visible from `maestro status`.
- Any harness guidance changes are grounded in files or commands you inspected.

## Safety

- `maestro init --dry-run` writes nothing.
- `maestro init --yes` keeps existing files and creates what is missing.
- Use `maestro init --force` only when a deliberate refresh is needed; it backs
  up existing managed files first.

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-setup`, and `activation_mode` set to `agent_selected`.
