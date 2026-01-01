# Orchestrator Demo

This demo shows multi-agent parallel execution using the Orchestrator skill with Agent Mail coordination.

## What It Does

The Orchestrator spawns autonomous worker agents to execute independent tracks in parallel:

1. **Main Orchestrator** - Parses plan.md, spawns workers, monitors progress
2. **Worker Agents** - Execute assigned beads, reserve files, report via Agent Mail
3. **Agent Mail** - Coordinates between agents with messages, file reservations, blockers

## Quick Start

### Prerequisites

- Agent Mail MCP server running (`npx beads-village` or Agent Mail MCP)
- Beads CLI installed (`bd` command available)
- A plan.md with Track Assignments section

### Run the Demo

```bash
# 1. Create a plan with Track Assignments
cat > conductor/tracks/demo-track/plan.md << 'EOF'
## Orchestration Config

epic_id: demo-epic
max_workers: 3
mode: autonomous

## Track Assignments

| Track | Agent | Beads | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | demo-1 | src/api/** | - |
| 2 | GreenCastle | demo-2 | src/web/** | demo-1 |
| 3 | RedStone | demo-3 | docs/** | demo-2 |
EOF

# 2. File beads from the plan
fb

# 3. Or trigger orchestration directly
/conductor-orchestrate
```

### Alternative Triggers

```bash
# Direct command
co

# Natural language
"run parallel"
"spawn workers"
"dispatch agents"
```

## Expected Output

```
📋 Parsed Track Assignments: 3 tracks
🔧 Initializing Agent Mail...
✓ Registered: OrchestratorDemo

🚀 Spawning workers...
  → Track 1: BlueLake (src/api/**)
  → Track 2: GreenCastle (src/web/**)
  → Track 3: RedStone (docs/**)

📬 Monitoring progress...
  [BlueLake] ✓ demo-1 complete
  [GreenCastle] ✓ demo-2 complete
  [RedStone] ✓ demo-3 complete

✅ EPIC COMPLETE: All 3 tracks succeeded
```

## Key Concepts

| Concept | Description |
|---------|-------------|
| Track Assignments | Maps beads to workers with file scopes |
| File Reservations | Prevents conflicts via `file_reservation_paths()` |
| Agent Mail | Async messaging for progress/blockers |
| Cross-Track Dependencies | Workers wait for upstream beads |

## Fallback

If Agent Mail is unavailable, the orchestrator degrades to sequential execution via `/conductor-implement`.

## See Also

- [Orchestrator SKILL.md](../skills/orchestrator/SKILL.md)
- [Worker Protocol](../skills/orchestrator/references/worker-prompt.md)
- [Agent Routing](../skills/orchestrator/references/agent-routing.md)
