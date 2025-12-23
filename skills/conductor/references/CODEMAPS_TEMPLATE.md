# Codemaps Templates

Copy and customize these templates for your codebase.

---

## Module Codemap

```markdown
# [Module Name] Codemap

## Purpose
[One sentence: what this module does and why it exists]

## Key Files
| File | Responsibility |
|------|----------------|
| `src/module/index.ts` | Public API, exports |
| `src/module/core.ts` | Core logic |
| `src/module/types.ts` | Type definitions |

## Data Flow
```
Input → Validation → Processing → Output
         ↓
      Error Handler → Logger
```

## Patterns
- [Pattern 1]: [When/how to use]
- [Pattern 2]: [When/how to use]

## Dependencies
- **Uses**: [other modules this depends on]
- **Used by**: [modules that depend on this]

## Common Tasks
| Task | How |
|------|-----|
| Add new X | Create in `types.ts`, implement in `core.ts`, export from `index.ts` |
| Debug Y | Check logs in `~/.logs/module.log`, enable DEBUG=module:* |

## Gotchas
- [Thing that trips people up]
- [Non-obvious behavior]
```

---

## API Codemap

```markdown
# API Codemap

## Base Structure
```
src/api/
├── routes/          # Route definitions
├── middleware/      # Auth, validation, logging
├── handlers/        # Request handlers
├── schemas/         # Zod/Joi validation schemas
└── types/           # TypeScript types
```

## Authentication
- **Method**: [JWT / Session / API Key]
- **Middleware**: `src/api/middleware/auth.ts`
- **Protected routes**: All routes except `/health`, `/auth/*`

## Request Lifecycle
```
Request
  → Rate Limiter (middleware/rateLimit.ts)
  → Auth Check (middleware/auth.ts)
  → Validation (schemas/*.ts)
  → Handler (handlers/*.ts)
  → Response
```

## Endpoints
| Method | Path | Handler | Auth | Description |
|--------|------|---------|------|-------------|
| GET | /api/users | users.list | Yes | List all users |
| POST | /api/users | users.create | Yes | Create user |
| GET | /api/health | health.check | No | Health check |

## Error Handling
- All errors go through `middleware/errorHandler.ts`
- Standard format: `{ error: string, code: string, details?: object }`
- HTTP codes: 400 (validation), 401 (auth), 403 (forbidden), 500 (internal)

## Adding a New Endpoint
1. Define schema in `schemas/[resource].ts`
2. Create handler in `handlers/[resource].ts`
3. Add route in `routes/[resource].ts`
4. Register route in `routes/index.ts`
5. Add tests in `__tests__/api/[resource].test.ts`
```

---

## Database Codemap

```markdown
# Database Codemap

## Technology
- **Database**: [PostgreSQL / MongoDB / SQLite]
- **ORM**: [Prisma / Drizzle / Mongoose / None]
- **Migrations**: [Tool and location]

## Schema Overview
```
┌─────────────┐     ┌─────────────┐
│   users     │────<│   posts     │
└─────────────┘     └─────────────┘
       │                   │
       └───────┬───────────┘
               │
        ┌──────┴──────┐
        │  comments   │
        └─────────────┘
```

## Key Tables/Collections
| Table | Purpose | Key Columns |
|-------|---------|-------------|
| users | User accounts | id, email, password_hash, created_at |
| posts | User content | id, user_id, title, body, published_at |

## Indexes
- `users.email` (unique)
- `posts.user_id` (foreign key)
- `posts.published_at` (for sorting)

## Common Queries
```sql
-- Get user with posts
SELECT u.*, p.*
FROM users u
LEFT JOIN posts p ON p.user_id = u.id
WHERE u.id = ?

-- Recent posts (uses published_at index)
SELECT * FROM posts
WHERE published_at IS NOT NULL
ORDER BY published_at DESC
LIMIT 20
```

## Migrations
- Location: `prisma/migrations/` or `db/migrations/`
- Create: `pnpm prisma migrate dev --name description`
- Apply: `pnpm prisma migrate deploy`

## Connection
- Config: `DATABASE_URL` in `.env`
- Pool: `src/db/pool.ts`
- Max connections: 20 (configurable)
```

---

## Auth Codemap

```markdown
# Auth Codemap

## Method
[JWT with refresh tokens / Session-based / OAuth only]

## Flow
```
Login Request
  → Validate credentials (bcrypt compare)
  → Generate tokens (access + refresh)
  → Set refresh token cookie (httpOnly)
  → Return access token in body
```

## Token Structure
```typescript
// Access token payload
{
  sub: string,      // User ID
  email: string,
  roles: string[],
  exp: number,      // 15 minutes
  iat: number
}

// Refresh token payload
{
  sub: string,      // User ID
  jti: string,      // Token ID (for revocation)
  exp: number,      // 7 days
  iat: number
}
```

## Key Files
| File | Purpose |
|------|---------|
| `src/auth/jwt.ts` | Token generation/verification |
| `src/auth/password.ts` | Hashing with bcrypt |
| `src/auth/middleware.ts` | Route protection |
| `src/auth/refresh.ts` | Token refresh logic |

## Protected Routes
```typescript
// Apply to routes needing auth
app.use('/api/protected/*', authMiddleware)

// Check specific roles
app.use('/api/admin/*', authMiddleware, requireRole('admin'))
```

## Security Considerations
- Access tokens: Short-lived (15 min), stored in memory
- Refresh tokens: Long-lived (7 days), httpOnly cookie
- Password hashing: bcrypt with cost factor 12
- Rate limiting: 5 login attempts per minute per IP
```

---

## Frontend Codemap

```markdown
# Frontend Codemap

## Stack
- **Framework**: [Next.js / React / Vue]
- **Styling**: [Tailwind / CSS Modules / Styled Components]
- **State**: [Zustand / Redux / React Query]

## Structure
```
src/
├── app/              # Next.js app router pages
├── components/       # Reusable UI components
│   ├── ui/          # Primitives (Button, Input, Card)
│   └── features/    # Feature-specific components
├── hooks/           # Custom React hooks
├── lib/             # Utilities and helpers
├── stores/          # Zustand stores
└── types/           # TypeScript types
```

## Component Patterns
```typescript
// Prefer composition over configuration
<Card>
  <CardHeader>Title</CardHeader>
  <CardBody>Content</CardBody>
</Card>

// Use hooks for data fetching
const { data, isLoading } = useUsers()

// Colocate styles with components
// components/Button/Button.tsx
// components/Button/Button.module.css
```

## State Management
| Store | Purpose | Location |
|-------|---------|----------|
| auth | User session | `stores/auth.ts` |
| ui | Modal state, theme | `stores/ui.ts` |

## Data Fetching
- Use React Query for server state
- Queries in `hooks/queries/`
- Mutations in `hooks/mutations/`

## Adding a New Page
1. Create route in `app/[route]/page.tsx`
2. Add layout if needed in `app/[route]/layout.tsx`
3. Create feature components in `components/features/[Feature]/`
4. Add data hooks in `hooks/queries/use[Feature].ts`
```

---

## Minimal Codemap

For smaller projects, a single file may suffice:

```markdown
# Project Codemap

## Stack
Next.js 16 + Prisma + PostgreSQL + Tailwind

## Structure
app/           → Pages (app router)
components/    → React components
lib/           → Utilities, db client
prisma/        → Schema and migrations

## Key Files
- `lib/db.ts` - Prisma client
- `lib/auth.ts` - NextAuth config
- `app/api/` - API routes

## Patterns
- Server components by default
- Client components marked with 'use client'
- All data fetching in server components
- Mutations via server actions

## Commands
pnpm dev       → Start dev server
pnpm db:push   → Push schema to db
pnpm db:studio → Open Prisma Studio
```
