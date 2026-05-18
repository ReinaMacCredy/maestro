---
slug: setup-agents-claude-md-emit
status: brainstorm
mode: light
work_type: design
---

# Brainstorm: AGENTS.md / CLAUDE.md emission in `maestro setup`

## Why this is being thought about

Two prompts converged:

1. **User ask (2026-05-18):** make `maestro setup` check for `AGENTS.md` and
   `CLAUDE.md` at the project root and "add it if missing." Greenfield is
   easy; brownfield (the user already has content) is the hard case.
2. **walkinglabs Lecture 04** — *Why one giant instruction file fails*. Root
   instruction docs become unreadable when every concern lands in them. The
   root should be a *navigation pointer*; concrete knowledge lives in scoped
   sibling docs (`.maestro/context/architecture.md`,
   `.maestro/context/quality-gates.md`, etc.).
3. **walkinglabs Lecture 06** — *Why initialization needs its own phase*.
   Init optimizes for "reliability and efficiency of all subsequent
   implementation," not feature completion. A complete init produces:
   (1) runnable environment, (2) verifiable test framework, (3) a
   **bootstrap contract document** (commands + project structure +
   current state), (4) a task breakdown, (5) a clean git checkpoint.

Maestro already pattern-matches the lecture: `.maestro/` is the project
state; `maestro mission decompose` is the task breakdown; the bootstrap
contract document is the missing piece if you read it strictly. The
existing `maestro-setup` skill already documents how to produce it. The
question this sketch answers: *what should the CLI do, what should the
skill keep doing, and how does it not fight init-deep.*

## Current state (verified, not assumed)

This is what actually exists in the tree today, not what I think exists.

### Helpers that *exist* but are *unused* in production code

`src/infra/lib/agent-block.ts` already exports the managed-block primitives
the design needs:

- `hasBlock(content)` — does the file already contain a maestro-managed block?
- `wrapBlock(content)` — wrap content in the markers
- `injectBlock(content, block)` — append a fresh block at file end
- `extractBlock(content)` — pull the current block body out
- `replaceBlock(content, newBlock)` — swap the block body in-place
- `removeBlock(content)` — strip the block (preserve everything else)
- `removeLegacyBlock(content)` — strip the pre-marker `## Cross-Agent Handoff`
  heading-based section
- `hasReference(content)` / `injectReference(content)` / `removeReference(content)`
  — for the lighter `@MAESTRO.md` pointer-line model

A grep across `src/` shows **zero callers** of `injectBlock` /
`replaceBlock` / `injectReference` outside the helper file itself. The
machinery is fully built; the wiring into a use-case is missing.

### Markers in the wild — two systems, not one

Three marker pairs live in this repo. Mixing them up is the easiest way
to break this design:

| Marker | Defined at | Owner | Used where |
|---|---|---|---|
| `<!-- maestro:start -->` … `<!-- maestro:end -->` | `src/infra/domain/agents.ts:11-12` | the agent-block helper above (unused) | nowhere in production |
| `<!-- maestro-setup:start -->` … `<!-- maestro-setup:end -->` | maestro-setup SKILL.md | the maestro-setup skill | embedded in root AGENTS.md, after `## OVERVIEW` |
| `<!-- maestro-setup:generated:start -->` … `<!-- maestro-setup:generated:end -->` | maestro-setup SKILL.md | the maestro-setup skill | inside `.maestro/context/*.md` for the generated body |
| `<!-- AGENTS-HIERARCHY:START -->` … `<!-- AGENTS-HIERARCHY:END -->` | the init-deep skill (vendored at `skills/bundled/maestro-setup/reference/init-deep.md`) | init-deep | the parent/children block at the bottom of every AGENTS.md it touches |

**If the CLI ever writes a managed block, it MUST use the same marker
pair the skill documents (`maestro-setup:start/end`), not the
`maestro:start/end` pair the unused helpers default to.** The helpers
will need a marker override or a parallel `setupBlock` helper, or the
markers in `src/infra/domain/agents.ts` get changed to match (cheaper).

### Who writes what today

