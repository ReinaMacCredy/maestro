# Scout Patterns -- Fan-Out Strategies

## Codebase Survey

**Goal:** Build a structural map of a subsystem or domain.

**When:** You need to understand "how does X work?" across implementation, configuration, and wiring.

**Scout decomposition:**

| Scout | Focus | Prompt shape |
|-------|-------|-------------|
| Structure | File layout, exports, module boundaries | "Map the file structure of [subsystem]. List each file, its exports, and what it imports." |
| Data flow | How data enters, transforms, exits | "Trace data flow for [operation]. Start at entry point, follow through transforms, end at output." |
| Configuration | Env vars, config files, defaults | "Find all configuration points for [subsystem]. Include defaults, env overrides, and runtime switches." |

**Example -- 3-scout codebase survey:**

```
Scout 1 (Structure):
  "Map the skills/ directory. For each file: purpose, exports, and
   which other modules import it. Return as a table."

Scout 2 (Data flow):
  "Trace how a skill gets loaded: from CLI invocation through
   registry lookup to file read. List each function in the chain
   with file:line."

Scout 3 (Configuration):
  "Find all configuration that affects skill loading: env vars,
   config file keys, CLI flags, defaults. Return as a table with
   source, key, default value, and effect."
```

## Dependency Analysis

**Goal:** Understand what depends on what before making changes.

**When:** You need to answer "what breaks if I change X?"

**Scout decomposition:**

| Scout | Focus | Prompt shape |
|-------|-------|-------------|
| Imports | Who imports the target module/symbol | "Find all files that import from [module]. Return file paths and which symbols they use." |
| Callers | Who calls the target function/method | "Find all call sites for [function]. Return file:line and the calling context (function name, class)." |
| Type consumers | Who uses the target type/interface | "Find all uses of [Type] as a parameter type, return type, or generic argument." |

**Key:** Each scout searches a different relationship (imports vs. calls vs. types). This avoids overlap because the same file might import a module without calling the specific function you care about.

## Pattern Search

**Goal:** Find all instances of a code pattern across the codebase.

**When:** You need to answer "how is pattern X used?" or "are there other examples of Y?"

**Scout decomposition:**

| Scout | Focus | Prompt shape |
|-------|-------|-------------|
| Implementation patterns | How the pattern is implemented | "Find all implementations of [pattern]. Show the code shape, not just file names." |
| Test patterns | How the pattern is tested | "Find all tests for [pattern]. What assertions are used? What edge cases are covered?" |
| Anti-patterns | Where the pattern is done wrong | "Find places where [pattern] is attempted but done incorrectly or inconsistently." |

**Example -- error handling audit:**

```
Scout 1 (Implementation):
  "Find all try/catch blocks in src/commands/. For each: what
   error types are caught, what action is taken (rethrow, log,
   swallow, wrap). Return as a table."

Scout 2 (Tests):
  "Find all tests that assert error behavior. What patterns are
   used: expect().toThrow, try/catch in test, .rejects? Return
   as a table with file, pattern, and error type."

Scout 3 (Anti-patterns):
  "Find catch blocks that swallow errors silently (empty catch,
   catch with only console.log). Return file:line and the
   swallowed error context."
```

## API Surface Mapping

**Goal:** Document the public interface of a subsystem.

**When:** You need to understand "what can I call and how?" before designing an integration.

**Scout decomposition:**

| Scout | Focus | Prompt shape |
|-------|-------|-------------|
| Public API | Exported functions, classes, types | "List all exports from [module/package]. For each: name, signature, JSDoc summary if present." |
| Usage examples | How the API is actually called | "Find 3-5 representative call sites for [API]. Show the calling code with enough context to understand the usage pattern." |
| Constraints | Validation, limits, error conditions | "What validation does [API] perform on its inputs? What errors can it throw? What are the implicit constraints?" |

## Scoping Rules

### How many scouts?

| Situation | Scouts | Rationale |
|-----------|--------|-----------|
| Focused question, 2 clear domains | 2 | Minimum useful parallelism |
| Typical investigation | 3 | Sweet spot: structure + behavior + tests/config |
| Complex unfamiliar subsystem | 4 | Maximum before synthesis overhead dominates |
| 5+ | Stop. Reframe. | You're probably asking the wrong question. Decompose into 2 sequential rounds of 2-3 scouts each. |

### How to scope each scout

**The one-paragraph rule:** If you can't describe the scout's job in one paragraph, it's too broad. Split it.

**The file-count heuristic:** A well-scoped scout should touch 3-15 files. If it needs to read 30+ files, the scope is too wide.

**The overlap test:** Write out each scout's expected output. If two scouts would return the same files, merge them or redefine boundaries.

### Avoiding overlap

Overlap wastes tokens and produces duplicate findings that complicate synthesis.

**Boundary strategies:**

| Strategy | How | Example |
|----------|-----|---------|
| By layer | Each scout owns a layer (API, business logic, data) | Scout 1: route handlers. Scout 2: service functions. Scout 3: database queries. |
| By artifact | Each scout owns an artifact type (code, tests, config) | Scout 1: implementation files. Scout 2: test files. Scout 3: config/docs. |
| By question | Each scout answers one specific question | Scout 1: "Where is it defined?" Scout 2: "How is it used?" Scout 3: "How is it tested?" |

**Anti-pattern -- overlapping scopes:**
```
Scout 1: "How does auth work?"          # Too broad, overlaps with everything
Scout 2: "How does the login flow work?" # Subset of scout 1
Scout 3: "What middleware runs on auth?"  # Also subset of scout 1
```

**Fixed -- clear boundaries:**
```
Scout 1: "What middleware runs before auth routes? List file:line for each."
Scout 2: "How does token validation work? Trace from header extraction to user lookup."
Scout 3: "How are auth failures tested? What error codes are asserted?"
```
