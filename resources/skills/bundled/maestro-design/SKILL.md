---
name: maestro-design
description: Spec authoring and design grilling protocol for Maestro work.
---

# Maestro Design

Use this skill when turning a rough idea into a Maestro-ready spec or task plan.

Clarify the user-visible outcome, constraints, non-goals, acceptance checks, and rollout risks.
Prefer concrete examples and repository evidence over generic architecture language, then hand off a
plan that can be implemented and verified in small steps.

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`maestro-design`, and `activation_mode` set to `agent_selected`.
