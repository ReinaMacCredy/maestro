
# Pipeline Smoke Test

## Discovery

This feature validates the full maestro pipeline end-to-end. Every MCP tool group must be exercised: feature, memory, plan, task, handoff, doctrine, graph, search, DCP, stage, skill, visual, config, and meta tools. The test uses a synthetic feature with 2 tasks to verify claim/done/verification, dependency resolution, handoff send/receive, and doctrine generation.

## Non-Goals

- No real code changes -- this is a validation-only feature
- No external integrations (CI, GitHub)

## Ghost Diffs

- No source files modified
- No test files modified

## Tasks

### 1. validate-core-tools
Exercise feature, memory, plan, task, config, doctor, ping, skill, and DCP tools.

### 2. validate-advanced-tools
Exercise handoff, doctrine, graph, search, visual, stage, and execution-insights tools.
Depends on: 1
