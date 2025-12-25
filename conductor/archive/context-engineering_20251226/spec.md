# Spec: Context Engineering Integration

## Summary

Integrate context engineering patterns into Conductor workflow by extending existing systems with routing intelligence and session lifecycle management.

## Requirements

### FR-1: Design Routing

**FR-1.1**: Display COMPLEXITY_EXPLAINER after `ds` skill activation, before Double Diamond phases.

**FR-1.2**: Calculate weighted complexity score (max 18) based on:
- Multiple epics (+3)
- Cross-module changes (+2)
- New abstractions (+3)
- External dependencies (+2)
- Files > 5 (+1)
- Unclear scope (+2)
- Security/auth (+2)
- Data migration (+3)

**FR-1.3**: Route based on score:
- Score < 4 → SPEED MODE (1 phase, quick spec)
- Score 4-6 → ASK USER (default FULL after 2 prompts with no response)
- Score > 6 → FULL MODE (4 phases, A/P/C checkpoints)

**FR-1.4**: Allow escalation from SPEED to FULL mid-session via `[E]` marker.

### FR-2: Execution Routing

**FR-2.1**: Add Phase 2b to implement.md after track selection.

**FR-2.2**: Evaluate TIER 1 weighted score:
- Epics > 1 (+2)
- [PARALLEL] markers (+3)
- Domains > 2 (+2)
- Independent tasks > 5 (+1)
- PASS threshold: score >= 5

**FR-2.3**: If TIER 1 passes, evaluate TIER 2 compound conditions:
- (files > 15 AND tasks > 3) OR
- (est_tool_calls > 40) OR
- (est_time > 30min AND independent_ratio > 0.6)

**FR-2.4**: Route to:
- SINGLE_AGENT if TIER 1 fails
- PARALLEL_DISPATCH if TIER 1 and TIER 2 pass
- SINGLE_AGENT if TIER 1 passes but TIER 2 fails

**FR-2.5**: Store execution_mode in implement_state.json.

### FR-3: RECALL (Session Start)

**FR-3.1**: Add RECALL hook to preflight-beads.md.

**FR-3.2**: Load `.conductor/session-context.md` if exists.

**FR-3.3**: Display token budget with thresholds:
- Available, Prompt, Reserved, Usable
- <20% usable = WARN
- <10% usable = force compression

**FR-3.4**: Verify context contract (Intent, Track ID, Key decisions).

**FR-3.5**: Cold start: create skeleton session-context.md with version header.

### FR-4: Extended Progress Checkpointing

**FR-4.1**: Add degradation signals section to workflows/beads/workflow.md.

**FR-4.2**: Evaluate degradation signals after each task completion:
- `tool_repeat`: same tool on same target >= per-tool threshold
- `backtrack`: revisiting completed task
- `quality_drop`: test failures increase OR lint errors appear
- `contradiction`: output conflicts with Decisions anchor

**FR-4.3**: Define per-tool thresholds:
- file_write: 3
- bash_command: 3
- search: 5
- file_read: 10

**FR-4.4**: If 2+ degradation signals fire → trigger compression.

### FR-5: Extended Handoff (REMEMBER)

**FR-5.1**: Extend beads-session.md Handoff Protocol for SA mode.

**FR-5.2**: SA mode saves to `.conductor/session-context.md`.

**FR-5.3**: Use anchored format with [PRESERVE] markers:
- Intent [PRESERVE]
- Constraints & Ruled-Out [PRESERVE]
- Decisions Made (with Why)
- Files Modified
- Open Questions / TODOs
- Current State
- Next Steps

**FR-5.4**: Add version header: `<!-- session-context v1 -->`.

**FR-5.5**: Validate PRESERVE sections not empty before save.

### FR-6: Facades for Discoverability

**FR-6.1**: Create `workflows/conductor/checkpoint.md` facade pointing to Progress Checkpointing.

**FR-6.2**: Create `workflows/conductor/remember.md` facade pointing to Handoff Protocol.

### FR-7: Documentation Updates

**FR-7.1**: Update workflows/README.md with context-engineering links.

**FR-7.2**: Update workflows/agent-coordination/workflow.md with execution-routing pattern.

## Non-Functional Requirements

**NFR-1**: Changes must be additive - no breaking changes to existing flows.

**NFR-2**: LOE must not exceed 4 hours.

**NFR-3**: All new files must follow existing naming conventions.

**NFR-4**: Facades must be under 20 lines each.

## Out of Scope (v1.1)

- VALIDATE dispatch (dependency graph, cycle detection)
- Signal combinators and hysteresis
- Multi-agent ownership model
- Memory TTL and compaction rules
- Per-model token budget tracking

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| AC-1 | COMPLEXITY_EXPLAINER displays score breakdown | Manual test: run `ds` |
| AC-2 | Score < 4 routes to SPEED mode | Manual test: simple task |
| AC-3 | Score > 6 routes to FULL mode | Manual test: complex task |
| AC-4 | Phase 2b evaluates TIER 1/2 | Check implement_state.json |
| AC-5 | Degradation signals evaluated after task | Check checkpoint triggers |
| AC-6 | Session context saved at session end | Check session-context.md created |
| AC-7 | Cold start creates skeleton | Delete file, verify recreation |
| AC-8 | Facades exist and link correctly | Check files exist |

## Dependencies

- Existing `workflows/beads/workflow.md` Progress Checkpointing section
- Existing `workflows/conductor/beads-session.md` Handoff Protocol
- Existing `skills/design/SKILL.md` Double Diamond flow
- Existing `workflows/implement.md` phase structure
- Existing `workflows/agent-coordination/patterns/parallel-dispatch.md`

## Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Score 4-6 user unavailable | Medium | Low | Default FULL after 2 prompts |
| Corrupted session-context.md | Low | Medium | Validation step, recreate skeleton |
| Breaking existing checkpoint | Low | High | Additive changes only, test existing |
| Degradation false positives | Medium | Medium | Per-tool thresholds, tune later |
