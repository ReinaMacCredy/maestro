# Step 11: Codebase Pattern Scan

**Progress: Step 11 of 16** -- Next: Implementation Plan

## Goal
Scan the existing codebase for patterns relevant to this track. Feed findings into plan generation context so the implementation plan builds on existing conventions rather than inventing new ones.

## Execution Rules
- This step is informational -- it produces context for step 12, not user-facing deliverables
- Scan based on project type and FR capability areas from step 8
- Focus on EXISTING patterns, not proposing new ones
- Keep findings concise -- this feeds into plan context, not the spec
- Time-box: spend no more than 5 minutes scanning

## Context Boundaries
- Approved spec (spec.md) is written and available
- Classification (project type) guides what to scan for
- FR capability areas guide which patterns to look for
- This step does NOT modify the spec

## Scan Sequence

1. **Determine Scan Focus**
   Based on project type and FR capability areas, decide what to scan for. Use this decision matrix:

   | FR capability area | What to scan | Example search |
   |--------------------|-------------|----------------|
   | API endpoints | Routing, middleware, error handling | Glob `**/routes/**`, Grep `app.get\|app.post\|router` |
   | Data persistence | ORM, schema, migrations | Glob `**/models/**`, `**/migrations/**` |
   | Authentication | Auth middleware, session handling | Grep `auth\|session\|jwt\|passport` |
   | Testing | Framework, conventions, helpers | Glob `**/*.test.*`, `**/*.spec.*` |
   | Configuration | Env vars, config loading | Glob `**/*.config.*`, Grep `process.env\|dotenv` |
   | UI components | Component structure, state management | Glob `**/components/**`, Grep `useState\|useEffect` |

   **Skip anything the track's FRs do not touch.** If no FR mentions auth, do not scan auth patterns.

2. **Execute Scan**
   Use Glob and Grep to search the codebase. For each capability area:

   a. **Find the closest existing analog.** If the track adds a "notification service" and the codebase already has an "email service", that is the pattern to follow.

   b. **Identify the file-level convention.** Where do tests go? Co-located or in a `tests/` directory? What is the naming pattern? (`foo.test.ts` vs `test_foo.py` vs `foo_test.go`)

   c. **Note the import/dependency style.** Does the codebase use dependency injection? Barrel exports? Direct imports?

   **Time-box: 5 minutes maximum.** If you cannot find a pattern in 5 minutes, note "no existing pattern found" and move on. The plan will establish the pattern.

3. **Compile Findings**
   Format as a concise context block:
   ```
   ## Codebase Patterns (for plan generation)

   - Test framework: {framework} in {directory pattern}
   - API pattern: {routing/controller style}
   - Data layer: {ORM/query pattern}
   - Config: {how config is managed}
   - Similar existing: {components/modules that are analogous}
   - Conventions: {naming, file organization, code style notes}
   ```

   Omit categories that are not relevant. Only include what was actually found.

   **Concrete example:**
   ```
   ## Codebase Patterns (for plan generation)

   - Test framework: vitest with co-located tests (src/foo.ts -> src/foo.test.ts)
   - API pattern: Hono router in src/routes/, middleware in src/middleware/
   - Data layer: Drizzle ORM, schemas in src/db/schema.ts, migrations via drizzle-kit
   - Similar existing: src/services/email.ts is analogous to the notification service we are building
   - Conventions: barrel exports in index.ts per directory, zod for input validation
   ```

4. **Present to User**
   Show findings briefly:
   - "I scanned the codebase and found these relevant patterns that will inform the implementation plan: {summary}"
   - No approval needed -- this is informational context
   - User can add notes or corrections before proceeding

## Quality Checks
- [ok] Scan focused on relevant patterns (not everything)
- [ok] Findings are concise and actionable
- [ok] Existing conventions identified (not inventing new patterns)
- [ok] Time-boxed (not an exhaustive audit)

## Anti-patterns
- [x] Exhaustive codebase audit -- this is a quick scan for plan context
- [x] Proposing new patterns instead of documenting existing ones
- [x] Skipping this step entirely -- even small codebases have conventions worth noting
- [x] Spending more than 5 minutes scanning
- [x] Including irrelevant patterns that do not relate to this track's FRs

## Next Step
Read and follow `reference/steps/step-12-plan.md`.
