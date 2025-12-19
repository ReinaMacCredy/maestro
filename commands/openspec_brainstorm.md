# OpenSpec Brainstorm Mode

**Purpose:** Integrate OpenSpec spec-driven development with Amp's brainstorm/planning workflow.

---

## Phase 0: Initialize OpenSpec (Auto-Check)

**Before starting any brainstorm session:**

1. Check if `openspec/` directory exists in the current working directory:
   ```bash
   ls -la openspec/ 2>/dev/null || echo "NOT_INITIALIZED"
   ```

2. If NOT initialized, run with **selected tools**:
   ```bash
   openspec init --tools cursor,claude,cline,factory,antigravity,codex
   ```
   This creates instruction files for: Cursor, Claude, Cline, Factory, Antigravity, and Codex.

3. After init, add tool-specific files to `.git/info/exclude` (keeps them untracked but local):
   ```bash
   # Ensure .git/info directory exists
   mkdir -p .git/info
   
   # Add OpenSpec tool files to exclude (if not already present)
   EXCLUDE_FILE=".git/info/exclude"
   PATTERNS=(
     "# OpenSpec AI tool files"
     ".cursor/"
     ".claude/"
     ".factory/"
     ".clinerules/"
     ".agent/"
     "CLAUDE.md"
     "CLINE.md"
   )
   
   for pattern in "${PATTERNS[@]}"; do
     grep -qxF "$pattern" "$EXCLUDE_FILE" 2>/dev/null || echo "$pattern" >> "$EXCLUDE_FILE"
   done
   ```
   
   **Note:** Using `.git/info/exclude` instead of `.gitignore` keeps these exclusions local to your machine without affecting the repo.

4. After init completes, **automatically populate project context**:
   
   > "OpenSpec initialized. Let me analyze your project and populate the context."

   **Auto-research the codebase:**
   - Read `README.md`, `package.json`, `Cargo.toml`, `go.mod`, `pyproject.toml`, or similar
   - Check for existing config files (`.eslintrc`, `tsconfig.json`, `.prettierrc`, etc.)
   - Look at directory structure to understand architecture
   - Check for existing tests to understand testing patterns
   - Read any existing `AGENTS.md`, `CLAUDE.md`, or similar instruction files

   **Then fill out `openspec/project.md`:**
   - Populate Purpose, Tech Stack, Conventions based on findings
   - Fill in Architecture Patterns from directory structure
   - Add Testing Strategy from test setup
   - Include Git Workflow from branch patterns or existing docs
   - Note External Dependencies from package files

   **Only ask user about:**
   - Missing information that couldn't be inferred
   - Domain-specific context not in code
   - Business constraints not documented

   **Show the populated project.md to user for approval before proceeding.**

---

## Phase 1: Discovery & Context Gathering

**Objective:** Understand the feature/change before creating a proposal.

### Step 1.1: Ask Clarifying Questions

Ask the user about their idea. Key questions:

1. **What problem are you solving?** (The "Why")
2. **What should change?** (High-level description)
3. **Who/what is affected?** (Impact scope)
4. **Are there breaking changes?** (API, schema, behavior)
5. **What's the complexity?** (Simple fix, new feature, architecture change)

### Step 1.2: Research Existing Context

Before creating a proposal:

1. Review project context:
   ```bash
   cat openspec/project.md
   ```

2. Check existing specs:
   ```bash
   openspec list --specs
   ```

3. Check active changes (avoid conflicts):
   ```bash
   openspec list
   ```

4. Search for related code/specs:
   ```bash
   rg -n "Requirement:|Scenario:" openspec/specs 2>/dev/null || echo "No specs yet"
   ```

### Step 1.3: Decision Gate

Determine if a proposal is needed:

| Request Type | Action |
|-------------|--------|
| Bug fix (restoring intended behavior) | Fix directly, no proposal |
| Typo/formatting/comments | Fix directly, no proposal |
| Dependency update (non-breaking) | Fix directly, no proposal |
| **New feature/capability** | **Create proposal** |
| **Breaking change** | **Create proposal** |
| **Architecture change** | **Create proposal** |
| **Performance/security work** | **Create proposal** |
| Unclear scope | **Create proposal (safer)** |

---

## Phase 2: Create OpenSpec Change Proposal

**Objective:** Scaffold a structured change proposal.

### Step 2.1: Choose Change ID

- Use kebab-case, verb-led naming: `add-`, `update-`, `remove-`, `refactor-`
- Examples: `add-user-auth`, `update-payment-flow`, `refactor-api-client`
- Ensure uniqueness (check with `openspec list`)

### Step 2.2: Scaffold Change Directory

Create the change structure:

```bash
CHANGE_ID="<chosen-id>"
mkdir -p "openspec/changes/$CHANGE_ID/specs"
```

### Step 2.3: Create proposal.md

**Template:**

```markdown
# Change: [Brief description]

## Why
[1-2 sentences on the problem/opportunity]

## What Changes
- [Bullet list of changes]
- [Mark breaking changes with **BREAKING**]

## Impact
- **Affected specs:** [list capabilities]
- **Affected code:** [key files/systems]
- **Risk level:** Low / Medium / High

## Success Criteria
- [ ] [Measurable outcome 1]
- [ ] [Measurable outcome 2]
```

### Step 2.4: Create Spec Deltas

