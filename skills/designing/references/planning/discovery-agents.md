# Discovery Agents Reference

> Parallel agent orchestration for Phase 5 discovery research.

## Overview

Discovery uses parallel agents to gather context before planning. Each agent specializes in a different research domain, running concurrently with graceful degradation.

## Hook Definition

| Hook | Phase | Trigger |
|------|-------|---------|
| `pl-discovery` | 5 | After problem definition, before spec writing |

## Parallel Agent Specs

### 1. finder (Codebase Search)

**Purpose:** Search codebase for patterns, similar implementations, related code.

```
Tool: finder
Scope: Project workspace
```

**Query Examples:**
- "Find authentication middleware implementations"
- "Locate existing validation patterns for user input"
- "Search for similar API endpoint structures"

**Output:** File paths, line numbers, code snippets

---

### 2. Librarian (Documentation)

**Purpose:** Search README files, architecture docs, project documentation.

```
Tool: Read + glob
Scope: docs/, README.md, *.md files
```

**Search Targets:**
- `README.md` - Project overview, setup instructions
- `docs/` - Architecture, API docs, guides
- `AGENTS.md`, `CLAUDE.md` - Agent configuration
- `conductor/` - Product context, tech stack

**Output:** Relevant documentation excerpts, architectural decisions

---

### 3. web_search (External Research)

**Purpose:** Find external APIs, libraries, best practices.

```
Tool: web_search
Scope: Internet
```

**Query Types:**
- API documentation for external services
- Library comparison and recommendations
- Industry best practices for the problem domain
- Security considerations, common pitfalls

**Output:** URLs, summaries, recommendations

## Execution Policy

**No timeout** - agents run until completion. Discovery is critical; don't cut off valuable research.

| Agent | Execution | Rationale |
|-------|-----------|-----------|
| finder | Run to completion | Thorough codebase search |
| Librarian | Run to completion | Complete documentation scan |
| web_search | Run to completion | Full external research |

## Fallback Behavior

### Per-Agent Failure

If an individual agent fails (exception, not timeout):

```
1. Log warning: "[DISCOVERY] {agent} failed: {error}"
2. Continue with results from other agents
3. Mark agent section as "unavailable" in report
```

### Total Failure

If ALL agents fail:

```
1. Log error: "[DISCOVERY] All agents failed"
2. Prompt user for manual research:
   
   ⚠️ Discovery agents unavailable.
   Please provide:
   - Related files/code to reference
   - Relevant documentation
   - External APIs/libraries to consider
   
3. Continue with user-provided context
```

## Discovery Report Template

Output location: `conductor/tracks/<id>/discovery.md`

```markdown
# Discovery Report

**Track:** <track-id>
**Generated:** <timestamp>
**Status:** complete | partial | manual

---

## Codebase Analysis

**Agent:** finder
**Status:** ✅ complete | ⚠️ partial | ❌ unavailable

### Related Files
- `path/to/file.ts` - Description of relevance
- `path/to/other.ts` - Description of relevance

### Existing Patterns
- **Pattern Name:** Description, location
- **Pattern Name:** Description, location

### Potential Conflicts
- Files that may need modification
- Existing code that overlaps with planned work

---

## Documentation

**Agent:** Librarian
**Status:** ✅ complete | ⚠️ partial | ❌ unavailable

### Relevant Docs
- `docs/architecture.md` - Relevant section summary
- `README.md` - Setup/config considerations

### Architectural Decisions
- ADR references if applicable
- Existing constraints to honor

### Tech Stack Notes
- Framework versions
- Dependency constraints

---

## External Research

**Agent:** web_search
**Status:** ✅ complete | ⚠️ partial | ❌ unavailable

### APIs/Libraries
| Name | Purpose | Link |
|------|---------|------|
| library-name | Use case | URL |

### Best Practices
- Industry standard approach
- Security considerations
- Performance recommendations

### Risks/Considerations
- Known issues with recommended approach
- Alternative approaches considered

---

## Summary

### Key Findings
1. Finding one
2. Finding two
3. Finding three

### Recommendations
- Recommendation for implementation approach
- Dependencies to add/update
- Patterns to follow

### Open Questions
- [ ] Question needing user input
- [ ] Clarification needed
```

## Integration with Planning Phase

The discovery report feeds into Phase 6 (Spec Writing):

```
Phase 5 (Discovery)
    │
    ├── Run parallel agents
    ├── Generate discovery.md
    │
    ▼
Phase 6 (Spec Writing)
    │
    ├── Read discovery.md
    ├── Incorporate findings into spec
    └── Reference discovered patterns
```

## Example Usage

```python
# Pseudocode for discovery orchestration
async def run_discovery(track_id: str) -> DiscoveryReport:
    agents = [
        Agent("finder", query=build_finder_query(track_id)),
        Agent("librarian", query=build_doc_query(track_id)),
        Agent("web_search", query=build_research_query(track_id)),
    ]
    
    # No timeout - run until all agents complete
    results = await asyncio.gather(
        *[a.run() for a in agents],
        return_exceptions=True
    )
    
    report = DiscoveryReport()
    for agent, result in zip(agents, results):
        if isinstance(result, Exception):
            report.mark_unavailable(agent.name, str(result))
            log.warning(f"[DISCOVERY] {agent.name} failed: {result}")
        else:
            report.add_section(agent.name, result)
    
    if report.all_failed():
        return await prompt_manual_research()
    
    return report
```

## Related

- [Phase 5: Planning](../../../conductor/references/workflows/implement.md) - Integration point
- [orchestrator](../../../orchestrator/SKILL.md) - Parallel agent dispatch
- [finder tool](finder) - Codebase search semantics
