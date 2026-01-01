# Spec: Orchestrator Stress Test

## Overview

Test the restructured orchestrator skill with 4 parallel workers coordinated via Agent Mail.

## Requirements

### Functional

1. **Worker Spawning** - All 4 workers spawn via `Task()` tool
2. **Agent Mail Integration** - Workers register, send messages, fetch inbox
3. **File Reservations** - Each worker reserves its target file exclusively
4. **Dependency Handling** - Wave 2 waits for Wave 1; Wave 3 waits for Wave 2
5. **Completion** - All beads closed with `completed` status

### Non-Functional

- Workers execute in <30 seconds each
- Agent Mail latency <3 seconds
- No file conflicts

## Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC1 | Wave 1 tasks (A+B) run in parallel | Check spawn times are within 1s |
| AC2 | Wave 2 task (C) waits for A+B | Check start time > A,B end times |
| AC3 | Wave 3 task (D) waits for C | Check start time > C end time |
| AC4 | All 4 files created | `ls demo/` shows all files |
| AC5 | Agent Mail messages logged | `search_messages` returns entries |
| AC6 | All beads closed | `bd list --status closed` shows 5 |

## Out of Scope

- Error recovery testing
- Performance benchmarking
- Multi-codebase coordination
