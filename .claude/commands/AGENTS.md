# .claude/commands/

## Purpose

User-invocable commands for the Atlas workflow system. Commands are markdown files with YAML frontmatter that Claude Code loads when users type `/command-name`. They define allowed tools, arguments, and workflow instructions for specific Atlas operations.

## Key Files

| File | Purpose | Triggers |
|------|---------|----------|
| `atlas-plan.md` | Interview-driven planning (main context does interview) | `/atlas-plan <request>` |
| `atlas-work.md` | Execute plan in orchestrator mode | `/atlas-work [plan] [ulw]` |
| `ralph-loop.md` | Autonomous execution until completion | `/ralph-loop [--max N] [--ultrawork]` |
| `cancel-ralph.md` | Stop Ralph loop | `/cancel-ralph` |
| `refactor.md` | Intelligent refactoring with TDD | `/refactor` |
| `init-deep.md` | Generate hierarchical AGENTS.md files | `/init-deep` |
| `codex-plan.md` | Plan generation with Codex enhancement | `/codex-plan <request>` |

## Command Structure

All commands follow this YAML frontmatter format:

```yaml
---
description: Brief description for command palette
argument-hint: "<what user should provide>"
allowed-tools: [Tool1, Tool2, ...]
---
```

Followed by markdown documentation:

1. **Usage** - Command syntax and options
2. **How It Works** - Step-by-step explanation
3. **Workflow** - Detailed phase-by-phase instructions
4. **References** - Links to related skills/agents
5. **Quick Reference** - Tables and diagrams

## Command Details

### atlas-plan.md

**Interview-driven planning workflow executed in main context.**

**Phases:**
1. **Interview** (main context) - Domain detection, structured questions, codex choice
2. **Generate** - Spawn prometheus with GENERATE
3. **Review** - Metis gap analysis → Momus approval loop (max 5 iterations)
4. **Enhance** (optional) - Codex enhancement if user chose "enhanced"
5. **File Tasks** - Parse TODOs, create TaskCreate entries
6. **Exit** - ExitPlanMode()

**Key innovation:** Main context conducts interview, then spawns prometheus only for plan writing. This avoids double-spawning and keeps interview in user's session.

**Allowed tools:**
- Read, Glob, Grep - Codebase exploration
- Task - Spawn agents (prometheus, metis, momus)
- AskUserQuestion - Interview user
- EnterPlanMode, ExitPlanMode - Plan mode lifecycle
- Write, Edit - Update pipeline state, fix plans

**State management:**
- Creates `.atlas/pipeline-state.json` with `codex_choice`, `plan_mode_active`
- Transitions through phases: PLANNING → GAP_ANALYSIS → MOMUS_REVIEW → COMPLETE

### atlas-work.md

**Execute plan in Orchestrator+ mode.**

**Flow:**
1. Hook detects `/atlas-work` keyword
2. Hook injects plan content and orchestrator context
3. Main session loads orchestration skill
4. Session adopts orchestrator behavior (delegates via Task)

**Key innovation:** No subagent spawning - the main session becomes the orchestrator. This allows continuous user interaction during execution.

**Allowed tools:**
- Task - Delegate to implementing agents (leviathan, kraken, spark)
- TodoWrite - Track session-local tasks
- Read - Verify work and gather context

**State management:**
- Initializes `.atlas/boulder.json` with `active: true`, `agent: "orchestrator"`
- Tracks progress and wisdom accumulation

**Options:**
- `[plan-name]` - Execute specific plan (default: most recent)
- `[ulw]` or `[ultrawork]` - Enable high-priority thoroughness mode

### ralph-loop.md

**Autonomous execution loop until completion.**

**Flow:**
1. Run `./scripts/start-ralph.sh` to initialize state
2. Load orchestration skill
3. Execute plan tasks autonomously
4. Self-chain handler continues loop after each session end
5. Stop when `<promise>DONE</promise>` detected or max iterations reached

**Key innovation:** Self-chaining via Stop hook enables continuous autonomous work without manual restarts.

**Allowed tools:**
- Bash - Run start-ralph.sh
- Read, Write, Edit - State management
- Task - Delegate work
- TaskCreate, TaskUpdate - Track tasks

**State management:**
- Creates `.atlas/ralph-loop.local.md` with iteration count, promise text
- Self-chain handler checks state on Stop event

**Options:**
- `--max N` - Limit iterations (default: 100)
- `--promise TEXT` - Custom completion promise (default: "DONE")
- `--ultrawork` - Enable maximum thoroughness

