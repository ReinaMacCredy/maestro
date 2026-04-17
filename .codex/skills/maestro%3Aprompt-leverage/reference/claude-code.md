# Claude Code Prompt Patterns

Patterns for prompts targeting Claude Code (Anthropic's CLI agent), Claude in API usage, and Claude-based coding assistants. Covers Claude Opus 4.6, Sonnet 4.6, and Haiku 4.5.

## CLAUDE.md Integration

Claude Code reads CLAUDE.md files for persistent project instructions. Understanding what belongs in CLAUDE.md vs. inline prompts is the most important leverage point.

### What Goes in CLAUDE.md

Persistent instructions that apply to every interaction with the project:

- Build and test commands (`bun test`, `bun run build`)
- Code style rules (naming conventions, error handling patterns)
- Architectural constraints (no new dependencies, prefer X over Y)
- File organization conventions
- Tool usage rules (which tools to use, which to avoid)

### What Goes in the Inline Prompt

Task-specific instructions that change per interaction:

- The specific objective for this task
- Files and functions relevant to this task
- Done criteria for this specific change
- Context that only matters for this work

### Before/After: CLAUDE.md Setup

<Bad>
```markdown
# Project Rules
- Use TypeScript
- Write tests
- Be careful with the code
- Follow best practices
```
Every line is either obvious or unmeasurable.
</Bad>

<Good>
```markdown
# Build
- `bun test` -- run all tests
- `bun run build` -- compile TypeScript
- `bun test --filter <name>` -- run specific test file

# Code Style
- TypeScript strict mode. No `any` outside test fixtures.
- Error handling: return Result<T, E> types. Never throw in library code.
- Imports: node builtins, then external packages, then internal. Blank line between groups.

# Architecture
- No new npm dependencies without explicit approval.
- Database queries go in src/db/queries/. No raw SQL in route handlers.
- All API responses use the envelope type in src/types/api.ts.

# Workflow
- Run `bun test` after every file change. Do not proceed until green.
- Commit after each working increment.
```
Every rule is concrete, verifiable, and actionable.
</Good>

## XML Tags

XML tags are Claude's primary structural mechanism. Use them to separate instructions, context, and examples.

### Structural Separation

```xml
<instructions>
Refactor the authentication middleware to support both JWT and API key auth.
</instructions>

<context>
Current middleware: src/api/middleware/auth.ts (JWT only)
API key storage: src/db/queries/apiKeys.ts
Test file: src/api/middleware/__tests__/auth.test.ts
</context>

<constraints>
- Do not break existing JWT authentication
- API key auth checks the X-API-Key header
- Both auth methods set req.user with the same shape
</constraints>
```

### Few-Shot Examples

The most reliable way to steer output format. Use 3-5 examples wrapped in `<example>` tags.

```xml
<examples>
<example>
<input>Add validation for the email field</input>
<output>
Added email validation to src/api/validators/user.ts:
- Checks for @ symbol and valid TLD
- Returns { valid: false, error: "Invalid email format" } on failure
- Added 3 test cases: valid email, missing @, missing TLD
</output>
</example>
</examples>
```

### When NOT to Use XML Tags

- Simple one-line prompts (adds noise for no benefit)
- When the prompt has no structural ambiguity
- When markdown headers achieve the same separation

**Rule:** XML tags help when there are 3+ distinct sections that could be confused. Below that threshold, plain text or markdown headers work better.

## Clarity and Structure

**Golden rule:** Show your prompt to a colleague with minimal context. If they would be confused, Claude will be too.

**Explain WHY, not just WHAT.** Claude generalizes from motivation better than from bare rules.

```
-- Weak: "NEVER use console.log"
-- Strong: "Do not use console.log because this project uses a structured
   logger (src/utils/logger.ts) that includes request IDs and timestamps.
   Use logger.info(), logger.error(), etc."
```

**Be specific about desired output format and constraints:**
- Use numbered lists when step order matters
- Use bullet points when completeness matters
- Use code blocks when you want code, not description

## Output Control

**Tell Claude what TO DO, not what NOT to do.**

```
-- Weak: "Do not use markdown formatting in your response"
-- Strong: "Respond in plain prose paragraphs. No bullet points, no headers,
   no bold text."
```

**Match your prompt style to the desired output.** If your prompt is in markdown with heavy formatting, the response will mirror that. If you want prose, write your prompt in prose.

## Tool Use Patterns

### Action vs. Suggestion

Claude follows literal intent. Be explicit about whether you want action or analysis.

```
-- "Can you suggest improvements to this function?" --> Claude suggests only
-- "Improve this function's error handling." --> Claude edits the file
-- "What would you change about this function?" --> Claude describes changes
-- "Change this function to handle null inputs." --> Claude makes the change
```

### Proactive Action (Default for Claude Code)

```xml
<tool_behavior>
Default to implementing changes rather than describing them.
If the intent is unclear, infer the most useful action and proceed.
Read files to discover context instead of guessing.
</tool_behavior>
```

### Conservative Action (When You Want Analysis First)

```xml
<tool_behavior>
Do not make changes until explicitly instructed.
Default to providing analysis, options, and recommendations.
Ask before modifying files.
</tool_behavior>
```

### Parallel Tool Calls

```xml
<parallel_tools>
Make all independent tool calls in parallel. Do not serialize
calls that can run concurrently. Never use placeholder values
for parameters that depend on previous results.
</parallel_tools>
```

### Tool Intensity

Claude 4.6 is more responsive to system prompts than earlier versions. Dial back aggressive tool prompting.

```
-- Weak: "CRITICAL: You MUST ALWAYS use this tool when..."
   (causes overtriggering, tool is called when not needed)
-- Strong: "Use this tool when you need to verify file contents before editing."
   (normal language, appropriate triggering)
```

## Thinking and Reasoning

### Extended Thinking

For complex tasks, Claude can use extended thinking to reason before acting. Control this with prompt structure, not API parameters.

```
-- Light thinking (simple tasks):
   "Add a null check for user.email in the handler."
   (no thinking needed, direct action)

-- Standard thinking (moderate tasks):
   "Before implementing, analyze the current error handling pattern in
   src/api/handlers/ and propose a consistent approach."
   (triggers natural reasoning without explicit thinking blocks)

-- Deep thinking (complex architecture):
   "Think carefully about the tradeoffs between approach A and approach B.
   Consider: performance under load, maintainability, migration cost,
   and backwards compatibility. Then recommend one approach with justification."
   (explicit thinking request for genuinely complex decisions)
```

### Thinking Calibration Rule

If the task has one obviously correct approach, do not ask for extended thinking. Reserve it for genuine ambiguity where reasoning visibly improves the outcome.

## Agentic Patterns

### Multi-Step Task Decomposition

For complex tasks, structure the prompt to guide Claude through phases:

```
## Phase 1: Understand
- Read src/api/routes/ to catalog all endpoints
- Read src/db/queries/ to understand the data layer
- List all places where auth is checked

## Phase 2: Implement
- Add rate limit middleware to src/api/middleware/
- Apply to all routes in src/api/routes/
- Add rate limit tests

## Phase 3: Verify
- Run `bun test` (all existing + new tests pass)
- Check: no route is missing rate limiting
```

### Iterative Refinement

When the first pass may not be perfect, build in review:

```
After implementing, review your changes:
- Does every new function have a test?
- Are there any edge cases the tests miss?
- Is there duplicated logic that should be extracted?
Fix any issues found before declaring done.
```

## Before/After: Full Prompt Transformation

### Raw Prompt (Weak)

```
Refactor the API to use better error handling
```

### Upgraded Prompt (Strong)

```
## Objective
Refactor error handling in src/api/handlers/ to use Result types
instead of thrown exceptions.

## Context
- 8 handler files in src/api/handlers/, each exports 2-4 route handlers
- Current pattern: handlers throw HttpError, caught by global error middleware
- Target pattern: handlers return Result<Response, ApiError>
- Result type: already defined in src/types/result.ts
- ApiError type: needs to be created (include statusCode, message, details)

## Constraints
- Migrate one handler file at a time (keep existing handlers working)
- Global error middleware stays as fallback for uncaught errors
- API response shapes must not change (same JSON, same status codes)

## Done Criteria
- All 8 handler files return Result types instead of throwing
- ApiError type created in src/types/errors.ts
- Each handler file has updated tests verifying error paths
- `bun test` passes, `bun run build` compiles cleanly
- Global error middleware logs a warning if it catches anything
  (indicates a missed migration)
```

## Common Pitfalls

| Pitfall | What Happens | Fix |
|---------|-------------|-----|
| Telling Claude what NOT to do | It focuses on the forbidden thing | Tell it what TO do instead |
| ALL CAPS emphasis | Overtriggering, reduced nuance | Normal language. Claude reads system prompts carefully. |
| Pasting entire files as context | Dilutes attention, wastes tokens | Point to files by path. Claude can read them. |
| Vague "follow best practices" | Claude picks its own "best" | State the specific practices you want |
| "Be careful" / "Be thorough" | No behavioral change | State specific checks: "Run tests", "Check for null" |
| Asking Claude to "not hallucinate" | Not actionable by the model | Provide verification: "Confirm file exists before editing" |
