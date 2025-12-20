# Maestro: The Complete Guide

> **For humans**: Read this to understand what these skills do and why they matter.
> **For agents**: See the Quick Reference at the end for copy-paste triggers.

---

## Why This Exists

You're using AI coding agents to write code. That's great. But you've probably noticed some problems:

**The agent forgets what it was doing.** You come back the next day and it has no memory of yesterday's work. You have to re-explain everything.

**Plans evaporate into chat.** You discuss a feature for 20 minutes, then the agent jumps into coding without capturing what you agreed on. When context compacts, all that planning disappears.

**There's no clear "what's next."** You have 15 half-baked ideas but no structured way to see dependencies, blockers, or which task actually matters most.

**Tests are an afterthought.** The agent writes code first, maybe adds tests later. Those tests pass immediately, proving nothing.

**Debugging is chaotic.** When something breaks, the agent proposes random fixes instead of systematically tracing the problem.

**This plugin solves all of these problems.**

---

## What Problem Does Each Skill Solve?

| Problem | What Happens | Skill That Fixes It |
|---------|--------------|---------------------|
| **Amnesia** | Agent forgets tasks between sessions | `beads` — persistent issue tracking |
| **Fuzzy planning** | Vague discussions, no written spec | `conductor` — structured specs + plans |
| **No visibility** | Can't see dependencies or blockers | `beads` — dependency-aware graph |
| **Tests as afterthought** | Tests written after code, prove nothing | `test-driven-development` — RED-GREEN-REFACTOR |
| **Messy handoffs** | Can't resume where you left off | `beads` notes — session-surviving context |
| **Conflicts in multi-agent** | Multiple agents edit same files | `beads` + Village — file locking and task claiming |

These skills work together as a system, not a bag of independent tools.

---

## Key Insights (Aha Moments)

Before diving in, understand these principles:

1. **"Spend tokens once on a good plan; reuse it many times."**
   Long, fuzzy chats chew context. A structured spec+plan document is cheaper to revisit.

2. **"Your project's state lives in git, not in Claude's memory."**
   Chat history is ephemeral. Beads issues persist in `.beads/` and survive compaction.

3. **"Beads are dependency-aware, not a flat todo list."**
   You encode constraints once ("A blocks B"). All later sessions respect them.

4. **"Skills are mental modes, not just commands."**
   Invoking `tdd` or `debug` switches the agent into a specific methodology, not just runs a script.

5. **"Evidence before assertions."**
   Don't claim "tests pass" — show the passing output. Don't claim "fixed" — show the verification.

---

## Part 2: The Core Workflow

The spine of Maestro is three components working together:

```
Conductor (Planning) → Beads + Village (Tracking & Coordination) → TDD (Execution)
```

Everything else in the plugin supports this core pipeline.

---

### Conductor — Structured Planning

**What it is**: A guided planning flow that turns fuzzy goals into `spec.md` and `plan.md`.

**Why you need it**: It forces you to clarify scope, risks, and phases before the agent starts coding. Without this, you waste tokens rewriting half-baked designs.

**Key insight**: Conductor is for *project-level* thinking — tracks, specs, and plans. Not micro-tasks.

**For humans**:
- Think of it as the "measure twice, cut once" step
- You'll answer 3-5 clarifying questions, then get a structured spec and phased plan
- The output lives in `conductor/tracks/<track_id>/` and persists across sessions

**For agents**:
- When user says "new feature" or "build X", prefer `/conductor-newtrack <description>`
- Always ask clarifying questions before generating specs
- Confirm summaries before generating large artifacts
- Output: `spec.md` (requirements + acceptance criteria) and `plan.md` (phased tasks)

**The 4-Phase Framework**:

| Phase | Question to Answer | Conductor Output |
|-------|-------------------|------------------|
| **Requirements** | "Does the AI understand what we're building?" | Questions → `spec.md` |
| **Plan** | "Does this plan fit our architecture?" | `spec.md` → `plan.md` |
| **Implement** | "Can this be tested independently?" | Execute via beads + TDD |
| **Reflect** | "Would I bet my job on this code?" | Verify + close |

**Directory Structure**:

```
conductor/
├── product.md          # Product vision, users, goals
├── tech-stack.md       # Technology choices
├── workflow.md         # Development standards (TDD, commits)
├── tracks.md           # Master track list with status
└── tracks/
    └── auth_20241215/  # Track directory
        ├── spec.md     # Requirements and acceptance criteria
        └── plan.md     # Phased task list with status
```

