# Pipeline Architecture

Complete workflow pipeline with all loops, agent dispatch patterns, and the 12 BMAD agents.

## Complete Pipeline Overview

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
            DESIGNMD["design.md"]
        end
        
        subgraph SPEC["SPEC GENERATION"]
            NEWTRACK["/conductor-newtrack"]
            SPECMD["spec.md"]
            PLANMD["plan.md"]
        end
        
        subgraph BEADS["ISSUE FILING LOOP"]
            FB["fb (file-beads)"]
            EPIC["Create Epic"]
            ISSUES["Create Issues<br/>(batches of 5)"]
            DEPS["Wire Dependencies"]
            RB["rb (review-beads)"]
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
    
    subgraph BMAD["PARTY MODE: 12 BMAD AGENTS"]
        subgraph PRODUCT["Product Module"]
            PM["John<br/>Product Manager"]
            ANALYST["Mary<br/>Business Analyst"]
            UX["Sally<br/>UX Researcher"]
        end
        
        subgraph TECHNICAL["Technical Module"]
            ARCH["Winston<br/>Architect"]
            DEV["Amelia<br/>Developer"]
            QA["Murat<br/>QA Engineer"]
            DOCS["Paige<br/>Tech Writer"]
        end
        
        subgraph CREATIVE["Creative Module"]
            STORY["Sophia<br/>Storyteller"]
            BRAIN["Carson<br/>Brainstormer"]
            DESIGN["Maya<br/>Design Thinker"]
            STRAT["Victor<br/>Strategist"]
            SOLVER["Dr. Quinn<br/>Problem Solver"]
        end
    end
    
    DS --> DISCOVER
    DISCOVER --> DEFINE
    DEFINE --> DEVELOP
    DEVELOP --> DELIVER
    DELIVER --> APC
    APC -->|"C"| DESIGNMD
    APC -->|"P"| BMAD
    BMAD -->|"Synthesize"| APC
    DESIGNMD --> NEWTRACK
    
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
    classDef bmad fill:#553c9a,stroke:#b794f4,color:#e2e8f0
    classDef product fill:#285e61,stroke:#4fd1c5,color:#e2e8f0
    classDef technical fill:#2c5282,stroke:#63b3ed,color:#e2e8f0
    classDef creative fill:#744210,stroke:#f6ad55,color:#e2e8f0
    
    class DS,DISCOVER,DEFINE,DEVELOP,DELIVER,APC,DESIGNMD planning
    class NEWTRACK,SPECMD,PLANMD spec
    class FB,EPIC,ISSUES,DEPS,RB beads
    class COORDINATOR,W1,W2,W3,WN,MERGE dispatch
    class READY,CLAIM,CLOSE,SYNC agent
    class RED,GREEN,REFACTOR tdd
    class VERIFY,BRANCH,FINISH_CMD finish
    class PM,ANALYST,UX product
    class ARCH,DEV,QA,DOCS technical
    class STORY,BRAIN,DESIGN,STRAT,SOLVER creative
```

---

## Workflow Loops

### 1. Planning Loop (Double Diamond)

```mermaid
flowchart LR
    subgraph DD["DOUBLE DIAMOND"]
        D1["DISCOVER<br/>(Diverge)"]
        D2["DEFINE<br/>(Converge)"]
        D3["DEVELOP<br/>(Diverge)"]
        D4["DELIVER<br/>(Converge)"]
        
        D1 --> D2 --> D3 --> D4
    end
    
    subgraph CHECKPOINTS["A/P/C CHECKPOINTS"]
        A["[A] Advanced<br/>Deep analysis"]
        P["[P] Party<br/>Multi-agent review"]
        C["[C] Continue<br/>Next phase"]
    end
    
    D1 & D2 & D3 & D4 -.-> CHECKPOINTS
```

**Trigger**: `ds` or `/conductor-design`

**Output**: `conductor/tracks/{id}/design.md`

---

### 2. Spec Generation

```mermaid
flowchart LR
    DESIGN["design.md"] --> NEWTRACK["/conductor-newtrack"]
    NEWTRACK --> SPEC["spec.md<br/>Requirements + Acceptance"]
    NEWTRACK --> PLAN["plan.md<br/>Phased Tasks"]
