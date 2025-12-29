---
name: design
description: Design Session - collaborative brainstorming to turn ideas into designs using Double Diamond methodology. Use when user types "ds" or wants to explore/design a feature before implementation.
license: Apache-2.0
compatibility: Works with Claude Code, Amp Code, Codex, and any Agent Skills compatible CLI
metadata:
  version: "2.3.0"
  keywords:
    - brainstorming
    - design
    - planning
    - exploration
    - double-diamond
    - party-mode
---

## Prerequisites

**REQUIRED SUB-SKILL:** [maestro-core](../maestro-core/SKILL.md)

Load maestro-core first for orchestration context (hierarchy, HALT/DEGRADE policies, trigger routing).

# Design Session (ds)

Turn ideas into fully-formed designs through collaborative dialogue using the Double Diamond methodology.

## When To Use

Trigger on:

- User types `ds`
- User runs `/conductor-design`
- User wants to brainstorm or explore an idea
- User says "design a feature" or "let's think through X"
- Before creating a conductor track

## Session Initialization

When starting a design session:

### 0. Load Continuity Context

Check for prior session context:

1. Run `continuity load` workflow
2. If `conductor/sessions/active/LEDGER.md` exists:
   - Display prior context summary
   - Show: `📋 Prior context: <goal summary>`
3. If missing: Start fresh session

**Non-blocking:** Continue normally if no prior context exists.

### 1. Load CODEMAPS for Context

Check for `conductor/CODEMAPS/` directory:

**If exists:**

1. Load `overview.md` (always)
2. Load relevant module codemaps based on topic (skills.md, api.md, etc.)
3. Display: `📚 Loaded CODEMAPS for context`

**If missing:**

1. Display: `⚠️ No CODEMAPS found. Run /conductor-setup to generate initial CODEMAPS.`
2. Continue session normally (CODEMAPS are optional but recommended)

### 2. Verify Conductor Setup

Check for `conductor/` directory with core files:

- `product.md` - Product vision
- `tech-stack.md` - Technical constraints
- `workflow.md` - Development standards

If missing: Display `Conductor unavailable. Standalone mode. Run /conductor-setup to enable full features.` and continue session.

> **Note:** In standalone mode, CODEMAPS and product context are skipped. Double Diamond still works but without project-specific context.

### 2.5. Auto-Research Context (NEW)

**BEFORE asking user any questions**, run parallel research to ground context:

See [conductor/references/research/hooks/discover-hook.md](../conductor/references/research/hooks/discover-hook.md) for full protocol.

**Quick Summary:**

1. Extract topic from user's initial message
2. Spawn parallel agents:
   - **Locator**: Find related files
   - **Pattern**: Find similar features
   - **CODEMAPS**: Load relevant modules
3. Display research context:

```
┌─ RESEARCH CONTEXT ─────────────────────────┐
│ Topic: {extracted topic}                   │
│ Duration: Xs                               │
├────────────────────────────────────────────┤
│ EXISTING RELATED CODE:                     │
│ • [path/file.ts] - Description             │
├────────────────────────────────────────────┤
│ SIMILAR FEATURES:                          │
│ • [FeatureName] in [location]              │
└────────────────────────────────────────────┘
```

4. Proceed to DISCOVER with research context

**⚠️ Research ALWAYS runs. No skip conditions.**

Parallel agents are fast and context is always valuable.

**Timeout:** 10s max, partial results OK

### 3. Complexity Scoring (Design Routing)

After loading context, evaluate task complexity to determine routing:

**Scoring Criteria** (max 18 points):

| Factor | Weight | Check |
|--------|--------|-------|
| Multiple epics | +3 | Work spans multiple epics |
| Cross-module | +2 | Changes touch multiple modules |
| New abstractions | +3 | Creating new patterns/interfaces |
| External deps | +2 | New external dependencies |
| Files > 5 | +1 | Touching more than 5 files |
| Unclear scope | +2 | Scope not well-defined |
| Security/auth | +2 | Involves security or authentication |
| Data migration | +3 | Database or data migration |