---

### Beads — Persistent Issue Tracking

**What it is**: A git-backed issue tracker that persists tasks across sessions and tracks dependencies.

**Why you need it**: It turns Conductor's plans into durable, executable steps that exist outside any one session. Chat history compacts; beads survive.

**Key insight**: Beads is the execution queue; Conductor is the blueprint.

**For humans**:
- Think of it as a todo list that understands "I can't do X until Y is done"
- Issues live in `.beads/` and get committed with your code
- Come back in 2 weeks, run `bd ready`, and pick up where you left off

**For agents**:
- After Conductor creates `plan.md`, immediately use `fb` (file-beads) to convert it to issues
- At session start: `bd ready --json` to see what's unblocked
- At session end: Update notes with COMPLETED/IN_PROGRESS/NEXT format
- Always commit `.beads/` with code changes

**Core Commands**:

```bash
# Finding work
bd ready --json              # What's unblocked and ready?
bd blocked --json            # What's waiting on dependencies?
bd list --status in_progress # What's currently active?

# Working on issues
bd update bd-42 --status in_progress   # Claim a task
bd show bd-42                          # Read full context
bd close bd-42 --reason "Completed"    # Mark done

# Dependencies
bd dep add bd-child bd-blocker --type blocks   # A blocks B
bd dep add bd-new bd-current --type discovered-from  # Found during work
```

**What Survives Compaction**:

| Survives | Doesn't Survive |
|----------|-----------------|
| All bead data (issues, notes, deps) | Conversation history |
| Complete work history | TodoWrite lists |
| Key decisions in notes | Recent discussion |

**Writing Good Notes** (for post-compaction recovery):

```
COMPLETED: Specific deliverables ("implemented JWT refresh + rate limiting")
IN PROGRESS: Current state + next step ("testing reset flow, need email template")
BLOCKERS: What's preventing progress
KEY DECISIONS: Important context ("RS256 over HS256 per security review")
NEXT: Immediate next action
```

---

### Village — Multi-Agent Coordination

**What it is**: MCP server that adds atomic task claiming, file locking, and agent messaging to Beads.

**Why you need it**: When multiple Claude instances work on the same codebase, they can step on each other. Village prevents conflicts through file reservations and ensures only one agent claims each task.

**Key insight**: Single-agent workflows work fine without Village. Multi-agent workflows require it.

**For humans**:
- Think of it as "mutex locks for AI agents"
- One agent reserves a file, others wait or work on something else
- Tasks are claimed atomically — no two agents grab the same work

**For agents**:
- At session start: `init` to join workspace with your role
- Before work: `claim` to atomically get a task (replaces manual selection)
- Before editing: `reserve` to lock files
- When done: `done` releases locks and notifies team
- If blocked on a file: `msg` the owner, `inbox` to check responses

**Core Commands**:

```
# Joining and claiming
init team="backend" role="be"     # Join as backend developer
claim                              # Get next ready task for your role

# File coordination
reserve path="src/auth.ts"        # Lock file for editing (10 min TTL)
release path="src/auth.ts"        # Release when done

# Task completion
done id="bd-42" msg="Implemented JWT refresh"  # Complete + release all locks

# Communication
msg to="frontend" content="API ready"  # Direct message
inbox                              # Read your messages
status                             # See team state and locks
```

---

### TDD — Safe Execution

**What it is**: RED-GREEN-REFACTOR methodology for implementing each task.

**Why you need it**: Tests written after code pass immediately, proving nothing. Tests written first prove the test actually catches the bug.

**Key insight**: If you didn't watch the test fail, you don't know if it tests the right thing.

**For humans**:
- Every feature starts with a failing test
- Code is written to make the test pass, nothing more
- Refactoring happens only after tests are green

**For agents**:
- **Iron Law**: No production code without a failing test first
- Write code before the test? Delete it. Start over.
- Don't keep it as "reference" or "adapt" it — delete means delete

**The Cycle**:

```
RED     → Write one failing test (watch it fail)
GREEN   → Write minimal code to pass (watch it pass)
REFACTOR → Clean up (stay green)
REPEAT  → Next failing test
```

**Verification Checklist**:

- [ ] Every new function has a test
- [ ] Watched each test fail before implementing
- [ ] Failed for expected reason (feature missing, not typo)
- [ ] Wrote minimal code to pass
- [ ] All tests pass
- [ ] Output pristine (no errors, warnings)

