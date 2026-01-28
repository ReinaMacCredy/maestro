# skills

## Purpose
Modular capability packages that provide specialized instructions and context to agents.

## Key Directories

| Directory | Purpose |
|-----------|---------|
| atlas/ | Interview-driven planning and workflow orchestration |
| git-master/ | Expert Git operations (atomic commits, history navigation) |
| orchestration/ | Framework for delegating work to specialized agents |
| playwright/ | Browser automation for E2E testing and visual verification |

## Skill Structure Pattern

Each skill follows this structure:
```
skill-name/
  SKILL.md         # Main definition (YAML frontmatter + Markdown)
  references/      # Deep documentation and examples
```

## Patterns

- **YAML Frontmatter**: All SKILL.md files have name, description, tools in frontmatter
- **Delegation Chain**: User -> Atlas -> Orchestrator -> Worker Agents
- **Skill Loading**: Agents explicitly declare which skills they need
- **Verification First**: Skills emphasize self-verification and skepticism of outputs

## Skill Dependencies

```
atlas
  -> orchestration (delegation patterns)
  -> git-master (version control)
  -> playwright (browser testing)

orchestration
  -> (foundational, no dependencies)

git-master
  -> (standalone utility)

playwright
  -> (standalone, requires Node.js + Playwright binaries)
```

## Notes for AI Agents

- Always check SKILL.md for the canonical skill definition
- Skills are loaded via load_skills=["skill-name"] in delegate_task
- The references/ directory contains deep docs - read when you need details
- To create a new skill, follow the SKILL.md + references/ pattern
