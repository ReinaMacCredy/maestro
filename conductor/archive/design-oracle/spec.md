# Specification: Auto Oracle Design Review

## Overview

Implement automatic design review via Oracle at CP4 (DELIVER) checkpoint in design sessions. The Oracle performs a 6-dimension audit and appends findings to design.md. Works on Amp (built-in oracle tool) and Claude Code/Gemini/Codex (Task-based fallback).

## Functional Requirements

### FR-1: Auto-Trigger at CP4

- Oracle review triggers automatically when design session reaches CP4 (DELIVER phase)
- Trigger occurs before A/P/C checkpoint menu is displayed
- Applies to both FULL and SPEED modes (different enforcement behavior)

### FR-2: Platform Detection

- Detect if Amp's built-in `oracle` tool is available
- **Amp**: Use built-in `oracle(task=..., files=[...])` 
- **Claude Code / Gemini / Codex**: Use `Task` to spawn Oracle agent from `oracle.md` template

### FR-3: 6-Dimension Audit

Oracle analyzes design.md against 6 dimensions:

| # | Dimension | Checks |
|---|-----------|--------|
| 1 | Completeness | All requirements addressed? Missing features? |
| 2 | Feasibility | Can this be built with current tech-stack? |
| 3 | Risks | What could break? Security/perf/reliability? |
| 4 | Dependencies | What other files/systems affected? |
| 5 | Ordering | Correct implementation sequence? |
| 6 | Alignment | Traces to product.md? Testable acceptance criteria? |

### FR-4: Input Files

Oracle receives these inputs for analysis:

| File | Required | Purpose |
|------|----------|---------|
| design.md | Yes | Primary document to review |
| tech-stack.md | No | Feasibility validation |
| product.md | No | Alignment validation |
| CODEMAPS/ | No | Pattern consistency |

### FR-5: Output Format

Oracle appends `## Oracle Audit` section to design.md with:

- Timestamp and mode (FULL/SPEED)
- Verdict (APPROVED / NEEDS_REVISION)
- Summary table (6 dimensions with status)
- Critical issues (must fix before proceeding)
- Warnings (recommended to address)
- Questions for clarification

### FR-6: Idempotent Updates

- If `## Oracle Audit` section exists, overwrite it (not append)
- Preserve timestamp history in audit section
- Handle repeated CP4 visits gracefully

### FR-7: Critical vs Minor Classification

**CRITICAL (blocks proceeding in FULL mode):**
- Missing acceptance criteria
- Conflicts with tech-stack constraints
- Unaddressed security/privacy risk
- No traceability to product goals
- Technically infeasible

**MINOR (warning only):**
- Missing edge case coverage
- Vague but non-blocking requirements
- Missing non-critical documentation

### FR-8: Mode-Specific Behavior

| Mode | Oracle Runs | On Critical Gap |
|------|-------------|-----------------|
| FULL | Always | HALT - fix before proceeding |
| SPEED | Always | WARN - log but allow continue |

## Non-Functional Requirements

### NFR-1: Performance

- Oracle audit should complete within 30 seconds
- Large design.md files (>500 lines) should be summarized before analysis

### NFR-2: Graceful Degradation

- Missing optional files (tech-stack.md, product.md) → WARN and continue
- Platform detection failure → default to Task-based approach
- Oracle timeout → WARN and allow manual proceed

### NFR-3: Consistency

- Same audit dimensions applied regardless of platform
- Output format identical across Amp and non-Amp platforms

## Acceptance Criteria

- [ ] AC-1: Oracle agent template (`oracle.md`) created in `skills/orchestrator/agents/review/`
- [ ] AC-2: `double-diamond.md` updated to call Oracle at CP4
- [ ] AC-3: `apc-checkpoints.md` updated with [C] Continue behavior at DELIVER
- [ ] AC-4: `design/SKILL.md` Session Flow step 5 mentions Oracle
- [ ] AC-5: Platform detection works (Amp uses built-in, others use Task)
- [ ] AC-6: `## Oracle Audit` section appended/updated in design.md
- [ ] AC-7: Critical gaps HALT in FULL mode, WARN in SPEED mode
- [ ] AC-8: All 6 dimensions checked and reported
- [ ] AC-9: Graceful degradation when input files missing
- [ ] AC-10: `lifecycle.md` documents Oracle gate at CP4

## Out of Scope

- Manual trigger command (`/oracle-review`) - future enhancement
- Custom dimension configuration per project
- Multi-file design reviews (single design.md only)
- Integration with external review tools
