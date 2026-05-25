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
- `src/features/<name>/` -- feature-scoped code; each feature is a bounded context with its own `commands/`, `usecases/`, `domain/`, `ports/`, `adapters/`, `services.ts`, and `index.ts` public surface
- `src/infra/` -- plumbing that isn't a feature (init, doctor, status, install, update, uninstall, mission-control commands; config and git ports/adapters)
- `src/shared/` -- generic utilities with no domain knowledge (`lib/` for fs, yaml, shell, output; `domain/` for IDs, UI config; top-level `errors.ts`, `version.ts`)
- `src/tui/` -- TUI rendering if the change affects Mission Control
- `skills/built-in/` -- if the change affects built-in skills
- `.maestro/context/` -- read product.md and tech-stack.md for project context
- `.maestro/tracks/` -- check for related existing tracks
- `tests/` -- existing test patterns

**What to look for:**
- Existing implementations that can be reused
- The feature-folder layout: cross-feature imports must go through `@/features/<name>` (the feature's `index.ts`); deep paths into another feature are forbidden and enforced by `bun run check:boundaries`
- The hexagonal pattern inside each feature: port -> adapter -> usecase -> command -> test
- Conventions from AGENTS.md (types, naming, async, imports, feature-folder layout)
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
| `src/features/<name>/commands/foo.command.ts` | CLI command | New |
| `src/features/<name>/usecases/foo.usecase.ts` | Business logic | New |
| `src/features/<name>/ports/foo.port.ts` | Port interface | New |
| `src/features/<name>/adapters/foo.adapter.ts` | Adapter | New |
| `src/features/<name>/services.ts` | Feature composition | Wire new adapter |
| `src/features/<name>/index.ts` | Public surface | Export new command/types |
| `src/services.ts` | Composition root | Wire feature services (if new feature) |
| `src/index.ts` | CLI root | Add command registration |

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
- [ ] TUI (if applicable): `maestro mission-control --render-check --size 120x40`
- [ ] CLI: `maestro <command-under-test>`
- [ ] Release: `bun run release:local && command -v maestro && maestro --version` (if user-facing)
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

**Next step**: if the user accepts the plan and wants to track it as a formal mission, invoke `maestro:mission-planning` with the plan content as input. The blueprint skill produces artifacts; mission-planning turns them into a tracked mission, then `maestro:conduct` executes it.

## Maestro Conventions

Follow these when generating plans:

- **Feature-folder layout**: place feature-scoped work under `src/features/<name>/` and follow the hexagonal pattern inside it: port -> adapter -> usecase -> command -> test. Plumbing goes in `src/infra/`, generic utilities in `src/shared/`. Cross-feature imports must go through `@/features/<other>` (the feature's `index.ts`); enforced by `bun run check:boundaries`
- **Conventional commits**: reference the commit types in the plan (feat, fix, refactor)
- **Version bumps**: note if the change requires a minor/patch bump
- **Build verification**: always include `bun run build && ./dist/maestro --version` to prove the fresh repo build
- **Release verification**: for user-facing CLI or TUI work, include `bun run release:local && command -v maestro && maestro --version`
- **TUI changes**: include `--render-check` verification if the change touches TUI
- **Binary verification**: default examples should use installed `maestro`; call out `./dist/maestro` only when validating the fresh build artifact

## Quality Checks

Before delivering:
- Plan spec covers all files that need changing
- Tasks have concrete acceptance criteria (not vague "it works")
- Verification commands are copy-pasteable
- Architecture diagram shows the change, not the entire system
- HTML renders cleanly with no console errors
- Both light and dark themes look intentional
