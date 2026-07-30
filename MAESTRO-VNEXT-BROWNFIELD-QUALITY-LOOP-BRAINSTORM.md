# Maestro vNext Brownfield Quality Loop Brainstorm

Status: side-chat brainstorm only. This file is non-authoritative evidence for later discussion with the main design conductor. It is not a Maestro Decision, canonical design amendment, Contract Component, build approval, implementation plan or permission to modify runtime behavior. `FINAL-MAIN.md` and the live canonical feature artifacts remain untouched.

## Problem

Agents learn heavily from the code and instructions already present in a repository. They reproduce good patterns, but they also reproduce accidental architecture, stale conventions and low-quality patterns. A periodic manual “AI slop cleanup day” does not scale because debt accumulates continuously and each cleanup has to rediscover the intended quality bar.

Maestro could support a continuous, governed quality-improvement loop that makes a project's intended engineering principles discoverable, testable and progressively improvable without becoming an autonomous refactoring authority.

## Core thesis

The durable system should separate what the Agent understands from what Maestro governs:

- The Agent understands the purpose of a principle, how to diagnose a violation and how to repair it.
- Maestro knows the exact principle/check identity, applicable project/profile, evidence, current finding, advertised Operation and authority boundary.
- Prose, a Memory entry, a scanner result or an Agent conclusion never becomes an active project rule by itself.
- Quality grade is a non-authoritative Projection of exact findings and trends, not lifecycle, Gate truth or merge authority.

The conceptual loop is:

```text
observe repository
  -> detect candidate drift
  -> publish source-qualified finding
  -> advertise a bounded repair opportunity
  -> route the Agent to Adapt
  -> create a small reviewable change
  -> rerun checks and project proof
  -> merge only under exact current authority
  -> reobserve
```

An external scheduler may trigger the loop periodically, but Maestro remains passive and owns no daemon, cron service, hidden queue, worker launcher or retry engine.

## Project-independent principle model

The earlier “domain logic does not belong in a CLI adapter” example is only a Maestro-repository example. The system must work across many technologies and project shapes.

Four layers are useful:

1. **Maestro core laws**: non-optional governance and safety boundaries such as no hidden mutation, no candidate self-authorization and recoverable external effects.
2. **Technology or practice packs**: explicitly selected and version-pinned guidance/checks for Rust, Swift, React, Python, local-first persistence, accessibility, security and similar concerns.
3. **Repository principles**: project-specific boundaries such as “feature modules may not import persistence adapters directly.”
4. **Learning candidates**: newly observed patterns that remain inactive until reviewed and explicitly promoted.

A project may add or tighten local principles. It must not use a local rule or external pack to weaken mandatory core safety and authority laws. Pack updates are proposals; they never silently change a project's active quality contract.

Each usable principle needs more than prose. A candidate shape is:

```text
stable principle identity and version
purpose and applicability
source provenance
machine-checkable detector when possible
severity and confidence treatment
repair guidance
allowed exception grammar
proof required after repair
automation and merge limits
reopen or retirement conditions
```

Principles may be classified as:

- **Invariant**: hard and mechanically falsifiable.
- **Heuristic**: advisory signal requiring diagnosis and judgment.
- **Reference pattern**: a positive example that teaches a shape but is not itself normative.

Promotion should follow:

```text
repeated finding
  -> Principle Candidate
  -> evidence across representative cases
  -> adversarial review
  -> authorized promotion
  -> versioned active principle plus exact check contract
```

## Brownfield first setup

For a brownfield project, the first Maestro setup should not ask the user to describe the entire architecture from scratch. The Agent should perform bounded read-only discovery and produce a reviewable Project Constitution Candidate.

Setup coordinates this journey but does not own or silently activate the discovered knowledge.

### Discovery inputs

Read in progressive layers:

1. Deliberate repository guidance:
   - the applicable `AGENTS.md` hierarchy;
   - `CLAUDE.md` and other host pointers;
   - `README`, architecture, contributing, design and Decision/ADR documents;
   - repository-local workflow and release guidance.
2. Actual toolchain:
   - language and package manifests;
   - compiler, formatter and linter configuration;
   - CI workflows, test commands, code generation and release scripts.
3. Observable architecture:
   - module and dependency boundaries;
   - public interfaces;
   - persistence, authority and external-effect boundaries;
   - repeated patterns, exceptions, hotspots and known debt.
4. Demonstrated behavior:
   - tests and existing architecture checks;
   - compiler/linter enforcement;
   - representative recent changes where locally available and explicitly in scope.

Discovery should start with inventory and structural maps, then load only relevant instructions, representative modules, contradictions, exceptions and hotspots. It must not ingest the entire repository into one prompt or scan undeclared roots, secrets, vendored dependencies, caches and build output indiscriminately.

### Candidate output

The result should preserve provenance and uncertainty rather than flattening everything into guessed rules:

