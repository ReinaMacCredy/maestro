---
name: qa-baseline
version: 1.0.0
description: Capture a real-scenario behavior contract for a feature before edits start. Run at feature accept; the accept gate blocks until baseline.md exists, and the ship gate proves every behavioral scenario against it.
---

# QA Baseline

On activation, log the skill activation by piping a compact JSON payload to
`maestro hook record` with `event_type` set to `skill_activation`, `skill_name` set to
`qa-baseline`, and `activation_mode` set to `agent_selected`.

Create a real-scenario behavior contract before implementation changes a feature.

This is the gate that prevents a feature's edits from drifting away from the behavior users
already rely on. It is captured at `maestro feature accept` (before edits, by construction) and
lives at `.maestro/features/<id>/baseline.md`. The `feature accept` gate refuses until the file
is present; the `feature ship` gate later proves every behavioral scenario it names.

QA here means "prove the product/workflow still behaves correctly in reality", not "run whatever
tests already exist." Tests are one evidence source; real flows, fixtures, screenshots, command
transcripts, state files, API samples, rendered output, and release/install checks can all be stronger
evidence. Describe behavior in the project's domain language, capture expected vs actual behavior,
and make every scenario reproducible from a user's point of view.

## Inputs

- The feature contract (`maestro feature show <id>`): acceptance criteria + affected areas.
- Current repo instructions and available test/docs surfaces.
- Current code behavior, command outputs, screenshots, fixtures, API samples, state files, generated
  artifacts, release/install commands, or docs as applicable.

## Workflow

1. Identify changed surfaces by capability, not framework:
   - UI routes/views, CLI commands, APIs, libraries, data models, integrations, jobs, auth/security,
     persistence, config, build/release, docs, performance, accessibility, and compatibility.
2. Identify critical workflow chains:
   - Name the product's trunk journeys: setup/onboarding, create work, modify work, execute work,
     verify/approve work, inspect/report work, recover/resume, publish/release, and cleanup/removal
     as applicable.
   - A workflow chain is a connected user/operator sequence with upstream preconditions and
     downstream consumers, not a group of unrelated tests. For a workflow tool, that might mean
     initialize project -> create task/work item -> run or record evidence -> verify status ->
     inspect/report output -> resume or clean up.
   - If the feature touches any link in a trunk journey, include at least one minimal chain that
     crosses the changed link plus its nearest upstream setup and downstream consumer. If no
     meaningful chain applies, record why.
   - Workflow-chain proof is additive. Do not replace isolated contract probes for surfaces whose
     correctness is local and safety-critical, such as install ownership, rollback, parser edge cases,
     auth checks, schema validation, or destructive-operation guards.
3. Build a Scenario Matrix by dimensions. Pick the smallest set that covers the feature risk:
   - **Actor and intent**: end user, operator, developer, admin, integrator, CI/release system.
   - **Entrypoint**: UI route, CLI command, API call, SDK/library call, job, plugin, file import/export.
   - **State and lifecycle**: first run, existing data, upgraded data, partial/corrupt state, stale
     cache, interrupted/resumed work, rollback.
   - **Data shape and scale**: empty, typical, edge-case, large, malformed, legacy, concurrent.
   - **Environment and channel**: local dev, installed binary/app, package, CI, production-like config,
     OS/browser/runtime variation.
   - **Integration boundary**: external service, auth provider, filesystem, database, network, MCP/API,
     package manager, generated artifact.
   - **Permissions and trust**: auth roles, privacy, secrets, destructive operations, sandbox limits.
   - **Failure and recovery**: invalid input, timeout, crash, retry, idempotency, error messaging.
   - **Non-functional**: performance, accessibility, observability, compatibility, release safety.
   - Greenfield and brownfield are useful state variants, but do not over-index on them when another
     dimension carries the real risk.
4. Give every Scenario Matrix entry a stable id `[bl-NNN]` (`[bl-001]`, `[bl-002]`, ...). This id is
   the coverage unit: the ship gate requires a counting QA slice (see `qa-slice`) for each `[bl-NNN]`
   you record here. Number scenarios that name observable user/operator behavior; do not invent
   scenarios to pad the count. A baseline with zero `[bl-NNN]` ids declares "no behavioral surface"
   and ships with no slices required.