**Display COMPLEXITY_EXPLAINER:**

```text
┌─ COMPLEXITY EXPLAINER ─────────────────┐
│ Factor              │ Score │          │
├─────────────────────┼───────┼──────────┤
│ Multiple epics      │   0   │          │
│ Cross-module        │   2   │ ✓        │
│ New abstractions    │   0   │          │
│ External deps       │   0   │          │
│ Files > 5           │   1   │ ✓        │
│ Unclear scope       │   0   │          │
│ Security/auth       │   0   │          │
│ Data migration      │   0   │          │
├─────────────────────┼───────┼──────────┤
│ TOTAL               │   3   │ SPEED    │
└─────────────────────────────────────────┘
```

**Routing Decision:**

| Score | Route | Description |
|-------|-------|-------------|
| < 4 | SPEED MODE | 1-phase quick design, minimal ceremony |
| 4-6 | ASK USER | Soft zone: "[S]peed or [F]ull?" |
| > 6 | FULL MODE | 4-phase Double Diamond with A/P/C |

**Soft Zone Behavior (score 4-6):**
- Prompt: "Score is X (soft zone). [S]peed or [F]ull?"
- After 2 prompts without response → default to FULL
- Track prompt count in session

**Escalation:**
- User can type `[E]` during SPEED mode to escalate to FULL
- Escalation preserves current progress and enters DEFINE phase

See [design-routing-heuristics.md](references/design-routing-heuristics.md) for full scoring details.

### SPEED Mode Flow

For simple tasks (score < 4):

1. **Quick Discovery** - 2-3 clarifying questions max
2. **Output** - Generate design.md directly
3. **Handoff** - "Design complete. Run `/conductor-newtrack` to continue."

No A/P/C checkpoints in SPEED mode (unless user escalates with `[E]`).

### FULL Mode Flow

For complex tasks (score > 6 or user-selected):

Proceed with full Double Diamond (4 phases, A/P/C checkpoints).

## Double Diamond Framework

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

