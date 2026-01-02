# Validation Lifecycle

Gate routing and metadata.json integration for the 5 validation gates.

## Gate Registry

| Gate | Trigger | File | Enforcement |
|------|---------|------|-------------|
| 1. design | Each checkpoint (1-4) | `shared/validate-design.md` | WARN (progressive), HALT at CP4 |
| 2. spec | spec.md generated | `shared/validate-spec.md` | WARN |
| 3. plan-structure | plan.md generated | `shared/validate-plan-structure.md` | WARN |
| 4. plan-execution | TDD REFACTOR complete | `shared/validate-plan-execution.md` | HALT |
| 5. completion | before /conductor-finish | `shared/validate-completion.md` | HALT |

### Design Gate: Per-Checkpoint Validation

The design gate runs **progressively at each checkpoint**, not just at DELIVER:

| Checkpoint | Phase | Checks | Enforcement |
|------------|-------|--------|-------------|
| CP1 | DISCOVER | Product alignment, no duplicate features | WARN |
| CP2 | DEFINE | Problem clear, success measurable, scope explicit | WARN |
| CP3 | DEVELOP | 3+ options, tech-stack alignment, risk analysis | WARN |
| CP4 | DELIVER | Full validation + grounding + impact scan | HALT |

## metadata.json State

Track validation progress in `metadata.json.validation`:

```json
{
  "validation": {
    "gates_passed": ["design", "spec"],
    "current_gate": "plan-structure",
    "retries": 1,
    "last_failure": null
  }
}
```

### State Fields

| Field | Type | Description |
|-------|------|-------------|
| `gates_passed` | array | Gates that have passed for this track |
| `current_gate` | string \| null | Gate currently being validated |
| `retries` | number | Retry count for current gate (resets on pass) |
| `last_failure` | string \| null | Reason for last failure |

## Behavior Matrix

| Gate | On Fail | Behavior |
|------|---------|----------|
| design | HALT | Block, retry up to 2x, then escalate |
| spec | WARN | Log warning, continue workflow |
| plan-structure | WARN | Log warning, continue workflow |
| plan-execution | HALT | Block, retry up to 2x, then escalate |
| completion | HALT | Block, retry up to 2x, then escalate |

## Fallback Policy

| Condition | Action |
|-----------|--------|
| `bd` unavailable | HALT - Cannot proceed without beads CLI |
| Agent Mail unavailable | HALT - Cannot proceed without coordination |
| `conductor/` missing | DEGRADE - Standalone mode, limited features |

## Retry Logic

For HALT gates:

1. **First attempt fails**: Log to metadata.json, increment retries, return to previous phase
2. **Second attempt fails**: Log to metadata.json, increment retries, one more chance
3. **Third attempt fails**: Escalate with message:

```text
WARNING: Validation failed 2x. Human review needed.

Gate: [gate-name]
Failures:
- Attempt 1: [reason]
- Attempt 2: [reason]

Action required: Review and fix issues before continuing.
```

## Missing File Handling

When validation sources are missing:

| Missing File | Behavior |
|--------------|----------|
| product.md | WARN: "WARNING: product.md not found. Skipping product alignment check." |
| tech-stack.md | WARN: "WARNING: tech-stack.md not found. Skipping tech-stack compliance check." |
| CODEMAPS/ | WARN: "WARNING: CODEMAPS not found. Skipping pattern consistency check." |
| design.md | HALT: "ERROR: design.md required for spec validation." |
| spec.md | HALT: "ERROR: spec.md required for plan validation." |
| plan.md | HALT: "ERROR: plan.md required for execution validation." |

**Aggregate warnings**: Collect all missing file warnings and report them together at the end of validation.

## Skip Flags

Override validation behavior with flags:

| Flag | Effect |
|------|--------|
| `--no-validate` | Skip all gates |
| `--validate-warn` | All gates WARN only (force SPEED-like behavior) |

## Integration Points

### 1. Design Session (ds)

**At each checkpoint (progressive validation):**
```
→ CP1 (DISCOVER): Load validate-design.md (DISCOVER checks)
→ CP2 (DEFINE): Load validate-design.md (DEFINE checks)
→ CP3 (DEVELOP): Load validate-design.md (DEVELOP checks) + spawn research agents
→ CP4 (DELIVER): Load validate-design.md (FULL checks) + grounding + impact scan
→ Update metadata.json validation state at each checkpoint
→ WARN at CP1-3, HALT at CP4
```

**Research triggers:**
```
→ Session start: discover-hook (Locator + Pattern + CODEMAPS)
→ CP3 (DEVELOP): grounding-hook (Locator + Analyzer + Pattern)
→ CP4 (DELIVER): Full grounding + impact scan
```

### 2. Track Creation (/conductor-newtrack)

After spec.md generation:
```
→ Load shared/validate-spec.md
→ Run validation
→ Update metadata.json validation state
→ WARN
```

After plan.md generation:
```
→ Load shared/validate-plan-structure.md
→ Run validation
→ Update metadata.json validation state
→ WARN
```

### 3. TDD Cycle

After REFACTOR phase complete:
```
→ Load shared/validate-plan-execution.md
→ Run validation
→ Update metadata.json validation state
→ HALT on failure
```

### 4. Track Finish (/conductor-finish)

Before Phase 0 (preflight):
```
→ Load shared/validate-completion.md
→ Run validation
→ Update metadata.json validation state
→ HALT on failure
```

## Validation History

Log all validation events to metadata.json:

```markdown
## Validation History

| Gate | Status | Time | Notes |
|------|--------|------|-------|
| design | ✅ PASS | 10:15 | All checks passed |
| spec | ⚠️ WARN | 10:20 | Missing edge case coverage |
| plan-structure | ✅ PASS | 10:25 | - |
| plan-execution | ❌ FAIL | 11:00 | Test coverage 60%, required 80% |
| plan-execution | ✅ PASS | 11:30 | Retry successful |
```

## References

- [gate.md](../verification/gate.md) - Core verification principles
- [metadata.schema.json](../schemas/metadata.schema.json) - metadata.json schema
- [shared/](shared/) - Individual gate implementations
