# Plan: Validation Gates

## Epic 1: Core Validation Infrastructure

### 1.1 Create validation folder structure
- [ ] Create `skills/conductor/references/validation/shared/` directory
- [ ] Create `skills/conductor/references/validation/lifecycle.md`

**Acceptance Criteria:**
- Directory structure matches design
- lifecycle.md contains gate registry table

### 1.2 Update LEDGER format
- [ ] Edit `skills/conductor/references/ledger/format.md`
- [ ] Add `validation:` section to frontmatter template
- [ ] Add `## Validation History` section to body template
- [ ] Document validation state fields

**Acceptance Criteria:**
- LEDGER format includes validation fields
- Documentation is clear

---

## Epic 2: Create Validation Gate Files

### 2.1 Create validate-design.md
- [ ] Create `skills/conductor/references/validation/shared/validate-design.md`
- [ ] Follow humanlayer format (Initial Setup, Process, Guidelines, Checklist)
- [ ] Include product.md, tech-stack.md, CODEMAPS checks
- [ ] Include LEDGER integration section

**Acceptance Criteria:**
- File follows humanlayer format
- All checks documented

### 2.2 Create validate-spec.md
- [ ] Create `skills/conductor/references/validation/shared/validate-spec.md`
- [ ] Follow humanlayer format
- [ ] Include design coverage checks
- [ ] Include LEDGER integration section

**Acceptance Criteria:**
- File follows humanlayer format
- Spec vs design checks documented

### 2.3 Create validate-plan-structure.md
- [ ] Create `skills/conductor/references/validation/shared/validate-plan-structure.md`
- [ ] Follow humanlayer format
- [ ] Include task structure checks (acceptance criteria, atomic tasks)
- [ ] Include "Automated Verification" section check

**Acceptance Criteria:**
- File follows humanlayer format
- Plan structure checks documented

### 2.4 Create validate-plan-execution.md
- [ ] Create `skills/conductor/references/validation/shared/validate-plan-execution.md`
- [ ] Use EXACT humanlayer format from source
- [ ] Include git diff, verification commands
- [ ] Include parallel research tasks

**Acceptance Criteria:**
- File matches humanlayer format exactly
- Implementation vs plan checks documented

### 2.5 Create validate-completion.md
- [ ] Create `skills/conductor/references/validation/shared/validate-completion.md`
- [ ] Follow humanlayer format
- [ ] Include beads status, git status, docs checks
- [ ] Include archive readiness check

**Acceptance Criteria:**
- File follows humanlayer format
- Completion checks documented

---

## Epic 3: Integration Points

### 3.1 Integrate with design/SKILL.md
- [ ] Edit `skills/design/SKILL.md`
- [ ] Add validation gate call after DELIVER phase
- [ ] Document SPEED vs FULL mode behavior

**Acceptance Criteria:**
- validate-design called after DELIVER
- Mode behavior documented

### 3.2 Integrate with tdd/cycle.md
- [ ] Edit `skills/conductor/references/tdd/cycle.md`
- [ ] Add validation gate call after REFACTOR phase
- [ ] Document retry logic

**Acceptance Criteria:**
- validate-plan-execution called after REFACTOR
- Retry logic documented

### 3.3 Integrate with conductor-newtrack workflow
- [ ] Edit `skills/conductor/references/workflows/newtrack.md`
- [ ] Add validate-spec after spec generation
- [ ] Add validate-plan-structure after plan generation

**Acceptance Criteria:**
- Both gates integrated into newtrack flow
- WARN behavior documented

### 3.4 Integrate with conductor-finish workflow
- [ ] Edit `skills/conductor/references/finish-workflow.md`
- [ ] Add validate-completion as Phase 0 (before preflight)

**Acceptance Criteria:**
- validate-completion runs before finish
- HALT behavior documented

---

## Epic 4: Documentation & Testing

### 4.1 Update CODEMAPS
- [ ] Update `conductor/CODEMAPS/overview.md` with validation section
- [ ] Document gate flow

**Acceptance Criteria:**
- CODEMAPS reflects new validation system

### 4.2 Update conductor/AGENTS.md
- [ ] Add validation-related learnings
- [ ] Add validation commands to Commands section

**Acceptance Criteria:**
- AGENTS.md has validation patterns

---

## Automated Verification

```bash
# Check all validation files exist
ls skills/conductor/references/validation/shared/*.md | wc -l
# Expected: 5

# Check lifecycle.md exists
test -f skills/conductor/references/validation/lifecycle.md && echo "OK"

# Check LEDGER format updated
grep -q "validation:" skills/conductor/references/ledger/format.md && echo "OK"

# Check design/SKILL.md integration
grep -q "validate-design" skills/design/SKILL.md && echo "OK"

# Check tdd/cycle.md integration
grep -q "validate-plan-execution" skills/conductor/references/tdd/cycle.md && echo "OK"
```

## Dependencies

- Epic 1 must complete before Epic 2
- Epic 2 must complete before Epic 3
- Epic 3 must complete before Epic 4