```mermaid
flowchart TB
    subgraph PIPELINE["COMPLETE PIPELINE WORKFLOW"]
        direction TB

        subgraph PLANNING["PLANNING LOOP"]
            DS["ds (Design Session)"]
            DISCOVER["DISCOVER<br/>Explore Problem"]
            DEFINE["DEFINE<br/>Frame Problem"]
            DEVELOP["DEVELOP<br/>Explore Solutions"]
            DELIVER["DELIVER<br/>Finalize Design"]
            APC{{"A/P/C"}}
            DESIGND["design.md"]
        end

        subgraph SPEC["SPEC GENERATION"]
            NEWTRACK["/conductor-newtrack"]
            SPECMD["spec.md"]
            PLANMD["plan.md"]
        end

        subgraph BEADS["ISSUE FILING LOOP"]
            FB["fb"]
            EPIC["Create Epic"]
            ISSUES["Create Issues<br/>(batches of 5)"]
            DEPS["Wire Dependencies"]
            RB["rb"]
        end

        subgraph DISPATCH["PARALLEL AGENT DISPATCH"]
            COORDINATOR["Coordinator Agent"]

            subgraph WORKERS["WORKER AGENTS (Task tool)"]
                W1["Agent 1<br/>Independent Task"]
                W2["Agent 2<br/>Independent Task"]
                W3["Agent 3<br/>Independent Task"]
                WN["Agent N<br/>Independent Task"]
            end

            MERGE["Merge Results"]
        end

        subgraph AGENT_LOOP["AGENT EXECUTION LOOP"]
            READY["bd ready"]
            CLAIM["bd update --status in_progress"]

            subgraph TDD["TDD CYCLE"]
                RED["RED: Write Failing Test"]
                GREEN["GREEN: Make It Pass"]
                REFACTOR["REFACTOR: Clean Up"]
            end

            CLOSE["bd close"]
            SYNC["bd sync"]
        end

        subgraph FINISH["COMPLETION"]
            VERIFY["Verification"]
            BRANCH["finish branch"]
            FINISH_CMD["/conductor-finish"]
        end
    end

    subgraph BMAD["PARTY MODE: 25 BMAD AGENTS"]
        subgraph CORE["Core Module"]
            MASTER["BMad Master (Orchestrator)"]
        end

        subgraph BMM["BMM Module (9 agents)"]
            PM["John (PM)"]
            ANALYST["Mary (Analyst)"]
            ARCH["Winston (Architect)"]
            DEV["Amelia (Developer)"]
            SM["Bob (Scrum Master)"]
            QA["Murat (QA)"]
            UX["Sally (UX Designer)"]
            DOCS["Paige (Tech Writer)"]
            BARRY["Barry (Quick Flow)"]
        end

        subgraph CIS["CIS Module (6 agents)"]
            BRAIN["Carson (Brainstorm)"]
            SOLVER["Dr. Quinn (Problem Solver)"]
            DESIGN["Maya (Design Thinking)"]
            STRAT["Victor (Innovation)"]
            CARAVAGGIO["Caravaggio (Presentation)"]
            STORY["Sophia (Storyteller)"]
        end

        subgraph BMB["BMB Module (3 agents)"]
            BOND["Bond (Agent Builder)"]
            MORGAN["Morgan (Module Builder)"]
            WENDY["Wendy (Workflow Builder)"]
        end

        subgraph BMGD["BMGD Module (6 agents)"]
            CLOUD["Cloud Dragonborn (Game Architect)"]
            SAMUS["Samus Shepard (Game Designer)"]
            LINK["Link Freeman (Game Dev)"]
            GLADOS["GLaDOS (Game QA)"]
            MAX["Max (Game Scrum Master)"]
            INDIE["Indie (Game Solo Dev)"]
        end
    end

    DS --> DISCOVER
    DISCOVER --> DEFINE
    DEFINE --> DEVELOP
    DEVELOP --> DELIVER
    DELIVER --> APC
    APC -->|"C"| DESIGND
    APC -->|"P"| BMAD
    BMAD -->|"Synthesize"| APC
    DESIGND --> NEWTRACK

    NEWTRACK --> SPECMD
    SPECMD --> PLANMD
    PLANMD --> FB

    FB --> EPIC
    EPIC --> ISSUES
    ISSUES --> DEPS
    DEPS --> RB
    RB --> READY

    READY --> CLAIM
    CLAIM --> COORDINATOR
    COORDINATOR --> W1 & W2 & W3 & WN
    W1 & W2 & W3 & WN --> MERGE
    MERGE --> RED
    RED --> GREEN
    GREEN --> REFACTOR
    REFACTOR -->|"More tests?"| RED
    REFACTOR -->|"Done"| CLOSE
    CLOSE --> SYNC
    SYNC -->|"More issues?"| READY
    SYNC -->|"All done"| VERIFY

    VERIFY --> BRANCH
    BRANCH --> FINISH_CMD

    classDef planning fill:#1a365d,stroke:#63b3ed,color:#e2e8f0
    classDef spec fill:#234e52,stroke:#4fd1c5,color:#e2e8f0
    classDef beads fill:#553c9a,stroke:#b794f4,color:#e2e8f0
    classDef dispatch fill:#742a2a,stroke:#fc8181,color:#e2e8f0
    classDef agent fill:#744210,stroke:#f6ad55,color:#e2e8f0
    classDef tdd fill:#2d3748,stroke:#a0aec0,color:#e2e8f0
    classDef finish fill:#22543d,stroke:#68d391,color:#e2e8f0
    classDef core fill:#5b21b6,stroke:#a78bfa,color:#e2e8f0
    classDef bmm fill:#285e61,stroke:#4fd1c5,color:#e2e8f0
    classDef cis fill:#744210,stroke:#f6ad55,color:#e2e8f0
    classDef bmb fill:#4c1d95,stroke:#c4b5fd,color:#e2e8f0
    classDef bmgd fill:#7c2d12,stroke:#fdba74,color:#e2e8f0

    class DS,DISCOVER,DEFINE,DEVELOP,DELIVER,APC,DESIGND planning
    class NEWTRACK,SPECMD,PLANMD spec
    class FB,EPIC,ISSUES,DEPS,RB beads
    class COORDINATOR,W1,W2,W3,WN,MERGE dispatch
    class READY,CLAIM,CLOSE,SYNC agent
    class RED,GREEN,REFACTOR tdd
    class VERIFY,BRANCH,FINISH_CMD finish
    class MASTER core
    class PM,ANALYST,ARCH,DEV,SM,QA,UX,DOCS,BARRY bmm
    class BRAIN,SOLVER,DESIGN,STRAT,CARAVAGGIO,STORY cis
    class BOND,MORGAN,WENDY bmb
    class CLOUD,SAMUS,LINK,GLADOS,MAX,INDIE bmgd
```