---

### How They Connect

```
┌─────────────────────────────────────────────────────────────┐
│                      CONDUCTOR                               │
│  /conductor-newtrack "Add user auth"                        │
│       ↓                                                      │
│  Questions → spec.md → plan.md                              │
└─────────────────────────────────────────────────────────────┘
                           ↓
                    fb (file-beads)
                           ↓
┌─────────────────────────────────────────────────────────────┐
│                        BEADS                                 │
│  bd-001: Set up OAuth credentials                           │
│  bd-002: Implement auth flow (blocked by bd-001)            │
│  bd-003: Add token refresh (blocked by bd-002)              │
└─────────────────────────────────────────────────────────────┘
                           ↓
                    bd ready → pick bd-001
                    claim                    # Atomic task selection (multi-agent)
                    reserve                  # Lock files before editing
                           ↓
┌─────────────────────────────────────────────────────────────┐
│                         TDD                                  │
│  RED:    test('OAuth credentials are configured')           │
│  GREEN:  Implement credential loading                        │
│  REFACTOR: Extract config helper                            │
└─────────────────────────────────────────────────────────────┘
                           ↓
                    done bd-001              # Releases locks, notifies team
                           ↓
                    bd-002 becomes ready
```

**The Bridge Sentence**: Conductor turns ideas into structured documents. File-beads converts plans into trackable issues. Beads becomes your source of truth for "what's next." TDD describes *how* to execute each issue safely.

---

## Entry Points (You Don't Always Start at the Beginning)

The full pipeline assumes you're starting fresh. But you can jump in anywhere.

### Skip-Ahead Table

| You Already Have | Skip To | Trigger |
|------------------|---------|---------|
| An idea to explore | Brainstorm first | `bs` → conductor/plans/ |
| A plan (markdown, PRD, spec) | File beads directly | `fb` (file-beads skill) |
| Existing issues/tasks | Claim and execute | `bd ready` → `bd update` |
| A bug to fix | Debug first | `debug` → `systematic-debugging` |
| Code that needs tests | Add tests | `tdd` skill |

### Why This Matters: Token Economics

Conductor generates context files (`spec.md`, `plan.md`). These files:
- Get loaded every session when working on a track
- Consume tokens in your context window
- Are valuable for complex, multi-phase features

**If you already have a plan from elsewhere** (Notion doc, GitHub issue, PRD from PM), skip Conductor and go straight to `fb` to convert it into beads. Don't regenerate what already exists.

**Rule of thumb**:
- **Complex feature, starting from scratch** → Full Conductor flow
- **Clear task, plan exists** → `fb` to file beads, then execute
- **Single bug or small fix** → Direct to `debug` or `tdd`
- **Exploratory/uncertain** → `bs` (brainstorm) first

### The Minimal Path

For a quick task with no dependencies:

```bash
# Skip everything, just track and do
bd create "Fix login button alignment" -t bug -p 1
bd update bd-xyz --status in_progress
# ... use TDD to fix ...
bd close bd-xyz --reason "Fixed CSS flexbox alignment"
```

No Conductor, no elaborate planning. Match the process to the task.

---

## Part 3: Complete Session Walkthrough

Here's how everything fits together in a real coding session.

---

### Phase 1: Planning (Conductor)

**Goal**: Turn a fuzzy idea into a structured, executable plan.

#### Step 1: Initialize project (first time only)

```
/conductor-setup
```

This creates:
- `conductor/product.md` — Product vision
- `conductor/tech-stack.md` — Technology choices
- `conductor/workflow.md` — Development standards

#### Step 2: Create a new track

```
/conductor-newtrack "Add user authentication with OAuth"
```

The agent will:
1. Ask 3-5 clarifying questions based on track type
2. Generate `spec.md` with requirements and acceptance criteria
3. Generate `plan.md` with phased tasks

**Output structure**:
```
conductor/tracks/auth_20241215/
├── spec.md    # WHAT we're building
└── plan.md    # HOW we'll build it (phases + tasks)
```

#### Step 3: Review and confirm

Read the generated spec and plan. Adjust if needed. This is your last chance to catch scope issues cheaply.

---

### Phase 2: Filing (Beads)

**Goal**: Convert the plan into trackable, dependency-aware issues.

#### Step 1: File beads from plan

```
fb
```

