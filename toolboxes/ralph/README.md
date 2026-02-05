# Ralph - Autonomous AI Agent Loop

Ralph is an autonomous AI agent loop that runs Amp repeatedly until all PRD (Product Requirements Document) items are complete. Each iteration spawns a fresh Amp instance with clean context, enabling long-running tasks to be broken into manageable chunks.

## How It Works

1. Ralph reads a `prd.json` file containing user stories with acceptance criteria
2. Each iteration picks the highest priority incomplete story
3. The agent implements the story, runs quality checks, and commits
4. Progress is logged to `progress.txt` for future iterations
5. Loop continues until all stories pass or max iterations reached

## Usage

```bash
./ralph.sh [max_iterations]
```

- `max_iterations` - Maximum number of iterations to run (default: 10)

### Example

```bash
# Run with default 10 iterations
./ralph.sh

# Run with 20 iterations
./ralph.sh 20
```

## Maestro Integration

Ralph can integrate with Maestro v2 workflow for autonomous execution of plans.

## Files

| File | Description |
|------|-------------|
| `ralph.sh` | The bash loop that spawns fresh Amp instances |
| `prompt.md` | Instructions given to each Amp instance |

## Requirements

- Amp CLI installed and configured
- A `prd.json` file in this directory (see project's `prd.json.example` for format)

## Memory Model

Ralph maintains context across iterations through:

- **Git history** - All changes are committed
- **progress.txt** - Logs what was done and learnings
- **prd.json** - Tracks completion status of each story
- **AGENTS.md** - Discovered patterns for future iterations

## References

- [Geoffrey Huntley's Ralph article](https://ghuntley.com/ralph/)
- [Anthropic's agent harnesses research](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents)
- [Amp documentation](https://ampcode.com/manual)