5. For each selected scenario and workflow chain, capture the current-behavior oracle:
   - setup/preconditions, action, expected observable result, artifact to compare, and command/manual
     check. Prefer cheap real commands over invented assertions.
   - Write it like a durable QA issue could be filed from it: what happened, what was expected, steps
     to reproduce, and additional context in domain language.
6. For each relevant surface, capture the cheapest current-behavior proof:
   - Existing tests, type/lint/build commands, smoke commands, screenshots, golden files, fixtures,
     API request/response examples, sample CLI output, migration dry-runs, logs, state snapshots,
     render checks, installed-app/binary output, or manual checks.
   - Keep two proof layers when both apply: focused isolated probes for the local invariant, plus a
     workflow-chain probe showing the changed invariant still composes with real upstream/downstream
     behavior.
7. Mark any untestable critical behavior as a baseline gap with the smallest proposed probe.
8. Write the `QA Baseline Contract` block below into `.maestro/features/<id>/baseline.md`, with the
   `amend_log_position` frontmatter set as described. Do not edit product code; this is the contract,
   not the implementation.

## Baseline freshness (amend-log position)

The feature contract can grow after accept via `maestro feature amend`. A baseline captured before a
behavioral amend is stale. Record the amend-log position the baseline was captured against in
frontmatter:

```markdown
---
amend_log_position: 0
---
```

- At accept there are no amends yet, so `amend_log_position: 0`.
- The ship gate blocks if any amend recorded after this position added an acceptance criterion or an
  affected area (behavioral amends; non-goal/question amends do not block).
- To refresh: re-run this skill, extend the Scenario Matrix to cover the new behavior, and set
  `amend_log_position` to the current amend-log length (count of entries in `amend-log.yaml`).
- Absent or out-of-range frontmatter is treated as `0` (fail-closed: the gate re-checks every amend).

## Output

```markdown
---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: <feature id and surface>
- Critical workflow chains:
  - <chain name>
    - Steps: <setup -> action -> downstream consumer -> recovery/cleanup if relevant>
    - Touched link: <link changed by this feature, or `None`>
    - Minimal proof: <command/manual flow/artifact comparison>
- Scenario Matrix:
  - [bl-001] <scenario name>
    - Dimensions: <actor/entrypoint/state/data/environment/integration/trust/failure/non-functional>
    - Setup: <state, fixture, account, repo, command, or manual precondition>
    - Action: <real command/click/API call/user flow>
    - Oracle: <observable pass condition>
    - Evidence to capture: <output, screenshot, artifact, state file, log, response, render check>
    - Reproduction: <concise steps a developer/operator can rerun>
  - [bl-002] <scenario name>
    - ...
- Preserved behaviors:
  - <behavior> -> Proof: `<command>` or <manual artifact/check>
- Changed behaviors:
  - <intentional change from plan/spec, or `None`>
- Critical probes before commit:
  - <probe name> -> `<command>` or <manual check>
- Required artifacts:
  - <artifact path or description, or `None`>
- Baseline gaps:
  - <gap> -> Proposed probe: <smallest useful check>
```

## Blocking Rules

- The `feature accept` gate blocks until `baseline.md` exists and is non-empty. Capture it before edits.
- Record a `[bl-NNN]` scenario when the feature touches user-visible, data, security, persistence, or
  compatibility behavior; otherwise record an explicit baseline gap.
- Record a workflow-chain proof when a broad or cross-module feature touches a trunk workflow; isolated
  surface probes alone are not enough.
- Keep a focused isolated probe for a safety-critical local invariant even when a workflow-chain proof
  exists.
- Do not over-index on greenfield/brownfield when state/lifecycle, permissions, release channel, or
  data shape carries the real risk.
- "Tests pass" alone is not a baseline for a user/operator-facing change; name a real scenario,
  artifact, or observable behavior.
- Do not block on exhaustive test coverage. Cover the highest-risk behavior this feature can
  realistically break.
- Keep the contract short enough to read at a glance.