For detailed pipeline documentation, see [docs/PIPELINE_ARCHITECTURE.md](../../docs/PIPELINE_ARCHITECTURE.md).

## The Process

### Phase 1: DISCOVER (Diverge)

**Goal:** Understand the problem deeply before jumping to solutions.

- Explore the problem space broadly
- Ask about pain points, users, impact, constraints
- One question at a time, prefer multiple choice
- **Exit:** Problem clearly articulated, users identified

#### Transition: DISCOVER → DEFINE

**RESEARCH VERIFICATION (Mini, Advisory ⚠️):**

> **Deprecated:** "Grounding" terminology replaced by "Research Verification"

1. **Run:** `finder` with query: "similar problems to [problem statement]"
2. **Calculate confidence:**
   - 3+ matches → HIGH
   - 1-3 matches → MEDIUM
   - 0 matches → LOW
   - Timeout (5s) → MEDIUM + warning
   - Error → LOW
3. **Display:**
   ```
   ┌─ RESEARCH (Mini) ───────────────────────┐
   │ Query: [problem summary]                │
   │ Found: [N] matches                      │
   │ Confidence: [HIGH/MEDIUM/LOW]           │
   │ Status: ✓ Complete                      │
   └─────────────────────────────────────────┘
   ```
4. **On skip:** Log warning, display `⚠️ Research skipped`, proceed (Advisory allows skip)
5. **Proceed** to A/P/C checkpoint

---

### Phase 2: DEFINE (Converge)

**Goal:** Synthesize discoveries into a clear problem statement.

- Create a one-sentence problem statement
- Define success criteria (measurable)
- Bound the scope (in/out)
- Present 2-3 approaches with trade-offs
- **Exit:** Problem statement agreed, approach selected

#### Transition: DEFINE → DEVELOP

**RESEARCH VERIFICATION (Mini, Advisory ⚠️):**

> **Deprecated:** "Grounding" terminology replaced by "Research Verification"

1. **Run:**
   - `finder` with query: "existing patterns for [selected approach]"
   - `Grep` for key terms from problem statement
2. **Calculate confidence:**
   - 3+ matches → HIGH
   - 1-3 matches → MEDIUM
   - 0 matches → LOW
   - Timeout (5s) → MEDIUM + warning
   - Error → LOW
3. **Display:**
   ```
   ┌─ RESEARCH (Mini) ───────────────────────┐
   │ Query: [approach summary]               │
   │ Found: [N] matches                      │
   │ Confidence: [HIGH/MEDIUM/LOW]           │
   │ Status: ✓ Complete                      │
   └─────────────────────────────────────────┘
   ```
4. **On skip:** Log warning, display `⚠️ Research skipped`, proceed (Advisory allows skip)
5. **Proceed** to A/P/C checkpoint

---

### Phase 3: DEVELOP (Diverge)

**Goal:** Design the solution architecture and components.

