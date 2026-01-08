# Session Initialization

When starting a design session:

## 0. Load Prior Session Context

Check for prior session context:

1. Run `/resume_handoff` command
2. If prior handoffs exist in `conductor/handoffs/`:
   - Display prior context summary
   - Show: `📋 Prior context: <goal summary>`
3. If missing: Start fresh session

**Non-blocking:** Continue normally if no prior context exists.

## 1. Load CODEMAPS for Context

Check for `conductor/CODEMAPS/` directory:

**If exists:**
1. Load `overview.md` (always)
2. Load relevant module codemaps based on topic (skills.md, api.md, etc.)
3. Display: `📚 Loaded CODEMAPS for context`

**If missing:**
1. Display: `⚠️ No CODEMAPS found. Run /conductor-setup to generate initial CODEMAPS.`
2. Continue session normally (CODEMAPS are optional but recommended)

## 2. Verify Conductor Setup

Check for `conductor/` directory with core files:

- `product.md` - Product vision
- `tech-stack.md` - Technical constraints
- `workflow.md` - Development standards

If missing: Display `Conductor unavailable. Standalone mode. Run /conductor-setup to enable full features.` and continue session.

> **Note:** In standalone mode, CODEMAPS and product context are skipped. Double Diamond still works but without project-specific context.

## 3. Auto-Research Context

**BEFORE asking user any questions**, run parallel research to ground context.

### Discover Hook Protocol

1. Extract topic from user's initial message
2. Spawn parallel agents:
   - **Locator**: Find related files
   - **Pattern**: Find similar features
   - **CODEMAPS**: Load relevant modules
3. Display research context:

```
┌─ RESEARCH CONTEXT ─────────────────────────┐
│ Topic: {extracted topic}                   │
│ Duration: Xs                               │
├────────────────────────────────────────────┤
│ EXISTING RELATED CODE:                     │
│ • [path/file.ts] - Description             │
├────────────────────────────────────────────┤
│ SIMILAR FEATURES:                          │
│ • [FeatureName] in [location]              │
└────────────────────────────────────────────┘
```

4. Proceed to DISCOVER with research context

**⚠️ Research ALWAYS runs. No skip conditions.**

Parallel agents are fast and context is always valuable.

**Timeout:** 10s max, partial results OK

## 4. Complexity Scoring (Design Routing)

After loading context, evaluate task complexity to determine routing.

See [design-routing-heuristics.md](design-routing-heuristics.md) for full scoring details.

### Scoring Criteria (max 18 points)

| Factor | Weight | Check |
|--------|--------|-------|
| Multiple epics | +3 | Work spans multiple epics |
| Cross-module | +2 | Changes touch multiple modules |
| New abstractions | +3 | Creating new patterns/interfaces |
| External deps | +2 | New external dependencies |
| Files > 5 | +1 | Touching more than 5 files |
| Unclear scope | +2 | Scope not well-defined |
| Security/auth | +2 | Involves security or authentication |
| Data migration | +3 | Database or data migration |

### Routing Decision

| Score | Route | Description |
|-------|-------|-------------|
| < 4 | SPEED MODE | 1-phase quick design, minimal ceremony |
| 4-6 | ASK USER | Soft zone: "[S]peed or [F]ull?" |
| > 6 | FULL MODE | 4-phase Double Diamond with A/P/C |

### SPEED Mode Flow

For simple tasks (score < 4):

1. **Quick Discovery** - 2-3 clarifying questions max
2. **Output** - Generate design.md directly
3. **Handoff** - "Design complete. Run `/conductor-newtrack` to continue."

No A/P/C checkpoints in SPEED mode (unless user escalates with `[E]`).

### FULL Mode Flow

For complex tasks (score > 6 or user-selected):

Proceed with full Double Diamond (4 phases, A/P/C checkpoints).

### Soft Zone Behavior (score 4-6)

- Prompt: "Score is X (soft zone). [S]peed or [F]ull?"
- After 2 prompts without response → default to FULL
- Track prompt count in session

### Escalation

User can type `[E]` during SPEED mode to escalate to FULL.
Escalation preserves current progress and enters DEFINE phase.
