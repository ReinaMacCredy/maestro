---
name: maestro:scrutiny-validator
description: "Run code scrutiny validation during mission checkpoints. Spawns review subagents, synthesizes results, and produces validation reports."
argument-hint: "[<milestone>|--all] [--strict]"
---

# Scrutiny Validator

Run code scrutiny validation during mission checkpoints. Spawns review subagents, synthesizes results, and produces validation reports.

## Arguments

`$ARGUMENTS`

- `<milestone>`: Validate a specific milestone (e.g., `bootstrap`, `core`)
- `--all`: Validate all milestones
- `--strict`: Fail on warnings as well as errors

---

## When to Use This Skill

Use during Mission Control validation checkpoints:
- After completing a milestone
- Before sealing a milestone
- During `maestro validation show` review
- When `maestro:agent-base` instructs final validation

---

## Required Skills

- `maestro:agent-base` - For startup/cleanup procedures

---

## Work Procedure

### Step 1: Load Validation State

Check current validation status:

```bash
maestro validation show
```

This displays:
- Active milestone
- Completed validations
- Pending validations
- Blockers

### Step 2: Determine Validation Scope

**If milestone specified:**
- Validate only features in that milestone

**If `--all` specified:**
- Validate all completed features across all milestones

**Default (no args):**
- Validate current active milestone

### Step 3: Identify Features to Validate

Query features needing validation:

```bash
maestro feature list --mission <id> --milestone <name>
```

Filter for features with status `implemented` or `pending_validation`.

### Step 4: Spawn Review Subagents

For each feature requiring validation:

1. **Prepare review context**:
   - Feature specification
   - Implementation changes (git diff)
   - Test results
   - Verification steps from feature definition

2. **Spawn scrutiny-feature-reviewer**:
   - Pass feature context
   - Pass validation contract assertions
   - Request structured review output

### Step 5: Synthesize Review Results

Collect and synthesize all subagent reports:

```typescript
interface ScrutinySynthesis {
  milestone: string;
  featuresReviewed: number;
  featuresPassed: number;
  featuresFailed: number;
  blockers: Array<{
    featureId: string;
    severity: "blocking" | "warning";
    issue: string;
  }>;
  recommendations: string[];
}
```

### Step 6: Update Validation State

Record results in validation state:

```bash
maestro validation update --milestone <name> --status <passed|failed>
```

**Next step**: return results to `maestro:conduct`. If status is `passed`, conduct proceeds to `maestro:user-testing-validator` (if applicable) or directly to `maestro milestone seal`. If status is `failed`, conduct re-opens the affected features for agent re-dispatch. Do not seal the milestone yourself -- that is the conductor's gate.

### Step 7: Produce Validation Report

Generate human-readable and JSON reports:

**Human format:**
```
=== Scrutiny Validation: <milestone> ===

Features Reviewed: 5
Passed: 4
Failed: 1

Blockers:
- feat-003: Missing error handling (blocking)

Recommendations:
1. Add input validation for edge cases
2. Increase test coverage for error paths

Status: FAILED (1 blocking issue)
```

**JSON format** (with `--json`):
```json
{
  "milestone": "core",
  "summary": {
    "featuresReviewed": 5,
    "featuresPassed": 4,
    "featuresFailed": 1
  },
  "blockers": [...],
  "recommendations": [...],
  "status": "failed"
}
```

---

## Review Dimensions

Each feature is scrutinized on:

| Dimension | Check |
|-----------|-------|
| **Correctness** | Implementation matches spec |
| **Edge Cases** | Error paths and boundaries handled |
| **Tests** | Adequate coverage exists |
| **Code Quality** | Follows project patterns |
| **Security** | No obvious vulnerabilities |
| **Performance** | No obvious inefficiencies |

---

## Validation States

```
proposed → approved → in_progress → implemented → validated
                                              ↓
                                        validation_failed → in_progress
```

States map to CLI commands:
- `maestro feature approve` → `approved`
- Agent completes → `implemented`
- `maestro:scrutiny-validator` passes → `validated`
- `maestro:scrutiny-validator` fails → `validation_failed`

---

## Related Commands

| Command | Purpose |
|---------|---------|
| `maestro validation show` | Show current validation state |
| `maestro validation update` | Update validation status |
| `maestro feature list` | List features to validate |
| `maestro milestone seal` | Seal milestone after validation |
| `maestro checkpoint save` | Save validation checkpoint |

---

## Integration with Milestones

Scrutiny validation runs automatically when:
1. All features in a milestone are `implemented`
2. Before `maestro milestone seal` can complete
3. During `maestro validation show` if pending

---

## Best Practices

1. **Validate incrementally** - Don't wait until the end
2. **Fix blockers immediately** - Don't seal with blockers
3. **Document warnings** - Even non-blocking issues should be tracked
4. **Use strict mode for critical missions** - `--strict` treats warnings as failures
5. **Save checkpoint after validation** - `maestro checkpoint save`
