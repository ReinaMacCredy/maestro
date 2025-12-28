# Grounding Trigger Implementation — Plan

## Phase 1: Core Transition Triggers

### Task 1.1: Add DISCOVER→DEFINE grounding block
- [ ] Add grounding execution section after Phase 1 in SKILL.md
- [ ] Include: finder query, confidence calculation, display block
- [ ] Enforcement: Advisory (warn + proceed)

### Task 1.2: Add DEFINE→DEVELOP grounding block
- [ ] Add grounding execution section after Phase 2 in SKILL.md
- [ ] Include: finder + Grep queries, confidence calculation
- [ ] Enforcement: Advisory (warn + proceed)

### Task 1.3: Add DEVELOP→DELIVER grounding block
- [ ] Add grounding execution section after Phase 3 in SKILL.md
- [ ] Include: Grep, finder, conditional web_search
- [ ] Enforcement: Gatekeeper (halt if skipped, allow with warning)
- [ ] Add halt block with [R]un / [S]kip options

### Task 1.4: Add DELIVER→Complete grounding block
- [ ] Add grounding execution section after Phase 4 in SKILL.md
- [ ] Include: full cascade + impact scan
- [ ] Enforcement: Mandatory (block on LOW, require justification)
- [ ] Add SKIP_GROUNDING override logic

**Phase 1 Verification:**
- Run `ds` command and verify grounding blocks appear at each transition
- Test skip behavior at each enforcement level

---

## Phase 2: State Tracking & Display

### Task 2.1: Add state tracking instructions
- [ ] Add grounding_state object documentation
- [ ] Add instructions to update state after each grounding
- [ ] Add display block template for state summary

### Task 2.2: Add state display at transitions
- [ ] Show cumulative state at each phase transition
- [ ] Use ✓/○ markers for completed/pending

**Phase 2 Verification:**
- Run full design session and verify state accumulates correctly
- Verify state displays at each transition

---

## Phase 3: Edge Case Handling

### Task 3.1: Add truncation handling
- [ ] Add instructions for 100+ match display
- [ ] Show "showing top 10" note

### Task 3.2: Add empty justification rejection
- [ ] Add validation for SKIP_GROUNDING input
- [ ] Show rejection block for empty/whitespace

### Task 3.3: Add conditional tool skipping
- [ ] Add logic to detect external refs in design
- [ ] Skip web_search if no external refs
- [ ] Display skipped tools with ⊘ marker

### Task 3.4: Add loop-back state reset
- [ ] Add instructions for "revisit [PHASE]" handling
- [ ] Reset that transition + all subsequent
- [ ] Display "(reset)" note in state block

### Task 3.5: Add network failure handling
- [ ] Add fallback behavior for web_search failure
- [ ] Degrade confidence to MEDIUM
- [ ] Display ✗ marker with error note

**Phase 3 Verification:**
- Test each edge case manually
- Verify UI blocks display correctly

---

## Phase 4: Documentation & Testing

### Task 4.1: Update grounding.md reference
- [ ] Add link to new SKILL.md sections
- [ ] Document trigger mechanism

### Task 4.2: Manual testing
- [ ] Run 3 design sessions (SPEED, FULL with skip, FULL complete)
- [ ] Verify all 14 acceptance criteria pass

**Phase 4 Verification:**
- All acceptance criteria verified
- No regressions in design session flow

---

## Summary

| Phase | Tasks | Estimated Time |
|-------|-------|----------------|
| Phase 1 | 4 tasks | 45 min |
| Phase 2 | 2 tasks | 20 min |
| Phase 3 | 5 tasks | 30 min |
| Phase 4 | 2 tasks | 25 min |
| **Total** | **13 tasks** | **~2 hours** |
