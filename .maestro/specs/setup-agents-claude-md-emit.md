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

Block body the CLI writes (verbatim, from the skill's own template):

```md
<!-- maestro-setup:start -->
## Maestro Context

Before non-trivial work:
- Load `.maestro/context/index.md` first.
- Open only the specific context docs relevant to the task.
- Follow detected language guides under `.maestro/context/code_styleguides/`.
- Preserve user content outside managed setup sections.
- If context docs conflict with closer repo instructions, follow the closer
  instruction file and report the conflict.
<!-- maestro-setup:end -->
```

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

**B.** It closes the lecture's bootstrap-contract gap with the helpers
that already exist, preserves the skill's role as the rich-content
owner, and the idempotency rule (CLI writes only when `hasBlock` is
false) makes the CLI-vs-skill ordering safe in either direction.

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

## Open questions (not blocking; flag for design review)

1. **Reference target.** `agent-block.ts` hard-codes `REFERENCE_FILE =
   "MAESTRO.md"`. Most projects today reference `@AGENTS.md` instead.
   Pick one: either change the constant, parameterize it, or accept
   that the reference points at a file Maestro itself doesn't currently
   emit (it would need to land too).
2. **Marker mismatch.** Helpers in `agent-block.ts` default to
   `<!-- maestro:start -->`. The skill uses
   `<!-- maestro-setup:start -->`. If the CLI is going to call these
   helpers as-is, the constants need to change to match the skill;
   otherwise the helpers produce blocks the skill can't read.
3. **Clean git checkpoint** (Lecture 06's 5th output). Worth a `maestro
   setup --commit` flag, or out of scope? Maestro is currently
   passive about git state outside its own paths; an opt-in flag
   feels right but is a separable design.
4. **`.maestro/AGENTS.md` vs project-root `AGENTS.md`.** The former
   already exists (Maestro-internal bootstrap). Cross-reference them
   from each other? Or keep them strictly disjoint? Today the
   maestro-setup skill embeds a pointer to `.maestro/context/` from the
   root file; that's the only link.

## Phasing if approved

1. **Spec sign-off.** Lock option B, settle marker + reference-file
   constants.
2. **Code.** Change `BLOCK_START_MARKER` / `BLOCK_END_MARKER` constants
   in `src/infra/domain/agents.ts` to `maestro-setup:start/end`. Add a
   step to `setup.usecase.ts` that calls `injectBlock` / `injectReference`
   against project-root `AGENTS.md` and `CLAUDE.md` after the existing
   `init.sh` emission. Idempotent via `hasBlock` / `hasReference`.
3. **Tests.** Greenfield, brownfield-with-content, brownfield-with-
   legacy-heading-only, rerun-is-noop, skill-wrote-richer-block-don't-
   clobber. Mirror the existing `.maestro/AGENTS.md` test shape in
   `tests/unit/service/setup.usecase.test.ts`.
4. **Skill alignment.** Update `maestro-setup` SKILL.md to note that
   the CLI now seeds the block; the skill's Step 5 still owns the
   richer init-deep path and uses `replaceBlock` to upgrade in place.
5. **Optional follow-up.** `--commit` flag for the lecture-06 clean
   checkpoint output.

## Non-goals

- Re-implementing init-deep's hierarchical knowledge base inside the
  CLI. The skill owns that.
- Running language detection or copying style guides from the CLI. That
  stays in the skill (Step 2 + Step 4 of `maestro-setup`).
- Writing to any agent config file outside `AGENTS.md` and `CLAUDE.md`.
  Codex's `AGENTS.md` lives under `.codex/`, not the project root, and
  is already handled by the user-level install path; not part of this
  brainstorm.
