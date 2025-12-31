# Spec: Continuous-Claude-v2 Integration

## Problem Statement

Maestro's current architecture pollutes the main conversation thread with task execution details, lacks specialized sub-agents for common tasks, and doesn't leverage Agent Mail for persistent context preservation across sessions.

## Success Criteria

| Criteria | Metric |
|----------|--------|
| Clean main thread | Task execution in sub-agents, main sees summaries only |
| Context preservation | All sub-agent output persisted to Agent Mail |
| Specialized agents | 15+ agents for specific tasks |
| Token efficiency | Main context < 300 lines (AGENTS.md + thin-router) |
| Handoff support | Manual `/create_handoff` works with Agent Mail |

## Solution Overview

### 1. Thin Router in AGENTS.md

Add ~50 lines to AGENTS.md with:
- Intent → Agent routing table
- Spawn pattern for Task() calls
- Summary protocol for sub-agent returns
- First-message context loading from Agent Mail

### 2. Agent Directory

Create `skills/orchestrator/agents/` with specialized agents:

```
agents/
├── README.md                    # Index + routing table
├── research/
│   ├── codebase-locator.md      # Find files
│   ├── codebase-analyzer.md     # Analyze code
│   ├── pattern-finder.md        # Find conventions
│   ├── impact-assessor.md       # Assess changes
│   ├── web-researcher.md        # External docs
│   └── github-researcher.md     # GitHub repo research
├── review/
│   ├── security-reviewer.md     # Security audit
│   ├── code-reviewer.md         # Code quality
│   ├── pr-reviewer.md           # PR review
│   └── spec-reviewer.md         # Spec compliance
├── planning/
│   ├── plan-agent.md            # Create plans
│   └── validate-agent.md        # Validate plans
├── execution/
│   ├── implement-agent.md       # TDD implementation
│   └── worker-agent.md          # Generic worker
└── debug/
    └── debug-agent.md           # Root cause analysis
```

### 3. Sub-Agent Protocol

Every sub-agent MUST:
1. Do assigned work
2. Call `send_message()` with FULL report to Agent Mail
3. Return SUMMARY ONLY to main thread

Summary format:
```markdown
## Summary
[1-2 sentences]

## Result
- Status: SUCCEEDED | PARTIAL | FAILED
- Files changed: N
- Key decisions: [list]

## Full Report
📧 Agent Mail thread: [thread-id]
```

### 4. Agent Mail Integration

| Operation | When |
|-----------|------|
| `fetch_inbox()` | First message of session |
| `register_agent()` | Before spawning workers |
| `send_message()` | Sub-agent saves context |
| `summarize_thread()` | Resume prior work |
| `search_messages()` | Find past context |

### 5. Handoff System (Manual)

Since Amp has no hooks:
- `/create_handoff` - User triggers handoff to Agent Mail
- `/resume_handoff` - Load context from Agent Mail
- No automatic PreCompact or SessionEnd saves

## Technical Design

### Intent Routing Table

```
| Intent | Agent |
|--------|-------|
| "security audit", "review security" | security-reviewer |
| "research repo", "check github" | github-researcher |
| "review code", "code review" | code-reviewer |
| "review PR" | pr-reviewer |
| "debug", "why failing" | debug-agent |
| "find where", "locate" | codebase-locator |
| "how does X work", "analyze" | codebase-analyzer |
| "what patterns" | pattern-finder |
| "create plan" | plan-agent |
| "validate plan" | validate-agent |
| "implement" | implement-agent |
| "assess impact" | impact-assessor |
| "parallel", "orchestrate" | orchestrator (full skill) |
```

### Spawn Pattern

```javascript
Task(
  prompt: `
    You are ${agent_name} agent.
    
    ${Read(agents/${category}/${agent_name}.md)}
    
    ## Your Task
    ${user_request}
    
    ## CRITICAL
    Before returning, you MUST:
    1. send_message(project_key, sender_name="${agent_name}", 
       to=["Orchestrator"], subject="...", body_md="[FULL REPORT]")
    2. Return ONLY summary to main thread
  `,
  description: "${agent_name}: ${brief_description}"
)
```

### Flow Diagram

```
User Request
     │
     ▼
AGENTS.md (thin-router)
├─ First message? → fetch_inbox()
├─ Match intent → select agent
└─ Task(agent, prompt)
     │
     ▼
Sub-Agent (fresh context)
├─ Do work
├─ send_message() → Agent Mail
└─ Return summary
     │
     ▼
Main Thread
└─ Display summary (token efficient)
```

## Dependencies

| Dependency | Status |
|------------|--------|
| Agent Mail MCP | ✅ Available |
| Task tool | ✅ Available |
| Skills system | ✅ Available |
| Hooks | ❌ Not available (Amp) |

## Risks

| Risk | Mitigation |
|------|------------|
| Sub-agent forgets to save | Explicit instruction in prompt |
| Agent Mail down | Fallback to direct return |
| Too many agents | Start with core 10, expand later |
| Reference migration breaks | Update all refs in same PR |

## Out of Scope

- Lifecycle hooks (Amp limitation)
- Automatic context capture
- MCP execution layer (deferred)
- Compound learnings (deferred)