- Present design in 200-300 word sections
- Cover: architecture, components, data model, user flow, errors, testing
- Ask after each section: "Does this look right so far?"
- Be ready to revise earlier sections
- **Exit:** Architecture understood, components defined

#### Transition: DEVELOP → DELIVER

**RESEARCH-BASED VERIFICATION (Gatekeeper 🚫):**

> **NEW:** Replaces sequential grounding with parallel research agents.
> See [conductor/references/research/hooks/grounding-hook.md](../conductor/references/research/hooks/grounding-hook.md) for full protocol.

1. **Spawn parallel agents:**
   ```
   ┌─────────────┬─────────────┬─────────────┐
   │  Locator    │  Analyzer   │  Pattern    │  (parallel)
   │  (files)    │  (deps)     │  (similar)  │
   └─────────────┴─────────────┴─────────────┘
   ```
   - **Locator**: Verify proposed file locations exist
   - **Analyzer**: Confirm interfaces match design
   - **Pattern**: Verify proposed patterns match conventions
   - **Web** (if external deps): Verify API/library documentation

2. **Timeout:** 15s total (parallel execution)

3. **Calculate confidence:**
   - 3+ verifications pass → HIGH
   - 1-3 pass → MEDIUM
   - 0 pass or conflicts → LOW

4. **Display:**
   ```
   ┌─ VERIFICATION RESULT ──────────────────────┐
   │ Phase: DEVELOP → DELIVER                   │
   │ Agents: 4 spawned, 4 completed             │
   │ Duration: 12s                              │
   ├────────────────────────────────────────────┤
   │ VERIFIED:                                  │
   │ ✓ [src/auth/jwt.ts] exists, interface OK   │
   │ ✓ Error handling matches project pattern   │
   ├────────────────────────────────────────────┤
   │ CONFLICTS:                                 │
   │ ⚠ Design uses `AuthError`, codebase uses   │
   │   `AuthenticationError` - recommend align  │
   ├────────────────────────────────────────────┤
   │ Confidence: HIGH (3/4 verified)            │
   └────────────────────────────────────────────┘
   ```

5. **HALT if not run:**
   ```
   ┌─ VERIFICATION REQUIRED ─────────────────┐
   │ 🚫 Cannot proceed without verification  │
   │                                         │
   │ [R]un verification  [S]kip with warning │
   └─────────────────────────────────────────┘
   ```

6. **On skip:** Display warning banner, log for audit, proceed
7. **Proceed** to A/P/C checkpoint only after verification complete or user skips

---

### Phase 4: DELIVER (Converge)

**Goal:** Finalize the design and prepare for implementation.

- **Full Research Verification (required)** - verify against codebase and current docs
- Ensure acceptance criteria are testable
- Document risks and open questions
- **Exit:** Design verified and approved

#### Transition: DELIVER → Complete

**FULL RESEARCH VERIFICATION (Mandatory 🔒):**

> **NEW:** Replaces sequential grounding with comprehensive parallel research.
> See [conductor/references/research/hooks/grounding-hook.md](../conductor/references/research/hooks/grounding-hook.md) for full protocol.

1. **Spawn ALL research agents in parallel:**
   ```
   ┌─────────────┬─────────────┬─────────────┬─────────────┬─────────────┐
   │  Locator    │  Analyzer   │  Pattern    │  Impact     │  Web        │
   │  (all)      │  (deep)     │  (verify)   │  (scope)    │  (if ext)   │
   └─────────────┴─────────────┴─────────────┴─────────────┴─────────────┘
   ```
   - **Locator**: All affected files identified
   - **Analyzer**: Deep interface/dependency analysis
   - **Pattern**: Verify ALL patterns match conventions
   - **Impact**: Full scope assessment (files, modules, risk)
   - **Web** (if external deps): Verify external API docs

2. **Timeout:** 20s total (parallel execution)

3. **Calculate confidence:**
   - All agents pass, no conflicts → HIGH
   - Minor conflicts or warnings → MEDIUM
   - Major conflicts or agent failures → LOW

