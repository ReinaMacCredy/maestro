# Skill Hierarchy

## 6-Level Priority

| Level | Skill | Role | Decides |
|-------|-------|------|---------|
| 1 | maestro-core | Central orchestrator | Routing, fallback policy, hierarchy |
| 2 | conductor | Track orchestrator | Workflow state, track lifecycle, beads integration |
| 3 | orchestrator | Multi-agent parallel | Worker dispatch, Agent Mail coordination, cross-track deps |
| 4 | design | Design sessions | Double Diamond phases, Party Mode, research verification |
| 5 | beads | Issue tracking | Dependencies, multi-session persistence |
| 6 | specialized | Tools | worktrees, sharing, writing-skills |

### Conflict Resolution

When skills disagree, **higher level wins**:

1. maestro-core defines HALT/DEGRADE → all skills follow
2. conductor owns workflow state → orchestrator/design/beads defer
3. orchestrator owns parallel dispatch → workers execute independently
4. design owns session flow → beads handles issues only
5. specialized skills are leaf nodes → no conflicts

## HALT vs DEGRADE Policy

### Decision Matrix

| Condition | Blocks All? | Action | Standard Message |
|-----------|-------------|--------|------------------|
| `bd` CLI unavailable | Yes | **HALT** | ❌ Cannot proceed: bd CLI not found. Install beads_viewer. |
| `conductor/` missing | No | **DEGRADE** | ⚠️ Conductor unavailable. Standalone mode. |
| Village MCP unavailable | No | **DEGRADE** | ⚠️ Village unavailable. Using single-agent mode. |
| Agent Mail MCP unavailable | No | **DEGRADE** | ⚠️ Agent coordination unavailable. Falling back to sequential. |
| CODEMAPS missing | No | **DEGRADE** | ⚠️ No CODEMAPS found. Context limited. |
| `product.md` missing | No | **DEGRADE** | ⚠️ No product context. Run /conductor-setup. |
| Network unavailable | No | **DEGRADE** | ⚠️ Network unavailable. Web research skipped. |

### HALT Criteria

**HALT when dependency blocks ALL functionality:**

- `bd` CLI is required for beads operations - no fallback exists
- Corrupted JSON state files - cannot recover safely

**DEGRADE when feature is optional:**

- `conductor/` missing - can still use beads standalone
- Village MCP unavailable - fall back to single-agent mode
- CODEMAPS missing - proceed without architecture context

### Message Format Standards

**HALT messages:**
```
❌ Cannot proceed: [specific reason]. [Fix instruction].
```

**DEGRADE messages:**
```
⚠️ [Feature] unavailable. [Fallback behavior].
```

### Examples

```
❌ Cannot proceed: bd CLI not found. Install beads_viewer.
❌ Cannot proceed: metadata.json corrupted. Manual repair required.

⚠️ Conductor unavailable. Standalone mode.
⚠️ Village unavailable. Using single-agent mode.
⚠️ No CODEMAPS found. Context limited.
```

## Enforcement

### At Session Start

1. Check `bd` availability → HALT if missing
2. Detect mode (SA/MA) → DEGRADE if Village unavailable
3. Check `conductor/` → DEGRADE if missing
4. Load CODEMAPS → DEGRADE if missing

### During Execution

Skills check their specific dependencies:

| Skill | Checks | On Failure |
|-------|--------|------------|
| conductor | metadata.json, plan.md | HALT (corrupted) or DEGRADE (missing) |
| orchestrator | Agent Mail MCP, plan.md Track Assignments | DEGRADE (sequential mode) |
| design | conductor/, CODEMAPS | DEGRADE (standalone mode) |
| beads | bd CLI, .beads/ | HALT (no bd) or DEGRADE (empty .beads) |
| worktrees | git, .gitignore | HALT (no git) or DEGRADE (missing .gitignore) |

## Skill Loading Order

When multiple skills apply:

```
1. Always load maestro-core first
2. maestro-core triggers handoff load if workflow context exists (ds, /conductor-implement)
3. Load primary skill (conductor/design/beads)
4. Load specialized skills as needed
```

Primary skill is determined by user intent (see routing.md).

**Handoff load:** At step 2, if a Conductor workflow entry point is detected, prior session context is loaded via `/resume_handoff` before proceeding.
