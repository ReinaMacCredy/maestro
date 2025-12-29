# Newtrack Hook — Pre-Spec Research

## Purpose

Research affected areas BEFORE generating spec.md and plan.md for more accurate specifications.

## Trigger

When `/conductor-newtrack` runs, BEFORE spec generation phase.

## Integration Point

```
/conductor-newtrack
    │
    ▼
Load design.md
    │
    ▼
┌─────────────────────────┐
│  NEWTRACK HOOK          │  ◄── THIS HOOK
│  Pre-spec research      │
└─────────────────────────┘
    │
    ▼
Generate spec.md (with research context)
    │
    ▼
Generate plan.md
```

## Execution Protocol

### Step 1: Extract Research Targets from Design

From `design.md`:
- Components to be created/modified
- Integration points mentioned
- Dependencies identified
- File paths referenced

### Step 2: Spawn Parallel Agents

```
┌─────────────────────────────────────────────────────┐
│              PARALLEL DISPATCH                      │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │  Locator    │  │  Analyzer   │  │  Pattern    │ │
│  │  (files)    │  │  (deps)     │  │  (similar)  │ │
│  └─────────────┘  └─────────────┘  └─────────────┘ │
│                                                     │
│  ┌─────────────┐  ┌─────────────┐                  │
│  │  Impact     │  │  Test       │                  │
│  │  (scope)    │  │  (patterns) │                  │
│  └─────────────┘  └─────────────┘                  │
│                                                     │
└─────────────────────────────────────────────────────┘
```

| Agent | Task |
|-------|------|
| Locator | Find all files that will be affected |
| Analyzer | Understand current implementation |
| Pattern | Find similar implementations to follow |
| Impact | Estimate scope (files, modules) |
| Test | Find test patterns to follow |

### Step 3: Synthesize for Spec Generation

```
┌─ PRE-SPEC RESEARCH ────────────────────────┐
│ Track: {track_id}                          │
│ Duration: Xs                               │
├────────────────────────────────────────────┤
│ AFFECTED FILES:                            │
│ • [path/file.ts] - Will be modified        │
│ • [path/new.ts] - Will be created          │
├────────────────────────────────────────────┤
│ DEPENDENCIES:                              │
│ • Uses: [module1, module2]                 │
│ • Used by: [consumer1, consumer2]          │
├────────────────────────────────────────────┤
│ PATTERNS TO FOLLOW:                        │
│ • Error handling: AppError pattern         │
│ • Testing: Jest + mock pattern             │
├────────────────────────────────────────────┤
│ IMPACT ESTIMATE:                           │
│ • Files: 8                                 │
│ • Modules: 3                               │
│ • Risk: MEDIUM                             │
└────────────────────────────────────────────┘
```

### Step 4: Inject into Spec Generation

Research context is passed to spec generation:
- Accurate file lists
- Known dependencies
- Existing patterns to follow
- Test approach

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| enabled | true | Enable pre-spec research |
| timeout | 20s | Max research duration |
| max_agents | 5 | Parallel agent limit |
| include_tests | true | Research test patterns |

## Skip Conditions

**NONE. Research ALWAYS runs.**

Removed all previous skip conditions:
- ~~`--skip-research` flag~~ → Deprecated, ignored
- ~~Track is a hotfix~~ → Research still runs (smaller scope)

**Rationale:** Pre-spec research ensures accurate specifications.

## Output Storage

Research stored in track metadata:

```json
{
  "research": {
    "timestamp": "2025-12-29T10:00:00Z",
    "affected_files": [...],
    "dependencies": {...},
    "patterns": [...],
    "impact": {
      "files": 8,
      "modules": 3,
      "risk": "MEDIUM"
    }
  }
}
```

## Benefits

| Without Research | With Research |
|------------------|---------------|
| Spec may miss files | Comprehensive file list |
| Unknown dependencies | Dependencies mapped |
| Guess test approach | Follow existing patterns |
| Vague impact | Quantified scope |

## Example

**design.md mentions:** "Add research capability to conductor"

**Hook executes:**
1. Locator: Find conductor-related files
2. Analyzer: Understand current conductor structure
3. Pattern: Find similar features (verification, existing patterns)
4. Impact: 8 files, 3 modules
5. Test: Find test patterns in conductor/

**Spec generated with:**
- Accurate list of files to modify
- Known integration points
- Test patterns to follow
- Realistic task breakdown

## Error Handling

| Error | Action |
|-------|--------|
| Timeout | Partial results, continue |
| No design.md | Skip research |
| Agent failure | Continue with others |

## Related

- [protocol.md](../protocol.md) - Main research protocol
- [agents/](../agents/) - Agent definitions
- [../../workflows.md](../../workflows.md) - Newtrack workflow
