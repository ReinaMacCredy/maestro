# Prompt Anti-Patterns

Named patterns that weaken prompts. Each entry includes the pattern, why it fails, an example, and the fix.

## Vagueness Anti-Patterns

### The Fog

**Pattern:** Objective is a feeling, not a specification.

**Why it fails:** The agent fills in "better" with its own defaults, which may not match yours.

```
-- Weak: "Make this code better"
-- Strong: "Reduce the cyclomatic complexity of `processOrder` from 14 to under 6
   by extracting validation into a separate function."
```

**Fix:** Replace adjectives (better, cleaner, faster) with measurable criteria.

### The Ghost

**Pattern:** References entities without naming them.

**Why it fails:** The agent guesses which file, function, or variable you mean -- often wrong.

```
-- Weak: "Fix the bug in the handler"
-- Strong: "Fix: `handleWebhook` in src/api/webhooks.ts silently drops events
   when payload.type is undefined."
```

**Fix:** Every reference must include a file path, function name, or variable name.

### The Wish

**Pattern:** Expresses a desire without a concrete target.

**Why it fails:** No way to verify success. The agent does something and declares victory.

```
-- Weak: "It would be nice if the API was faster"
-- Strong: "Reduce p95 latency of GET /api/users from 200ms to under 50ms.
   Profile with `autocannon` before and after."
```

**Fix:** Convert wishes to measurable outcomes with verification steps.

### The Hedge

**Pattern:** Tentative language that weakens instruction priority.

**Why it fails:** Agents interpret hedging as optional. "Maybe consider" becomes "skip."

```
-- Weak: "You might want to perhaps consider looking at the error handling"
-- Strong: "Add error handling to all database calls in src/db/queries/.
   Wrap each query in try/catch and return Result<T, DbError>."
```

**Fix:** Use imperative mood. State what to do, not what to maybe think about.

## Over-Specification Anti-Patterns

### The Novel

**Pattern:** 2000-word prompt for a task that needs 50 words.

**Why it fails:** Real instructions are buried in noise. The agent may miss them or weight them equally with filler.

```
-- Weak: [500 words of context, style guide, philosophy, and caveats
   for adding a null check]
-- Strong: "Add null check for `user.email` in src/api/handlers/profile.ts:47.
   Return 400 with { error: 'Email required' } if null."
```

**Fix:** Match prompt length to task complexity. Use the Proportionality Principle.

### The Cage

**Pattern:** Dictates every line of implementation.

**Why it fails:** You lose the agent's ability to find better solutions. Also brittle -- if the code does not match your mental model exactly, the agent forces it anyway.

```
-- Weak: "On line 47, add `if (x === null) return`. On line 48, change
   the variable name from `data` to `userData`. On line 52..."
-- Strong: "Refactor `fetchUserData` to handle null responses gracefully.
   Return early with a default value instead of letting null propagate."
```

**Fix:** Specify WHAT and WHY. Let the agent decide HOW. Constrain only when the implementation approach genuinely matters.

### The Parrot

**Pattern:** Copy-pasted template with unfilled placeholders or irrelevant blocks.

**Why it fails:** The agent processes placeholder text as real instructions, producing confused output. Irrelevant blocks dilute attention from real instructions.

```
-- Weak:
   "## Objective
   [describe your objective here]
   ## Context
   [list relevant files]
   ## Constraints
   - Follow best practices
   - Write clean code"

-- Strong:
   "## Objective
   Add pagination to GET /api/posts. Return 20 items per page.
   ## Context
   - Route handler: src/api/routes/posts.ts
   - DB query: src/db/queries/posts.ts (already supports LIMIT/OFFSET)"
```

**Fix:** Fill every placeholder with task-specific content, or delete the block entirely.

## Structural Anti-Patterns

### The Shotgun

**Pattern:** Multiple unrelated objectives in one prompt.

**Why it fails:** The agent must context-switch between unrelated tasks, increasing the chance it drops or conflates requirements.

