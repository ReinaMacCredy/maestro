# Spec: Orchestrator Mode Test

## Overview

Validate orchestrator Light/Full mode selection and pre-registration improvements.

## Requirements

### REQ-1: Mode Auto-Selection
- System MUST auto-select LIGHT mode when:
  - No cross-track dependencies exist
  - Estimated task duration < 10 minutes
  - Agent Mail is not required for coordination

### REQ-2: Parallel Worker Execution
- System MUST spawn 2 workers in parallel via Task() tool
- Workers MUST execute independently without shared state
- Workers MUST NOT require Agent Mail registration in LIGHT mode

### REQ-3: Task Return Collection
- Orchestrator MUST collect structured results from Task() return values
- Results MUST include: status, files_changed, key_decisions, issues, beads_closed
- Orchestrator MUST aggregate results into final summary

### REQ-4: Bead Lifecycle
- Workers MUST claim beads with `bd update --status in_progress`
- Workers MUST close beads with `bd close --reason completed`
- Epic MUST be closed after all workers complete

## Acceptance Criteria

1. **Mode Selection**: LIGHT mode is selected (no FULL mode Agent Mail overhead)
2. **Parallel Execution**: Both workers run simultaneously
3. **No Errors**: No Agent Mail registration errors occur
4. **Beads Closed**: All beads show status=closed after completion
5. **Results Collected**: Orchestrator displays aggregated summary

## Out of Scope

- Full mode testing (Agent Mail coordination)
- Cross-track dependencies
- Heartbeat protocol
- File reservations
