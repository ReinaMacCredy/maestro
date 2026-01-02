# Design: Auto Oracle Design Review

## Problem Statement

Design sessions produce design.md but lack automated validation before proceeding to implementation. Users manually invoke oracle reviews (as seen in T-019b7e70-dcc5-7580-ba6e-24c6c8c8632d), but this should be automatic at CP4 (DELIVER) to catch gaps early.

## Success Criteria

1. Oracle review runs automatically at CP4 checkpoint
2. Works on Amp (built-in oracle tool) AND Claude Code/Gemini/Codex (Task-based fallback)
3. 6-dimension audit appended to design.md as `## Oracle Audit` section
4. Critical gaps block proceeding; minor gaps allow continue with warnings
5. Idempotent - re-running CP4 overwrites (not appends) the audit section

## Scope

### In Scope

- Auto-trigger at CP4 (DELIVER phase)
- Platform detection (Amp vs others)
- 6-dimension audit framework
- Integration with existing validation lifecycle
- Oracle agent template for non-Amp platforms

### Out of Scope

- Manual trigger command (future enhancement)
- Custom dimension configuration
- Multi-file design reviews

## Solution Design

### Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    CP4 (DELIVER)                         │
│                         │                                │
│              Detect Platform                             │
│                         │                                │
│         ┌───────────────┴───────────────┐               │
│         │                               │               │
│        Amp                    Claude Code / Others       │
│         │                               │               │
│         ▼                               ▼               │
│   oracle(task=...)              Task(Oracle agent)       │
│   (GPT-5 reasoning)             (Claude/Gemini model)    │
│         │                               │               │
│         └───────────────┬───────────────┘               │
│                         │                                │
│                  6-Dimension Audit                       │
│                         │                                │
│         ┌───────────────┴───────────────┐               │
│         │                               │               │
│    Critical Gaps?                  Minor/None            │
│         │                               │               │
│         ▼                               ▼               │
│   HALT + Fix First              Continue (A/P/C)        │
└──────────────────────────────────────────────────────────┘
```

### 6-Dimension Audit Framework

| # | Dimension | Checks | Severity |
|---|-----------|--------|----------|
| 1 | **Completeness** | All requirements addressed? Missing features? | Critical if major gaps |
| 2 | **Feasibility** | Can this be built with current tech-stack? | Critical if impossible |
| 3 | **Risks** | What could break? Security/perf/reliability? | Critical if unaddressed security |
| 4 | **Dependencies** | What other files/systems affected? | Minor |
| 5 | **Ordering** | Correct implementation sequence? | Minor |
| 6 | **Alignment** | Traces to product.md? Testable acceptance criteria? | Critical if no traceability |

### Input Files

| File | Required | Purpose |
|------|----------|---------|
| design.md | Yes | Primary document to review |
| tech-stack.md | No | Feasibility validation |
| product.md | No | Alignment validation |
| CODEMAPS/ | No | Pattern consistency |
| spec.md | No | Traceability (if exists) |

### Output Format

Appended to design.md:

```markdown
## Oracle Audit

**Timestamp:** 2025-01-02T10:30:00
**Mode:** FULL
**Verdict:** NEEDS_REVISION | APPROVED

### Summary

| Dimension | Status | Finding |
|-----------|--------|---------|
| Completeness | ⚠️ WARN | Missing edge case for X |
| Feasibility | ✅ OK | Aligns with tech-stack |
| Risks | ❌ CRITICAL | Unaddressed security concern |
| Dependencies | ✅ OK | 3 files affected |
| Ordering | ✅ OK | Correct sequence |
| Alignment | ✅ OK | Traces to product goals |

### Critical Issues (must fix before proceeding)

1. **[Risks]** Security: No input validation specified for user data

### Warnings (recommended to address)

1. **[Completeness]** Edge case: What happens when X is empty?

### Questions for Clarification

1. Should Y persist across sessions?
```

### Platform Detection

```
At CP4 checkpoint:

IF oracle tool is available (Amp):
  → oracle(
      task="6-dimension design audit...",
      context="Maestro workflow design review at CP4",
      files=[design.md, tech-stack.md, product.md, CODEMAPS/]
    )
ELSE (Claude Code / Gemini / Codex):
  → Task(
      description="Oracle Design Review",
      prompt="[Load oracle.md agent template with inputs]"
    )
```

### Critical vs Minor Classification

**CRITICAL (blocks proceeding):**
- Missing acceptance criteria
- Conflicts with tech-stack constraints
- Unaddressed security/privacy risk
- No traceability to product goals
- Technically infeasible

**MINOR (warning only):**
- Missing edge case coverage
- Vague but non-blocking requirements
- Missing non-critical documentation
- Suboptimal ordering

### Mode Behavior

| Mode | Oracle Runs | On Critical Gap |
|------|-------------|-----------------|
| FULL | Always | HALT - fix before proceeding |
| SPEED | Always | WARN - log but allow continue |

### Idempotency

When `## Oracle Audit` section exists:
1. Find section start marker
2. Find next `##` heading or EOF
3. Replace entire section with new audit
4. Preserve timestamp history (optional)

## Files to Change

### Priority 1 (Core Implementation)

| File | Change |
|------|--------|
| `skills/orchestrator/agents/review/oracle.md` | **NEW** - Agent template for non-Amp |
| `skills/design/references/double-diamond.md` | Add Oracle call at CP4 (lines 74-86) |
| `skills/design/references/apc-checkpoints.md` | Update [C] behavior at DELIVER |
| `skills/design/SKILL.md` | Add Oracle to Session Flow step 5 |

### Priority 2 (Validation Integration)

| File | Change |
|------|--------|
| `skills/conductor/references/validation/lifecycle.md` | Document Oracle gate at CP4 |

### Priority 3 (Documentation)

| File | Change |
|------|--------|
| `conductor/CODEMAPS/skills.md` | Update agent directory |

## Implementation Sequence

```
Phase 1: Agent Template
├─ 1.1 Create oracle.md agent with 6-dimension prompt

Phase 2: CP4 Integration
├─ 2.1 Update double-diamond.md CP4 section
├─ 2.2 Update apc-checkpoints.md [C] behavior
└─ 2.3 Update design/SKILL.md Session Flow

Phase 3: Validation Integration
└─ 3.1 Update lifecycle.md with Oracle gate

Phase 4: Documentation
└─ 4.1 Update CODEMAPS/skills.md
```

## Risks

| Risk | Mitigation |
|------|------------|
| Token limits with large inputs | Filter CODEMAPS to relevant sections |
| False positives causing friction | Clear critical vs minor criteria |
| Platform detection fails | Default to Task-based approach |
| Duplicate audit sections | Idempotent overwrite logic |
| SPEED mode accidentally blocks | Oracle always WARN in SPEED mode |

## Acceptance Criteria

- [ ] Oracle agent template created and documented
- [ ] CP4 automatically triggers Oracle review in FULL mode
- [ ] Platform detection works (Amp uses built-in, others use Task)
- [ ] `## Oracle Audit` section appended/updated in design.md
- [ ] Critical gaps HALT in FULL mode, WARN in SPEED mode
- [ ] All 6 dimensions checked
- [ ] Graceful degradation when input files missing
