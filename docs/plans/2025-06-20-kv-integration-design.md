# Knowledge & Vibes Integration Design

## Overview

Integrate K&V concepts into my-workflow to enhance planning, decomposition, and implementation quality.

## Integration Points

| # | K&V Concept | Target | Approach |
|---|-------------|--------|----------|
| 1 | 4-Phase Framework | `skills/conductor/` | Terminology mapping |
| 2 | Codemaps | `skills/codemaps/` | New skill + template |
| 3 | Grounding | `commands/ground.md` + skills | Command + integrated |
| 4 | LOSSLESS decomposition | `skills/beads/file-beads/` | Enhance existing |
| 5 | Phase sizing | `skills/plan-executor/` | Enhance existing |
| 6 | `/decompose-task` | `commands/decompose-task.md` | New command |
| 7 | `/ground` | `commands/ground.md` | New command |

## Tasks

### Phase 1: New Components

- [ ] Task 1.1: Create codemaps skill
  - Files: `skills/codemaps/SKILL.md`, `skills/codemaps/references/CODEMAPS_TEMPLATE.md`
  - Acceptance: Skill teaches codemap creation, includes template
  - Depends: none

- [ ] Task 1.2: Create /ground command
  - Files: `commands/ground.md`
  - Acceptance: Routes to repo/web/history truth sources, outputs verified patterns
  - Depends: none

- [ ] Task 1.3: Create /decompose-task command
  - Files: `commands/decompose-task.md`
  - Acceptance: Breaks phases into atomic beads with LOSSLESS verification
  - Depends: none

### Phase 2: Skill Enhancements

- [ ] Task 2.1: Enhance file-beads with LOSSLESS rules
  - Files: `skills/beads/file-beads/SKILL.md`
  - Acceptance: LOSSLESS rule documented, verification steps added
  - Depends: 1.3

- [ ] Task 2.2: Enhance plan-executor with phase sizing + grounding
  - Files: `skills/plan-executor/SKILL.md`
  - Acceptance: 500-1000 line sizing rules, grounding step before external deps
  - Depends: 1.2

- [ ] Task 2.3: Enhance conductor with 4-phase mapping
  - Files: `skills/conductor/SKILL.md`
  - Acceptance: Terminology mapping table added
  - Depends: none

### Phase 3: Grounding Integration

- [ ] Task 3.1: Add grounding to brainstorming
  - Files: `skills/brainstorming/SKILL.md`
  - Acceptance: Grounding step after design finalized
  - Depends: 1.2

- [ ] Task 3.2: Add grounding to implementer prompt
  - Files: `skills/subagent-driven-development/implementer-prompt.md`
  - Acceptance: Grounding step before coding external deps
  - Depends: 1.2

## Detailed Designs

### Codemaps Skill

Token-aware architecture documentation for AI context.

**When to Use:**
- Starting work on unfamiliar codebase
- Before planning features that touch multiple areas
- When agent lacks context on "how things connect"

**What Codemaps Contain:**
- Module responsibilities (1-2 sentences each)
- Data flows between services (ASCII diagrams)
- Key integration points
- Common patterns ("we always do X when Y happens")

**Guidelines:**
- Concise: Navigation aids, not comprehensive docs
- Current: Stale codemaps are worse than none
- Scoped: One file per major area (API, database, auth)

**File Structure:**
```
CODEMAPS/
├── overview.md      # System-wide architecture
├── api.md           # API layer patterns
├── database.md      # Data models, migrations
├── auth.md          # Authentication flow
└── integrations.md  # Third-party services
```

### /ground Command

Verify patterns against current truth before implementation.

**Usage:** `/ground <question-or-pattern>`

**Truth Sources:**

| Source | When | Tool |
|--------|------|------|
| Repo truth | "How do we do X here?" | Grep, finder |
| Web truth | External libs/APIs | web_search, read_web_page |
| History truth | "Did we solve this before?" | memory search |

**Process:**
1. Identify what needs grounding (library, API, pattern)
2. Determine truth source
3. Query and verify
4. Return verified pattern with source

**Output Format:**
```
GROUNDING: <what was verified>
SOURCE: <repo|web|history>
STATUS: ✅ Current | ⚠️ Outdated | ❌ Not found
PATTERN: <the verified pattern to use>
```

### /decompose-task Command

Break a phase into atomic beads for agent execution.

**Usage:** `/decompose-task <phase-file-or-section>`

**Process:**
1. Read phase content (must be 500-1000 lines)
2. Create parent bead (epic-level)
3. Create sub-beads following structure:
   - .0 Context Brief
   - .1 Schema/Types
   - .2-.3 Implementation
   - .4-.8 Tests (happy, edge, error, property, integration)
   - .9 Reference Data
   - .10 Verification Checklist

**LOSSLESS Verification:**
After decomposition, verify:
- Total sub-bead chars >= original chars
- Every section mapped to a sub-bead
- No "see parent" references

### LOSSLESS Rule (for file-beads)

When decomposing plans into beads:
- NEVER paraphrase or summarize
- NEVER write "see parent for details"
- NEVER skip "obvious" content
- COPY content verbatim

Verification:
- Sub-bead character count >= original (overhead expected)
- Every section from original appears somewhere
- Each sub-bead makes sense standalone

### Phase Sizing Rules (for plan-executor)

Agents work best on files under 1000 lines, preferably ~500.

| Metric | Target | Action if Wrong |
|--------|--------|-----------------|
| Phase size | 500-1000 lines | Split if too large |
| Sub-bead size | ~500 lines | Merge if too small |
| Work duration | 30-120 min per task | Re-scope if outside |

**Why this matters:**
- Large docs cause context bombing (agent skips detail)
- Lazy summarization ("implement auth" vs specific methods)
- Critical edge cases get paraphrased away

### 4-Phase Framework Mapping (for conductor)

| K&V Phase | Conductor Equivalent |
|-----------|---------------------|
| Requirements | `/conductor-newtrack` questions → `spec.md` |
| Plan | `spec.md` → `plan.md` generation |
| Implement | `bd ready` → execution via plan-executor |
| Reflect | `retro-workflow` + `bd close` |

## Implementation Order

1. Task 1.1: Create codemaps skill
2. Task 1.2: Create /ground command
3. Task 1.3: Create /decompose-task command
4. Task 2.1: Enhance file-beads
5. Task 2.2: Enhance plan-executor
6. Task 2.3: Enhance conductor
7. Task 3.1: Add grounding to brainstorming
8. Task 3.2: Add grounding to implementer prompt