The `file-beads` skill reads `plan.md` and creates beads issues with:
- One issue per task
- Dependencies encoded (`blocks`, `parent-child`)
- Priorities set

#### Step 2: Review filed beads

```
rb
```

The `review-beads` skill lets you:
- Check issue quality
- Adjust priorities
- Add missing dependencies

#### Step 3: See what's ready

```bash
bd ready --json
```

Output shows unblocked issues ready to work on.

---

### Phase 3: Execution (TDD Loop)

**Goal**: Implement each task safely with tests.

#### Step 1: Claim a task

```bash
bd update bd-001 --status in_progress
bd show bd-001
```

Read the issue's description, design, and acceptance criteria.

#### Step 2: RED — Write failing test

```typescript
test('OAuth callback exchanges code for tokens', async () => {
  const result = await handleOAuthCallback({ code: 'test-code' });
  expect(result.accessToken).toBeDefined();
  expect(result.refreshToken).toBeDefined();
});
```

Run it. Watch it fail.

```bash
npm test -- --grep "OAuth callback"
# FAIL: handleOAuthCallback is not defined
```

#### Step 3: GREEN — Write minimal code

```typescript
async function handleOAuthCallback({ code }) {
  const response = await fetch(tokenEndpoint, {
    method: 'POST',
    body: new URLSearchParams({ code, grant_type: 'authorization_code' })
  });
  const data = await response.json();
  return { accessToken: data.access_token, refreshToken: data.refresh_token };
}
```

Run it. Watch it pass.

```bash
npm test -- --grep "OAuth callback"
# PASS
```

#### Step 4: REFACTOR — Clean up

Extract constants, improve names, add types. Keep tests green.

#### Step 5: Update progress

```bash
bd update bd-001 --notes "COMPLETED: OAuth token exchange. NEXT: Add PKCE support."
```

#### Step 6: Close when done

```bash
bd close bd-001 --reason "Implemented OAuth callback with token exchange"
```

The next blocked issue (bd-002) now becomes ready.

---

### Phase 4: Checkpointing

**Goal**: Preserve context for future sessions.

#### When to checkpoint

- Before ending session
- At major milestones
- When context is running low (>70% token usage)
- Before asking user for decisions

#### How to checkpoint

```bash
bd update bd-002 --notes "
COMPLETED: PKCE challenge generation, token exchange
IN PROGRESS: Refresh token rotation
KEY DECISION: Using httpOnly cookies (not localStorage) per security review
NEXT: Implement silent refresh in background
BLOCKERS: None
"
```

**Self-check**: "Could I resume this work in 2 weeks with zero conversation history?"

---

### Phase 5: Session End

**Goal**: Leave clean state for next session.

#### Step 1: Update notes on active work

```bash
bd update bd-002 --notes "IN PROGRESS: Refresh rotation 80% done. NEXT: Add retry logic."
```

#### Step 2: Commit everything

```bash
git add -A && git commit -m "feat: OAuth token exchange

Implements bd-001, progress on bd-002.
- Added handleOAuthCallback
- PKCE challenge flow working
- Token storage in httpOnly cookies"
```

**Important**: `.beads/` gets committed with your code. This is your persistent memory.

#### Step 3: Push

```bash
git push
```

#### Step 4: Sync documentation (after epic completion)

When you close an epic (not every session), capture learnings:

```
doc-sync
```

The `doc-sync` skill:
1. Finds closed issues with thread URLs in notes
2. Reads each thread to extract patterns, commands, decisions
3. Updates relevant AGENTS.md files with new knowledge
4. Shows diff for your review before committing

**Why this matters**: Lessons learned during implementation often get lost when context compacts. Doc-sync preserves them in version-controlled AGENTS.md files.

---

### Phase 6: Next Session Resume

**Goal**: Pick up exactly where you left off.

```bash
# What's in progress?
bd list --status in_progress --json

# What's ready to start?
bd ready --json

# Read context for active work
bd show bd-002
```

The notes field tells you:
- What was completed
- What's in progress
- What to do next
- Any blockers or decisions

No need to re-explain context. It's all in beads.

---

## Part 4: Specialist Skills Quick Reference

Beyond the core workflow, Maestro includes specialist skills for specific situations.

---

### Planning & Exploration

| Skill | Trigger | When to Use |
|-------|---------|-------------|
| `brainstorming` | `bs` | Before any creative work. Explores intent and requirements before implementation. |
| `writing-plans` | `write plan` | When you have requirements but need a detailed implementation plan. |