### cancel-ralph.md

**Stop Ralph loop immediately.**

**Flow:**
1. Run `./scripts/cancel-ralph.sh`
2. Script clears `.atlas/ralph-loop.local.md`
3. Self-chain handler skips continuation

**Simple command, no complex workflow.**

### refactor.md

**Intelligent refactoring with TDD.**

**Flow:**
1. Identify refactor target (file, function, module)
2. Write characterization tests (capture current behavior)
3. Spawn atlas-kraken for TDD refactoring
4. Verify tests still pass
5. Document changes

**Spawns:** atlas-kraken (TDD specialist)

### init-deep.md

**Generate hierarchical AGENTS.md files.**

**Flow:**
1. Analyze directory structure
2. For each major directory, create AGENTS.md with:
   - Purpose
   - Key files
   - Patterns
   - Dependencies
   - Notes for AI agents
3. Recurse into depth-2 subdirectories

**Spawns:** atlas-document-writer

### codex-plan.md

**Plan generation with Codex enhancement (legacy).**

**Note:** Replaced by `atlas-plan.md` with optional Codex enhancement during review phase. Kept for backward compatibility.

## Patterns

### Command Invocation

**User types command:**
```
/atlas-plan create authentication system
```

**Claude Code:**
1. Loads `atlas-plan.md` into context
2. Sets `$ARGUMENTS` variable to "create authentication system"
3. Restricts tools to `allowed-tools` list
4. Executes command instructions

### Hook Integration

Many commands trigger hooks:

**Example: /atlas-work**
```
User types: /atlas-work my-plan ulw
    ↓
keyword-detector.sh hook fires (UserPromptSubmit)
    ↓
Hook injects plan content + orchestrator spawn instructions
    ↓
Command instructions tell Claude to load orchestration skill
    ↓
Session becomes Orchestrator+
```

### State Transitions

Commands manage state files:

**atlas-plan.md:**
- Writes `.atlas/pipeline-state.json` (codex_choice, plan_mode_active)
- Prometheus writes `.claude/plans/{name}.md`

**atlas-work.md:**
- Hook writes `.atlas/boulder.json` (active, agent, plan, started)

**ralph-loop.md:**
- Writes `.atlas/ralph-loop.local.md` (active, iterations, promise)

### Skill Loading

Commands can load skills:

```markdown
## After Hook Injection

When you see `[EXECUTION MODE]` in the injected context:

1. **Load the orchestration skill**: Read `skills/atlas/references/agents/atlas-orchestrator.md`
2. **You ARE now the Orchestrator+**: The skill transforms this session
```

## Dependencies

### Internal

- **`skills/atlas/SKILL.md`** - Component registry, chaining rules
- **`skills/orchestration/SKILL.md`** - Orchestration patterns
- **`skills/atlas/references/agents/*.md`** - Agent definitions
- **`scripts/keyword-detector.sh`** - Detects commands in prompts
- **`scripts/start-ralph.sh`, `scripts/cancel-ralph.sh`** - Ralph loop control

### External

- **Claude Code command system** - Loads markdown files when user types `/command`
- **YAML frontmatter parser** - Extracts metadata (description, allowed-tools)

## Notes for AI Agents

### Critical Rules

1. **Commands are for users** - Users type `/command`, not agents
2. **Allowed tools are enforced** - Claude Code blocks disallowed tools
3. **Arguments via $ARGUMENTS** - Commands receive user input as variable
4. **State management matters** - Commands create/update state files
5. **Hooks can intercept** - Commands may trigger hook behaviors

### Command Lifecycle

**Load time:**
- User types `/command` in chat
- Claude Code searches `.claude/commands/` for `command.md`
- Loads markdown into context as system message
- Parses YAML frontmatter
- Sets `$ARGUMENTS` variable
- Restricts tool access

**Execution:**
- Command instructions guide Claude's behavior
- Claude follows workflow phases
- Tools invoked as specified
- Hooks fire on tool use

**Completion:**
- Command finishes when instructions complete
- State files updated
- User receives results

### Writing New Commands

**Structure:**
```markdown
---
description: One-line description for command palette
argument-hint: "<what user provides>" # Optional
allowed-tools: [Tool1, Tool2, ...]
---

# Command Name

Brief intro.

## Usage

\```
/command [options] <arguments>
\```

## How It Works

1. Step 1
2. Step 2
3. ...

## Workflow

### Phase 1: ...

Details...

## References

- [Related doc](path/to/doc.md)
```

