# Notepad Wisdom System

> Persistent memory that survives sessions and accumulates across tasks.

## Directory Structure

```
.atlas/notepads/{plan-name}/
├── learnings.md      # Patterns, conventions, successful approaches
├── issues.md         # Problems, blockers, gotchas encountered
├── decisions.md      # Architectural choices and rationales
└── problems.md       # Unresolved issues, technical debt
```

## How Wisdom Flows

### 1. Atlas-Leviathan Writes
During task execution, Atlas-Leviathan appends findings to notepad files.

### 2. Orchestrator Reads
Before delegating each task, Orchestrator reads all notepad files.

### 3. Wisdom Compounds
Each subsequent task receives ALL prior wisdom as context.

## Orchestrator Integration

### After Each Task
Instruct subagent to append findings:
```
When complete, append any discoveries to:
- .atlas/notepads/{plan}/learnings.md for patterns
- .atlas/notepads/{plan}/issues.md for gotchas
- .atlas/notepads/{plan}/decisions.md for choices
- .atlas/notepads/{plan}/problems.md for debt
```

## Lifecycle

1. **Creation**: Notepad directory created on first `append` or `init`
2. **Growth**: Entries accumulate throughout plan execution
3. **Usage**: Orchestrator reads and passes to each task
4. **Cleanup**: Manual deletion when plan is complete

```bash
# Remove notepad when done
rm -rf .atlas/notepads/my-plan/
```
