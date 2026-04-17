# Prompt Leverage Framework

Vendor-neutral framework for strengthening prompts. Use these blocks selectively -- not every prompt needs every block.

## Source Synthesis

- Agent Flywheel contributes behavior controls: intensity, wider search, deeper analysis, fresh eyes, first-principles thinking, and future-self clarity.
- OpenAI prompt guidance contributes execution controls: clear objectives, explicit output contracts, tool persistence, dependency checks, verification loops, and completion criteria.

The framework chain: `Objective -> Context -> Constraints -> Work Style -> Tool Rules -> Output Contract -> Verification -> Done Criteria`

## Block Definitions

Use blocks selectively based on the Proportionality Principle. Each block below includes its definition and a filled-in example.

### Objective

State the task in one or two lines. Define success in observable terms.

```
## Objective
Add request rate limiting to the Express API in src/api/.
Limit: 100 requests per minute per API key. Return 429 with
{ error: "Rate limit exceeded", retryAfter: <seconds> } when exceeded.
```

**Weak objective signals:** "improve", "clean up", "make better", "fix things". Replace with specific, verifiable outcomes.

### Context

List sources, files, constraints, and unknowns the agent needs to act correctly.

```
## Context
- API routes: src/api/routes/ (12 route files, all use Express Router)
- Auth middleware: src/api/middleware/auth.ts (extracts API key from header)
- Redis: already configured in src/config/redis.ts (used for session cache)
- Traffic: ~500 req/s peak, ~50 unique API keys active concurrently
- Unknown: whether rate limit state should survive server restart (assume yes)
```

**Rule:** If removing a context item would not change the agent's behavior, remove it. Every line must be load-bearing.

### Constraints

Boundaries the agent must not cross. Separate from context because constraints restrict action, while context informs it.

```
## Constraints
- Do not modify existing route handlers. Rate limiting must be middleware-only.
- Do not add new npm dependencies. Use the existing `ioredis` client.
- Backwards compatible: existing API keys must work without changes.
- Rate limit headers (X-RateLimit-Remaining, X-RateLimit-Reset) on every response.
```

### Work Style

Set expectations for depth, breadth, and care level.

```
## Work Style
- Read all 12 route files before implementing to understand the middleware chain.
- Check for existing rate limiting (partial implementations, commented-out code).
- Consider edge cases: clock skew between Redis and app server, key expiry races.
```

Use sparingly. Most tasks do not need work style instructions -- the agent's defaults are adequate.

### Tool Rules

State when tools, browsing, or file inspection are required or forbidden.

```
## Tool Rules
- Read src/api/middleware/auth.ts first to understand the API key extraction.
- Run `bun test` after implementing the middleware.
- Do not run the dev server (it connects to production Redis).
```

**Only add tool rules when the default behavior would be wrong.** If the agent would naturally read the right files and run the right commands, skip this block.

### Output Contract

Define structure, formatting, and level of detail for the response.

```
## Output Contract
- Modified/created files only (no unchanged files).
- Include the full middleware file, not a diff.
- After the code: one paragraph explaining the rate limiting algorithm chosen and why.
```

### Verification

Checks the agent should perform before declaring done.

```
## Verification
- `bun test` passes (existing tests + new rate limit tests).
- Manual test: send 101 requests in 60 seconds, verify 429 on request 101.
- Check: rate limit state persists across server restart (Redis-backed).
```

**Scale verification to risk.** Read-only analysis needs no verification. Code that touches production data needs multiple checks.

### Done Criteria

What must be true for the task to be complete. This is the final gate.

```
## Done Criteria
- Rate limit middleware applied to all routes in src/api/routes/.
- Tests cover: normal request, limit hit (429), limit reset after window, header presence.
- No new dependencies in package.json.
- TypeScript compiles with no errors (`bun run build`).
```

## Intensity Levels

The same task at three intensity levels. Most tasks need Light or Standard.

### Light (simple, low-risk)

```
Add a `createdAt` timestamp field to the User type in src/types/user.ts.
Default to `new Date()` in the factory function. Update tests.
```

Two sentences. Objective and implicit done criteria. Enough for a straightforward addition.

### Standard (multi-step, moderate risk)

