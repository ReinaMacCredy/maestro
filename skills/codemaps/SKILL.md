---
name: codemaps
description: Create token-aware architecture documentation for AI agent context. Use when starting work on unfamiliar codebase or before planning features that touch multiple areas.
---

# Codemaps

Token-efficient architecture documentation that gives AI agents the context they need to work effectively—navigation aids, not comprehensive docs.

## When to Use

- Starting work on an unfamiliar codebase
- Before planning features that touch multiple areas
- When agent lacks context about system architecture
- After major architectural changes (to update existing codemaps)

## Principles

1. **Concise**: Optimize for tokens, not prose
2. **Current**: Stale codemaps are worse than none
3. **Scoped**: One file per major area (API, database, auth, etc.)
4. **Actionable**: Tell agents what to do, not just what exists

## File Structure

```
CODEMAPS/
├── overview.md       # Project-level summary (always start here)
├── api.md            # API routes, middleware, request lifecycle
├── database.md       # Schema, queries, migrations
├── auth.md           # Authentication flow, tokens, security
├── frontend.md       # Components, state, patterns
└── [module].md       # One per major domain area
```

## How to Create

### Step 1: Identify Scope
Determine what area needs documentation. Start with `overview.md` for new projects.

### Step 2: Map Key Files
List the 5-10 most important files with one-line responsibilities.

### Step 3: Draw Data Flow
Create ASCII diagram showing how data moves through the area.

```
Input → Validation → Processing → Output
         ↓
      Error Handler → Logger
```

### Step 4: Document Patterns
List 3-5 patterns the codebase uses with when/how to apply.

### Step 5: Add Common Tasks
Table format: "If you want to X, do Y"

### Step 6: List Gotchas
Non-obvious behaviors that trip people up.

## What to Include

### Key Files Table
| File | Responsibility |
|------|----------------|
| `src/api/routes.ts` | Route definitions |
| `src/api/middleware/auth.ts` | JWT verification |

### Data Flow Diagrams
```
Request → Rate Limiter → Auth → Validation → Handler → Response
```

### Common Tasks
| Task | How |
|------|-----|
| Add endpoint | Define in routes.ts, create handler, add tests |
| Debug auth | Check JWT expiry, verify middleware order |

### Gotchas
- Cache invalidation requires manual Redis flush
- Auth middleware must come before validation

## Keeping Current

### When to Update
- After significant architectural changes
- When adding new modules/patterns
- When repeatedly explaining the same thing

### Review Cadence
- Quick scan: Weekly
- Full review: Monthly or after major features

### Automation Ideas
```bash
# Pre-commit reminder
if git diff --name-only | grep -q "src/api/"; then
  echo "API changed - consider updating CODEMAPS/api.md"
fi
```

## Anti-Patterns

**Too Detailed**: Codemaps are not comprehensive documentation
```markdown
# Bad
The UserService class was created in 2023 by the platform team...
[500 more lines]
```

**Too Vague**: Must provide actionable information
```markdown
# Bad
## Files
- There are files in src/
- They do things
```

**Out of Date**: Stale information is worse than none
```markdown
# Bad
Use the old AuthController (note: was replaced 6 months ago)
```

## Reference

See [references/CODEMAPS_TEMPLATE.md](references/CODEMAPS_TEMPLATE.md) for complete templates:
- Module codemap
- API codemap
- Database codemap
- Auth codemap
- Frontend codemap
