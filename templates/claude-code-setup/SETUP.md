# Claude Code Setup Guide

Quick guide to configure Claude Code for your project using the templates in this directory.

## Directory Structure

After setup, your project will have:

```
your-project/
├── CLAUDE.md                    # Project context (auto-loaded)
├── AGENTS.md                    # Agent workflow instructions (auto-loaded)
└── .claude/
    ├── settings.local.json      # Permissions (optional)
    ├── rules/                   # Constraints (auto-loaded)
    │   └── safety.md
    └── skills/                  # Capabilities and workflows (on-demand)
        └── example-skill/
            └── SKILL.md
```

## Quick Start

### 1. Copy Templates

```bash
# From your project root
cp path/to/templates/claude-code-setup/CLAUDE.md ./CLAUDE.md
cp path/to/templates/claude-code-setup/AGENTS.md ./AGENTS.md
mkdir -p .claude/rules .claude/skills
cp path/to/templates/claude-code-setup/.claude/rules/safety.md .claude/rules/
```

### 2. Customize CLAUDE.md

Edit CLAUDE.md to reflect your project:
- [ ] Replace `[Project Name]` with actual name
- [ ] Update tech stack
- [ ] Add your key paths
- [ ] Add your commands (build, test, etc.)
- [ ] Draw your architecture

### 3. Customize AGENTS.md

Edit AGENTS.md for your workflow:
- [ ] Update project overview
- [ ] Add any project-specific safety rules
- [ ] List your skills and commands
- [ ] Add common tasks table

### 4. Add Safety Rules

The `safety.md` rule is included by default. Add more as needed:

```bash
# Example: TypeScript-specific rules
cat > .claude/rules/typescript.md << 'EOF'
---
paths: **/*.ts
---

# TypeScript Rules

- Strict mode, no `any` without justification
- Async/await for all promises
- Zod for runtime validation at boundaries
EOF
```

### 5. Commit

```bash
git add CLAUDE.md AGENTS.md .claude/
git commit -m "Add Claude Code configuration"
```

---

## Configuration Layers

| Layer | Location | Purpose | Loaded |
|-------|----------|---------|--------|
| **Memory** | `CLAUDE.md` | Project context | Auto at startup |
| **Workflow** | `AGENTS.md` | Agent instructions | Auto at startup |
| **Rules** | `.claude/rules/*.md` | Constraints | Auto (can filter by path) |
| **Skills** | `.claude/skills/*/SKILL.md` | Capabilities and workflows | When relevant |

---

## Rules

Rules are short constraints that always apply.

### Format

```markdown
---
paths: src/**/*.ts   # Optional: only apply to matching files
---

# Rule Title

Clear, direct instructions.
```

### Path Filtering

```yaml
# All TypeScript files
paths: **/*.ts

# Multiple patterns
paths:
  - src/**/*.ts
  - lib/**/*.ts

# No paths = applies everywhere
```

---

## Skills

Skills are detailed guides Claude discovers automatically.

### Format

```markdown
---
name: skill-name
description: What it does. When to use it.
---

# Skill Title

Detailed instructions...
```

### Key Points

- `name`: lowercase, hyphens, max 64 chars
- `description`: max 1024 chars - **critical for discovery**
- Optional `allowed-tools`: restrict which tools the skill can use

---

## Commands

Slash commands are user-triggered workflows.

### Format

```markdown
---
description: Brief description
allowed-tools: Read, Grep, Glob
argument-hint: <file-or-pattern>
---

# Command Instructions

Target: $ARGUMENTS

Do something with the arguments...
```

### Special Syntax

| Syntax | Purpose |
|--------|---------|
| `$ARGUMENTS` | All arguments passed |
| `$1, $2, $3` | Positional arguments |
| `!`backticks | Execute bash |
| `@path` | Include file contents |

---

## Tips

1. **Keep CLAUDE.md short** - It's loaded every session
2. **Rules for constraints** - Things that must always happen
3. **Skills for guidance** - Detailed how-to documentation
4. **Commands for workflows** - Multi-step procedures

---

## What Goes Where

| Content | Location | Why |
|---------|----------|-----|
| Project architecture | `CLAUDE.md` | Auto-loaded context |
| Build/test commands | `CLAUDE.md` | Quick reference |
| Session workflow | `AGENTS.md` | Agent behavior |
| Safety constraints | `.claude/rules/` | Always enforced |
| Coding conventions | `.claude/rules/` | Path-specific |
| Detailed workflows | `.claude/skills/` | Auto-discovered |
