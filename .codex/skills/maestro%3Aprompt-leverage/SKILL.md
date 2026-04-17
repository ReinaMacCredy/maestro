---
name: maestro:prompt-leverage
description: Strengthen a raw user prompt into an execution-ready instruction set for Claude Code, Amp, Codex, or another AI agent. Use when the user wants to improve an existing prompt, build a reusable prompting framework, wrap the current request with better structure, add clearer tool rules, or create a hook that upgrades prompts before execution.
---

# Prompt Leverage

Turn the user's current prompt into a stronger working prompt without changing the underlying intent. Preserve the task, fill in missing execution structure, and add only enough scaffolding to improve reliability.

**Core principle:** A prompt is strong when removing any part of it would make the output worse. Everything else is noise.

## When to Use

**Always:**
- Multi-step coding tasks (refactor, feature, migration)
- Research or analysis with specific deliverables
- Prompts destined for CLAUDE.md, AGENTS.md, or similar persistent instructions
- Reusable templates or hooks

**Sometimes (light touch only):**
- Single-file bug fixes with clear reproduction
- Writing tasks with well-defined audience and format

**Never (skip prompt leverage entirely):**
- One-shot questions ("What does this function do?")
- Simple commands ("Run the tests", "Format this file")
- Tasks the agent already handles well without scaffolding

Thinking "this simple task needs full framework blocks"? Stop. That is over-engineering. A three-line task needs a three-line prompt.

## The Proportionality Principle

```
Prompt scaffolding should be proportional to task complexity.
More scaffolding than needed = noise that dilutes real instructions.
```

| Task Complexity | Scaffolding Level | What to Add |
|----------------|-------------------|-------------|
| Trivial (one action) | None | Nothing. The raw prompt is fine. |
| Simple (2-3 steps, one file) | Light | Objective + done criteria only |
| Medium (multi-file, clear scope) | Standard | Objective + context + output contract + done criteria |
| Complex (cross-cutting, ambiguous) | Full | All relevant framework blocks |
| Persistent (CLAUDE.md, templates) | Full + examples | Framework blocks + few-shot examples + anti-pattern guards |

## Workflow

1. Read the raw prompt and identify the real job to be done.
2. Diagnose weaknesses using the Transformation Checklist below.
3. Determine task complexity and match to proportionality level.
4. Infer the target agent if specified or implied (see Provider References below).
5. Apply fixes from the checklist, using framework blocks from `reference/framework.md`.
6. If a specific provider is targeted, consult the relevant vendor reference.
7. Verify: remove any block that does not materially improve execution.
8. Return the upgraded prompt and a short explanation of what changed.

## Transformation Checklist

Diagnose the raw prompt against this table. Each row is a weakness, its signal, and the fix.

| Weakness | Signal | Fix |
|----------|--------|-----|
| **Vague objective** | "Make this better", "fix the code" | State exact desired outcome: "Refactor `parseConfig` to return `Result<Config, ParseError>` instead of throwing" |
| **Missing context** | References files/APIs without naming them | Add explicit file paths, function signatures, or API endpoints |
| **No success criteria** | No way to verify the output is correct | Add done criteria: "Tests pass, no type errors, handles empty input" |
| **Unbounded scope** | "Improve the codebase", "clean up everything" | Constrain: "Refactor only the validation module. Do not touch routing." |
| **Wrong intensity** | Simple task wrapped in 500 words of scaffolding | Strip to essentials. Match proportionality level. |
| **Missing constraints** | No mention of backwards compatibility, perf, or style | Add constraints that the agent would otherwise guess wrong |
| **No output format** | Agent picks its own structure, often wrong | Specify: "Return a markdown table", "Output only the changed file", "Respond with JSON" |
| **Implicit tool expectations** | Assumes agent will read files or run tests without saying so | Make explicit: "Read the test file first", "Run `bun test` after changes" |
| **Kitchen sink** | Every possible instruction crammed in | Remove anything that does not change the output for THIS task |
| **Copy-pasted template** | Generic blocks that do not apply to this task | Delete irrelevant blocks. Fill in task-specific details for kept blocks. |

## Before/After Examples

### Example 1: Simple Bug Fix (Light Touch)

