# .claude

## Purpose
Runtime kernel for the Atlas Workflow System - transforms LLM sessions into persistent, multi-agent orchestration.

## Key Directories

| Directory | Purpose |
|-----------|---------|
| agents/ | Agent definitions (prompts/personas for specialized workers) |
| commands/ | Slash-command definitions bridging user intent to workflows |
| hooks/ | Event bus configuration (hooks.json triggers scripts on events) |
| skills/ | Modular capability packages loaded by agents |
| scripts/ | Shell/Python scripts enforcing rules and managing state |
| plans/ | Storage for generated project plans (.md files) |

## Patterns

- **Command-Hook-Skill Architecture**: Commands define interface, hooks enforce process, skills provide logic
- **Agent Symlinks**: Agents mirror definitions in `skills/atlas/references/agents/` for single source of truth
- **Plan-Execute Cycle**: /atlas-plan spawns prometheus for planning, /atlas-work loads orchestration for execution
- **Orchestrator Pattern**: Main session never implements directly, uses Task() to spawn sub-agents

## Key Files

| File | Purpose |
|------|---------|
| settings.json | Claude Code configuration |
| hooks/hooks.json | Event-to-script mapping |

## Dependencies

- **Internal**: Skills depend on each other (atlas -> orchestration -> git-master)
- **External**: Claude Code runtime, bd CLI for beads

## Notes for AI Agents

- This is the "Atlas Operating System" - all workflow coordination happens here
- Never modify hooks.json without understanding the event chain
- Commands are macros - they inject context and trigger skill loading
- Plans in plans/ are READ-ONLY during execution
