# Skills Architecture Refactor - Design Document

**Track ID:** skills-arch-refactor  
**Created:** 2026-01-08  
**Status:** Design Complete  
**Complexity:** 7 (FULL mode)

---

## Problem Statement

The current Maestro workflow has grown organically, resulting in:
1. **Skill overlap / unclear responsibilities** - design vs conductor vs orchestrator
2. **Fragile agent coordination** - orchestrator worker protocol
3. **Too many workflow commands** - `/conductor-*` surface area
4. **Too many skills / hard to navigate** - 10+ skills with unclear boundaries

## Research Summary

### Industry Best Practices (Sources)
- AgentSquare paper: 4-module architecture (Planning, Reasoning, Tool Use, Memory)
- Agentic patterns: Orchestrator-Workers pattern
- Anthropic official docs: Progressive disclosure, gerund naming, SKILL.md ≤500 lines

### Current Overlap Issues Found
1. `design` vs `orchestrator` — Both spawn workers differently
2. `design` vs `conductor` — Track generation redundancy
3. `skill-creator` vs `writing-skills` — Conflicting guidelines
4. Oracle ownership scattered across skills
5. Research protocol ambiguity
6. Implementation auto-routing can override user choice

---

## Solution: Clear Ownership Model

### Architecture Decision

**Approach:** Keep separate skills with clear ownership boundaries (not mega-consolidation)

**Rationale:** 
- Anthropic best practices favor modularity + progressive disclosure
- Each skill should own specific artifacts
- Keep SKILL.md ≤500 lines

### Skill Naming

| Original | Final | Action |
|----------|-------|--------|
| design | **designing** | RENAME (gerund) |
| conductor | **conductor** | KEEP |
| orchestrator | **orchestrator** | KEEP |
| beads | **tracking** | RENAME |
| handoff | **handoff** | KEEP |
| skill-creator + writing-skills | **creating-skills** | MERGE |
| maestro-core | **maestro-core** | UPDATE |

### Ownership Matrix

| Concern | designing | conductor | orchestrator | tracking | handoff |
|---------|:---------:|:---------:|:------------:|:--------:|:-------:|
| Design phases 1-8 | ✅ | | | | |
| Oracle audits | ✅ | | | | |
| Research protocol | ✅ | | | | |
| Spec/Plan creation | ✅ | | | | |
| TDD implementation | | ✅ | | | |
| `/conductor-*` commands | | ✅ | | | |
| Validation gates | | ✅ | | | |
| Worker spawning | | | ✅ | | |
| Agent coordination | | | ✅ | | |
| File reservations | | | ✅ | | |
| Beads CLI (bd) | | | | ✅ | |
| Dependencies | | | | ✅ | |
| Persistent memory | | | | ✅ | |
| Session context | | | | | ✅ |
| Archive/finish | | | | | ✅ |

### Command Ownership

| Command | Old Owner | New Owner | Change |
|---------|-----------|-----------|--------|
| `ds` | design | designing | Trigger rename |
| `cn` | conductor | designing | MOVE (phases 5-8) |
| `ci` | conductor | conductor | KEEP |
| `co` | orchestrator | orchestrator | KEEP |
| `bd *` | beads | tracking | Trigger rename |
| `fb` | beads | tracking | Trigger rename |
| `rb` | beads | tracking | Trigger rename |
| `ho` | handoff | handoff | KEEP |
| `/conductor-setup` | conductor | conductor | KEEP |
| `/conductor-implement` | conductor | conductor | KEEP |
| `/conductor-newtrack` | conductor | designing | MOVE |
| `/conductor-design` | conductor | designing | MOVE (alias for ds) |
| `/conductor-orchestrate` | conductor | orchestrator | MOVE |
| `/conductor-status` | conductor | conductor | KEEP |
| `/conductor-revise` | conductor | conductor | KEEP |
| `/conductor-finish` | conductor | handoff | MOVE |
| `/conductor-handoff` | conductor | handoff | MOVE |

---

## Reference Structure

