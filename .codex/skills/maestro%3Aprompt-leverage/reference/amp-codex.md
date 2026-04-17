# Amp, Codex, and Other Coding Agent Patterns

Patterns for prompts targeting Amp (Sourcegraph), Codex CLI (OpenAI), Cursor, Windsurf, and other coding agents. Focuses on differences from Claude Code and GPT that affect prompt construction.

## Amp (Sourcegraph)

### AGENTS.md Integration

Amp reads AGENTS.md for project-level instructions. Similar role to CLAUDE.md but with different conventions.

#### What Goes in AGENTS.md

```markdown
# Project: my-api

## Build & Test
- `npm test` -- run all tests
- `npm run build` -- compile TypeScript
- `npm run lint` -- run ESLint

## Code Conventions
- Use functional components with hooks (no class components)
- Error handling: throw typed errors from src/errors/
- All API responses use the envelope in src/types/response.ts

## File Organization
- Routes: src/routes/<domain>.ts
- Services: src/services/<domain>.ts
- Types: src/types/<domain>.ts
- Tests: colocated as <file>.test.ts
```

#### AGENTS.md vs. Inline Prompt

Same principle as CLAUDE.md: persistent rules go in AGENTS.md, task-specific instructions go inline.

```
-- AGENTS.md: "Run `npm test` after every change"
-- Inline: "Add pagination to the /posts endpoint, 20 items per page"
```

### Amp-Specific Patterns

**Context awareness:** Amp has strong codebase awareness through Sourcegraph's code graph. Leverage this.

```
-- Weak: "Find all usages of the deprecated `fetchUser` function"
   (Amp can do this, but the prompt should state what to DO with the results)
-- Strong: "Replace all usages of `fetchUser` with `getUser` from
   src/services/user.ts. There are approximately 15 call sites. Update
   imports and verify each call site's arguments match the new signature."
```

**Multi-file operations:** Amp handles cross-file changes well. Be explicit about scope.

```
## Objective
Rename the `UserDTO` type to `UserResponse` across the codebase.

## Scope
- Type definition: src/types/user.ts
- All importers (use code search to find them)
- Update JSDoc references in any comments

## Constraints
- Do not rename UserDTO in test fixtures (keep as-is for backwards compat with API snapshots)
- Verify each import compiles after rename
```

### Before/After: Amp Prompt

<Bad>
```
Clean up the user service
```
</Bad>

<Good>
```
Refactor src/services/user.ts:
- Extract the email validation logic (lines 45-78) into src/utils/validation.ts
- Replace the 3 duplicated error formatting blocks with a shared
  `formatServiceError` helper
- Keep all existing tests passing

Run `npm test -- --filter user` after each extraction.
```
</Good>

## Codex CLI (OpenAI)

### Sandbox Model

Codex runs in a sandboxed environment with network disabled by default. This fundamentally affects prompt construction.

**Key difference from Claude Code:** Codex cannot fetch external resources during execution. All context must be provided upfront or exist in the repository.

#### Prompts Must Be Self-Contained

```
-- Weak (fails in sandbox): "Look up the latest Express.js middleware
   documentation and implement rate limiting based on it"

-- Strong (works in sandbox): "Implement rate limiting middleware for Express.
   Use the sliding window algorithm: track request timestamps per IP in memory.
   Config: 100 requests per 60-second window. Return 429 with Retry-After header."
```

#### File Context Is Local

Codex reads files from the repo but cannot browse the web or access external APIs.

```
-- Weak: "Check the API documentation for the correct endpoint format"
-- Strong: "The endpoint format is POST /api/v2/users with body
   { name: string, email: string }. See src/api/routes/users.ts for the
   current implementation."
```

### Codex-Specific Patterns

**Instruction clarity:** Codex benefits from very explicit step-by-step instructions.

```
## Task
Add input validation to the user registration endpoint.

## Steps
1. Read src/api/routes/users.ts to understand the current handler
2. Create src/api/validators/user.ts with validation functions for:
   - email: must contain @ and valid TLD
   - name: 1-100 characters, no special characters except hyphen and space
   - password: minimum 8 characters, at least one number and one letter
3. Import validators in the route handler
4. Return 400 with { errors: [...] } array if validation fails
5. Add tests in src/api/validators/__tests__/user.test.ts
6. Run: npm test
```