```
-- Weak: "Add rate limiting to the API, also fix the login bug,
   and update the README with the new deployment steps."

-- Strong: Three separate prompts:
   1. "Add rate limiting to the API. [details]"
   2. "Fix: handleLogin returns 200 on invalid password. [details]"
   3. "Update README: add Docker deployment steps. [details]"
```

**Fix:** One objective per prompt. If tasks are related, use a numbered plan with explicit sequencing.

### The Echo Chamber

**Pattern:** Multiple blocks say the same thing in different words.

**Why it fails:** Wastes tokens. Worse, slight wording differences between repetitions create ambiguity about which version is authoritative.

```
-- Weak:
   "Objective: Make the function handle errors properly.
   Constraints: Ensure all errors are handled.
   Verification: Check that error handling is complete.
   Done: All errors are handled properly."

-- Strong:
   "Objective: Add try/catch to all async functions in src/api/handlers/.
   Return { error: string, code: number } on failure instead of throwing.
   Done: All 8 handler functions return error objects. No unhandled rejections."
```

**Fix:** Say it once, in the most specific block. Delete the echoes.

### The Time Capsule

**Pattern:** Instructions reference outdated APIs, deprecated patterns, or old file structures.

**Why it fails:** The agent follows stale instructions and produces code that does not compile or conflicts with the current codebase.

```
-- Weak: "Use the `request` library for HTTP calls" (deprecated since 2020)
-- Strong: "Use the native `fetch` API for HTTP calls (Node 18+)"
```

**Fix:** Review instructions against current project state before including them. For persistent instructions (CLAUDE.md), schedule periodic reviews.

## Intensity Anti-Patterns

### The Steamroller

**Pattern:** Full framework applied to a trivial task.

**Why it fails:** The agent spends time processing blocks that add no value. Worse, it may over-engineer the implementation to match the perceived complexity.

```
-- Weak: [Full Objective/Context/Constraints/Tool Rules/Output Contract/
   Verification/Done Criteria for renaming a variable]

-- Strong: "Rename `usr` to `user` in src/models/account.ts. Update all
   references in the same file."
```

**Fix:** Trivial tasks get trivial prompts. Zero scaffolding for single-action work.

### The Undercoat

**Pattern:** Complex multi-step task with no structure at all.

**Why it fails:** The agent makes assumptions about scope, order, constraints, and success criteria. Each assumption is a potential failure point.

```
-- Weak: "Migrate the database from MySQL to PostgreSQL"

-- Strong: [Full framework with scope boundaries, migration order,
   data mapping, rollback strategy, verification queries]
```

**Fix:** Complex tasks need structure. Use the framework blocks proportional to the real complexity.

## Context Anti-Patterns

### The Assumption

**Pattern:** Assumes the agent knows things it does not.

**Why it fails:** The agent hallucinates the missing context or asks a question, breaking flow.

```
-- Weak: "Update the config to use the new format"
-- Strong: "Update src/config/app.ts: change the `database` field from
   a connection string (string) to a structured object ({ host, port,
   name, user, password }). See src/config/types.ts for the new type."
```

**Fix:** Provide every piece of context the agent needs to act without guessing. If in doubt, include it.

### The Dump Truck

**Pattern:** Includes massive amounts of irrelevant context.

**Why it fails:** Real instructions compete with noise for attention. In long contexts, agents may lose track of what matters.

```
-- Weak: [Entire file contents pasted inline when only 3 lines matter]
-- Strong: "In src/utils/parse.ts, function `parseDate` (lines 45-60):
   change the date format from 'MM/DD/YYYY' to 'YYYY-MM-DD'."
```

**Fix:** Include only context that changes the agent's behavior. Point to files by path instead of pasting them.

## Verification Summary

When diagnosing a prompt, scan for these patterns in order:

1. **Vagueness** (Fog, Ghost, Wish, Hedge) -- most common, fix first
2. **Structure** (Shotgun, Echo Chamber, Time Capsule) -- fix second
3. **Intensity** (Steamroller, Undercoat) -- calibrate third
4. **Context** (Assumption, Dump Truck) -- fix last
5. **Over-specification** (Novel, Cage, Parrot) -- check after everything else

A prompt can have multiple anti-patterns. Fix them in this priority order.
