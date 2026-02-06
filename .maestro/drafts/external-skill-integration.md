# External Skill Integration - Design Draft

## Status: INTERVIEW IN PROGRESS

## Understanding So Far

### Current Architecture
- **Skills**: Single skill at `.claude/skills/maestro/SKILL.md` - defines triggers, workflows, agents
- **Plugin manifest**: `.claude-plugin/plugin.json` - points to commands, agents, skills, hooks
- **Agents**: 6 agents in `.claude/agents/` - each can reference skills and other agents
- **Commands**: `/design`, `/work` - entry points that invoke agents
- **No external skill imports currently** - everything is self-contained

### Key Observations
1. Skills are markdown files with YAML frontmatter (name, description)
2. Plugin.json defines the skill directory path, not individual skills
3. Agents reference other agents by `subagent_type` parameter in Task tool
4. No existing mechanism for loading skills from external sources

---

## Confirmed Requirements

(None yet - pending interview)

---

## Open Questions

### 1. Direction of Integration
- **Import external skills INTO Maestro?** (use v0's components inside Maestro workflows)
- **Export Maestro skills to external systems?** (make Maestro agents available to v0)
- **Bidirectional?** (both import and export)

### 2. External Skill Sources
What specific sources do you want to integrate with?
- Anthropic official plugins/skills
- v0 (Vercel)
- Other Claude Code plugins
- MCP servers
- Custom/private skill repositories

### 3. Integration Depth
- **Loose coupling**: External skills invoked as standalone tools/commands
- **Composable**: External skills can be teammates in Agent Teams
- **Deep integration**: External skills can access Maestro's TaskList, plans, etc.

### 4. Discovery and Installation
- Manual configuration (add to plugin.json)?
- CLI tool to add skills?
- Registry/marketplace lookup?

### 5. Skill Compatibility
- What happens when external skill conflicts with Maestro agent?
- How to handle different model requirements?
- Namespace/prefix for external skills?

---

## Technical Constraints

(To be discovered)

---

## Scope Boundaries

**IN**: (pending)

**OUT**: (pending)

---

## Test Strategy

(pending)

---

## CLEARANCE CHECKLIST

- [ ] Core objective clearly defined?
- [ ] Scope boundaries established (IN/OUT)?
- [ ] No critical ambiguities remaining?
- [ ] Technical approach decided?
- [ ] Test strategy confirmed?
- [ ] No blocking questions outstanding?

**STATUS: NOT CLEARED** - Need answers to open questions