```text
Observed principle:
  Feature modules do not access SQLite directly.

Sources:
  AGENTS.md:42
  ARCHITECTURE.md:81
  dependency scan: 47 of 49 modules comply
  two exact existing violations

Confidence:
  high

Candidate disposition:
  adopt as a repository principle
  grandfather the two exact existing violations as debt
  reject new violations
  create gradual repair Work
```

The complete brownfield result should include:

- project and toolchain map;
- applicable instruction hierarchy and provenance;
- observed architectural boundaries;
- existing machine-enforced rules;
- principle/profile candidates;
- exact contradictions and unknowns;
- baseline findings and grandfathered debt;
- proposed checks and repair guidance;
- proposed technology packs with exact versions;
- excluded roots and incomplete evidence;
- review questions required before activation.

### Ownership boundary candidate

- Setup coordinates onboarding and installation/adoption UX.
- Intake owns the imported source artifacts and their dispositions.
- Research owns source-bound analysis and synthesis revisions.
- Evidence owns immutable observations and assessments.
- Design synthesizes the Project Constitution Candidate.
- An explicit authorized action pins accepted principles/profiles.
- Adapt maintains the accepted baseline and proposes future improvements.
- Projection alone recommends the next governed action.

This prevents Setup prose or stack detection from becoming a second policy owner.

## Brownfield ratchet

Brownfield adoption must not require the whole repository to become clean before Maestro is useful. It should use a ratchet:

```text
existing debt
  -> recorded exact baseline
  -> temporarily grandfathered with location and reason
  -> cannot spread into new code
  -> repaired incrementally through governed Work

new or materially changed code
  -> must satisfy the pinned current principles
```

Grandfathering is not a generic waiver. It binds exact findings, locations, versions and review conditions. Moving, copying or expanding the violation must not inherit the exception automatically.

## Contradiction handling

Documentation, code, tests and CI may disagree. Discovery must expose the contradiction instead of silently selecting a winner.

Example:

```text
AGENTS.md says:
  services never access the database directly

Observed code says:
  six services access it directly

Tests imply:
  two accesses may be deliberate

Result:
  contradiction requiring classification as
  intentional exception | existing debt | stale documentation | misunderstood boundary
```

Instruction precedence helps identify intended authority, but it does not turn stale prose into proven current architecture. Observable code and tests are evidence, not automatic policy. The review must decide the canonical disposition.

## Continuous Adapt behavior

After initial adoption:

1. A declared scanner/check observes only its authorized scope.
2. It emits an exact source-qualified finding against a pinned principle identity.
3. Projection may advertise a bounded repair Operation.
4. The root `maestro` Skill routes the Agent to the Adapt job.
5. Adapt progressively loads only the violated principle, repair guidance and relevant code context.
6. The Agent prepares a small change or PR.
7. The same check, relevant tests and cross-principle regression proof rerun.
8. Merge requires current project authority; possible external uncertainty routes to recovery rather than retry.

The Agent must not update the principle merely to make a failing change pass. Principle amendment and code repair are separate governed paths.

## Auto-merge boundary

Auto-merge, if supported at all, should require an explicit narrow Grant and a closed low-risk change class. Candidate restrictions include:

- bounded diff size and file scope;
- no public API, schema, dependency, authority, security, persistence or migration change;
- no unresolved reviewer disagreement;
- exact formatter, compiler, tests and affected principle checks pass;
- no regression on another pinned principle;
- no unknown or `in_doubt` external effect;
- deterministic rollback and complete receipts.

Everything outside that class produces a reviewable proposal and waits for human authority. There is no generic force or “quality score improved, therefore merge” rule.

## Questions for later main-thread reconciliation

1. Should the project-level artifact be called a Quality Constitution, Engineering Constitution, Principle Profile or remain a composition of existing Contract/Policy Profile concepts?
2. Is brownfield discovery a governed method reached through Setup, a Research/Design journey coordinated by Setup, or both at different phases?
3. Which principle fields are core typed contracts, and which remain content-bearing Capability Resources?
4. How are existing-debt exceptions represented without creating a second waiver or authority system?
5. Which technology packs, if any, are first-party, and how are external pack provenance, licensing and compatibility admitted?
6. What exact event triggers Adapt routing while preserving Projection as the sole next-action authority?
7. What is the smallest safe auto-merge class, or should vNext initially remain proposal-only despite designing the full durable authority boundary?
8. How are principle quality, false positives and harmful promoted rules reviewed and rolled back without hiding history?

## Current side-chat recommendation

Carry this into the main thread as one coherent candidate: brownfield onboarding performs bounded, provenance-preserving discovery and produces a Project Constitution Candidate; accepted principles become version-pinned project policy; existing debt uses an exact ratchet; and a continuous Adapt plus Learning-guided loop proposes small evidence-backed repairs. Maestro governs identities, applicability, proof and authority, while the Agent supplies semantic diagnosis and repair. No hidden scheduler, self-promoting rule, autonomous refactor authority or universal Maestro coding taste is introduced.
