# Scenario test architecture: two-agent end-to-end with rubric scoring

Phase 6 introduces scenario tests as the behavioral gate before the 2.0 release. This ADR fixes the test architecture so Phase 6 work starts from a stable shape.

**Eight scenarios across three axes:** project state (greenfield, brownfield) × user familiarity (novice, expert) × workflow mode (light, heavy). 2×2×2 = 8. Runtime variation (Claude Code vs Codex) deferred to v2.1; task-shape variation (bug/feature/refactor) parametrized within scenarios as one shape per cell.

**End-to-end loop.** Each scenario runs the full user → coding-agent → maestro → PR cycle, not a slice of it. Two sub-agents per scenario:

- **User-mock**: gives prompts at the scenario's familiarity tier.
- **Coding-agent**: drives maestro verbs against a real workspace, responding to user-mock prompts.

**Familiarity tier = prompting style.** Novice = generic prompts without verb names ("set this up", "fix the bug"). Expert = explicit verb references ("decompose the spec at `.maestro/specs/x.md`", "claim the first task"). Tests the coding-agent's ability to navigate maestro under low steering. Rejected: read-but-haven't-done (essentially documentation legibility — Phase 4's job), mis-prompting / v1-vocab drift (post-sunset there is no v1 vocab to confuse).

**Rubric pass.** Each scenario carries a written checklist of must-happen and must-not-happen events against the evidence trail (e.g., "task transitioned to `claimed`", "evidence row with `kind=transition, to=ready` exists", "no `kind=lint-violation` after the fix"). Evaluation is **deterministic code** run against `.maestro/evidence/<date>.jsonl` after the scenario ends — no LLM-as-judge in the rubric path, since judge-LLM drift would invalidate the very stability the rubric is meant to provide. Pass iff all checks pass. Stable across model-version drift; aligns with ADR-0009 (evidence on every transition). Rejected: snapshot-based pass (breaks on every model bump), outcome-only (allows agents to skip the verbs the test is meant to cover), LLM-judged rubric (introduces a second non-deterministic stage atop the agent's own non-determinism).

**Brownfield = v1 migration.** Frozen v1 `.maestro/` snapshot reused from `tests/fixtures/v1-maestro/`. Brownfield scenarios open with `maestro setup --migrate-v2` and continue. Tests the §9 migration mapping tables under varied user prompts. Rejected: brownfield = existing non-maestro project (essentially greenfield with a `package.json` — duplicates greenfield coverage).

**Execution model: swarm-fix-loop, not scheduled CI.** Phase 6 is a development phase, not a perpetual gate. The developer (or a meta-agent) in an interactive Claude Code session:

1. Dispatches all 8 scenarios in parallel as sub-agents via the `Agent` tool with `run_in_background: true`.
2. Waits for completion; collects pass/fail + rubric trace + evidence dump.
3. For each failure, fixes maestro (or sharpens the rubric where the rubric was the bug).
4. Re-dispatches only the failed scenarios.
5. Loops until a swarm pass completes all-green with zero intervening fixes.

The `Agent` tool is the right transport precisely *because* Phase 6 runs interactively. Rejected: scheduled CI cron (turns a one-time release gate into perpetual infra burden; API-key-in-CI risk; flake-aging policy needed; cost compounds), Anthropic SDK direct (we'd reinvent the agentic loop the Agent tool already gives us), Claude Code CLI subprocess (transcript export is unstable). Post-2.0 monitoring (if added) can pick any transport — that decision is out of scope here.

**User-mock is scripted, not freeform.** Each scenario carries an ordered list of N user messages (typically 2–6) plus a termination condition (PR open, `verify=PASS`, or scenario timeout). User-mock replays the script; only the prompting *style* varies per familiarity tier. Determinism lives on the user-mock side so all the non-determinism is concentrated in the coding-agent's reasoning, which is what the rubric is testing. Rejected: freeform user-mock (compounds non-determinism — the test no longer measures the coding-agent, it measures the joint distribution).

**Coding-agent surface = production skill bundle.** Each spawned sub-agent loads maestro CLI + the 5 bundled `SKILL.md` files verbatim — the same surface a real Claude Code user gets. No test-only system prompt, no curated steering. Otherwise the rubric measures the test harness, not maestro's usability.

**Sandboxing.** Every dispatched sub-agent runs against a per-run `mktemp -d` copy of greenfield or brownfield fixture so parallel scenarios don't stomp each other and a failed run's `.maestro/evidence/` is preserved for triage.

**Open items deferred to Phase 6 kickoff:** test file location (`tests/scenarios/` vs `tests/e2e/scenarios/`), exact rubric line format per scenario, the swarm dispatcher script shape (`bun scripts/scenarios/swarm.ts` or inline Agent calls in the developer's Claude Code session).
