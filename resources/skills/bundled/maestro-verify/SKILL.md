---
name: maestro-verify
description: Verification protocol for Maestro tasks and feature work.
---

# Maestro Verify

Use this skill when proving a task or feature is complete.

Identify the smallest checks that can falsify the change, run them from the repository root, and
record exact commands and outcomes. If verification cannot run, state the blocker and the remaining
risk instead of marking the work complete.

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-verify`, and `activation_mode` set to `agent_selected`.
