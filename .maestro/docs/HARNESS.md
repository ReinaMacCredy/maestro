# Harness

This document explains the **harness** — the development infrastructure layer that wraps the product code in this repository.

## Product Delta vs Harness Delta

Every change in this repo falls into one of two buckets, and many touch both.

**Product Delta** — changes that deliver user-facing value:

- New features, bug fixes, API endpoints
- UI components, business logic
- User-facing documentation

**Harness Delta** — changes that improve the development process itself:

- New validation rules, policy updates
- Workflow improvements, skill enhancements
- Process documentation, risk flags

A single PR can carry both. The `harness-delta` evidence kind exists so the harness improvements are not invisible — they show up alongside product evidence in the same task.

## Work Types

Six classifications, mutually exclusive. See `FEATURE_INTAKE.md` for the decision tree.

### new-spec

Creating a new feature from a specification. No existing implementation.

### spec-slice

Implementing part of an existing specification or feature area.

### change-request

Modifying, fixing, or refining existing behavior.

### initiative

Large cross-domain work requiring multiple tasks.

### maintenance

Technical work: dependencies, configuration, tooling.

### harness-improvement

Improving the development harness itself: policies, skills, validation, hooks.

## Examples

- **Auth feature** (product) — new login endpoint → `spec-slice`
- **New risk flag** (harness) — add security policy → `harness-improvement`
- **Multi-service auth system** (both, large) — → `initiative`, expect `harness-delta` evidence rows as policies evolve

## How this fits the rest of the system

- Work-type is declared on the product-spec frontmatter (`work_type: <one of six>`); `maestro spec validate` enforces the value.
- The `maestro-design` skill walks the work-type decision tree during spec authoring (grill protocol, ADR-0016).
- The `harness-delta` evidence kind is recorded when a task closes and the work touched `.maestro/`, `policies/`, `skills/`, or `hooks/`.
- See `VALIDATION_LADDER.md` for how harness changes get verified.
