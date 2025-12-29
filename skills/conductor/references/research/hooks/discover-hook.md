# Discover Hook — Auto-Research at Design Session Start

## Purpose

Automatically research codebase context BEFORE entering DISCOVER phase of design session.

## Trigger

When `ds` (design session) starts, BEFORE asking user any questions.

## Integration Point

```
ds triggered
    │
    ▼
┌─────────────────────────┐
│  DISCOVER HOOK          │  ◄── THIS HOOK
│  Auto-research context  │
└─────────────────────────┘
    │
    ▼
DISCOVER phase begins
(with research context)
```

## Execution Protocol

### Step 1: Extract Research Query

From user's initial message, extract:
- Topic/feature mentioned
- Related keywords
- Affected areas (if mentioned)

### Step 2: Spawn Parallel Agents

```
┌─────────────────────────────────────────────────────┐
│              PARALLEL DISPATCH                      │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │  Locator    │  │  Pattern    │  │  CODEMAPS   │ │
│  │  (topic)    │  │  (similar)  │  │  (context)  │ │
│  └─────────────┘  └─────────────┘  └─────────────┘ │
│                                                     │
└─────────────────────────────────────────────────────┘
```

| Agent | Query |
|-------|-------|
| Locator | Find files related to {topic} |
| Pattern | Find similar features already implemented |
| CODEMAPS | Load relevant module codemaps |

### Step 3: Synthesize & Display

```
┌─ RESEARCH CONTEXT ─────────────────────────┐
│ Topic: {user's topic}                      │
│ Duration: Xs                               │
├────────────────────────────────────────────┤
│ EXISTING RELATED CODE:                     │
│ • [path/file.ts] - Description             │
│ • [path/other.ts] - Description            │
├────────────────────────────────────────────┤
│ SIMILAR FEATURES:                          │
│ • [FeatureName] in [location]              │
├────────────────────────────────────────────┤
│ AFFECTED MODULES:                          │
│ • skills/ - Skill definitions              │
│ • conductor/ - Workflow orchestration      │
└────────────────────────────────────────────┘
```

### Step 4: Proceed to DISCOVER

With research context loaded, begin DISCOVER phase questions.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| enabled | true | Enable auto-research |
| timeout | 10s | Max research duration |
| max_agents | 3 | Parallel agent limit |
| skip_if_empty | false | Skip if no topic detected |

## Skip Conditions

**NONE. Research ALWAYS runs.**

Removed all previous skip conditions:
- ~~User says "quick" or "skip research"~~ → Research still runs
- ~~No topic extractable~~ → Use session topic or ask
- ~~SPEED mode selected~~ → Research still runs (faster agents)

**Rationale:** Parallel agents are fast. Context is always valuable.

## Output Storage

Research results stored in session memory for use throughout design:

```json
{
  "discover_research": {
    "timestamp": "2025-12-29T10:00:00Z",
    "topic": "authentication",
    "findings": [...],
    "related_files": [...],
    "similar_features": [...]
  }
}
```

## Example

**User says:** "ds I want to add research capability to the workflow"

**Hook executes:**
1. Extract: topic = "research capability", "workflow"
2. Spawn:
   - Locator: Find research-related files
   - Pattern: Find similar features (verification, existing patterns)
   - CODEMAPS: Load conductor/, design/ modules
3. Display research context
4. Begin DISCOVER: "I found existing research system and verification patterns. What specific aspect do you want to add?"

## Error Handling

| Error | Action |
|-------|--------|
| Timeout | Display partial, continue |
| No results | Skip display, continue |
| Agent failure | Log, continue with others |

## Related

- [protocol.md](../protocol.md) - Main research protocol
- [agents/](../agents/) - Agent definitions
- [../../design/SKILL.md](../../../design/SKILL.md) - Design skill integration