4. **Display:**
   ```
   ┌─ FULL VERIFICATION ─────────────────────────┐
   │ Phase: DELIVER → Complete                   │
   │ Agents: 5 spawned, 5 completed              │
   │ Duration: 18s                               │
   ├──────────────────────────────────────────────┤
   │ VERIFIED:                                   │
   │ ✓ All file locations confirmed              │
   │ ✓ Interfaces compatible                     │
   │ ✓ Patterns match conventions                │
   │ ✓ External APIs documented                  │
   ├──────────────────────────────────────────────┤
   │ IMPACT ASSESSMENT:                          │
   │ • Files: 12                                 │
   │ • Modules: 4                                │
   │ • Risk: MEDIUM                              │
   ├──────────────────────────────────────────────┤
   │ Confidence: HIGH                            │
   │ Status: ✓ Ready for implementation          │
   └──────────────────────────────────────────────┘
   ```

5. **BLOCK if:**
   - Verification not run
   - Confidence = LOW
   - Major conflicts detected
   
   Display:
   ```
   ┌─ VERIFICATION REQUIRED ─────────────────────┐
   │ 🔒 Cannot proceed: [reason]                 │
   │                                             │
   │ To override, type:                          │
   │ SKIP_VERIFICATION: <your justification>     │
   └─────────────────────────────────────────────┘
   ```

6. **On empty justification:** Reject, require actual reason
7. **On valid skip:** Log override with reason, add warning banner to design, proceed
8. **Proceed** to design approval only after verification passed or user provides justification

#### Validation Gate: validate-design

After research verification passes, run the design validation gate:

1. **Load gate**: `../conductor/references/validation/shared/validate-design.md`
2. **Run validation**: Check design vs product.md, tech-stack.md, CODEMAPS
3. **Update LEDGER**: Add to `validation.gates_passed` or `validation.last_failure`
4. **Behavior by mode**:
   - **SPEED mode**: WARN on failure, continue to A/P/C
   - **FULL mode**: HALT on failure, retry up to 2x, then escalate

```text
┌─ VALIDATION GATE: design ──────────────────────┐
│ Status: [PASS] | [WARN] | [FAIL]               │
│                                                │
│ Checks:                                        │
│ [OK] Product alignment verified                │
│ [OK] Tech-stack constraints respected          │
│ [OK] Pattern consistency confirmed             │
│                                                │
│ LEDGER updated: gates_passed: [design]         │
└────────────────────────────────────────────────┘
```

See [validate-design.md](../conductor/references/validation/shared/validate-design.md) for full validation process.

---

## A/P/C Checkpoints

At the end of each phase, present the checkpoint menu:

```
📍 End of [PHASE] phase.

Choose:
[A] Advanced - deeper analysis, assumption audit
[P] Party - multi-perspective feedback from expert agents
[C] Continue - proceed to next phase
[↩ Back] - return to previous phase
```

### [A] Advanced Mode

Phase-specific deep dives:

- **DISCOVER:** Challenge assumptions, explore biases, consider alternative users
- **DEFINE:** Stress-test scope, challenge metrics, identify hidden dependencies
- **DEVELOP:** Deep-dive components, explore alternatives, security/performance review
- **DELIVER:** Edge case audit, security check, documentation completeness

### [P] Party Mode

Invokes multi-agent collaborative review using BMAD v6 integration. See `references/bmad/workflows/party-mode/workflow.md`.

**25 Agents Available:**
- **Core (1):** BMad Master (🧙) - Orchestrator
- **BMM (9):** PM, Analyst, Architect, Developer, Scrum Master, Test Architect, UX Designer, Tech Writer, Quick Flow Solo Dev
- **CIS (6):** Brainstorming Coach, Problem Solver, Design Thinking Coach, Innovation Strategist, Presentation Master, Storyteller
- **BMB (3):** Agent Builder, Module Builder, Workflow Builder
- **BMGD (6):** Game Architect, Game Designer, Game Dev, Game QA, Game Scrum Master, Game Solo Dev

