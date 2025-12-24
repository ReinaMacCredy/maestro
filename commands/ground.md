---
description: Verify patterns against current truth before implementation
argument-hint: <question-or-pattern>
---

# Ground — Verification Protocol

Use this command when you're about to implement something and need to verify the truth is **in the repo**, **on the web**, or **in prior sessions**.

---

## Why Grounding Matters

**AI-generated code follows training data, which may be outdated, deprecated, or wrong.**

LLMs are trained on snapshots of documentation from months or years ago:

- **Deprecated APIs**: Using `library.oldMethod()` when `library.newMethod()` replaced it
- **Outdated patterns**: The "recommended approach" from 2023 might be an anti-pattern today
- **Hallucinated methods**: Confidently inventing methods that never existed
- **Wrong defaults**: Configuration options and parameters change across versions

**Grounding pulls your code back to reality.**

---

## Usage

```
/ground <question-or-pattern>
```

## Decision Rule

| Truth Type        | Tool                          | Use When                    |
| ----------------- | ----------------------------- | --------------------------- |
| **Repo truth**    | `Grep`, `finder`              | "How do we do X here?"      |
| **Web truth**     | `web_search`, `read_web_page` | External libs/APIs/docs     |
| **History truth** | `find_thread`                 | "Did we solve this before?" |
| **Task truth**    | `bd` commands                 | "What should I do next?"    |

---

## When to Use What

### Grep / finder (Repo Discovery)

Use when the answer exists in the current codebase:

- "Where is X implemented?"
- "What pattern does this project use for Y?"
- "How does data flow from A → B?"

```bash
# Find pattern usage
Grep "pattern-name" --path src/

# Semantic search for concept
finder "how authentication middleware validates tokens"
```

### web_search / read_web_page (External Grounding)

Use when truth depends on **current** external information:

- Library/framework docs that change (APIs, deprecations)
- Vendor integrations, auth flows, latest patterns
- Finding real-world examples

```bash
# Search for current docs
web_search "stripe API create customer 2025"

# Read specific documentation
read_web_page "https://docs.library.io/api/method"
```

### find_thread (History)

Use when you suspect we've solved it before:

- "Have we seen this bug before?"
- "What conventions do we follow for X?"
- "What did we decide about Y?"

```bash
# Find related thread
find_thread "similar error message"
```

### bd Commands (Task Graph)

Use when the question is about work state:

- "What should I work on next?"
- "What's blocking this issue?"
- "What's the current priority?"

```bash
bd ready              # Available work
bd blocked            # What's stuck
bd show <id>          # Issue details
```

---

## Protocol

1. **Identify** what needs grounding (library, API, pattern, decision)
2. **Determine** truth source (repo/web/history/task)
3. **Query** using appropriate tool
4. **Verify** information is current
5. **Return** verified pattern with source

---

## Output Format

```
GROUNDING: <what was verified>
SOURCE: <repo|web|history|task>
STATUS: ✅ Current | ⚠️ Outdated | ❌ Not found
PATTERN: <the verified pattern to use>
```

---

## Examples

### Example 1: Library API

```
/ground how to create Stripe customer with new API
```

Output:

```
GROUNDING: Stripe customer creation API
SOURCE: web (stripe.com/docs)
STATUS: ✅ Current (v2025-11-20)
PATTERN: stripe.customers.create({ email, metadata })
```

### Example 2: Project Convention

```
/ground how do we handle errors in this codebase
```

Output:

```
GROUNDING: Error handling pattern
SOURCE: repo (src/lib/errors.ts)
STATUS: ✅ Current
PATTERN: throw new AppError(code, message, { cause })
```

### Example 3: Prior Decision

```
/ground did we decide on auth strategy
```

Output:

```
GROUNDING: Authentication strategy decision
SOURCE: history (find_thread)
STATUS: ✅ Current
PATTERN: JWT with refresh tokens, 15min access / 7day refresh
```

---

## Defaults (Safe)

- **Repo first** for "how do we do X here?"
- **Web only** when you need external/current facts
- **History first** for "what did we decide?"
- **Task graph** for "what should I do next?"
