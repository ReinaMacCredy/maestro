# Style Enforcement Patterns

## Detecting Project Code Style

Before writing style rules, discover what the project actually does -- not what you think it should do.

### Discovery Sequence

1. **Read linter/formatter configs first** -- these are ground truth:
   - `.eslintrc*`, `biome.json`, `prettier*`, `ruff.toml`, `.rubocop.yml`, `clippy.toml`, `.editorconfig`
   - `tsconfig.json` (strict mode, module resolution, path aliases)
   - `pyproject.toml` `[tool.ruff]` / `[tool.black]` sections

2. **Sample 3-5 recent files** in the hot path (not generated code, not vendored):
   - Note: quote style, semicolons, indent width, trailing commas, import ordering
   - Check: naming conventions (camelCase vs snake_case, file naming)
   - Look for: custom patterns (barrel exports, re-export indices, path aliases)

3. **Check CI enforcement** -- `.github/workflows/*.yml`, `Makefile`, `justfile`:
   - Which linters run in CI? Those are non-negotiable.
   - Which checks are warnings vs errors?

4. **Cross-reference**: If linter config says `"semi": true` and all sampled files use semicolons, that's a confirmed convention. If config and code disagree, the code wins (config may be stale).

### What to Extract

| Source | Extract | AGENTS.md? |
|--------|---------|------------|
| Linter config | Quote style, semicolons, indent | No -- linter enforces it |
| Linter config | Rules agents can't auto-fix (naming, patterns) | Yes |
| Code samples | Import style, file naming | Yes, if not linter-enforced |
| Code samples | Architecture patterns (barrel exports, etc.) | Yes |
| tsconfig/pyproject | Path aliases, module resolution | Yes |
| CI config | Required checks before commit | Yes (build/test commands) |

## Writing Style Rules That Agents Follow

### Rule Anatomy

Every style rule needs three parts:

```
WHAT to do + WHEN it applies + WHY it matters
```

**Good rule:**
```markdown
- Imports: Use `.js` extension for all local imports (ESM strict mode -- build fails without it)
```
Three parts: what (use .js extension), when (local imports), why (build fails).

**Bad rule:**
```markdown
- Use proper import syntax
```
Missing all three parts. Agent can't act on this.

### Enforcement Levels

Not all style rules have equal weight. Use markers:

```markdown
## Code Style

### Hard Rules (build/lint fails)
- Import `.js` extensions on local imports (ESM strict)
- No default exports (eslint rule enforced in CI)

### Conventions (team preference)
- Prefer `type` over `interface` for object shapes
- Name files kebab-case, not camelCase

### Patterns (architectural)
- Database queries go through repository classes, never direct SQL in handlers
- Use `Result<T, E>` return type for fallible operations, not exceptions
```

Agents treat "Hard Rules" as non-negotiable and "Conventions" as strong preferences.

### Before/After: Style Section

**Before (noise):**
```markdown
## Code Style
- We use TypeScript
- Follow best practices
- Use consistent naming
- Keep code clean
- Write readable code
```
Zero behavior change. Agent already does all of this.

**After (signal):**
```markdown
## Code Style

### Hard Rules
- Semicolons: always (Biome enforces)
- Imports: `.js` extension on local imports (ESM strict)
- No `any` -- use `unknown` + type narrowing

### Conventions
- Files: kebab-case (`task-runner.ts` not `taskRunner.ts`)
- Types: `type` over `interface` unless extending
- Exports: named only, no default exports
- Errors: return `Result<T>`, don't throw (except in CLI entry points)
```

### Before/After: Architecture Section

**Before (describes code):**
```markdown
## Architecture
The system has a service layer that handles business logic.
Controllers receive HTTP requests and delegate to services.
Services interact with repositories for data access.
Repositories use the database driver for queries.
```
Agent can read the code and figure this out.

**After (changes behavior):**
```markdown
## Architecture
- New endpoints: controller in `src/routes/`, service in `src/services/`, repo in `src/repos/`
- Never import from `src/routes/` into `src/services/` (dependency direction violation)
- Shared types go in `src/types/`, not co-located with services
- Database migrations: `drizzle-kit generate` then manual review before `drizzle-kit push`
```

### Before/After: Gotchas Section

**Before (vague):**
```markdown
## Gotchas
- Be careful with async operations
- Watch out for circular dependencies
- Testing can be tricky
```

**After (actionable):**
```markdown
## Gotchas
- `ensureDir()` is sync despite the name -- do NOT use `ensureDirSync` (doesn't exist)
- `TaskPort.update()` merges shallow -- nested objects need spread: `{ config: { ...old, ...new } }`
- Test files must end `.test.ts` not `.spec.ts` (vitest config pattern match)
- `MAESTRO_HOME` env var overrides all path resolution -- unset it in test fixtures
```

## Validating Style Compliance

After generating or updating AGENTS.md, validate each style entry:

### The Three-Question Test

For each entry, ask:

1. **Is this already enforced by tooling?** If eslint/biome/ruff/clippy catches it, remove it from AGENTS.md. Duplicate enforcement adds noise.

2. **Would a fresh agent violate this without the entry?** If the convention matches language defaults or universal practice, remove it. Agents already know to use descriptive names.

3. **Can I show a specific failure this prevents?** If you can't point to a build failure, test failure, or real bug, the entry is speculative. Remove it or demote to a comment in relevant source files.

### Post-Generation Audit

```
For each AGENTS.md entry:
  [x] Not duplicated by linter/formatter config
  [x] Not observable from reading code structure
  [x] Prevents a specific, demonstrable mistake
  [x] Includes what + when + why
  [x] Uses enforcement level markers (Hard Rule / Convention / Pattern)
```

## What Goes Where

| Content | Location | Why |
|---------|----------|-----|
| Quote style, semicolons, indent | Linter config | Auto-fixable, auto-enforceable |
| Import ordering | Linter config + plugin | Auto-fixable |
| File naming convention | AGENTS.md | Linters don't enforce file names |
| Architecture boundaries | AGENTS.md | Not statically checkable |
| Build/test commands | AGENTS.md | Agent needs these immediately |
| API patterns, error handling style | AGENTS.md | Design decisions, not syntax |
| Type definition locations | AGENTS.md | Prevents scattered types |
| Dependency version constraints | `package.json` / lockfile | Tooling manages this |
| Commit message format | `.commitlintrc` or AGENTS.md | Depends on whether commitlint is set up |
| PR template | `.github/PULL_REQUEST_TEMPLATE.md` | GitHub enforces this automatically |

## Progressive Disclosure for Style

Not all style guidance belongs in AGENTS.md. Use progressive disclosure:

**AGENTS.md (always loaded, <100 lines):**
- Build/test commands
- Hard rules that break builds
- Architecture boundaries
- Non-obvious gotchas

**`.maestro/context/code_conventions.md` (loaded on demand):**
- Detailed naming patterns with examples
- File organization rules
- Test structure conventions
- API design patterns

**Source code comments (in-place):**
- Why a specific implementation choice was made
- Non-obvious constraints on a specific function
- Links to relevant docs/issues

The rule: if an agent needs it every session, it goes in AGENTS.md. If an agent needs it only when working in a specific area, it goes in a context file. If it's specific to one function, it's a code comment.