### Flattened Hierarchy (1-level deep)

```
designing/references/
├── pipeline.md              # phases 1-8 detail
├── oracle-audit.md          # oracle specifics
├── research-protocol.md     # research hooks
├── complexity-scoring.md    # SPEED vs FULL
├── apc-checkpoints.md       # A/P/C system
├── spike-execution.md       # spikes detail
└── bmad-index.md            # pointer to bmad/

conductor/references/
├── tdd-protocol.md          # TDD implementation
├── validation-gates.md      # gates
├── commands.md              # all /conductor-* docs
└── track-execution.md       # ci workflow

orchestrator/references/
├── worker-protocol.md       # 4-step worker flow
├── agent-coordination.md    # Agent Mail, conflicts
├── file-reservations.md     # locking
└── agents/                  # keep as subdirectory

tracking/references/
├── bd-commands.md           # CLI reference
├── dependency-graph.md      # deps management
└── workflow-integration.md  # beads + conductor

handoff/references/
├── session-context.md       # what gets saved
├── archive-protocol.md      # finish + cleanup
└── handoff-triggers.md      # 6 trigger types
```

---

## Task Breakdown

### Epic: Skills Architecture Refactor

| ID | Task | Type | Effort | Dependencies |
|----|------|------|--------|--------------|
| T1 | Update maestro-core routing table + hierarchy | Task | S | None |
| T2 | Rename `design/` → `designing/` | Task | M | T1 |
| T3 | Update `conductor/` (remove design phases) | Task | M | T1 |
| T4 | Rename `beads/` → `tracking/` | Task | M | T1 |
| T5 | Merge `skill-creator/` + `writing-skills/` → `creating-skills/` | Task | M | T1 |
| T6 | Move commands to correct owners | Task | S | T2, T3 |
| T7 | Flatten reference hierarchy | Task | L | T2, T3 |
| T8 | Update all cross-skill references | Task | M | T4, T5 |
| T9 | Update CODEMAPS | Task | S | T8 |
| T10 | Update AGENTS.md (root + conductor) | Task | S | T8 |

### Wave Execution

| Wave | Tasks | Parallel |
|------|-------|----------|
| Wave 1 | T1 | No |
| Wave 2 | T2, T3, T4, T5 | Yes |
| Wave 3 | T6, T7 | Yes |
| Wave 4 | T8, T9, T10 | Yes |

### Track Assignments

| Track | Agent | Tasks | Files |
|-------|-------|-------|-------|
| Track A | BlueLake | T1, T9, T10 | maestro-core/, CODEMAPS/, AGENTS.md |
| Track B | GreenCastle | T2, T7 | design/ → designing/, references/ |
| Track C | PurpleMountain | T3, T6 | conductor/, commands |
| Track D | OrangeRiver | T4, T5, T8 | beads/ → tracking/, creating-skills/ |

---

## Oracle Audit Summary

| Dimension | Score | Notes |
|-----------|-------|-------|
| Clarity of ownership | ✅ Good | Clear design→conduct→orchestrate chain |
| Overlap elimination | ⚠️ Partial | Ownership matrix added |
| Naming conventions | ✅ Good | Mixed gerund/noun per user preference |
| Migration complexity | ⚠️ Medium | Mapping table provided |
| User experience | ✅ Good | Triggers stable (ds, ci, co, bd) |
| Maintainability | ✅ Good | Flattened references planned |

**Verdict:** APPROVED with conditions (behavioral fixes as follow-up)

---

## Success Criteria

- [ ] All skills have clear ownership (no overlap)
- [ ] SKILL.md files ≤500 lines
- [ ] References 1-level deep (except bmad/, agents/)
- [ ] Cross-skill references eliminated
- [ ] Triggers work with new skill names
- [ ] CODEMAPS and AGENTS.md updated
- [ ] All tests pass

---

## Follow-up Tracks (Not in Scope)

1. Agent Mail coordination simplification
2. Validation gate complexity reduction
3. Beads CLI integration improvements