```
## Objective
Add `createdAt` and `updatedAt` timestamp fields to the User type.

## Context
- Type definition: src/types/user.ts
- Factory: src/factories/user.ts
- Database: src/db/migrations/ (need new migration)
- Queries: src/db/queries/user.ts (INSERT and UPDATE must set timestamps)

## Done Criteria
- Type updated with both fields (Date type, non-optional)
- Factory sets createdAt to now, updatedAt to now
- Migration adds columns with NOT NULL DEFAULT NOW()
- INSERT query sets createdAt, UPDATE query sets updatedAt
- All existing tests pass, new tests cover timestamp behavior
```

Five blocks. Enough structure to prevent wrong assumptions without over-specifying.

### Full (complex, high-risk, or persistent)

```
## Objective
Add audit timestamps (createdAt, updatedAt, deletedAt) to all entity types.
Implement soft delete across the query layer.

## Context
- Entity types: src/types/ (User, Post, Comment, Tag -- 4 types)
- Factories: src/factories/ (one per type)
- Database: PostgreSQL via src/db/. Migrations in src/db/migrations/.
- Queries: src/db/queries/ (CRUD for each entity)
- API layer: src/api/routes/ (must filter soft-deleted entities by default)
- Current state: no timestamp fields, hard delete via DELETE queries

## Constraints
- Soft delete only: never DELETE rows. Set deletedAt instead.
- All SELECT queries must exclude deletedAt IS NOT NULL unless explicitly
  requested (add `includeDeleted: boolean` parameter).
- Migration must be reversible (down migration drops columns).
- Do not change API response shapes -- createdAt/updatedAt are internal only.

## Work Style
- Implement in order: types -> migration -> queries -> factories -> API filters.
- Each step must compile before moving to the next.

## Tool Rules
- Run `bun test` after each entity is updated (4 checkpoints).
- Run the full migration up and down locally before finalizing.

## Verification
- All existing tests pass with updated query behavior.
- New tests: soft delete, undelete, list-with-deleted, list-without-deleted.
- Migration up/down/up produces identical schema.
- API returns 404 for soft-deleted entities (not 200 with null fields).

## Done Criteria
- All 4 entity types have createdAt, updatedAt, deletedAt fields.
- Soft delete works end-to-end: API DELETE -> sets deletedAt -> API GET returns 404.
- 0 TypeScript errors, 0 test failures, migration reversible.
```

Full framework. Every block is filled with task-specific content. No placeholders, no generic advice.

## Task-Type Adjustments

### Coding

- Emphasize repo context, file paths, smallest correct change, build/test validation, and edge cases.
- Always include a verification command (`bun test`, `npm test`, `cargo test`).

### Research

- Emphasize source quality, evidence gathering, synthesis, uncertainty disclosure, and citations.
- Specify output format (table, prose, bullet points) and length constraints.

### Writing

- Emphasize audience, tone, structure, length constraints, and revision criteria.
- Provide examples of desired style when the default is not what you want.

### Review

- Emphasize fresh-eyes critique, failure modes, alternatives, and severity classification.
- Specify whether the review should be advisory or blocking.

## Prompt Upgrade Heuristics

- Add missing blocks only when they materially improve execution.
- Do not turn a one-line request into a giant spec unless the task is genuinely complex.
- Preserve user language where possible so the upgraded prompt still feels native.
- Prefer concrete completion criteria over vague quality adjectives.
- If you remove a block and the expected output would not change, the block was noise.
- Test your prompt mentally: would two different competent agents interpret it the same way? If not, add specificity where they would diverge.

## Upgrade Rubric

Score each dimension 1-3. A prompt scoring 2+ on all dimensions is strong enough.

| Dimension | 1 (Weak) | 2 (Adequate) | 3 (Strong) |
|-----------|----------|--------------|------------|
| **Clarity** | Vague objective, adjective-heavy | Specific outcome, some ambiguity | Exact specification, no room for misinterpretation |
| **Context** | Missing file paths and references | Key files named, some gaps | All relevant files, types, and state documented |
| **Scope** | Unbounded or contradictory | Bounded but some edges unclear | Explicit boundaries, non-goals stated |
| **Verification** | None | Basic check ("tests pass") | Multi-step verification matched to risk level |
| **Proportionality** | Wildly over- or under-specified | Roughly matched to complexity | Exactly right amount of scaffolding |
