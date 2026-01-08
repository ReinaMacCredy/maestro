---
name: Gate 1 Design Validation
description: Gate 1 - Progressive validation at each checkpoint (CP1-4); validates design against product.md, tech-stack.md, and CODEMAPS
---

# Validate Design (Gate 1) - Progressive

Validates design decisions progressively at each checkpoint. **Run subset of checks based on current checkpoint.**

## Checkpoint-Specific Validation

| Checkpoint | Checks to Run | Enforcement |
|------------|---------------|-------------|
| CP1 (DISCOVER) | Product alignment only | WARN |
| CP2 (DEFINE) | Problem clarity, success criteria | WARN |
| CP3 (DEVELOP) | Tech-stack compliance, pattern consistency | WARN |
| CP4 (VERIFY) | ALL checks (full validation) | SPEED=WARN, FULL=HALT |

### CP1: DISCOVER Validation

Run only **Product Alignment Check** (subset):
- [ ] Problem relates to product.md vision
- [ ] Not duplicating existing feature
- [ ] Users mentioned align with product personas

### CP2: DEFINE Validation

Run only **Problem Clarity Check**:
- [ ] Problem statement is clear and specific
- [ ] Success criteria are measurable and testable
- [ ] Scope is explicit (in/out defined)
- [ ] Approach selection has rationale

### CP3: DEVELOP Validation

Run **Tech-Stack + Pattern Checks**:
- [ ] 3+ options presented with trade-offs
- [ ] Recommended approach aligns with tech-stack.md
- [ ] Risk/effort analysis complete
- [ ] Architecture fits existing patterns

### CP4: VERIFY Validation (Full Gate)

Run **ALL checks** from sections below, plus research verification.

---

## Full Validation Process (CP4 Only)

```text
BEFORE validation begins:

1. LOCATE: Find conductor/tracks/<track-id>/design.md
2. LOAD: Read product.md, tech-stack.md from conductor/
3. DISCOVER: Identify relevant CODEMAPS in conductor/CODEMAPS/
4. PREPARE: Create validation workspace in memory
```

## Validation Process

### Step 1: Context Discovery

Run parallel research tasks to gather validation context:

```text
PARALLEL RESEARCH TASKS:

Task A - Product Alignment:
  - Read product.md for goals, principles, constraints
  - Extract success metrics and user outcomes
  - Identify product boundaries and non-goals

Task B - Tech-Stack Compliance:
  - Read tech-stack.md for approved technologies
  - Note version constraints and compatibility requirements
  - Identify integration patterns and dependencies

Task C - Pattern Consistency:
  - Scan relevant CODEMAPS for existing patterns
  - Identify naming conventions and structure standards
  - Note component relationships and boundaries
```

### Step 2: Systematic Validation

Apply validation checks against design.md:

```text
VALIDATION CHECKS:

1. Product Alignment Check:
   □ Design goals map to product objectives
   □ User outcomes align with product vision
   □ No scope beyond product boundaries
   □ Success criteria match product metrics

2. Tech-Stack Compliance Check:
   □ All technologies are approved in tech-stack.md
   □ Version requirements satisfied
   □ Integration patterns follow established approaches
   □ No unapproved external dependencies

3. Pattern Consistency Check:
   □ Naming follows CODEMAPS conventions
   □ Component structure matches existing patterns
   □ API contracts consistent with neighbors
   □ File organization follows project standards
```

### Step 3: Generate Validation Report

