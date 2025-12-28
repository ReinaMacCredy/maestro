# Impact Scan Subagent Prompt

Template for the impact scan subagent that runs in parallel with full grounding at DELIVER→Complete.

## Purpose

Analyze the design to identify all files that will be affected by implementation.

## Constraints

- **Timeout:** 30 seconds
- **Max files:** 100
- **Max tokens:** 4000

---

## Subagent Prompt Template

```
You are an Impact Scan Agent analyzing a design to identify affected files.

## Input

**Design Summary:**
{{design_summary}}

**Grounding Results:**
{{grounding_results}}

**Project Context:**
- Tech stack: {{tech_stack}}
- Key directories: {{key_directories}}

## Your Task

Analyze the design and identify ALL files that will be:
1. Created (new files)
2. Modified (existing files)
3. Deleted (removed files)

For each file, determine:
- Change type: create | modify | delete
- Risk level: low | medium | high
- Dependencies: files that depend on this file
- Suggested implementation order

## Output Format

Return a structured analysis with a **Confidence** level (high | medium | low) indicating certainty in the impact assessment.

### Files Affected

| # | File Path | Change | Risk | Dependencies | Order |
|---|-----------|--------|------|--------------|-------|
| 1 | path/to/file.ts | create | low | none | 1 |
| 2 | path/to/existing.ts | modify | medium | file1.ts | 2 |

### Risk Summary

- **High risk files:** [list]
- **Total files:** [count]
- **Create/Modify/Delete:** [counts]

### Implementation Order

1. [First files to change - no dependencies]
2. [Second wave - depends on first]
3. [Final wave - depends on earlier changes]

### Warnings

- [Any potential issues or conflicts]
- [Files that may need careful review]

## Guidelines

1. Use Grep/finder to verify file existence
2. Check for import/export dependencies
3. Consider test files for each modified file
4. Flag files with many dependents as high risk
5. Order: create before modify, infrastructure before features
```

---

## Execution Protocol

```python
def run_impact_scan(
    design_summary: str,
    grounding_results: GroundingResult,
    project_context: ProjectContext
) -> ImpactScanResult:
    """Run impact scan subagent."""
    
    prompt = IMPACT_SCAN_TEMPLATE.format(
        design_summary=design_summary,
        grounding_results=format_grounding(grounding_results),
        tech_stack=project_context.tech_stack,
        key_directories=project_context.directories
    )
    
    result = run_subagent(
        prompt=prompt,
        timeout=30,
        max_tokens=4000,
        tools=["Grep", "finder", "Read", "glob"]
    )
    
    return parse_impact_result(result)
```

---

## Result Schema

```python
@dataclass
class ImpactScanResult:
    files: list[AffectedFile]
    total_count: int
    create_count: int
    modify_count: int
    delete_count: int
    high_risk_files: list[str]
    implementation_order: list[list[str]]
    warnings: list[str]
    confidence: str  # high | medium | low
    has_high_risk_files: bool
```

```python
@dataclass
class AffectedFile:
    path: str
    change_type: str  # create | modify | delete
    risk: str  # low | medium | high
    dependencies: list[str]
    order: int
    notes: str | None
```

---

## Integration with Grounding

Impact scan runs in **parallel** with full grounding:

```python
async def full_grounding_with_impact(
    design: Design,
    context: GroundingContext
) -> DeliverPhaseResult:
    """Execute full grounding first, then impact scan with grounding results."""
    
    # First: run full grounding
    grounding_result = await execute_full_grounding(design, context)
    
    # Then: run impact scan with grounding results
    impact_result = await run_impact_scan(
        design.summary,
        grounding_result,
        context.project_context
    )
    
    # Merge results
    return merge_grounding_and_impact(grounding_result, impact_result)
```

---

## Merge Protocol

```python
def confidence_from_rank(rank: int) -> str:
    """Convert confidence rank back to string enum."""
    rank_to_confidence = {
        confidence_rank("low"): "low",
        confidence_rank("medium"): "medium",
        confidence_rank("high"): "high",
    }
    return rank_to_confidence.get(rank, "low")

def merge_grounding_and_impact(
    grounding: GroundingResult,
    impact: ImpactScanResult
) -> DeliverPhaseResult:
    """Merge grounding and impact scan results."""
    
    combined_rank = min(
        confidence_rank(grounding.overall_confidence),
        confidence_rank(impact.confidence)
    )
    
    return DeliverPhaseResult(
        grounding=grounding,
        impact=impact,
        combined_confidence=confidence_from_rank(combined_rank),
        blocking=(
            grounding.blocking or 
            impact.has_high_risk_files
        ),
        notes=merge_notes(grounding.notes, impact.warnings)
    )
```

---

## Output Location

Impact scan results are saved to:

```
conductor/tracks/{track-id}/grounding/impact-scan.md
```

---

## Example Output

```markdown
# Impact Scan Results

**Track:** grounding-system-redesign_20251228
**Timestamp:** 2025-12-28T11:00:00Z
**Confidence:** HIGH

## Files Affected (11 total)

### Create (6 files)

| File | Risk | Order |
|------|------|-------|
| skills/design/references/grounding/tiers.md | low | 1 |
| skills/design/references/grounding/router.md | low | 1 |
| skills/design/references/grounding/cache.md | low | 1 |
| skills/design/references/grounding/sanitization.md | low | 1 |
| skills/design/references/grounding/schema.json | low | 1 |
| skills/design/references/grounding/impact-scan-prompt.md | low | 2 |

### Modify (5 files)

| File | Risk | Dependencies | Order |
|------|------|--------------|-------|
| skills/design/references/grounding.md | medium | tiers.md, router.md | 2 |
| skills/design/SKILL.md | high | grounding.md | 3 |
| skills/conductor/references/commands/design.toml | medium | SKILL.md | 3 |
| skills/conductor/SKILL.md | low | design.toml | 4 |
| skills/design/references/conductor-design-workflow.md | low | all above | 5 |

## Risk Summary

- **High risk:** skills/design/SKILL.md (many dependents)
- **Total:** 11 files
- **Create/Modify/Delete:** 6/5/0

## Implementation Order

1. Create all grounding/ reference files (parallel)
2. Rewrite grounding.md + create impact-scan-prompt.md
3. Update SKILL.md + design.toml
4. Update conductor/SKILL.md
5. Update conductor-design-workflow.md

## Warnings

- SKILL.md has 15+ imports - test after modification
- design.toml changes affect all design sessions
```

---

## Timeout Handling

If impact scan times out:

```python
def handle_impact_timeout(partial_result: PartialImpactResult) -> ImpactScanResult:
    """Handle partial results on timeout."""
    
    return ImpactScanResult(
        files=partial_result.files,
        confidence="low",
        has_high_risk_files=True,  # Assume risk on incomplete scan
        warnings=[
            "Impact scan timed out - results may be incomplete",
            "Manual review recommended before implementation"
        ]
    )
```

---

## Related

- [grounding.md](../grounding.md) - Main grounding documentation
- [tiers.md](tiers.md) - Tier definitions (Full tier uses impact scan)
- [router.md](router.md) - Source routing