**Commit messages:** Codex auto-commits. Guide the commit message style.

```
## Git
- Commit message format: "type(scope): description"
- Types: feat, fix, refactor, test, docs
- Example: "feat(auth): add API key validation middleware"
```

### Before/After: Codex Prompt

<Bad>
```
Add tests for the auth module
```
</Bad>

<Good>
```
Add unit tests for src/auth/token.ts.

Cover these functions:
- generateToken(userId: string): returns a valid JWT with userId in payload
- verifyToken(token: string): returns decoded payload for valid tokens,
  throws TokenExpiredError for expired tokens, throws InvalidTokenError
  for malformed tokens
- refreshToken(token: string): returns new token with extended expiry

Test file: src/auth/__tests__/token.test.ts
Use the existing test helper in src/test/setup.ts for JWT secret configuration.
Run: npm test -- --filter token
```
</Good>

## Cursor

### Rules Files

Cursor reads `.cursorrules` (project root) or `.cursor/rules` directory for persistent instructions.

#### .cursorrules Patterns

```
# Project: my-app

## Language & Framework
- TypeScript 5.x with strict mode
- React 19 with Server Components
- Next.js 15 App Router

## Code Style
- Functional components only. No class components.
- Use `use` hook for data fetching in client components.
- Server components are the default. Add "use client" only when needed.

## File Naming
- Components: PascalCase.tsx (UserProfile.tsx)
- Utilities: camelCase.ts (formatDate.ts)
- Types: camelCase.ts in src/types/ (user.ts)
- Tests: <name>.test.ts colocated with source

## Forbidden
- No `any` types
- No `eslint-disable` comments
- No barrel exports (index.ts re-exports)
```

### Cursor-Specific Patterns

**Inline edits:** Cursor's inline edit mode (Ctrl+K) works best with short, specific prompts.

```
-- Weak (for inline edit): "Improve this function's error handling
   by adding proper try-catch blocks and returning meaningful error
   messages to the caller"

-- Strong (for inline edit): "Add try-catch. Return { error: string }
   on failure."
```

**Chat mode vs. inline mode:** Different prompt styles for different modes.

```
-- Chat mode (exploration, multi-file): Use full framework blocks
-- Inline mode (single edit): One sentence, specific change
-- Composer mode (multi-file generation): Intermediate detail level
```

## Windsurf

### Cascade Rules

Windsurf reads `.windsurfrules` for project instructions. Similar to `.cursorrules`.

**Windsurf-specific:** Cascade has strong multi-file awareness. Explicit file listings are less critical than with other agents, but scope constraints are more important (to prevent over-eager changes).

```
## Scope Control
- Only modify files in src/api/. Do not touch src/frontend/ or src/db/.
- If a change requires modifying files outside the scope, stop and
  describe what is needed. Do not proceed without confirmation.
```

## Cross-Agent Patterns

Patterns that work across all coding agents regardless of provider.

### Universal Prompt Structure

```
## What (objective)
[one sentence: what to do and what success looks like]

## Where (scope)
[file paths, function names, line ranges]

## How (constraints)
[specific approaches required or forbidden]

## Verify (done criteria)
[commands to run, conditions to check]
```

This four-block structure works in Claude Code, Amp, Codex, Cursor, and Windsurf.

### Build/Test Commands

Every coding agent prompt for a change should include the verification command:

```
-- TypeScript: "Run `bun test` / `npm test` after changes"
-- Python: "Run `pytest` after changes"
-- Rust: "Run `cargo test` after changes"
-- Go: "Run `go test ./...` after changes"
```

Without this, agents may declare "done" without verification.

### Scope Discipline

All coding agents tend to make changes beyond what was requested. Constrain scope explicitly.

```
-- Weak: "Fix the bug" (agent may refactor surrounding code)
-- Strong: "Fix the null pointer in handleRequest, line 47.
   Do not modify any other functions."
```

### Error Handling Instructions

When the agent encounters errors, state what to do:

```
## On Failure
- If tests fail after your change, fix the issue. Do not revert.
- If the build breaks, diagnose the type error before making more changes.
- If a file you need does not exist, stop and ask. Do not create it.
```
