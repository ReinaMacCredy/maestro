# Design: Orchestrator Stress Test
https://ampcode.com/threads/T-019b78e0-3601-750c-9766-bb83d758c1b7
## Problem Statement

Verify the restructured orchestrator skill (87 lines) correctly coordinates 4+ parallel workers using Agent Mail, with proper file reservations and dependency handling.

## Success Criteria

1. All 4 workers spawn successfully via `Task()` tool
2. Agent Mail messages sent/received between workers
3. File reservations prevent conflicts
4. Dependent task waits for prerequisites
5. All beads closed with `completed` status

## Approach

**Mixed workload with dependency chain:**

```
┌─────────────────┐   ┌─────────────────┐
│ Track A: README │   │ Track B: Config │  (Wave 1 - parallel)
│ demo/README.md  │   │ demo/config.json│
└────────┬────────┘   └────────┬────────┘
         │                     │
         └──────────┬──────────┘
                    ▼
┌─────────────────────────────────────────┐
│ Track C: API Stub (depends on A+B)      │  (Wave 2)
│ demo/api.ts                             │
└────────────────────┬────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────┐
│ Track D: Test File (depends on C)       │  (Wave 3)
│ demo/api.test.ts                        │
└─────────────────────────────────────────┘
```

## Tasks

### Wave 1 (Parallel)

| Task | File | Description |
|------|------|-------------|
| A | demo/README.md | Create README documenting the demo |
| B | demo/config.json | Create config with project metadata |

### Wave 2 (Depends on A+B)

| Task | File | Description |
|------|------|-------------|
| C | demo/api.ts | Create API stub that reads config |

### Wave 3 (Depends on C)

| Task | File | Description |
|------|------|-------------|
| D | demo/api.test.ts | Create test for API stub |

## File Reservations

| Track | Reserved Paths | Exclusive |
|-------|---------------|-----------|
| A | demo/README.md | Yes |
| B | demo/config.json | Yes |
| C | demo/api.ts | Yes |
| D | demo/api.test.ts | Yes |

## Acceptance Criteria

- [ ] Wave 1: Tracks A+B complete in parallel
- [ ] Wave 2: Track C starts only after A+B complete
- [ ] Wave 3: Track D starts only after C completes
- [ ] All files created in demo/ directory
- [ ] Agent Mail shows coordination messages
- [ ] All beads closed with reason: completed

## Risks

- **Agent Mail unavailable**: Fallback to sequential execution
- **File reservation conflict**: Workers wait or escalate

## Next Steps

Run `/conductor-newtrack orchestrator-stress-test` to generate spec and plan.