---

### Development

| Skill | Trigger | When to Use |
|-------|---------|-------------|
| `test-driven-development` | `tdd` | Every feature and bugfix. RED-GREEN-REFACTOR, no exceptions. |
| `using-git-worktrees` | — | Starting feature work that needs isolation from current workspace. |
| `finishing-a-development-branch` | — | Implementation complete, tests pass, ready to integrate. |
| `subagent-driven-development` | — | Coordinating multiple subagents on independent tasks. |
| `dispatching-parallel-agents` | `dispatch` | 2+ independent tasks that can run in parallel. |

---

### Debugging

| Skill | Trigger | When to Use |
|-------|---------|-------------|
| `root-cause-tracing` | `trace` | Errors deep in execution. Traces backward through call stack. |
| `defense-in-depth` | — | Invalid data causes failures deep in execution. Validate at every layer. |

---

### Code Review

| Skill | Trigger | When to Use |
|-------|---------|-------------|
| `requesting-code-review` | `review code` | Completing tasks, before merging. Get structured review of your work. |
| `receiving-code-review` | — | When receiving feedback. Requires verification, not blind implementation. |

---

### Meta / Quality

| Skill | Trigger | When to Use |
|-------|---------|-------------|
| `using-superpowers` | — | Session initialization. Establishes how to find and use skills. |
| `verification-before-completion` | — | Before claiming work is complete. Evidence before assertions. |
| `writing-skills` | `write skill` | Creating or editing skills. |
| `sharing-skills` | `share skill` | Contributing skills upstream via PR. |

---

### Beads Helpers

| Skill | Trigger | When to Use |
|-------|---------|-------------|
| `beads/file-beads` | `fb` | Convert Conductor plan to beads issues. |
| `beads/review-beads` | `rb` | Review and refine filed beads issues. |

---

### Optional External Tools

Some skills work best with optional CLI tools. The skills still provide value without them (as mental models), but CLIs make them concrete.

| Skill | Optional CLI | What CLI Adds |
|-------|--------------|---------------|
| `beads` | `bd` | Persistent issue database, `bd ready`, `bd show` |
| `beads` | `bv` | Graph visualization, priority recommendations |
| `codemaps` | — | Token-aware architecture docs (no external CLI needed) |
| `ground` | — | Verification protocol: verify patterns against repo/web/history before implementation |

**If you have `bd` installed**: Commands like `bd ready --json` work directly.

**If you don't**: Treat "beads" as a conceptual task list. Track in GitHub Issues or markdown.

---

## Part 5: Troubleshooting & Scenarios

---

### Troubleshooting

| Problem | Solution |
|---------|----------|
| `bd: command not found` | Install via K&V setup or add `~/.local/bin` to PATH |
| Agent ignores the workflow | Say trigger phrase explicitly: `tdd`, `debug`, `bs` |
| Plan seems incomplete | Use `rb` (review-beads) to check and refine issues |
| Tests pass immediately | You wrote code first. Delete it. Start with failing test. |
| Context compacted, lost state | Run `bd show <issue-id>` — notes field has recovery context |
| Too many issues, overwhelmed | Run `bd ready` for unblocked only, or `bd blocked` to clear bottlenecks |
| Conductor files eating tokens | Skip Conductor if you already have a plan. Use `fb` directly. |

---

### Scenario A: New Feature From Scratch

**Problem**: "Build user invitations for our SaaS."

**Flow**:
```
/conductor-setup                    # First time only
/conductor-newtrack "user invitations feature"
  → Answer clarifying questions
  → Review spec.md and plan.md
fb                                  # File beads from plan
bd ready --json                     # See what's unblocked
bd update bd-001 --status in_progress
  → TDD loop for each task
  → bd close when done
```

**Value**: Full pipeline prevents scope creep, lost context, and untested code.

---

### Scenario B: Bugfix in Legacy Code

**Problem**: "Intermittent 500 in the billing endpoint."

**Flow**:
```
bd create "Investigate billing 500 errors" -t bug -p 0
bd update bd-xyz --status in_progress
debug                               # Systematic debugging skill
  → Phase 1: Reproduce
  → Phase 2: Isolate
  → Phase 3: Identify root cause
  → Phase 4: Fix with TDD
trace                               # If needed: trace backward through stack
verification-before-completion      # Show evidence of fix
bd close bd-xyz --reason "Fixed race condition in invoice calculation"
```

