# Intent Routing

> **Maps user intent keywords to specialized sub-agents.**

## Quick Reference

| Intent Keywords | Agent Type | Description |
|-----------------|------------|-------------|
| `research`, `find`, `locate`, `where is` | Research | Codebase exploration |
| `review`, `check`, `audit`, `security` | Review | Code/security review |
| `plan`, `design`, `architect`, `structure` | Planning | Design decisions |
| `implement`, `build`, `create`, `add` | Execution | Code implementation |
| `fix`, `debug`, `investigate`, `trace` | Debug | Bug investigation |
| `test`, `verify`, `validate` | Testing | Test creation/verification |
| `refactor`, `improve`, `optimize` | Refactor | Code improvement |
| `document`, `explain`, `describe` | Docs | Documentation |
| `analyze`, `understand`, `how does` | Analysis | Code comprehension |
| `migrate`, `upgrade`, `convert` | Migration | Version/format updates |
| `configure`, `setup`, `install` | Config | Environment setup |
| `deploy`, `release`, `ship` | Deploy | Deployment tasks |
| `monitor`, `log`, `track` | Observability | Monitoring setup |
| `benchmark`, `performance`, `profile` | Performance | Performance analysis |
| `integrate`, `connect`, `hook` | Integration | System integration |

## Agent Categories

### Research Agents

Handle codebase exploration and understanding.

| Agent | Keywords | Responsibilities |
|-------|----------|-----------------|
| Locator | `find`, `where`, `locate` | Find files, functions, symbols |
| Analyzer | `analyze`, `understand`, `how does` | Deep code analysis |
| Pattern | `pattern`, `convention`, `example` | Find existing patterns |
| Web | `docs`, `api`, `external` | External documentation |

### Review Agents

Handle code quality and security.

| Agent | Keywords | Responsibilities |
|-------|----------|-----------------|
| CodeReview | `review`, `check` | General code review |
| SecurityAudit | `security`, `audit`, `vulnerability` | Security analysis |
| PerformanceReview | `perf`, `slow`, `optimize` | Performance review |

### Planning Agents

Handle design and architecture decisions.

| Agent | Keywords | Responsibilities |
|-------|----------|-----------------|
| Architect | `design`, `architect`, `structure` | System design |
| Planner | `plan`, `approach`, `strategy` | Implementation planning |

### Execution Agents

Handle implementation work.

| Agent | Keywords | Responsibilities |
|-------|----------|-----------------|
| Implementer | `implement`, `build`, `create` | Write new code |
| Modifier | `add`, `change`, `update` | Modify existing code |
| Fixer | `fix`, `bug`, `patch` | Bug fixes |
| Refactorer | `refactor`, `improve`, `clean` | Code improvement |

### Debug Agents

Handle investigation and debugging.

| Agent | Keywords | Responsibilities |
|-------|----------|-----------------|
| Debugger | `debug`, `investigate` | Find root cause |
| Tracer | `trace`, `follow`, `track` | Trace execution |

## Routing Logic

```python
def route_intent(user_request: str) -> AgentType:
    keywords = extract_keywords(user_request.lower())
    
    # Priority order matters - first match wins
    if any(k in keywords for k in ['security', 'audit', 'vulnerability']):
        return AgentType.SECURITY_REVIEW
    
    if any(k in keywords for k in ['review', 'check']):
        return AgentType.CODE_REVIEW
    
    if any(k in keywords for k in ['debug', 'investigate', 'trace']):
        return AgentType.DEBUG
    
    if any(k in keywords for k in ['fix', 'bug']):
        return AgentType.FIXER
    
    if any(k in keywords for k in ['test', 'verify', 'validate']):
        return AgentType.TESTING
    
    if any(k in keywords for k in ['implement', 'build', 'create', 'add']):
        return AgentType.EXECUTION
    
    if any(k in keywords for k in ['refactor', 'improve', 'optimize']):
        return AgentType.REFACTOR
    
    if any(k in keywords for k in ['plan', 'design', 'architect']):
        return AgentType.PLANNING
    
    if any(k in keywords for k in ['document', 'explain', 'describe']):
        return AgentType.DOCS
    
    if any(k in keywords for k in ['research', 'find', 'locate', 'where']):
        return AgentType.RESEARCH
    
    if any(k in keywords for k in ['analyze', 'understand', 'how does']):
        return AgentType.ANALYSIS
    
    # Default to research for ambiguous requests
    return AgentType.RESEARCH
```

## Multi-Agent Dispatch

Some requests need multiple agents:

| Request Pattern | Agents Dispatched |
|-----------------|-------------------|
| "Review and fix X" | Review → Fix (sequential) |
| "Find and document X" | Research + Docs (parallel) |
| "Implement with tests" | Execution + Testing (sequential) |
| "Debug and trace X" | Debug + Tracer (parallel) |

## Agent Spawn Template

```bash
Task(
    description=f"""You are {agent_name}, a {agent_type} specialist.

## Assignment
{task_description}

## Context
{relevant_context}

## File Scope
{file_patterns}

## Protocol
1. Reserve files before editing:
   toolboxes/agent-mail/agent-mail.js file-reservation-paths \
     project_key:"$PROJECT_PATH" \
     agent_name:"$AGENT_NAME" \
     paths:'["<scope>"]' \
     exclusive:true
2. Do the work
3. Send summary via Agent Mail CLI before returning
4. Return structured result

## CRITICAL
- Stay within file scope
- Report via agent-mail.js send-message before returning
""",
    prompt=user_request
)
```

## Error Handling

| Scenario | Handling |
|----------|----------|
| Unknown intent | Default to Research agent |
| Multiple matches | Use priority order |
| Agent fails | Main thread handles, retries or escalates |
| File conflict | Agent waits for reservation |