**Best practices:**
- Clear usage section with examples
- Phase-by-phase workflow
- Error handling guidance
- Links to related skills/agents
- Quick reference tables

### Testing Commands

**Manual testing:**
1. Type `/command` in Claude Code
2. Observe context injection
3. Verify allowed tools
4. Check state file updates

**Integration testing:**
```bash
# Simulate command invocation
COMMAND_FILE=".claude/commands/atlas-plan.md"
ARGUMENTS="test request"

# Check hooks fire correctly
echo '{"prompt": "/atlas-plan test"}' | ./scripts/keyword-detector.sh | jq .
```

### Common Patterns

**Interview workflow:**
```markdown
## Phase 1: Interview

Ask user questions with AskUserQuestion:

\```javascript
AskUserQuestion({
  questions: [{
    question: "What is your goal?",
    // ...
  }]
})
\```
```

**Spawn agent:**
```markdown
## Phase 2: Generate

Spawn prometheus:

\```javascript
Task({
  subagent_type: "atlas-prometheus",
  prompt: "Generate plan for: $ARGUMENTS"
})
\```
```

**State management:**
```markdown
## Phase 3: Initialize State

\```javascript
Write({
  file_path: ".atlas/boulder.json",
  content: JSON.stringify({ active: true, ... })
})
\```
```

**Skill loading:**
```markdown
## Phase 4: Load Orchestration

Read and adopt patterns from:
\```
skills/orchestration/SKILL.md
\```
```

### Command Routing

**Keyword detection (in hooks):**
- `/atlas-plan` → keyword-detector.sh injects atlas planning context
- `/atlas-work` → keyword-detector.sh loads plan, initializes boulder
- `/ralph-loop` → starts autonomous loop

**Direct invocation (by user):**
- User types `/command`
- Claude Code loads command file directly
- No hook interception needed

### Argument Handling

**Simple arguments:**
```
/atlas-plan create user authentication
# $ARGUMENTS = "create user authentication"
```

**Options:**
```
/atlas-work my-plan ulw
# Parse options manually in command:
# - plan_name = "my-plan"
# - ultrawork = true (if "ulw" present)
```

**Flags:**
```
/ralph-loop --max 50 --ultrawork
# Parse flags manually:
# - --max 50 → max_iterations = 50
# - --ultrawork → ultrawork = true
```

### Error Handling

Commands should handle:

**Missing dependencies:**
```markdown
If plan file not found:
1. List available plans
2. Ask user to specify plan name
3. Suggest creating new plan with /atlas-plan
```

**Invalid state:**
```markdown
If boulder.json corrupted:
1. Run ./scripts/state-recover.sh
2. If recovery fails, re-initialize state
3. Log error for debugging
```

**Tool failures:**
```markdown
If EnterPlanMode fails:
1. Log error
2. Continue without plan mode (optional feature)
3. Note limitation in plan output
```

### Command Composition

Commands can chain:

```
User: /atlas-plan create API
  ↓
atlas-plan creates plan, files tasks
  ↓
User: /atlas-work api-plan
  ↓
atlas-work executes plan
  ↓
User: /ralph-loop --max 20
  ↓
ralph-loop continues autonomously
```

### Security Considerations

**Allowed tools:**
- Restrict to minimum needed tools
- Don't allow dangerous tools (e.g., Bash in read-only commands)

**State isolation:**
- Commands should only modify their own state files
- Don't accidentally overwrite other workflow states

**Input validation:**
- Sanitize user input in $ARGUMENTS
- Validate plan names (no path traversal)

### Debugging Commands

**Command not found:**
- Check file exists in `.claude/commands/`
- Check filename matches command (case-sensitive)

**Tools blocked:**
- Check `allowed-tools` in frontmatter
- Verify tool name spelling

**Wrong behavior:**
- Read full command markdown
- Check workflow phase instructions
- Enable ATLAS_DEBUG=1 for hook logs

**State issues:**
- Check `.atlas/` directory exists
- Validate state files with `scripts/lib/state_validate.py`
- Check hook logs: `.atlas/logs/hooks/{date}.log`

### Related Documentation

- **skills/atlas/SKILL.md** - Full Atlas workflow system
- **skills/orchestration/SKILL.md** - Orchestration patterns
- **hooks/AGENTS.md** - Hook system that intercepts commands
- **scripts/AGENTS.md** - Scripts called by commands
