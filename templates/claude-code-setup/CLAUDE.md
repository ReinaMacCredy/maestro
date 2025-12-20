# [Project Name]

<!-- 
CLAUDE.md - Project context loaded automatically at session start.
Keep this concise: architecture overview, key commands, important files.
For detailed workflows, use AGENTS.md. For constraints, use .claude/rules/.
-->

## Overview

[One paragraph: what this project does and its core purpose]

## Tech Stack

- **Language**: [TypeScript/Python/Go/Rust]
- **Framework**: [Next.js/FastAPI/Gin/Axum]
- **Database**: [PostgreSQL/MongoDB/SQLite]
- **Other**: [Redis, S3, etc.]

## Key Paths

| Path | Purpose |
|------|---------|
| `src/` | Application source code |
| `tests/` | Test files |
| `docs/` | Documentation |
| `scripts/` | Build and utility scripts |

## Commands

```bash
# Development
pnpm dev              # Start dev server
pnpm build            # Production build
pnpm test             # Run tests
pnpm lint             # Lint code
pnpm typecheck        # Type checking

# Database
pnpm db:push          # Push schema changes
pnpm db:migrate       # Run migrations
pnpm db:studio        # Open database UI
```

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Client    │────▶│     API     │────▶│  Database   │
└─────────────┘     └─────────────┘     └─────────────┘
                          │
                    ┌─────┴─────┐
                    │  Services │
                    └───────────┘
```

## Key Files

| File | What It Does |
|------|-------------|
| `src/index.ts` | Application entry point |
| `src/config.ts` | Configuration loading |
| `src/routes/` | API route definitions |

## Conventions

- [Naming convention for files/functions]
- [Error handling pattern]
- [Logging approach]

## Environment

Required environment variables:
- `DATABASE_URL` - Database connection string
- `API_KEY` - External API key (if applicable)

See `.env.example` for full list.

---

## References

<!-- Use @ to include other files -->
<!-- @docs/architecture.md -->
<!-- @AGENTS.md -->