```

**Trigger**: `/conductor-newtrack {track_id}`

**Output**: `spec.md` + `plan.md`

---

### 3. Issue Filing Loop (Beads)

```mermaid
flowchart TB
    FB["fb (file-beads)"] --> BATCH["Process in batches of 5"]
    BATCH --> EPIC["Create Epic"]
    EPIC --> ISSUES["Create Issues"]
    ISSUES --> DEPS["Wire Dependencies"]
    DEPS --> CHECKPOINT["Checkpoint Progress"]
    CHECKPOINT -->|"More phases?"| BATCH
    CHECKPOINT -->|"Done"| RB["rb (review-beads)"]
    RB --> VALIDATE["Parallel Validation"]
    VALIDATE --> READY["bd ready"]
```

**Trigger**: `fb` then `rb`

**State Files**:
- `.fb-progress.json` - Resume capability
- `.fb-progress.lock` - Concurrent session lock

---

### 4. Agent Execution Loop

```mermaid
flowchart TB
    READY["bd ready --json"]
    CLAIM["bd update <id> --status in_progress"]
    WORK["Execute Task"]
    VERIFY["Verify (tests, lint)"]
    CHECKPOINT["bd checkpoint"]
    CLOSE["bd close <id>"]
    SYNC["bd sync"]
    MORE{{"More issues?"}}
    
    READY --> CLAIM --> WORK --> VERIFY
    VERIFY -->|"pass"| CHECKPOINT --> CLOSE --> SYNC
    VERIFY -->|"fail"| WORK
    SYNC --> MORE
    MORE -->|"yes"| READY
    MORE -->|"no"| FINISH["finish branch"]
```

**Trigger**: `/conductor-implement` or `Start epic <id>`

---

### 5. TDD Micro-Loop

```mermaid
flowchart LR
    RED["RED<br/>Write failing test"]
    GREEN["GREEN<br/>Make it pass"]
    REFACTOR["REFACTOR<br/>Clean up"]
    
    RED --> GREEN --> REFACTOR --> RED
```

**Trigger**: `tdd`

**Iron Law**: No production code without a failing test first.

---

### 6. Parallel Agent Dispatch

```mermaid
flowchart TB
    COORD["Coordinator Agent"]
    
    subgraph PARALLEL["PARALLEL EXECUTION"]
        direction LR
        A1["Agent 1"]
        A2["Agent 2"]
        A3["Agent 3"]
        AN["Agent N"]
    end
    
    COLLECT["Collect & Merge Results"]
    
    COORD --> A1 & A2 & A3 & AN
    A1 & A2 & A3 & AN --> COLLECT
```

**Trigger**: `dispatch` or when 2+ independent tasks identified

**Use Cases**:
- Independent file modifications
- Parallel test execution
- Multi-file refactoring

---

### 7. Beads Village (Multi-Agent Coordination)

```mermaid
flowchart TB
    INIT["bv init --team=frontend --role=developer"]
    STATUS["bv status --robot-json"]
    CLAIM["bv claim <issue-id>"]
    RESERVE["bv reserve src/component.ts"]
    WORK["Execute Work"]
    MSG["bv msg --to=backend 'API ready'"]
    RELEASE["bv release src/component.ts"]
    DONE["bv done <issue-id>"]
    
    INIT --> STATUS --> CLAIM --> RESERVE --> WORK
    WORK --> MSG
    WORK --> RELEASE --> DONE
