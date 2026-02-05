# Agent Enhancement Plan

## Objective

Enhance all 6 Maestro agents with shared context, intelligent delegation, and enforced verification to reduce duplicate work, improve accuracy, and increase efficiency.

## Scope

**In**:
- Handoff directory structure for context sharing
- Orchestrator delegation heuristics
- Verification enforcement after each task
- All 6 agent definition updates

**Out**:
- New agent types
- Wisdom accumulation system
- Automated test scenarios

---

## Tasks

### Phase 1: Handoff Directory Structure

- [ ] **Task 1.1**: Create `.maestro/handoff/` directory structure
  - Add `research.md`, `decisions.md`, `progress.md` templates
  - Add `.gitkeep` or initial content

- [ ] **Task 1.2**: Update `explore.md` agent to write research findings to `.maestro/handoff/research.md`
  - Append findings with timestamp and context
  - Include file paths, patterns found, key observations

- [ ] **Task 1.3**: Update `oracle.md` agent to write decisions to `.maestro/handoff/decisions.md`
  - Record architectural recommendations
  - Include tradeoffs considered and rationale

- [ ] **Task 1.4**: Update worker agents (`kraken.md`, `spark.md`) to:
  - Read handoff files before starting work
  - Write completed work summary to `.maestro/handoff/progress.md`

### Phase 2: Delegation Heuristics

- [ ] **Task 2.1**: Update `orchestrator.md` with delegation decision table
  - Research tasks → explore
  - Architecture decisions → oracle
  - New features (multi-file, needs tests) → kraken
  - Simple fixes (single-file, config) → spark

- [ ] **Task 2.2**: Add delegation reasoning requirement
  - Orchestrator must state WHY it chose each agent
  - Include in task description when spawning

### Phase 3: Verification Enforcement

- [ ] **Task 3.1**: Update `orchestrator.md` verification section
  - MUST read all files claimed modified by agents
  - MUST run build/lint commands after implementation tasks
  - MUST run relevant tests after kraken tasks
  - Only mark task complete after verification passes

- [ ] **Task 3.2**: Update `scripts/verification-injector.sh`
  - Make reminder more prescriptive with specific steps
  - Include example verification commands

### Phase 4: Team Lead Updates

- [ ] **Task 4.1**: Update `prometheus.md` to use handoff directory
  - Write interview findings to handoff during design phase
  - Pass handoff context to spawned agents

- [ ] **Task 4.2**: Add handoff cleanup step to both team leads
  - Clear handoff directory at start of new workflow
  - Archive useful patterns to wisdom directory (future)

---

## Verification

1. Run `/design` with a sample request — verify explore/oracle write to handoff
2. Run `/work` with a multi-task plan — verify:
   - Orchestrator uses delegation table correctly
   - Agents read handoff context
   - Orchestrator verifies each task before marking complete
3. Intentionally create a broken agent output — verify orchestrator catches it

---

## Notes

### Handoff File Formats

**research.md**:
```markdown
## [Timestamp] - [Query]
- Files found: ...
- Patterns: ...
- Observations: ...
```

**decisions.md**:
```markdown
## [Timestamp] - [Decision Topic]
**Recommendation**: ...
**Alternatives considered**: ...
**Rationale**: ...
```

**progress.md**:
```markdown
## [Timestamp] - [Task ID] - [Agent]
**Files modified**: ...
**Changes**: ...
**Tests**: ...
```

### Delegation Table

| Task Pattern | Agent | Reason |
|--------------|-------|--------|
| Find/search/locate | explore | Read-only search specialist |
| Design/tradeoff/architecture | oracle | Deep reasoning with opus |
| New feature, multi-file, add tests | kraken | TDD specialist |
| Single-file fix, config change | spark | Quick fix specialist |
