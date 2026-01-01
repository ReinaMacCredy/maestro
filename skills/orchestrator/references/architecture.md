# Orchestrator Architecture

## System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ORCHESTRATOR                                   │
│                              (Main Agent)                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  1. Read plan.md Track Assignments                                          │
│  2. Initialize Agent Mail                                                   │
│  3. Spawn workers via Task()                                                │
│  4. Monitor progress via fetch_inbox                                        │
│  5. Handle cross-track blockers                                             │
│  6. Announce completion                                                     │
└─────────────────────────────────────────────────────────────────────────────┘
           │
           │ Task() spawns parallel workers
           ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  Worker A        │  │  Worker B        │  │  Worker C        │
│  Track 1         │  │  Track 2         │  │  Track 3         │
├──────────────────┤  ├──────────────────┤  ├──────────────────┤
│  For each bead:  │  │  For each bead:  │  │  For each bead:  │
│  • Reserve files │  │  • Reserve files │  │  • Reserve files │
│  • bd claim      │  │  • bd claim      │  │  • bd claim      │
│  • Do work       │  │  • Do work       │  │  • Do work       │
│  • bd close      │  │  • bd close      │  │  • bd close      │
│  • Report mail   │  │  • Report mail   │  │  • Report mail   │
└──────────────────┘  └──────────────────┘  └──────────────────┘
           │                   │                   │
           └───────────────────┼───────────────────┘
                               ▼
                    ┌─────────────────────┐
                    │     Agent Mail      │
                    │  ─────────────────  │
                    │  Epic Thread:       │
                    │  • Progress reports │
                    │  • Bead completions │
                    │  • Blockers         │
                    └─────────────────────┘
```

## Key Difference from /conductor-implement

| Aspect | /conductor-implement | /conductor-orchestrate |
|--------|---------------------|----------------------|
| Execution | Sequential, main agent | Parallel, worker subagents |
| bd access | Main agent only | **Workers CAN claim/close** |
| Coordination | N/A | Agent Mail MCP (Full) or Task return (Light) |
| File locking | N/A | file_reservation_paths |
| Context | In-memory | Track threads (persistent) |

## Auto-Orchestration Integration

When triggered from `fb` (file beads) auto-orchestration:

1. Track Assignments are **auto-generated** from beads dependency graph
2. No manual Track Assignments section needed in plan.md
3. Orchestrator receives assignments via in-memory call, not file parsing

### Auto-Generated vs Manual

| Source | How Detected | Behavior |
|--------|--------------|----------|
| Auto-generated | Called from fb Phase 6 | Assignments passed in-memory |
| Manual | User runs `/conductor-orchestrate` | Parse from plan.md |

Both flows converge at Phase 3 (Spawn Workers).

## Directory Structure

```
skills/orchestrator/
├── SKILL.md           # Main skill file
├── agents/            # Agent profiles by category
│   ├── research/      # Locator, Analyzer, Pattern, Web, GitHub
│   ├── review/        # CodeReview, SecurityAudit, PerformanceReview
│   ├── planning/      # Architect, Planner
│   ├── execution/     # Implementer, Modifier, Fixer, Refactorer
│   └── debug/         # Debugger, Tracer
├── references/        # Workflow documentation
│   ├── workflow.md    # 8-phase protocol
│   ├── preflight.md   # Session Brain preflight
│   ├── worker-prompt.md
│   └── patterns/
└── scripts/           # Session brain utilities
    └── preflight.py   # Preflight protocol implementation
```