```

**Trigger**: Multi-agent collaborative work

**Commands**:
| Command | Purpose |
|---------|---------|
| `bv init` | Join workspace with team/role |
| `bv claim` | Atomic task claiming |
| `bv reserve` | Lock files |
| `bv msg` | Team messaging |
| `bv done` | Complete task |

---

## The 12 BMAD Agents (Party Mode)

Invoked via **[P] Party** at any A/P/C checkpoint.

### Product Module

| Agent | Name | Focus |
|-------|------|-------|
| PM | John | Product priorities, roadmap, stakeholder needs |
| Analyst | Mary | Requirements, metrics, business value |
| UX | Sally | User needs, flows, accessibility |

### Technical Module

| Agent | Name | Focus |
|-------|------|-------|
| Architect | Winston | System design, patterns, scalability |
| Developer | Amelia | Implementation, code quality, performance |
| QA | Murat | Testing, edge cases, reliability |
| Docs | Paige | Documentation, API specs, tutorials |

### Creative Module

| Agent | Name | Focus |
|-------|------|-------|
| Storyteller | Sophia | Narrative, user journey, empathy |
| Brainstormer | Carson | Wild ideas, 10x thinking, innovation |
| Design Thinker | Maya | Methodology, process, iteration |
| Strategist | Victor | Long-term vision, trade-offs, positioning |
| Problem Solver | Dr. Quinn | Root cause analysis, debugging, solutions |

### Agent Selection

Party Mode selects 3 agents based on context:
- **Primary**: Best expertise match
- **Secondary**: Complementary perspective
- **Tertiary**: Devil's advocate

```mermaid
flowchart LR
    PHASE["Current Phase"] --> SELECT["Select 3 Agents"]
    SELECT --> PRIMARY["Primary<br/>Best match"]
    SELECT --> SECONDARY["Secondary<br/>Complement"]
    SELECT --> TERTIARY["Tertiary<br/>Devil's advocate"]
    
    PRIMARY & SECONDARY & TERTIARY --> DISCUSS["Cross-talk & Debate"]
    DISCUSS --> SYNTHESIZE["Synthesize Insights"]
    SYNTHESIZE --> RETURN["Return to A/P/C"]
```

---

## Complete Session Flow

```mermaid
flowchart TB
    subgraph SESSION1["SESSION 1: Planning"]
        S1_SETUP["/conductor-setup"]
        S1_DS["ds (Double Diamond)"]
        S1_NEWTRACK["/conductor-newtrack"]
        S1_FB["fb + rb"]
        S1_HANDOFF["HANDOFF block"]
        
        S1_SETUP --> S1_DS --> S1_NEWTRACK --> S1_FB --> S1_HANDOFF
    end
    
    subgraph SESSION2["SESSION 2: Epic 1"]
        S2_PASTE["Paste HANDOFF"]
        S2_IMPLEMENT["/conductor-implement"]
        S2_TDD["TDD Cycle"]
        S2_RB["rb (optional)"]
        S2_HANDOFF["HANDOFF to Epic 2"]
        
        S2_PASTE --> S2_IMPLEMENT --> S2_TDD --> S2_RB --> S2_HANDOFF
    end
    
    subgraph SESSION3["SESSION 3: Epic 2+"]
        S3_PASTE["Paste HANDOFF"]
        S3_IMPLEMENT["/conductor-implement"]
        S3_TDD["TDD Cycle"]
        S3_COMPLETE["Track Complete"]
        
        S3_PASTE --> S3_IMPLEMENT --> S3_TDD --> S3_COMPLETE
    end
    
    subgraph FINISH["FINISH"]
        F_VERIFY["Verification"]
        F_BRANCH["finish branch"]
        F_FINISH_CMD["/conductor-finish"]
        
        F_VERIFY --> F_BRANCH --> F_FINISH_CMD
    end
    
    S1_HANDOFF -.-> S2_PASTE
    S2_HANDOFF -.-> S3_PASTE
    S3_COMPLETE --> F_VERIFY
```

---

## Quick Reference

| Loop | Trigger | Purpose |
|------|---------|---------|
| Planning | `ds` | Design exploration (Double Diamond) |
| Spec Gen | `/conductor-newtrack` | Create spec.md + plan.md |
| Issue Filing | `fb` → `rb` | Create trackable beads |
| Agent Execution | `bd ready` → claim → close | Do the work |
| TDD | `tdd` | RED → GREEN → REFACTOR |
| Parallel Dispatch | `dispatch` | 2+ independent tasks |
| Village | `bv init` | Multi-agent coordination |
| Completion | `finish branch` → `/conductor-finish` | Finalize work |

---

## Related Documentation

- [README.md](../README.md) - Overview and installation
- [TUTORIAL.md](../TUTORIAL.md) - Complete workflow guide
- [workflows/party-mode/workflow.md](../workflows/party-mode/workflow.md) - Party Mode details
- [skills/design/SKILL.md](../skills/design/SKILL.md) - Double Diamond methodology
- [skills/dispatching-parallel-agents/SKILL.md](../skills/dispatching-parallel-agents/SKILL.md) - Parallel dispatch