The raw prompt is almost fine. Heavy scaffolding would be noise.

<Bad>
```
Fix the login bug
```
No context. Which bug? What file? What should happen?
</Bad>

<Good>
```
Fix: `handleLogin` in src/auth/login.ts returns 200 on invalid password.
Expected: return 401 with { error: "Invalid credentials" }.
Verify: existing auth tests pass after the fix.
```
Three lines. Objective, expected behavior, verification. Nothing more needed.
</Good>

### Example 2: Multi-File Refactor (Standard)

<Bad>
```
Refactor the database layer to use connection pooling instead of
creating a new connection per request.
```
Missing: which files, what pool library, constraints, done criteria.
</Bad>

<Good>
```
## Objective
Refactor the database layer (src/db/) to use connection pooling.
Replace per-request connections with a shared pool using the existing
`pg` library's built-in Pool class.

## Context
- Entry point: src/db/client.ts (creates connections)
- Callers: src/db/queries/*.ts (15 files, all use `getClient()`)
- Current: each query calls `new Client()` and `.end()` after use
- Constraint: do not change query logic, only connection management

## Done Criteria
- Single Pool instance created at startup in client.ts
- All query files use pool.query() or pool.connect()/release()
- `bun test` passes (existing integration tests cover all queries)
- No connection leaks: pool.end() called in shutdown handler
```
</Good>

### Example 3: Research Task (Different Blocks)

<Bad>
```
Research how other projects handle rate limiting and write up
recommendations for our API.
```
No scope, no output format, no constraints on depth.
</Bad>

<Good>
```
## Objective
Recommend a rate limiting strategy for our Express API (src/api/).
Current state: no rate limiting. ~500 req/s peak traffic.

## Constraints
- Must work with our existing Redis instance (no new infrastructure)
- Must support per-user and per-endpoint limits
- Prefer existing npm packages over custom implementation

## Output Contract
Return a markdown document with:
1. Three candidate approaches (max 1 paragraph each)
2. Recommendation with justification (max 2 paragraphs)
3. Implementation sketch: which files change and how

## Done Criteria
- Recommendation is specific enough to implement without further research
- At least one candidate is a well-maintained npm package with >1k weekly downloads
```
</Good>

### Example 4: Persistent Instructions (Full + Examples)

For CLAUDE.md or AGENTS.md, prompts run hundreds of times. Precision matters.

<Bad>
```
## Code Style
Write clean code. Follow best practices. Make sure tests pass.
```
Every word is vague. "Clean", "best practices" -- the agent fills these with its own defaults, which may not match yours.
</Bad>

<Good>
```
## Code Style
- TypeScript strict mode. No `any` types except in test fixtures.
- Functions over classes unless state management requires it.
- Error handling: return Result<T, E> types. Do not throw except
  in top-level entry points.
- Naming: camelCase for functions/variables, PascalCase for types,
  SCREAMING_SNAKE for constants.
- Imports: group by (1) node builtins, (2) external packages,
  (3) internal modules. Separate groups with a blank line.

## Testing
- Run `bun test` after every file change. Do not proceed if tests fail.
- New functions require tests. Test the behavior, not the implementation.
- Mock only external boundaries (network, filesystem, time).
```
Every rule is concrete and verifiable. The agent cannot misinterpret "clean".
</Good>

## Anti-Pattern Catalog

Common prompt weaknesses and their fixes. See `reference/anti-patterns.md` for the full catalog with extended examples.

| Anti-Pattern | Example | Fix |
|-------------|---------|-----|
| **The Fog** | "Make this better" | State what "better" means: faster, more readable, fewer dependencies |
| **The Novel** | 2000-word prompt for a 5-line change | Strip to proportional level. Three lines for a three-line task. |
| **The Parrot** | Copy-pasted template with unfilled placeholders | Fill every placeholder or delete the block entirely |
| **The Hedge** | "Maybe consider possibly looking at..." | Direct language: "Read X. Change Y. Verify Z." |
| **The Wish** | "It would be nice if the code was faster" | Measurable goal: "Reduce p95 latency from 200ms to under 50ms" |
| **The Shotgun** | 15 unrelated instructions in one prompt | Split into separate prompts, one objective each |
| **The Cage** | Over-constrained: every line of code dictated | Specify WHAT and WHY, let the agent decide HOW |
| **The Ghost** | References "the file" or "that function" without names | Use explicit paths: `src/utils/parse.ts`, function `parseConfig` |

