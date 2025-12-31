# Plan: Continuous-Claude-v2 Integration

## Track Assignments

| Track | Epic | Owner | Files |
|-------|------|-------|-------|
| 1 | Core Integration | Worker-1 | AGENTS.md, maestro-core |
| 2 | Agent Directory (Research) | Worker-2 | agents/research/* |
| 3 | Agent Directory (Review) | Worker-3 | agents/review/* |
| 4 | Agent Directory (Planning/Exec/Debug) | Worker-4 | agents/planning/*, agents/execution/*, agents/debug/* |
| 5 | Orchestrator Routing | Worker-5 | orchestrator/* |
| 6 | Handoff Migration | Worker-6 | conductor/references/handoff/* |

---

## Epic 0: Core Integration

**Goal:** Add thin router to AGENTS.md, enable clean main thread pattern.

### 0.1 Create thin-router section in AGENTS.md [M]
- **Acceptance:** AGENTS.md contains `## Thin Router` section with routing table, spawn pattern, summary protocol, and first-message context loading
- **Deliverable:** Updated `AGENTS.md` with ~50 lines thin-router section
- Files: `AGENTS.md`
- Deps: None

### 0.2 Create delegation.md reference [S]
- **Acceptance:** File exists with clear table of main-thread vs sub-agent responsibilities
- **Deliverable:** `skills/maestro-core/references/delegation.md`
- Files: `skills/maestro-core/references/delegation.md`
- Deps: 0.1

### 0.3 Create intent-routing.md reference [M]
- **Acceptance:** File contains full routing table with 15+ keyword→agent mappings
- **Deliverable:** `skills/orchestrator/references/intent-routing.md`
- Files: `skills/orchestrator/references/intent-routing.md`
- Deps: 0.1

### 0.4 Create summary-protocol.md reference [S]
- **Acceptance:** File defines summary format with Status/Files/Decisions fields and Agent Mail message structure
- **Deliverable:** `skills/orchestrator/references/summary-protocol.md`
- Files: `skills/orchestrator/references/summary-protocol.md`
- Deps: 0.1

### 0.5 Update maestro-core SKILL.md [S]
- **Acceptance:** SKILL.md references delegation.md and includes thin-router pattern description
- **Deliverable:** Updated `skills/maestro-core/SKILL.md`
- Files: `skills/maestro-core/SKILL.md`
- Deps: 0.2

---

## Epic 2: Agent Directory

**Goal:** Create `skills/orchestrator/agents/` with all specialized agents.

### 2.0 Create directory structure [S]
- **Acceptance:** Directory tree exists: agents/{research,review,planning,execution,debug}/ with README.md index
- **Deliverable:** `skills/orchestrator/agents/README.md` with agent index table
- Files: `skills/orchestrator/agents/README.md`
- Deps: None

### Research Agents (migrate from conductor)

### 2.1 Move codebase-locator.md [S]
- **Acceptance:** Agent file at new location includes Agent Mail `send_message()` section
- **Deliverable:** `skills/orchestrator/agents/research/codebase-locator.md`
- Files: `skills/orchestrator/agents/research/codebase-locator.md`
- Deps: 2.0

### 2.2 Move codebase-analyzer.md [S]
- **Acceptance:** Agent file at new location includes Agent Mail `send_message()` section
- **Deliverable:** `skills/orchestrator/agents/research/codebase-analyzer.md`
- Files: `skills/orchestrator/agents/research/codebase-analyzer.md`
- Deps: 2.0

### 2.3 Move pattern-finder.md [S]
- **Acceptance:** Agent file at new location includes Agent Mail `send_message()` section
- **Deliverable:** `skills/orchestrator/agents/research/pattern-finder.md`
- Files: `skills/orchestrator/agents/research/pattern-finder.md`
- Deps: 2.0

### 2.4 Move impact-assessor.md [S]
- **Acceptance:** Agent file at new location includes Agent Mail `send_message()` section
- **Deliverable:** `skills/orchestrator/agents/research/impact-assessor.md`
- Files: `skills/orchestrator/agents/research/impact-assessor.md`
- Deps: 2.0

### 2.5 Move web-researcher.md [S]
- **Acceptance:** Agent file at new location includes Agent Mail `send_message()` section
- **Deliverable:** `skills/orchestrator/agents/research/web-researcher.md`
- Files: `skills/orchestrator/agents/research/web-researcher.md`
- Deps: 2.0

### 2.6 Create github-researcher.md (NEW) [M]
- **Acceptance:** New agent with GitHub API patterns (repo search, issue search, commit history) and Agent Mail save
- **Deliverable:** `skills/orchestrator/agents/research/github-researcher.md`
- Files: `skills/orchestrator/agents/research/github-researcher.md`
- Deps: 2.0

### Review Agents (NEW)

### 2.7 Create security-reviewer.md [M]
- **Acceptance:** Agent with security checklist (secrets, injection, auth) and Agent Mail save
- **Deliverable:** `skills/orchestrator/agents/review/security-reviewer.md`
- Files: `skills/orchestrator/agents/review/security-reviewer.md`
- Deps: 2.0

### 2.8 Create code-reviewer.md [M]
- **Acceptance:** Agent with code quality checklist (style, complexity, DRY) and Agent Mail save
- **Deliverable:** `skills/orchestrator/agents/review/code-reviewer.md`
- Files: `skills/orchestrator/agents/review/code-reviewer.md`
- Deps: 2.0

### 2.9 Create pr-reviewer.md [M]
- **Acceptance:** Agent with PR review workflow (diff analysis, commit messages, test coverage) and Agent Mail save
- **Deliverable:** `skills/orchestrator/agents/review/pr-reviewer.md`
- Files: `skills/orchestrator/agents/review/pr-reviewer.md`
- Deps: 2.0

### 2.10 Create spec-reviewer.md [M]
- **Acceptance:** Agent with spec compliance validation and Agent Mail save
- **Deliverable:** `skills/orchestrator/agents/review/spec-reviewer.md`
- Files: `skills/orchestrator/agents/review/spec-reviewer.md`
- Deps: 2.0

### Planning Agents (from C-C-v2)

### 2.11 Create plan-agent.md [M]
- **Acceptance:** Agent ported from C-C-v2 with adapted prompts for Agent Mail context save
- **Deliverable:** `skills/orchestrator/agents/planning/plan-agent.md`
- Files: `skills/orchestrator/agents/planning/plan-agent.md`
- Deps: 2.0

### 2.12 Create validate-agent.md [M]
- **Acceptance:** Agent ported from C-C-v2 with validation rules and Agent Mail save
- **Deliverable:** `skills/orchestrator/agents/planning/validate-agent.md`
- Files: `skills/orchestrator/agents/planning/validate-agent.md`
- Deps: 2.0

### Execution Agents

### 2.13 Create implement-agent.md [L]
- **Acceptance:** Agent with TDD workflow (red-green-refactor), file reservation, Agent Mail progress updates
- **Deliverable:** `skills/orchestrator/agents/execution/implement-agent.md`
- Files: `skills/orchestrator/agents/execution/implement-agent.md`
- Deps: 2.0

### 2.14 Create worker-agent.md [M]
- **Acceptance:** Generalized worker template from worker-prompt.md with full Agent Mail protocol
- **Deliverable:** `skills/orchestrator/agents/execution/worker-agent.md`
- Files: `skills/orchestrator/agents/execution/worker-agent.md`
- Deps: 2.0

### Debug Agents (from C-C-v2)

### 2.15 Create debug-agent.md [M]
- **Acceptance:** Agent with root cause analysis workflow and Agent Mail save
- **Deliverable:** `skills/orchestrator/agents/debug/debug-agent.md`
- Files: `skills/orchestrator/agents/debug/debug-agent.md`
- Deps: 2.0

### Fix References

### 2.16 Update conductor/SKILL.md refs [S]
- **Acceptance:** All references to research agents point to `skills/orchestrator/agents/research/`
- **Deliverable:** Updated `skills/conductor/SKILL.md`
- Files: `skills/conductor/SKILL.md`
- Deps: 2.1-2.5

### 2.17 Update design/SKILL.md refs [S]
- **Acceptance:** All references to research agents point to `skills/orchestrator/agents/research/`
- **Deliverable:** Updated `skills/design/SKILL.md`
- Files: `skills/design/SKILL.md`
- Deps: 2.1-2.5

### 2.18 Update design/references/grounding.md refs [S]
- **Acceptance:** All references to research agents point to `skills/orchestrator/agents/research/`
- **Deliverable:** Updated `skills/design/references/grounding.md`
- Files: `skills/design/references/grounding.md`
- Deps: 2.1-2.5

### 2.19 Update conductor/references/research/protocol.md refs [S]
- **Acceptance:** All references to research agents point to `skills/orchestrator/agents/research/`
- **Deliverable:** Updated `skills/conductor/references/research/protocol.md`
- Files: `skills/conductor/references/research/protocol.md`
- Deps: 2.1-2.5

---

## Epic 3: Orchestrator Routing

**Goal:** Add agent routing and spawn logic to orchestrator.

### 3.1 Create agent-routing.md [M]
- **Acceptance:** File contains routing table and Task() spawn patterns for each agent category
- **Deliverable:** `skills/orchestrator/references/agent-routing.md`
- Files: `skills/orchestrator/references/agent-routing.md`
- Deps: Epic 2

### 3.2 Update worker-prompt.md [S]
- **Acceptance:** Template includes mandatory `send_message()` call and summary return format
- **Deliverable:** Updated `skills/orchestrator/references/worker-prompt.md`
- Files: `skills/orchestrator/references/worker-prompt.md`
- Deps: 3.1

### 3.3 Update workflow.md [M]
- **Acceptance:** File includes agent spawn section referencing routing table
- **Deliverable:** Updated `skills/orchestrator/references/workflow.md`
- Files: `skills/orchestrator/references/workflow.md`
- Deps: 3.1, 3.2

### 3.4 Update orchestrator SKILL.md [M]
- **Acceptance:** SKILL.md contains routing section and references to new docs
- **Deliverable:** Updated `skills/orchestrator/SKILL.md`
- Files: `skills/orchestrator/SKILL.md`
- Deps: 3.1-3.3

### 3.5 Add orchestrator self-registration [S]
- **Acceptance:** SKILL.md includes `register_agent()` call on spawn and inbox fetch
- **Deliverable:** Updated `skills/orchestrator/SKILL.md`
- Files: `skills/orchestrator/SKILL.md`
- Deps: 3.4

---

## Epic 4: Maestro-Core Tightening

**Goal:** Integrate thin-router pattern into maestro-core.

### 4.1 Update routing.md [M]
- **Acceptance:** File includes agent delegation rules with reference to intent-routing.md
- **Deliverable:** Updated `skills/maestro-core/references/routing.md`
- Files: `skills/maestro-core/references/routing.md`
- Deps: Epic 3

### 4.2 Update SKILL.md [M]
- **Acceptance:** SKILL.md references delegation pattern and includes Amp-specific notes (no hooks)
- **Deliverable:** Updated `skills/maestro-core/SKILL.md`
- Files: `skills/maestro-core/SKILL.md`
- Deps: 4.1

---

## Epic 5: Handoff Migration

**Goal:** Update handoff system for Agent Mail primary storage.

### 5.1 Create agent-mail-format.md [M]
- **Acceptance:** File defines handoff message schema with required fields and thread structure
- **Deliverable:** `skills/conductor/references/handoff/agent-mail-format.md`
- Files: `skills/conductor/references/handoff/agent-mail-format.md`
- Deps: None

### 5.2 Update /create_handoff [M]
- **Acceptance:** Command sends to Agent Mail first, then exports markdown as secondary
- **Deliverable:** Updated `skills/conductor/references/handoff/create.md`
- Files: `skills/conductor/references/handoff/create.md`
- Deps: 5.1

### 5.3 Update /resume_handoff [M]
- **Acceptance:** Command reads from Agent Mail using `summarize_thread()`, falls back to markdown
- **Deliverable:** Updated `skills/conductor/references/handoff/resume.md`
- Files: `skills/conductor/references/handoff/resume.md`
- Deps: 5.1

---

## Summary

| Epic | Tasks | Priority | Est. Time | Complexity |
|------|-------|----------|-----------|------------|
| 0. Core Integration | 5 | P0 | 2h | 2S, 3M |
| 2. Agent Directory | 19 | P0 | 4h | 10S, 8M, 1L |
| 3. Orchestrator Routing | 5 | P1 | 2h | 2S, 3M |
| 4. Maestro-Core | 2 | P1 | 1h | 2M |
| 5. Handoff Migration | 3 | P1 | 1h | 3M |
| **Total** | **34** | | **10h** | **14S, 19M, 1L** |

## Execution Order

```
Epic 0 (Core) ──────────────────────────────────────┐
                                                    │
Epic 2 (Agents) ────────────────────────────────────┤
                                                    │
                    ┌───────────────────────────────┘
                    │
                    ▼
            Epic 3 (Routing)
                    │
                    ▼
            Epic 4 (Maestro-Core)

Epic 5 (Handoff) ── can run parallel ──────────────►
```

## Automated Verification

Run these commands to verify implementation:

```bash
# Validate all markdown files have valid syntax
./scripts/validate-links.sh .

# Check agent directory structure exists
test -d skills/orchestrator/agents/research && \
test -d skills/orchestrator/agents/review && \
test -d skills/orchestrator/agents/planning && \
test -d skills/orchestrator/agents/execution && \
test -d skills/orchestrator/agents/debug && \
echo "✅ Agent directory structure complete"

# Count agent files (expect 15)
find skills/orchestrator/agents -name "*.md" -not -name "README.md" | wc -l

# Verify all agents include Agent Mail save section
grep -l "send_message" skills/orchestrator/agents/**/*.md | wc -l

# Check AGENTS.md has thin-router section
grep -q "## Thin Router" AGENTS.md && echo "✅ Thin router in AGENTS.md"

# Verify no broken references to old agent locations
! grep -r "conductor/references/research/agents" skills/ && echo "✅ No stale agent refs"

# Check handoff files updated
grep -q "Agent Mail" skills/conductor/references/handoff/create.md && \
grep -q "Agent Mail" skills/conductor/references/handoff/resume.md && \
echo "✅ Handoff migration complete"
```

**Success criteria:** All commands exit 0, agent count = 15, Agent Mail grep count = 15.