**Selection:** BMad Master selects 2-3 agents based on topic relevance:
- **Primary:** Best expertise match
- **Secondary:** Complementary perspective
- **Tertiary:** Devil's advocate

**CIS Workflow Triggers:**
- `*brainstorm` - 36 ideation techniques
- `*design-thinking` - 5-phase human-centered design
- `*innovate` - Strategic innovation planning
- `*problem-solve` - Systematic problem resolution
- `*story` - Narrative design

**Language:** Agents respond in English.

Agents respond in character, cross-talk (max 2 rounds), then synthesize insights.

## Loop-Back Support

User can say "revisit [PHASE]" at any time to return to an earlier phase. When looping back:

1. Summarize what was established
2. Ask what to reconsider
3. Update subsequent phases if decisions change

## Research-Based Verification System

> ⚠️ **The tiered grounding system has been replaced by the Research Protocol.**
> 
> See [conductor/references/research/protocol.md](../conductor/references/research/protocol.md) for complete documentation.

### Overview

Research verification uses **parallel sub-agents** instead of sequential grounding:

| Mode | Phase Transition | Agents | Enforcement |
|------|------------------|--------|-------------|
| SPEED | Any | 1 (Locator) | Advisory ⚠️ |
| FULL | DISCOVER→DEFINE | 2 (Locator + Pattern) | Advisory ⚠️ |
| FULL | DEFINE→DEVELOP | 2 (Locator + Pattern) | Advisory ⚠️ |
| FULL | DEVELOP→DELIVER | 4 (All agents) | Gatekeeper 🚫 |
| FULL | DELIVER→Complete | 5 (All + Impact) | Mandatory 🔒 |

### Key Changes from Old Grounding

- ❌ OLD: Sequential execution (Grep → finder → web)
- ✅ NEW: Parallel sub-agents (faster, more comprehensive)

- ❌ OLD: Skip conditions (SPEED mode, "quick", timeout)
- ✅ NEW: Research ALWAYS runs (no skip conditions)

- ❌ OLD: Tiered intensity (Light/Mini/Standard/Full)
- ✅ NEW: Always full agent dispatch

### Enforcement Levels (Preserved)

| Level | Symbol | Behavior |
|-------|--------|----------|
| Advisory | ⚠️ | Log skip, warn, proceed |
| Gatekeeper | 🚫 | Block if verification not run |
| Mandatory | 🔒 | Block if fails or low confidence |

### Research State Tracking

Track verification completion across phases in session memory:

```
research_state = {
    "DISCOVER→DEFINE": { "completed": true, "confidence": "HIGH", "timestamp": "..." },
    "DEFINE→DEVELOP": { "completed": true, "confidence": "MEDIUM", "timestamp": "..." },
    "DEVELOP→DELIVER": null,  // Not yet reached
    "DELIVER→Complete": null
}
```

### Documentation

- [Research Protocol](../conductor/references/research/protocol.md) - Main documentation
- [Agents](../conductor/references/research/agents/) - Sub-agent definitions
- [Hooks](../conductor/references/research/hooks/) - Integration points
- [grounding.md](references/grounding.md) - Deprecated, redirects to research

---

## After the Design

### Review and Handoff

1. Ask: "Review the design?"
2. Address any feedback
3. When approved, say: **"Design approved. Run `/conductor-newtrack {track_id}` to generate spec, plan, and file beads."**

If a track doesn't exist yet, suggest running `/conductor-newtrack <description>` first.

For the full implementation workflow after design, see `skills/conductor/SKILL.md`.

## Key Principles

- **One question at a time** - Don't overwhelm
- **Multiple choice preferred** - Easier to answer
- **YAGNI ruthlessly** - Remove unnecessary features
- **Explore alternatives** - Always propose 2-3 approaches
- **Incremental validation** - Present in sections, validate each
- **Be flexible** - Go back when something doesn't make sense
- **Research everything** - Verify with parallel agents before finalizing