For each affected capability, create `openspec/changes/<id>/specs/<capability>/spec.md`:

```markdown
## ADDED Requirements

### Requirement: [Name]
The system SHALL [behavior description].

#### Scenario: [Success case]
- **WHEN** [trigger condition]
- **THEN** [expected outcome]

#### Scenario: [Error case]
- **WHEN** [error condition]
- **THEN** [error handling]
```

**Delta Operations:**
- `## ADDED Requirements` - New capabilities
- `## MODIFIED Requirements` - Changed behavior (include FULL updated text)
- `## REMOVED Requirements` - Deprecated features (include reason + migration)
- `## RENAMED Requirements` - Name changes only

### Step 2.5: Create tasks.md

**Template:**

```markdown
# Implementation Tasks

## 1. [Phase/Category Name]
- [ ] 1.1 [Specific, verifiable task]
- [ ] 1.2 [Specific, verifiable task]

## 2. [Phase/Category Name]
- [ ] 2.1 [Specific, verifiable task]
- [ ] 2.2 [Specific, verifiable task]

## 3. Testing & Validation
- [ ] 3.1 Unit tests for [component]
- [ ] 3.2 Integration tests for [flow]
- [ ] 3.3 Manual verification of [scenario]
```

### Step 2.6: Create design.md (Optional)

Only create if:
- Cross-cutting change (multiple services/modules)
- New external dependency or data model changes
- Security, performance, or migration complexity
- Ambiguity requiring technical decisions before coding

**Template:**

```markdown
# Design: [Change Name]

## Context
[Background, constraints, stakeholders]

## Goals
- [Goal 1]
- [Goal 2]

## Non-Goals
- [What we're NOT doing]

## Decisions
| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| [Choice] | [Why] | [Other options] |

## Risks & Mitigations
| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| [Risk] | Low/Med/High | Low/Med/High | [How to address] |

## Open Questions
- [ ] [Question 1]
- [ ] [Question 2]
```

---

## Phase 3: Validate & Review

### Step 3.1: Validate the Proposal

```bash
openspec validate <change-id> --strict
```

Fix any validation errors before proceeding.

### Step 3.2: Show the Proposal

```bash
openspec show <change-id>
```

### Step 3.3: Review with User

Present the proposal summary:
- What's being changed
- Why it's being changed
- Implementation tasks
- Estimated complexity

Ask: **"Does this proposal look correct? Any refinements needed?"**

---

## Phase 4: Refinement Loop

Iterate on the specs until the user approves:

1. Listen to feedback
2. Update relevant files (proposal.md, tasks.md, spec deltas)
3. Re-validate: `openspec validate <change-id> --strict`
4. Show updated proposal
5. Repeat until approved

---

## Phase 5: Handoff to Implementation

Once approved, inform the user:

> **Proposal approved!** To implement this change:
> 
> 1. **Convert to Beads issues for tracking:**
>    ```
>    Say "file beads" to convert OpenSpec tasks to Beads issues
>    ```
>    This will:
>    - Read the `tasks.md` from the OpenSpec change
>    - Convert each phase to a Beads epic/feature
>    - Convert each task to a child Beads issue
>    - Set dependencies based on task order
>
> 2. After filing beads, say **"review beads"** to refine the issues.
>
> 3. Start implementation:
>    ```
>    "Let's implement the <change-id> change"
>    ```
>    or use: `/openspec:apply <change-id>` (in tools that support it)
>
> 4. After deployment, archive the change:
>    ```bash
>    openspec archive <change-id> --yes
>    ```

---

## Beads Village Integration

When user says **"file beads"** after OpenSpec proposal is complete:

1. **Locate the OpenSpec tasks.md:**
   - Find the active change: `openspec list`
   - Read tasks from: `openspec/changes/<change-id>/tasks.md`

2. **Convert to Beads issues:**
   - Each `## Phase/Category` becomes a Beads **epic** or **feature** issue
   - Each `- [ ] X.X Task` becomes a child **task** issue
   - Set parent-child relationships
   - Set dependencies based on phase order (Phase 2 depends on Phase 1, etc.)

3. **After conversion, inform user:**
   > "OpenSpec tasks converted to Beads issues. Auto-triggering review..."

4. **Auto-trigger review_beads** - Do NOT wait for user to say "review beads"

---

## Quick Reference Commands

```bash
# List active changes
openspec list

# List existing specs
openspec list --specs

# Show change details
openspec show <change-id>

# Validate change
openspec validate <change-id> --strict

# Archive completed change
openspec archive <change-id> --yes

# Interactive dashboard
openspec view
```

---

## Important Rules

1. **NO CODE during brainstorm.** Only create design documents (proposal.md, tasks.md, design.md, spec deltas).
2. **Every requirement MUST have at least one scenario** using `#### Scenario:` format.
3. **Use SHALL/MUST** for normative requirements.
4. **Validate before sharing** - always run `openspec validate --strict`.
5. **Keep changes scoped** - one proposal per logical feature/change.
6. **Check for conflicts** - review `openspec list` before starting.

---

## Scenario Format Reference

**CORRECT:**
```markdown
#### Scenario: User login success
- **WHEN** valid credentials provided
- **THEN** return JWT token
```

**WRONG:**
```markdown
- **Scenario: User login**     # Don't use bullets
**Scenario**: User login       # Don't use bold
### Scenario: User login       # Wrong header level (use ####)
```