```text
OUTPUT FORMAT:

## Design Validation Report

**Track:** <track-id>
**Date:** <timestamp>
**Status:** PASS | WARN | FAIL

### Product Alignment

| Design Element | Product Requirement | Status | Notes |
|----------------|---------------------|--------|-------|
| <goal-1> | <product-ref> | [PASS]/[WARN]/[FAIL] | <details> |
| <goal-2> | <product-ref> | [PASS]/[WARN]/[FAIL] | <details> |

### Tech-Stack Compliance

| Technology | Approved Version | Design Version | Status |
|------------|------------------|----------------|--------|
| <tech-1> | <approved> | <proposed> | [PASS]/[FAIL] |
| <tech-2> | <approved> | <proposed> | [PASS]/[FAIL] |

### Pattern Consistency

| Pattern Area | CODEMAPS Reference | Compliance | Notes |
|--------------|-------------------|------------|-------|
| Naming | <codemap-file> | [PASS]/[WARN]/[FAIL] | <details> |
| Structure | <codemap-file> | [PASS]/[WARN]/[FAIL] | <details> |
| APIs | <codemap-file> | [PASS]/[WARN]/[FAIL] | <details> |

### Issues Found

1. [SEVERITY] Issue description
   - Location: design.md line X
   - Violation: <what rule/pattern violated>
   - Suggested fix: <recommendation>

### Recommendations

- <actionable-recommendation-1>
- <actionable-recommendation-2>
```

## Important Guidelines

```text
MANDATORY RULES:

1. NEVER skip product.md check - design without product alignment is waste
2. NEVER approve unapproved technologies - tech-stack.md is authoritative
3. ALWAYS check CODEMAPS - consistency prevents refactoring debt
4. DOCUMENT gaps explicitly - silence is not approval
5. FAIL early - catching misalignment here saves implementation time
```

## Validation Checklist

Before marking Gate 1 complete:

```text
□ product.md read and goals extracted
□ tech-stack.md read and constraints noted
□ Relevant CODEMAPS identified and scanned
□ All design goals checked against product
□ All technologies verified against approved list
□ Pattern consistency verified against CODEMAPS
□ Validation report generated with status
□ All FAIL items documented with fixes
□ Recommendations actionable and specific
```

## metadata.json Integration

### Progressive Validation (CP1-3)

For checkpoints 1-3, only update current_gate but do NOT add to gates_passed:

```text
ON CP1-3 VALIDATION START:
  Update metadata.json.validation:
    "current_gate": "design-cpN"  (e.g., "design-cp1")

ON CP1-3 VALIDATION COMPLETE (PASS/WARN):
  Update metadata.json.validation:
    "current_gate": null
  Log: "CP[N] validation: [PASS|WARN]"
  Continue to next phase (do not add to gates_passed yet)
```

### Full Validation (CP4 Only)

Only CP4 updates gates_passed:

```text
ON CP4 VALIDATION START:
  Update metadata.json.validation:
    "current_gate": "design"

ON CP4 VALIDATION COMPLETE (PASS):
  Update metadata.json.validation:
    "gates_passed": [..., "design"]
    "current_gate": null
    "retries": 0

ON CP4 VALIDATION COMPLETE (FAIL):
  Update metadata.json.validation:
    "last_failure": "<failure reason>"
    "retries": <current + 1>
```

Example metadata.json.validation state:
```json
{
  "validation": {
    "gates_passed": ["design"],
    "current_gate": null,
    "retries": 0,
    "last_failure": null
  }
}
```

## Relationship to Other Commands

| Command | Relationship |
|---------|--------------|
| `/conductor-design` | Produces design.md that this gate validates |
| `/conductor-newtrack` | Runs this gate before creating spec.md |
| `validate-spec` | Next gate after this one passes |
| `/conductor-validate` | Orchestrates all validation gates |

## Failure Handling

```text
IF Gate 1 FAILS:

1. DO NOT proceed to spec generation
2. DOCUMENT all failures in validation report
3. RETURN to design phase with specific issues
4. REQUIRE design.md update before re-validation
5. RE-RUN full validation after changes

IF Gate 1 WARNS:

1. DOCUMENT warnings in validation report
2. REQUIRE explicit acknowledgment of risks
3. MAY proceed to spec with warnings carried forward
4. TRACK warnings through implementation
```

## Evidence Requirements

Following the Iron Law from gate.md:

```text
NO GATE PASSAGE WITHOUT EVIDENCE:

[PASS] "Product alignment verified" + table showing each mapping
[PASS] "Tech-stack compliant" + table showing each technology check
[PASS] "Patterns consistent" + CODEMAPS references checked

[FAIL] "Design looks good"
[FAIL] "Should be aligned"
[FAIL] "Probably consistent"
```