**Value**: Systematic approach prevents random fix attempts. TDD prevents regression.

---

### Scenario C: Messy Project Recovery

**Problem**: "We have 20 half-done tasks and no idea what to prioritize."

**Flow**:
```
bd list                             # See everything
bd blocked --json                   # Find what's stuck
rb                                  # Review and clean up issues
  → Merge duplicates
  → Add missing dependencies
  → Adjust priorities
bd ready --json                     # Now see what's actually actionable
/conductor-newtrack "Stabilization" # Optional: create recovery track
```

**Value**: Beads brings order to chaos. Dependencies surface what actually matters.

---

### Scenario D: "I Already Have a Plan"

**Problem**: "PM gave us a detailed spec in Notion. Just execute it."

**Flow**:
```
# Skip Conductor entirely
fb                                  # File beads from existing plan
  → Paste or reference the Notion spec
  → Agent converts to beads issues
bd ready --json
  → Pick first unblocked issue
  → TDD execution
```

**Value**: Don't regenerate what exists. Save tokens. Go straight to execution.

---

### Scenario E: Improving the Workflow

**Problem**: "We keep making the same mistakes. How do we learn?"

**Flow**:
```
# Capture patterns as you go
# If pattern is reusable:
write skill                         # Create new skill
  → Encode the pattern
  → Add to .claude/skills/ or contribute upstream
share skill                         # Optional: PR to upstream
```

**Value**: Encode learnings into skills for future reuse.

---

## Part 6: Quick Reference

### For Agents: Copy-Paste Triggers

**Session Start**:
```
bd ready --json                     # What's unblocked?
bd list --status in_progress --json # What's active?
bd show <id>                        # Read context
```

**Planning**:
```
/conductor-setup                    # Initialize project (once)
/conductor-newtrack "description"   # New feature/bug track
bs                                  # Brainstorm before implementing
fb                                  # File beads from plan
rb                                  # Review beads
```

**Execution**:
```
tdd                                 # Enter TDD mode
debug                               # Systematic debugging
trace                               # Root cause tracing
dispatch                            # Parallel subagents
```

**Progress**:
```bash
bd update <id> --status in_progress
bd update <id> --notes "COMPLETED: X. IN PROGRESS: Y. NEXT: Z."
bd close <id> --reason "summary"
```

**Session End**:
```bash
git add -A && git commit -m "message"
git push
```

---

### For Agents: Critical Rules

1. **No production code without a failing test first** — TDD is not optional
2. **Always checkpoint before session end** — Notes field survives compaction
3. **Commit `.beads/` with code** — It's your persistent memory
4. **Evidence before assertions** — Show test output, not just "tests pass"
5. **Match process to task size** — Small fix? Skip Conductor. Big feature? Full flow.

---

### For Humans: What to Remember

1. **Start with `bd ready`** — See what's actually unblocked
2. **Use `bs` when uncertain** — Brainstorm before committing
3. **Skip Conductor if you have a plan** — Go straight to `fb`
4. **Commit before leaving** — `git add -A && git commit && git push`

---

### Trigger Phrase Cheatsheet

| Phrase | Skill Activated |
|--------|-----------------|
| `bs`, `brainstorm` | brainstorming |
| `tdd` | test-driven-development |
| `trace`, `find source` | root-cause-tracing |
| `flaky`, `race condition` | condition-based-waiting |
| `fb`, `file beads` | beads/file-beads |
| `rb`, `review beads` | beads/review-beads |
| `dispatch` | dispatching-parallel-agents |
| `write skill` | writing-skills |
| `share skill` | sharing-skills |
| `review code` | requesting-code-review |
| `doc-sync`, `/doc-sync` | doc-sync |
| `/conductor-setup` | conductor (setup) |
| `/conductor-newtrack` | conductor (new track) |
| `/conductor-status` | conductor (status) |
| `init`, `claim`, `done` | beads-village (multi-agent) |
| `/ground` | grounding (context alignment) |
| `/decompose-task` | task decomposition |

---

*Built on foundations from [superpowers](https://github.com/obra/superpowers), [conductor](https://github.com/NguyenSiTrung/conductor), [beads](https://github.com/steveyegge/beads), and [Knowledge & Vibes](https://github.com/kyleobrien91/knowledge-and-vibes).*