## Provider References

Reference files support prompt construction for specific agent environments:

- `reference/framework.md` -- vendor-neutral framework blocks. Always loaded.
- `reference/claude-code.md` -- Claude Code and generic Claude patterns.
- `reference/openai-gpt.md` -- GPT and OpenAI reasoning model patterns.
- `reference/amp-codex.md` -- Amp, Codex CLI, and other coding agent patterns.
- `reference/anti-patterns.md` -- Extended anti-pattern catalog with examples.

### Detection: Which Reference to Load

**Claude Code / Claude** --> load `claude-code.md`

Keywords: "Claude", "Claude Code", "Anthropic", "Sonnet", "Opus", "Haiku"
Model IDs: `claude-opus-4-6`, `claude-sonnet-4-6`, `claude-haiku-4-5`, or any `claude-*` string
Prompt signals: XML tags, CLAUDE.md references, `<thinking>` blocks, tool rules in XML

**OpenAI / GPT** --> load `openai-gpt.md`

Keywords: "GPT", "OpenAI", "ChatGPT"
Model IDs: `gpt-5.4`, `gpt-5.3`, `gpt-5`, `gpt-4.1`, `gpt-4o`, or any `gpt-*` string
Reasoning models: "o1", "o3", "o4-mini", or any `o[0-9]*` pattern
Prompt signals: `<output_contract>`, `reasoning_effort`, Responses API references

**Amp / Codex / Other coding agents** --> load `amp-codex.md`

Keywords: "Amp", "Codex", "Cursor", "Windsurf", "Copilot"
Prompt signals: AGENTS.md, rules files, sandbox execution, `.cursorrules`

**Unspecified** --> `framework.md` only

When no provider is detected, the vendor-neutral framework produces strong prompts for any model. Load at most one vendor file.

## Output Modes

Choose one mode based on the user request.

- **Inline upgrade**: provide the upgraded prompt only.
- **Upgrade + rationale**: provide the prompt plus a brief list of improvements.
- **Template extraction**: convert the prompt into a reusable fill-in-the-blank template.
- **Hook spec**: explain how to apply the framework automatically before execution.

## Hook Pattern

When the user asks for a hook, model it as a pre-processing layer:

1. Accept the current prompt.
2. Classify the task type and complexity level.
3. Select proportionality level from the table above.
4. Expand the prompt using only the framework blocks appropriate for that level.
5. Return the upgraded prompt for execution.
6. Optionally keep a diff or summary of injected structure.

## Red Flags -- The Upgraded Prompt Went Wrong

Stop and re-evaluate if you see any of these:

- Upgraded prompt is 3x longer than the raw prompt for a simple task
- Framework blocks contain placeholder text ("describe your context here")
- Multiple blocks repeat the same instruction in different words
- The agent's actual job is buried under ceremony
- Constraints conflict with each other
- You added verification steps for a task with no side effects
- The prompt reads like documentation instead of an instruction

**All of these mean: strip back. Simpler is stronger.**

## Quality Bar

Before finalizing, check the upgraded prompt:

- [ ] Still matches the original intent (do not drift)
- [ ] Every block materially improves execution (remove those that do not)
- [ ] Proportional to task complexity (no over-engineering)
- [ ] Includes verification appropriate to the risk level
- [ ] Gives the agent a clear definition of done
- [ ] Uses direct language (no hedging, no "maybe", no "consider")
- [ ] File paths and function names are explicit, not vague references

If the prompt is already strong, say so and make only minimal edits. The best upgrade is sometimes "this prompt is fine as-is."

## When Stuck

| Problem | Solution |
|---------|----------|
| Do not know what the user really wants | Ask one clarifying question with a recommended default |
| Task seems too big for one prompt | Split into multiple prompts, each with one objective |
| Cannot determine the target agent | Use framework.md only -- it works for any model |
| Upgraded prompt feels bloated | Remove blocks one at a time. If removing it does not hurt, it was noise. |
| User says "just make it work" | Add objective + done criteria. Skip everything else. |