- **`maestro setup` (`src/service/setup.usecase.ts`)** writes
  `.maestro/AGENTS.md` (the *Maestro-internal* bootstrap doc, not the
  project-root one). It also writes `.maestro/config.yaml`, the policy
  yaml files, `init.sh` at the project root, and 6 maestro-* skill
  directories under `.claude/skills/` and `.codex/skills/`. It does
  **not** touch `<project-root>/AGENTS.md` or `<project-root>/CLAUDE.md`.
- **`maestro-setup` skill** (Step 5 of its SKILL.md) runs init-deep to
  generate the root `AGENTS.md` and then injects the
  `<!-- maestro-setup:start -->`-wrapped pointer block right after
  the `## OVERVIEW` section.
- **init-deep skill** generates the `## AGENTS Hierarchy` block wrapped
  in its own `<!-- AGENTS-HIERARCHY:START -->` markers.

So today the *skill* does all the AGENTS.md work; the *CLI* leaves the
project root alone except for `init.sh`. The user's ask is whether the
CLI should also do some of this work directly, so a project's bootstrap
is not contingent on someone invoking the skill.

## Grill resolutions (2026-05-18)

Five decisions locked during `/grill-me` on this sketch. Recorded inline
so a reader doesn't have to reconstruct them from chat history.

**Q1 — Block scope: option B2 (refined).** Pick option B, *and* shrink
the CLI-emitted block so it only references files Maestro CLI actually
emits. Anything forward-pointing (skills, context docs that don't yet
exist on disk) lives in the skill's richer replacement block, not in
the CLI's seed. Reasons:

- A CLI-only `maestro setup` run leaves a working bootstrap contract
  with zero dangling pointers.
- The skill remains the upgrade path that adds the `.maestro/context/`
  fan-out via `replaceBlock`.
- Idempotency is already covered by `hasBlock`; B2 doesn't change that.

