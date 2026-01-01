# Design: Orchestrator Demo

## Problem Statement

Developers evaluating Maestro have no quick way to validate the orchestrator works correctly.

## Success Criteria

- Demo runs in < 2 minutes
- Shows parallel agent execution
- Produces visible output confirming success

## Solution

Create `demo/orchestrator-demo/` with:
1. Two task files for parallel workers
2. Results directory for output
3. README with instructions

## Components

```
demo/
├── README.md                    # Quick start guide
└── orchestrator-demo/
    ├── task-a.md                # Worker A: count skills/ files
    ├── task-b.md                # Worker B: count conductor/ files
    └── results/                 # Output directory
        ├── worker-a-result.md
        └── worker-b-result.md
```

## Flow

1. User runs demo command
2. Orchestrator registers with Agent Mail
3. Spawns 2 parallel Task() workers
4. Workers execute independently, write results
5. Orchestrator verifies completion

## Acceptance Criteria

- [ ] Both workers complete successfully
- [ ] Results files are created
- [ ] Agent Mail messages are sent
