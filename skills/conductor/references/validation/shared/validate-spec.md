---
name: Gate 2 Spec Validation
description: Gate 2 - Validates spec.md against design.md for requirement capture, completeness, and clarity
---

# Validate Spec (Gate 2)

Validates specification against design decisions for complete and accurate requirement capture.

## Initial Setup

```text
BEFORE validation begins:

1. VERIFY: Gate 1 (validate-design) has PASSED
2. LOCATE: Find conductor/tracks/<track-id>/spec.md
3. LOAD: Read design.md from same track directory
4. EXTRACT: Build requirement map from design decisions
5. PREPARE: Create validation workspace for comparison
```

## Validation Process

### Step 1: Requirement Capture Analysis

Map design decisions to spec requirements:

```text
REQUIREMENT MAPPING:

For EACH decision in design.md:
  1. Locate corresponding requirement in spec.md
  2. Verify requirement captures decision intent
  3. Check implementation guidance is actionable
  4. Note any gaps or mismatches

Build Traceability Matrix:
  design.md decision → spec.md requirement(s)
```

### Step 2: Completeness Check

Verify nothing was lost or added inappropriately:

```text
COMPLETENESS CHECKS:

1. Missing Items (Design → Spec):
   □ All design goals have spec requirements
   □ All constraints captured in spec
   □ All acceptance criteria defined
   □ All edge cases addressed
   □ Error handling specified

2. Scope Creep (Spec → Design):
   □ No requirements without design backing
   □ No features beyond design scope
   □ No implicit assumptions added
   □ No gold-plating introduced

3. Dependency Coverage:
   □ All design dependencies in spec
   □ Integration points specified
   □ External system requirements captured
```

### Step 3: Clarity Validation

Ensure spec is implementable without ambiguity:

```text
CLARITY CHECKS:

1. Requirement Quality:
   □ Each requirement is testable
   □ Each requirement is unambiguous
   □ Each requirement has single interpretation
   □ No conflicting requirements

2. Implementation Guidance:
   □ Technical approach specified where needed
   □ Constraints clearly stated
   □ Performance requirements quantified
   □ Security considerations explicit

3. Acceptance Criteria:
   □ Each requirement has pass/fail criteria
   □ Criteria are measurable
   □ Edge cases have defined behavior
```

## Generate Validation Report

```text
OUTPUT FORMAT:

## Spec Validation Report

**Track:** <track-id>
**Date:** <timestamp>
**Status:** PASS | WARN | FAIL
**Gate 1 Status:** <inherited-status>

### Design Coverage

| Design Decision | Spec Requirement(s) | Coverage | Notes |
|-----------------|---------------------|----------|-------|
| <decision-1> | <req-ids> | [PASS]/[WARN]/[FAIL] | <details> |
| <decision-2> | <req-ids> | [PASS]/[WARN]/[FAIL] | <details> |

### Scope Check

| Check Type | Count | Status | Details |
|------------|-------|--------|---------|
| Missing Items | <n> | [PASS]/[FAIL] | <list if any> |
| Scope Creep | <n> | [PASS]/[FAIL] | <list if any> |
| Ambiguous Reqs | <n> | [PASS]/[FAIL] | <list if any> |

### Clarity Assessment

| Requirement | Testable | Unambiguous | Has Criteria | Status |
|-------------|----------|-------------|--------------|--------|
| <req-1> | [PASS]/[FAIL] | [PASS]/[FAIL] | [PASS]/[FAIL] | <overall> |
| <req-2> | [PASS]/[FAIL] | [PASS]/[FAIL] | [PASS]/[FAIL] | <overall> |

### Issues Found

1. [SEVERITY] Issue description
   - Location: spec.md section/line
   - Type: MISSING | CREEP | AMBIGUOUS | CONFLICT
   - Related design: <design.md reference>
   - Suggested fix: <recommendation>

### Recommendations

- <actionable-recommendation-1>
- <actionable-recommendation-2>
```

## Important Guidelines

```text
MANDATORY RULES:

1. NEVER validate spec without passing Gate 1
2. ALWAYS trace requirements back to design
3. NEVER allow scope creep - spec must match design
4. DOCUMENT all ambiguities - they become bugs
5. REQUIRE testable criteria - untestable = unverifiable
6. FAIL on conflicts - contradictions cause implementation chaos
```

## Validation Checklist

Before marking Gate 2 complete:

```text
□ Gate 1 (validate-design) status verified as PASS
□ design.md read and decisions extracted
□ spec.md read and requirements mapped
□ Traceability matrix complete
□ Missing items check performed
□ Scope creep check performed
□ Clarity assessment complete for all requirements
□ Validation report generated with status
□ All FAIL items documented with fixes
□ Recommendations actionable and specific
```

## metadata.json Integration

Update the track's `metadata.json` file:

```text
ON VALIDATION START:
  Update metadata.json.validation:
    "current_gate": "spec"

ON VALIDATION COMPLETE (PASS):
  Update metadata.json.validation:
    "gates_passed": [..., "spec"]
    "current_gate": null
    "retries": 0

ON VALIDATION COMPLETE (FAIL):
  Update metadata.json.validation:
    "last_failure": "<failure reason>"
    "retries": <current + 1>
```

Example metadata.json.validation state:
```json
{
  "validation": {
    "gates_passed": ["design", "spec"],
    "current_gate": null,
    "retries": 0,
    "last_failure": null
  }
}
```

## Relationship to Other Commands

| Command | Relationship |
|---------|--------------|
| `validate-design` | Previous gate - must PASS before this runs |
| `/conductor-newtrack` | Runs this gate after spec generation |
| `validate-plan` | Next gate after this one passes |
| `/conductor-validate` | Orchestrates all validation gates |

## Failure Handling

```text
IF Gate 2 FAILS:

1. DO NOT proceed to plan generation
2. DOCUMENT all failures in validation report
3. CATEGORIZE failures:
   - MISSING: Return to spec, add requirements
   - CREEP: Remove unauthorized scope
   - AMBIGUOUS: Clarify with testable criteria
   - CONFLICT: Resolve contradictions
4. REQUIRE spec.md update before re-validation
5. RE-RUN full validation (both gates) after changes

IF Gate 2 WARNS:

1. DOCUMENT warnings in validation report
2. REQUIRE explicit acknowledgment
3. MAY proceed with warnings tracked
4. WARNINGS become implementation review items
```

## Evidence Requirements

Following the Iron Law from gate.md:

```text
NO GATE PASSAGE WITHOUT EVIDENCE:

[PASS] "All design decisions covered" + traceability matrix
[PASS] "No scope creep detected" + spec-to-design mapping
[PASS] "Requirements are clear" + testability assessment

[FAIL] "Spec looks complete"
[FAIL] "Should cover everything"
[FAIL] "Requirements seem clear"
```

## Traceability Matrix Format

```text
| Design Decision ID | Design Description | Spec Req IDs | Coverage Notes |
|--------------------|-------------------|--------------|----------------|
| D-001 | User authentication | R-001, R-002 | Full coverage |
| D-002 | Error handling | R-005 | Partial - missing edge cases |
| D-003 | Performance target | - | MISSING - no spec requirement |
```

This matrix provides evidence for the coverage claim and identifies gaps systematically.
