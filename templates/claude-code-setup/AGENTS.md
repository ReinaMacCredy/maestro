# AGENTS.md

<!--
AGENTS.md - Agent workflow instructions (industry convention).
CLAUDE.md has project context; this file has workflow rules.
-->

## Project Overview

- **Name**: [Project Name]
- **Language**: [TypeScript/Python/Go/etc.]
- **Key Paths**: `src/`, `tests/`, `docs/`

---

## Safety Rules

### RULE 1 - File Deletion

You may NOT delete any file or directory unless the user explicitly gives the exact command **in this session**.

- This includes files you just created
- If you think something should be removed, stop and ask

### RULE 2 - Destructive Commands

Forbidden unless the user gives **exact command and explicit approval**:

- `git reset --hard`
- `git clean -fd`  
- `rm -rf`
- Any command that can delete or overwrite code/data

Before running destructive commands:
1. If unsure what it will delete, ask first
2. Prefer safe tools: `git status`, `git diff`, `git stash`
3. After approval, restate the command and list what it affects

---

## Code Editing Discipline

- Do **not** run scripts that bulk-modify code (codemods, giant sed/regex refactors)
- Large mechanical changes: break into smaller, explicit edits
- Subtle/complex changes: edit by hand, file-by-file

---

## Configuration Reference

| Location | Purpose | When to Use |
|----------|---------|-------------|
| `CLAUDE.md` | Project context, architecture | Understanding the codebase |
| `AGENTS.md` | This file - workflow instructions | Session startup, tool usage |
| `.claude/rules/` | Constraints and conventions | Auto-loaded, always follow |
| `.claude/skills/` | Detailed guides and capabilities | Reference when relevant |
| `.claude/commands/` | Slash commands | Invoke with `/command-name` |

### Rules (Auto-Loaded)

Rules in `.claude/rules/*.md` are automatically enforced:
- `safety.md` - File deletion, destructive commands

### Skills (On-Demand)

Skills in `.claude/skills/*/SKILL.md` provide detailed guidance:
- [List your skills here]

### Commands (User-Triggered)

Slash commands in `.claude/commands/*.md`:
- [List your commands here]

---

## Workflow

### Session Start

1. Read CLAUDE.md for project context
2. Check current git status
3. Understand the task before making changes

### During Work

1. Make incremental changes
2. Test after each change
3. Commit logical units of work

### Session End

1. Run tests and linting
2. Commit changes with descriptive messages
3. Summarize what was done

---

## Testing

- Write tests FIRST when implementing features
- Run tests after making changes
- Fix failing tests before moving on

```bash
# Run tests
pnpm test

# Run specific test
pnpm test path/to/test.ts
```

---

## Code Quality

Before committing:
```bash
pnpm lint          # Check for issues
pnpm typecheck     # Verify types
pnpm test          # Run tests
```

---

## Grounding External Dependencies

When using external libraries or APIs:

1. **Check current documentation** - training data may be outdated
2. **Verify patterns exist** - don't invent methods
3. **Use web search** for latest API changes

Truth sources:
- **Repo truth**: Grep, finder (how we do things here)
- **Web truth**: web_search, read_web_page (current external docs)
- **History truth**: Previous solutions in this codebase

---

## Common Tasks

| Task | How |
|------|-----|
| Add a feature | Write tests first, then implement, then refactor |
| Fix a bug | Reproduce first, then understand, then fix |
| Refactor | Ensure tests pass before and after |

---

## Contribution Policy

<!-- Customize as needed -->
[Your contribution guidelines here]
