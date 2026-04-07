---
name: maestro:blueprint
description: Generate visual HTML blueprint pages and structured plan specs for maestro project features. Explores the codebase, produces a `.md` plan in maestro format (Context, Critical Files, Design Decisions, Phases with Tasks and acceptance criteria) saved to `.maestro/plans/`, plus a visual HTML presentation. Use when the user asks to blueprint a maestro feature, plan an implementation for this project, or says "blueprint X" while working in the maestro codebase. Also use proactively for non-trivial maestro changes that span multiple files or architectural concerns.
argument-hint: "<feature description>"
---

# Maestro Blueprint

Generate two artifacts for every blueprint in the maestro project:

1. **Plan spec** (`.md`) -- maestro-format plan saved to `.maestro/plans/{feature-name}.md`
2. **Visual blueprint** (`.html`) -- interactive HTML presentation saved to `~/.agent/diagrams/{feature-name}-blueprint.html`

The `.md` plan follows maestro conventions and integrates with tracks, missions, and the existing planning workflow. The `.html` is the visual version of the same content.

## Workflow

### 1. Explore (Subagents)

Before generating anything, understand the affected codebase areas. Launch parallel explore subagents.

**Maestro-specific areas to check:**
- `src/commands/` -- CLI command definitions
- `src/tui/` -- TUI rendering if the change affects Mission Control
- `src/ports/`, `src/adapters/`, `src/use-cases/` -- hexagonal architecture layers
- `skills/built-in/` -- if the change affects built-in skills
- `.maestro/context/` -- read product.md and tech-stack.md for project context
- `.maestro/tracks/` -- check for related existing tracks
- `tests/` -- existing test patterns

**What to look for:**
- Existing implementations that can be reused
- The hexagonal architecture pattern: port -> adapter -> use-case -> command -> MCP tool -> test
- Conventions from AGENTS.md (types, naming, async, imports)
- Files that will need modification
- Related tracks or plans already in `.maestro/`

### 2. Synthesize and Decide

After subagents report back, synthesize findings. Determine depth level:

| Level | When | Sections |
|---|---|---|
| **Light** | 1-2 files, obvious change | Context, File Changes, Verification |
| **Standard** | Feature spanning several files | Context, Critical Files, Phases, Verification |
| **Full** | Architectural change, new command/tool, cross-cutting | All sections including Design Decisions, Risks |

If a structural fork exists (e.g., "should this be a port+adapter or a direct implementation?"), ask the user after exploration. Reference specific code you found.

### 3. Write the Plan Spec (.md)

Save to `.maestro/plans/{feature-name}.md`. Follow the maestro plan format:

```markdown
# Blueprint: Feature Name

> One-line summary.

## Context

What exists today, what's broken/missing, what the world looks like after.
Include motivation -- why this change matters for maestro.

## Critical Files

| File | Role | Change |
|------|------|--------|
| `src/commands/foo.ts` | CLI command | New |
| `src/use-cases/foo.ts` | Business logic | New |
| `src/ports/foo.port.ts` | Port interface | New |
| `src/adapters/foo.adapter.ts` | Adapter | New |
| `src/index.ts` | Entry point | Add command registration |

## Design Decisions

**Decision 1: Why X over Y**
- Considered: approach A, approach B
- Chose: approach A because [reason]
- Trade-off: [what we give up]

## Phases

### Phase 1: Phase Name (~duration)

**Delivers:** What this phase produces that can be verified.

#### Tasks

1. **Task 1.1: Task name**
   - Files: `src/path/file.ts`
   - Description: What to do
   - _Acceptance: what proves this works_

2. **Task 1.2: Task name**
   - Files: `src/path/file.ts`
   - Description: What to do
   - _Acceptance: criteria_
   - _Depends on: Task 1.1_

#### Test Plan
- Unit: `bun test tests/unit/foo/`
- Integration: what to test

### Phase 2: ...

## Dependencies

\`\`\`mermaid
graph TD
  T1.1 --> T1.2
  T1.2 --> T2.1
\`\`\`

## Risks

| Risk | Impact | Likelihood | Mitigation |
|------|--------|------------|------------|
| Specific risk | High/Med/Low | High/Med/Low | What to do |

## Verification

- [ ] Build: `bun run build && ./dist/maestro --version`
- [ ] Tests: `bun test`
- [ ] TUI (if applicable): `./dist/maestro mission-control --render-check --size 120x40`
- [ ] CLI: `./dist/maestro <command-under-test>`
- [ ] Release: `bun run release:local` (if user-facing)
```

### 4. Generate the Visual Blueprint (.html)

Read the reference template at `./templates/blueprint-full.html` before generating. Also read the CSS patterns at `./references/css-patterns.md`.

The HTML contains the same content as the `.md` plan but presented visually with:
- KPI summary cards (files, phases, LOC, risk)
- Architecture diagram (Mermaid with zoom controls)
- Phase timeline
- Collapsible per-phase details with file changes and tasks
- Dependency DAG (if full depth)
- Risk matrix table (if full depth)
- Verification checklist

For Mermaid theming and libraries, read `./references/libraries.md`.
For responsive navigation (4+ sections), read `./references/responsive-nav.md`.
For detailed guidance on each section's content, read `./references/sections-guide.md`.

Save to `~/.agent/diagrams/{feature-name}-blueprint.html`.

### 5. Style

Follow the visual-explainer quality bar:
- Distinctive Google Fonts pairing (never Inter/Roboto/Arial)
- CSS custom properties for full light/dark theme support
- Semantic color naming (`--phase-active`, `--file-add`)
- Staggered fade-in animations, `prefers-reduced-motion` respected
- Vary palette each time -- don't repeat the same look

### 6. Deliver

Open the HTML in the browser and tell the user both paths:
- Plan spec: `.maestro/plans/{name}.md`
- Visual: `~/.agent/diagrams/{name}-blueprint.html`

## Maestro Conventions

Follow these when generating plans:

- **Hexagonal architecture**: new features follow port -> adapter -> use-case -> command -> MCP tool -> test
- **Conventional commits**: reference the commit types in the plan (feat, fix, refactor)
- **Version bumps**: note if the change requires a minor/patch bump
- **Build verification**: always include `bun run build && ./dist/maestro --version` in verification
- **TUI changes**: include `--render-check` verification if the change touches TUI
- **Binary verification**: specify whether to test against `./dist/maestro` or installed `maestro`

## Quality Checks

Before delivering:
- Plan spec covers all files that need changing
- Tasks have concrete acceptance criteria (not vague "it works")
- Verification commands are copy-pasteable
- Architecture diagram shows the change, not the entire system
- HTML renders cleanly with no console errors
- Both light and dark themes look intentional
