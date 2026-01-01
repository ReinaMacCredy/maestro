# Spec: Orchestrator Demo

## Overview

Create a demo directory that validates the orchestrator parallel execution flow.

## Requirements

### Functional

1. **Demo Directory Structure**
   - `demo/orchestrator-demo/` with task files and results directory
   - README explaining how to run the demo

2. **Parallel Execution**
   - Two independent workers execute simultaneously
   - Each worker performs a distinct task (file counting)
   - Results written to separate files

3. **Verification**
   - Orchestrator confirms both workers completed
   - Results files exist and contain valid output

### Non-Functional

- Demo completes in under 2 minutes
- Works without external dependencies
- Self-documenting via README

## Out of Scope

- Agent Mail integration testing (separate concern)
- Performance benchmarking
- CI/CD integration

## Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC1 | Demo directory created | `ls demo/orchestrator-demo/` |
| AC2 | Workers run in parallel | Both Task() in same message |
| AC3 | Results files created | `ls demo/orchestrator-demo/results/` |
| AC4 | README exists | `cat demo/README.md` |
