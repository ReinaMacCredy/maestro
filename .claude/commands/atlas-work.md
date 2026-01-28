---
description: Start execution of an Atlas plan as orchestrator
allowed-tools: Task, TodoWrite, Read
---

# Atlas Work

Begin executing an Atlas plan in orchestrator mode.

## Usage

```
/atlas-work [plan-name] [ulw]
```

Options:
- `plan-name` - Specific plan to execute (optional, defaults to most recent)
- `ulw` or `ultrawork` - Enable high-priority thoroughness mode

## How It Works

The `keyword-detector.sh` hook automatically:
1. Detects `/atlas-work` in your prompt
2. Loads the plan from `.claude/plans/`
3. Initializes `boulder.json` with orchestrator state
4. Injects the plan content and spawning instructions

**You do NOT need to run any script.** The hook injects context telling you to spawn the orchestrator subagent.

## After Hook Injection

When you see `[EXECUTION MODE]` in the injected context:

1. **Load the orchestration skill**: Read `skills/atlas/references/agents/atlas-orchestrator.md` and adopt its patterns
2. **You ARE now the Orchestrator+**: The skill transforms this session into orchestration mode
3. **Begin execution**: Follow the orchestration skill's Execution Workflow to delegate tasks

You do NOT spawn a subagent. You adopt orchestrator behavior directly.

## Orchestrator+ Mode

When orchestration skill is loaded, you:
1. **Primarily delegate** complex implementation via Task()
2. **Handle directly** simple operations (reads, status checks)
3. **VERIFY every task** independently after delegation
4. **ONE task per Task() call** - atomic units only
5. **Accumulate wisdom** - record learnings in `.atlas/notepads/`

---

## Component Context

This command is part of the Atlas workflow. See `skills/atlas/SKILL.md` for:
- Full Component Registry
- Chaining patterns
- Related commands and agents

**Loads Skill**: skills/atlas/references/agents/atlas-orchestrator.md
**Uses Skills**: atlas
