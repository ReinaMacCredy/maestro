# Ralph Loop - Autonomous Execution

> Named after Ralph Wiggum ("I'm helping!")

**Note**: Ralph loop integration is available for Atlas but uses the shared infrastructure.

## Command Syntax

```
/ralph-loop                    # Continue from current plan/boulder state
/ralph-loop "implement auth"   # Start with specific task description
/cancel-ralph                  # Stop the loop immediately
```

## Atlas Integration

When using Ralph loop with Atlas:

1. **Plans**: Reads from `.claude/plans/*.md`
2. **Boulder**: Uses `.atlas/boulder.json` for progress
3. **Wisdom**: Accumulates learnings in `.atlas/notepads/`
4. **Agents**: Delegates to atlas-leviathan, atlas-kraken, or atlas-spark

## State File Format

`.atlas/ralph-loop.local.md`:

```markdown
# Ralph Loop State

- **active**: true
- **iteration**: 3
- **max_iterations**: 10
```

## The Promise

When ALL work is complete, include in your response:

```xml
<promise>DONE</promise>
```

## Triggers

| Trigger | Action |
|---------|--------|
| `/ralph-loop` | Start/continue autonomous mode |
| `/cancel-ralph` | Stop the loop, remove state |

## Safety

- **Max iterations**: 10 (default)
- **Manual cancel**: `/cancel-ralph` always available
- **State file**: `.atlas/ralph-loop.local.md` is gitignored
