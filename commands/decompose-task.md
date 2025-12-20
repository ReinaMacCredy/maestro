---
description: Break a phase plan into atomic beads for agent execution
argument-hint: <phase-file-or-section>
---

# Decompose Task - Turn a Phase Plan into Beads

Break a phase from your master plan into atomic tasks (beads) and subtasks (sub-beads).

## Usage

```
/decompose-task <phase-file-or-section>
```

## Arguments

- `$ARGUMENTS` - The phase to decompose. Can be:
  - A path to a phase file (e.g., `conductor/tracks/auth-001/plan.md`)
  - A phase section reference (e.g., `Phase 2: Authentication`)
  - A bead ID if the phase is already tracked as a bead

---

## The Workflow This Fits Into

This command is part of the Conductor planning workflow:

```
1. IDEATION (/conductor-setup, bs, spike)
   └─ Use frontier reasoning models with thinking cranked up
   └─ Go from idea → fully fleshed out plan (3,000-5,000+ lines)

2. PHASE BREAKDOWN (Human via /conductor-newtrack)
   └─ Break the massive plan into phases
   └─ Each phase is a logical chunk (1-2 weeks typically)

3. TASK DECOMPOSITION ← YOU ARE HERE
   └─ /decompose-task takes ONE phase
   └─ Creates parent bead (epic-level) + sub-beads
   └─ Each sub-bead is ~500 lines, 30-120 minutes of work

4. EXECUTION (fb → bd ready → ct)
   └─ Agents claim and execute individual beads
   └─ Use bd ready, bd update, bd close for coordination
```

**Input requirement:** Phase size MUST be 500-1000 lines. If larger, break into multiple phases first.

---

## Why Phases Need Breaking Down

**Agents perform poorly on large documents.**

This is not about human convenience—it's about agent performance:

- Agents work best on files under **1000 lines**, preferably closer to **500 lines**
- Give an agent a 5000-line planning document and it **will** turn that into subtasks horribly
- Large documents cause "context bombing"—the agent gets lazy and skips detail
- You'll see vague summaries like "implement authentication" instead of specific methods, types, and tests

**This is a content loss problem.**

Your massive plan contains critical detail: specific method signatures, edge cases, integration points. When you hand a 5000-line doc to an agent, that detail gets summarized away.

**The solution: make content digestible.**

By breaking your plan into phases BEFORE giving it to an agent:
- Each phase is small enough to fit in working memory
- The agent can preserve ALL the detail from your plan
- You get LOSSLESS decomposition instead of lossy summarization

---

## The LOSSLESS Rule

**CRITICAL: Everything from the phase plan must appear in a sub-bead.**

This is the most important rule of decomposition:

| Rule | Description |
|------|-------------|
| **NEVER paraphrase** | Don't reword content, copy it exactly |
| **NEVER summarize** | Don't write "4 tests for X", include actual test code |
| **NEVER reference elsewhere** | Don't write "see parent bead", include the content |
| **NEVER skip "obvious" content** | Include everything, even if it seems redundant |
| **COPY verbatim** | Typos and all—exact content transfer |

### Verification Process

After decomposition, verify:

1. **Character count**: Sub-beads total >= original (overhead is expected)
2. **Content check**: Every section from original appears somewhere
3. **Standalone test**: Each sub-bead makes sense without the others

---

## Standard Sub-Bead Structure

This standardized output format is **essential for automation**. Every decomposed task MUST use these suffixes to ensure agents can parse, execute, and verify beads consistently.

**Why This Structure Matters:**
- **Predictable parsing**: Automation tools rely on consistent suffix patterns
- **Complete context**: Every sub-bead carries the context needed for independent execution
- **Test coverage guaranteed**: Dedicated suffixes ensure tests aren't omitted
- **Verification built-in**: `.10` suffix enforces acceptance criteria checks

| Suffix | Content | Purpose |
|--------|---------|---------|
| `.0` | **Context Brief** | WHY this phase exists, architecture decisions, system map |
| `.1` | **Schema/Types** | Database migrations, type definitions, interfaces |
| `.2-.3` | **Implementation** | Core code with full imports, every method |
| `.4` | **Tests: Happy Path** | Success scenario tests |
| `.5` | **Tests: Edge Cases** | Boundary conditions, unusual inputs |
| `.6` | **Tests: Error Handling** | Failure modes, exceptions |
| `.7` | **Tests: Property-Based** | Hypothesis/fuzzing tests for invariants |
| `.8` | **Tests: Integration** | Cross-component verification |
| `.9` | **Reference Data** | Constants, addresses, lookup tables |
| `.10` | **Verification Checklist** | Acceptance criteria, completion checks |

Adjust based on what the phase actually contains. Not every phase needs all suffixes—but `.0` (context), at least one test suffix (`.4-.8`), and `.10` (verification) are strongly recommended.

---

## Sizing Guidelines

Each sub-bead should be:

