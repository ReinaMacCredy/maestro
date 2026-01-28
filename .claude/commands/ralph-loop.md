---
description: Start autonomous execution loop until completion promise detected
allowed-tools: Bash, Read, Write, Edit, Task, TaskCreate, TaskUpdate
---

# Ralph Loop

Start an autonomous execution loop that continues until a completion promise is detected.

## Usage

```
/ralph-loop [--max N] [--promise TEXT] [--ultrawork]
```

## Options

- `--max N` - Limit to N iterations (default: 100)
- `--promise TEXT` - Custom completion promise (default: "DONE")
- `--ultrawork` - Enable ultrawork mode for maximum thoroughness

## How It Works

1. **Start**: Run `./scripts/start-ralph.sh` to initialize loop state
2. **Monitor**: Check `.atlas/ralph-loop.local.md` for state
3. **Execute**: Work continuously via orchestrator delegation
4. **Complete**: Stop when promise detected, max iterations reached, or cancelled

## State File

`.atlas/ralph-loop.local.md` tracks:
- Active status
- Iteration count
- Completion promise
- Last action timestamp

## Cancellation

Use `/cancel-ralph` to stop the loop immediately.

---

## References

- [Atlas SKILL.md](../skills/atlas/SKILL.md)
- [Ralph Workflow](../skills/atlas/references/workflows/ralph.md)

**Spawns**: atlas-orchestrator (autonomous mode)
**Uses Skills**: atlas