This shrinks the block body documented below (see [block body](#block-body)).

**Q2 — Constants alignment: parameterize, do NOT change in place.**
The original sketch proposed changing `BLOCK_START_MARKER`,
`BLOCK_END_MARKER`, and `REFERENCE_FILE` in
`src/infra/domain/agents.ts`. **A grep across `src/` after the grill
showed live production callers of the legacy values:**

- `src/infra/usecases/manage-agents.usecase.ts:10` imports `agentReferencePath`
- `src/infra/usecases/manage-agents.usecase.ts:119` calls it inside
  `cleanupLegacyMaestroMd`, which strips `@MAESTRO.md` and removes the
  legacy `~/.claude/MAESTRO.md` file
- `tests/unit/infra/usecases/manage-agents.usecase.test.ts:23,26`
  consumes `REFERENCE_FILE` to assert the cleanup target

If `REFERENCE_FILE` flips to `"AGENTS.md"` in place, the legacy cleanup
would (a) remove the active `@AGENTS.md` line the user wants kept, and
(b) try to delete an `AGENTS.md` file at the agent reference path —
exactly what setup just wrote. Same shape for the markers: the helper's
`removeBlock` regex is built from `BLOCK_START_MARKER`; changing it
silently leaves stale `<!-- maestro:start -->` blocks unreclaimable.

Resolution: introduce a second, setup-only value. Concrete options:

1. **Parameterize the helpers** so `injectReference(content, fileName)`
   and `injectBlock(content, block, markers)` accept overrides; the
   legacy cleanup path passes the legacy constants, setup passes the
   new ones.
2. **Parallel constants** — keep `REFERENCE_FILE = "MAESTRO.md"` and
   `BLOCK_START_MARKER = "<!-- maestro:start -->"` for cleanup; add
   `SETUP_REFERENCE_FILE = "AGENTS.md"` and
   `SETUP_BLOCK_*_MARKER = "<!-- maestro-setup:start/end -->"` for
   emission; add `setupInjectBlock` / `setupInjectReference` wrappers.

(1) is the cleaner shape long-term; (2) is the smaller diff today.
Decision deferred to implementation.

**Q3 — Clean git checkpoint (Lecture 06 output 5): out of scope.**
Maestro stays passive about repository git state outside its own paths
(consistent with the existing "no cron/daemon/background" feedback).
Drop the `--commit` follow-up entirely; do not even file it as a
phasing step.

**Q4 — `.maestro/AGENTS.md` vs project-root `AGENTS.md`: stay disjoint.**
The two docs live at different layers: `.maestro/AGENTS.md` is
Maestro's internal bootstrap; the project-root file is the
project-knowledge contract. The only link is the pointer block (under
B2, the CLI's seed doesn't even include that — the skill adds it).
Do not cross-reference; do not auto-sync.

**Q5 — Destructive init-deep edge case.** If a user (or another tool)
hand-edits the `<!-- AGENTS-HIERARCHY:START -->` block, init-deep's
next run replaces it; the maestro-setup block sits in a different
marker pair and is untouched. The reverse holds. The single failure
mode is a user merging the two blocks into one — they then own the
result; neither tool can recover automatically. Document this in the
skill, not in the CLI; the CLI's idempotency check is marker-presence,
not content-shape.

## What Lecture 06 actually adds

Lecture 06 is not a new primitive. It's a restatement of why init needs
its own phase, with one observation Maestro already lives by: **init
optimizes for *future* reliability, not present feature delivery, and
mixing the two corrupts both.**

Mapping the lecture's five init outputs to Maestro:

| Lecture-06 output | Maestro equivalent | Status |
|---|---|---|
| Runnable environment | project-specific (npm install, bun install, etc.) | out of scope — Maestro doesn't run package managers |
| Verifiable test framework | project-specific | out of scope |
| **Bootstrap contract document** | root `AGENTS.md` + `.maestro/context/*.md` | **partially done** — the skill produces it; the CLI does not |
| Task breakdown | `maestro mission new --from-spec` + `maestro mission decompose` | already in CLI |
| Clean git checkpoint | not done | **gap** — could be a `maestro setup --commit` flag |

The lecture sharpens one thing: the bootstrap-contract document is not
optional. If a new session can't read one file at the root and know what
to do next, init failed. The current cold-start contract (`./init.sh` →
`maestro doctor` → `maestro status`) is a CLI-only version of this — it
tells you what *Maestro* knows, not what the *project* knows. The root
AGENTS.md (and its pointer block) is the project-knowledge half.

## Design options

### A — Status quo: skill-only, CLI hands-off

Keep `maestro setup` doing what it does today. The skill remains the
sole writer of root `AGENTS.md` and `CLAUDE.md`. The CLI never touches
either file.

Pros: zero new code, lowest risk of clobbering user content.
Cons: a user who runs `maestro setup` from the CLI without invoking the
skill ends up with a half-installed harness — `.maestro/` exists,
`init.sh` exists, but the project root has nothing pointing at any of
it. Brownfield repos with no skill author present never get the pointer.

### B — CLI emits a minimal pointer block; skill upgrades it

`maestro setup` becomes responsible for *one* thing at the project root:
making sure a maestro-setup managed block exists in `AGENTS.md` (and a
matching pointer in `CLAUDE.md` if Claude Code is detected). Greenfield
gets a fresh file containing only the block; brownfield gets the block
appended after the user's existing content (via `injectBlock` from
`agent-block.ts:46`).

The skill's Step 5 continues to be the canonical, expensive path: it
runs init-deep, generates a real hierarchical knowledge base, and uses
`replaceBlock` (`agent-block.ts:57`) to swap the CLI's minimal block
for the richer version embedded after `## OVERVIEW`.

<a id="block-body"></a>
Block body the CLI writes under B2 (shrunken from the skill's template
so it only references files Maestro CLI actually emits):

```md
<!-- maestro-setup:start -->
## Maestro

This project is wired into the Maestro harness. State and config live
under `.maestro/`. Run `./init.sh` to bring a fresh checkout up; run
`maestro doctor` and `maestro status` to see what Maestro knows.

Preserve content outside this managed block; the block is rewritten by
`maestro setup` and the `maestro-setup` skill, but everything else in
this file is yours.
<!-- maestro-setup:end -->
```

Three deliberate omissions vs the skill's richer block:

- No bullet pointing at `.maestro/context/index.md` — the CLI doesn't
  create that directory; the skill does. Including the pointer in the
  seed would violate B2's own rule.
- No "follow detected language guides" line — same reason; the CLI
  doesn't detect language or emit code-style guides.
- No "load only relevant context docs" instruction — that's
  skill-tier guidance; including it in the seed promises a context fan-out
  the CLI hasn't produced yet.

The skill's Step 5 swaps this seed for the richer block via
`replaceBlock`. Both blocks use the same `<!-- maestro-setup:start/end -->`
marker pair, so the swap is in-place and idempotent.

Pros: any `maestro setup` run produces a working bootstrap contract,
even without the skill. Skill remains the upgrade path. Helpers already
exist; this is wiring, not invention. Brownfield content is preserved
by definition (`injectBlock` only appends, never rewrites).

Cons: introduces a second writer of the same managed block, so re-runs
need to be careful about clobbering the skill's richer version. The
right rule is *idempotency by marker presence*: if `hasBlock(content)`
is true, skip — never overwrite, never re-inject. The skill is allowed
to replace; the CLI is not.

### C — CLI emits the full bootstrap contract; skill becomes optional

`maestro setup` writes a richer root `AGENTS.md` directly: project
structure (from `bun run check:boundaries`-style introspection),
detected commands (from `package.json` / `pyproject.toml` /
`Cargo.toml`), and the pointer block. The skill becomes one of several
upgrade paths instead of the primary path.

Pros: lecture-compliant init from the CLI alone; no skill dependency.
Cons: this is the maestro-setup skill, ported into TypeScript. Large
implementation. Re-implements language detection and codemap discovery
the skill already does well in prose. High risk of duplicating logic
that drifts.

### Recommendation

**B2** (option B with the shrunken seed block — see [Grill
resolutions](#grill-resolutions-2026-05-18)). It closes the lecture's
bootstrap-contract gap with the helpers that already exist, preserves
the skill's role as the rich-content owner, and the idempotency rule
(CLI writes only when `hasBlock` is false) makes the CLI-vs-skill
ordering safe in either direction. The B2 refinement keeps the CLI's
seed from making promises the CLI itself can't keep.

## Brownfield handling rule

`injectBlock` from `src/infra/lib/agent-block.ts:46` already implements
the right semantics for option B:

```ts
export function injectBlock(content: string, block: string): string {
  const wrapped = wrapBlock(block);
  const trimmed = content.trimEnd();
  if (trimmed.length === 0) return wrapped + "\n";
  return trimmed + "\n\n" + wrapped + "\n";
}
```

Greenfield (file absent or empty): write a new file with only the
wrapped block.

Brownfield (file exists with arbitrary content): preserve the entire
existing body, append the block at the end after one blank line.

Both branches respect the user's content. The skill's Step 5 later
moves the block to its canonical location (right after `## OVERVIEW`)
via `replaceBlock`, but the CLI doesn't need to know that — its job is
to make sure *some* block exists.

The only edge case: a file containing the legacy `## Cross-Agent
Handoff (maestro)` heading-based section but no markers. The helper
`removeLegacyBlock` (`agent-block.ts:78`) already handles this — strip
the legacy section first, then inject the new marker-wrapped block.

## Init-deep coexistence (locked)

The init-deep skill writes `<!-- AGENTS-HIERARCHY:START -->`-marked
blocks; the maestro-setup CLI/skill writes `<!-- maestro-setup:start -->`
blocks. The two never overlap because they use different marker pairs.
A single `AGENTS.md` can legally contain both:

```md
# PROJECT KNOWLEDGE BASE
...

## OVERVIEW
...

<!-- maestro-setup:start -->
## Maestro Context
...
<!-- maestro-setup:end -->

## STRUCTURE
...

<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent: - none (root)
Children:
- src/AGENTS.md
...
<!-- AGENTS-HIERARCHY:END -->
```

Ordering rule: whichever runs second leaves the other's block untouched.
Both use the *replace-only-your-own-marker* rule, so the file converges
no matter what order they run in.

## CLAUDE.md handling

`CLAUDE.md` is structurally simpler than `AGENTS.md`. The user-facing
pattern (visible in this very repo's CLAUDE.md) is a `@AGENTS.md`
reference line that pulls AGENTS.md content into the Claude Code session.

The `injectReference` helper (`agent-block.ts:18`) already does this:
it appends `@MAESTRO.md` (or whatever `REFERENCE_FILE` is set to) if
not already present. Greenfield: write a file containing only the
reference line. Brownfield: append the line if missing.

If we go with option B, `maestro setup` writes both:

- `<project-root>/AGENTS.md`: ensure a managed block exists
- `<project-root>/CLAUDE.md`: ensure a reference line exists

Both calls are idempotent — re-running setup is a no-op once the
artifacts are in place.

## Open questions (post-grill)

Most of the original open questions were closed by the grill (see
[Grill resolutions](#grill-resolutions-2026-05-18)). What remains:

1. **Helper-overrides vs parallel-constants.** Q2 of the grill ruled
   out an in-place constant change. The two surviving shapes (overload
   the helpers with override args, or add `setup*` constants and
   wrappers) are equivalent in behavior; pick at implementation time
   based on which produces the smaller, easier-to-test diff.
2. **Opt-out gap.** B2's idempotency rule is *marker-presence*: if a
   user deliberately deletes the maestro-setup block, the next
   `maestro setup` run silently re-injects it. Three ways to handle:
   - (a) sentinel file `.maestro/no-root-emit` — setup checks for it
     and skips block injection
   - (b) `maestro setup --skip-root-pointers` flag — explicit opt-out
     per invocation, no persistent state
   - (c) document as papercut — re-running setup re-installs maestro
     by definition, so a user who wants the block gone should also
     `maestro uninstall`
   Default to (c) until someone complains; revisit with (a) if it
   becomes a real friction point.

## Phasing if approved

1. **Spec sign-off.** Lock option B2 and the helper-override shape
   (helper args vs parallel constants — pick by smallest diff).
2. **Helpers.** Extend `agent-block.ts` so `injectBlock` /
   `replaceBlock` / `hasBlock` / `injectReference` / `hasReference`
   accept the marker pair and reference filename as overridable inputs
   (or add `setup*` wrappers that pass the new values). Keep the
   legacy `MAESTRO.md` / `<!-- maestro:start -->` defaults intact so
   `manage-agents.usecase.ts`'s `cleanupLegacyMaestroMd` continues to
   strip legacy installations.
3. **Setup step.** Add a step to `setup.usecase.ts` that calls the
   new setup-flavor helpers against project-root `AGENTS.md` and
   `CLAUDE.md` after the existing `init.sh` emission. Idempotent via
   `hasBlock` / `hasReference`. Block body is the shrunken B2 seed
   from this spec.
4. **Tests.** Greenfield, brownfield-with-content, brownfield-with-
   legacy-heading-only, rerun-is-noop, skill-wrote-richer-block-don't-
   clobber, legacy-cleanup-still-works (regression for Q2). Mirror the
   existing `.maestro/AGENTS.md` test shape in
   `tests/unit/service/setup.usecase.test.ts`. Add a paired test in
   `tests/unit/infra/usecases/manage-agents.usecase.test.ts` proving
   the cleanup path still targets the legacy `MAESTRO.md` value.
5. **Skill alignment.** Update `maestro-setup` SKILL.md to note that
   the CLI now seeds the block; the skill's Step 5 still owns the
   richer init-deep path and uses `replaceBlock` to upgrade in place.

Out of phasing (per grill Q3): no `--commit` flag, no clean-checkpoint
behavior. Maestro stays passive about git state outside its own paths.

## Non-goals

- Re-implementing init-deep's hierarchical knowledge base inside the
  CLI. The skill owns that.
- Running language detection or copying style guides from the CLI. That
  stays in the skill (Step 2 + Step 4 of `maestro-setup`).
- Writing to any agent config file outside `AGENTS.md` and `CLAUDE.md`.
  Codex's `AGENTS.md` lives under `.codex/`, not the project root, and
  is already handled by the user-level install path; not part of this
  brainstorm.
