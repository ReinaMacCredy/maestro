# Double Diamond Framework

The session flows through four phases, alternating between divergent and convergent thinking:

```
    DISCOVER          DEFINE           DEVELOP          DELIVER
   (Diverge)        (Converge)        (Diverge)        (Converge)
      ◇                ◇                ◇                ◇
     / \              / \              / \              / \
    /   \            /   \            /   \            /   \
   -----------      -----------      -----------      -----------
   Explore the      Frame the        Explore          Finalize
     Problem        Problem          Solutions        the Design
```

## Phase Details

| Phase | Purpose | Activities | Research | Validation |
|-------|---------|------------|----------|------------|
| **DISCOVER** | Explore problem space | Ask about pain, users, impact, constraints | (at session start) | CP1: Product alignment |
| **DEFINE** | Frame the problem | Problem statement, success criteria, scope | - | CP2: Problem clarity |
| **DEVELOP** | Explore solutions | Architecture, components, data model, user flow | grounding-hook | CP3: Tech-stack |
| **DELIVER** | Finalize design | Full verification, acceptance criteria, risks | Full + impact | CP4: Full gate |

## Research & Validation Triggers

| Checkpoint | Research | Validation |
|------------|----------|------------|
| Session start | discover-hook (Locator + Pattern + CODEMAPS) | - |
| CP1 (DISCOVER) | - | WARN (product alignment) |
| CP2 (DEFINE) | - | WARN (problem clarity) |
| CP3 (DEVELOP) | grounding-hook (Locator + Analyzer + Pattern) | WARN (tech-stack) |
| CP4 (DELIVER) | Full + impact scan | SPEED=WARN, FULL=HALT |

## Phase 1: DISCOVER (Diverge)

**Goal:** Explore the problem space broadly before narrowing.

- What problem are we solving?
- Who experiences this problem?
- What's the impact of not solving it?
- What constraints exist?
- What has been tried before?

**Exit:** Problem is clearly articulated and users are identified.

## Phase 2: DEFINE (Converge)

**Goal:** Narrow the problem to an actionable scope.

- Define the problem statement
- Establish success criteria
- Set scope boundaries (what's in/out)
- Propose 2-3 approaches
- Select preferred approach with rationale

**Exit:** Problem statement agreed, approach selected.

## Phase 3: DEVELOP (Diverge)

**Goal:** Explore solution space within the chosen approach.

- Architecture design
- Component breakdown
- Data model (if applicable)
- User flow/interactions
- Integration points

**Validation (CP3):** WARN if options don't align with tech-stack.md.
**Research:** grounding-hook runs here (Locator + Analyzer + Pattern).

**Exit:** Architecture understood, interfaces defined.

## Phase 4: DELIVER (Converge)

**Goal:** Finalize the design and prepare for implementation.

- Full research verification (mandatory)
- Ensure acceptance criteria are testable
- Document risks and open questions
- Validate design against product.md and tech-stack.md

**Oracle Audit (auto-triggered before A/P/C):**

```
Platform Detection:
  IF oracle tool available (Amp):
    → oracle(task="6-dimension design audit", files=[design.md, ...])
  ELSE (Claude Code / Gemini / Codex):
    → Task(description="Oracle Design Review", prompt="[oracle.md template]")

Oracle appends "## Oracle Audit" section to design.md with:
  - 6-dimension summary (Completeness, Feasibility, Risks, Dependencies, Ordering, Alignment)
  - Critical issues (must fix before proceeding)
  - Warnings (recommended to address)
  - Verdict: APPROVED or NEEDS_REVISION

On Critical Gap:
  - FULL mode: HALT - fix before proceeding
  - SPEED mode: WARN - log but allow continue
```

**Validation (CP4):** Full gate - SPEED=WARN, FULL=HALT.
**Research:** Full grounding + impact scan runs here.

**Exit:** Design verified and approved (Oracle audit passed).

## Loop-Back Support

User can say "revisit [PHASE]" at any time to return to an earlier phase:

1. Summarize what was established
2. Ask what to reconsider
3. Update subsequent phases if decisions change