- **~500 lines of code** (including tests)
- **30-120 minutes of focused work**
- **Single responsibility** (one thing done well)
- **Independently testable** (can verify without other sub-beads)

If a sub-bead exceeds 1000 lines, decompose it further.

---

## Instructions

### Step 1: Understand the Phase

Read the phase description carefully. Identify:

- What is the goal of this phase?
- What components/files will be created or modified?
- What are the dependencies (what must exist first)?
- What are the deliverables (how do we know it's done)?

If the phase is vague, **ask for clarification before decomposing**.

### Step 2: Create the Parent Bead (if needed)

If the phase isn't already tracked as a bead:

```bash
bd create "Phase N: <Phase Title>" -t epic -p 1 -d '<Phase description from the plan>'
```

Note the bead ID for creating sub-beads.

### Step 3: Create Content Manifest

Before creating sub-beads, list everything that needs to be captured:

```markdown
## Content Manifest for <Phase>

### Components
- [ ] Component 1: <description>
- [ ] Component 2: <description>

### Files to Create/Modify
- [ ] <path/to/file1.py>: <what changes>
- [ ] <path/to/file2.py>: <what changes>

### Tests Required
- [ ] Happy path: <scenarios>
- [ ] Edge cases: <scenarios>
- [ ] Error handling: <scenarios>

### Dependencies
- [ ] Depends on: <other beads/phases>
- [ ] Enables: <downstream beads/phases>

### Acceptance Criteria
- [ ] <Criterion 1>
- [ ] <Criterion 2>
```

### Step 4: Create Sub-Beads

For each logical chunk, create a sub-bead:

```bash
bd create "<Sub-task title>" --parent <phase-bead-id> --priority 1 -d '<Full description>'
```

**Critical: Apply LOSSLESS rules**

- Copy content verbatim from the phase plan
- Include complete code with all imports
- Include actual test code, not descriptions
- Each sub-bead must be executable without referencing others

### Step 5: Verify Completeness

Check that:

- [ ] Every item from the manifest is assigned to a sub-bead
- [ ] No content was summarized or lost
- [ ] Each sub-bead is atomic (~500 lines, 30-120 min)
- [ ] Dependencies between sub-beads are explicit

### Step 6: Set Dependencies

```bash
bd dep add <child-id> <blocker-id> --type blocks
```

Common patterns:
- Schema (.1) blocks implementation (.2, .3)
- Implementation blocks tests (.4-.8)
- All sub-beads block parent phase completion

### Step 7: Validate the Graph

```bash
bv --robot-suggest   # Missing deps, cycle breaks, duplicates
bv --robot-plan      # Parallel tracks + what unblocks what
bv --robot-alerts    # Proactive warnings (stale, cascades)
```

If anything looks wrong, fix the dependency structure before agents start executing.

---

## Anti-Patterns

| DON'T | DO |
|-------|-----|
| "4 tests for validation" | Include full test code |
| "See phase plan for details" | Copy content verbatim |
| Partial code blocks | Full runnable code with imports |
| 2000+ line beads | Split into multiple beads |
| Implementation without test beads | Always include .4-.8 |

---

## Example Decomposition

**Phase:** "User Authentication"

```
user-auth (parent bead - epic)
├── user-auth.0   Context Brief (WHY, ADR, integration map)
├── user-auth.1   Database schema (users table, migrations)
├── user-auth.2   Password hashing service
├── user-auth.3   JWT token service
├── user-auth.4   Registration endpoint
├── user-auth.5   Login endpoint
├── user-auth.6   Auth middleware
├── user-auth.7   Tests: Happy path (registration, login, token refresh)
├── user-auth.8   Tests: Edge cases (invalid email, weak password)
├── user-auth.9   Tests: Error handling (expired tokens, rate limits)
└── user-auth.10  Verification checklist
```

---

## Output Format

After decomposition, output:

```markdown
## Decomposition Complete: <Phase>

**Parent Bead:** <id>
**Sub-Beads Created:** <count>
**Character Count:** Original: X → Sub-beads: Y (+Z overhead)

| ID | Title | Est. Lines | Est. Time | Depends On |
|----|-------|------------|-----------|------------|
| <id>.0 | Context Brief | 200 | 15 min | - |
| <id>.1 | Schema | 400 | 30 min | - |
| <id>.2 | Implementation | 600 | 90 min | .1 |
| ... | ... | ... | ... | ... |

**LOSSLESS Verification:**
- [ ] All manifest items assigned
- [ ] No summarization detected
- [ ] Character count >= original

**Ready to Start:** <id>.0, <id>.1 (no blockers)
```

---

## When to STOP and Ask

Stop and ask the user if:

1. **Phase is too vague** - Can't identify concrete deliverables
2. **Phase is too large** - Would result in 20+ sub-beads (suggest splitting)
3. **Unclear dependencies** - Don't know what must exist first
4. **Multiple valid approaches** - Need architectural decision before decomposing
