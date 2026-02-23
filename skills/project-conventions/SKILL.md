---
name: project-conventions
description: Discovers host project conventions from CLAUDE.md, package.json, linter configs, and other configuration files. Use when agents need to understand the target project's coding standards.
user-invocable: false
---

# Project Conventions Discovery

> Discover and report the host project's conventions so agents follow existing patterns.

## When to Use

This skill is invoked automatically when agents need to understand the target project's conventions before making changes. It is NOT user-invocable.

## Discovery Process

### 1. Project Identity

Look for project configuration:
- `package.json` — name, scripts, dependencies, engines
- `pyproject.toml` / `setup.py` — Python project config
- `Cargo.toml` — Rust project config
- `go.mod` — Go module config
- `build.gradle` / `pom.xml` — Java/Kotlin project config

### 2. Claude Code Configuration

Read project-level Claude configuration:
- `CLAUDE.md` — Project instructions and conventions
- `.claude/settings.json` — Project settings
- `.claude/skills/` — Available skills and commands

### 3. Code Style

Check for linter and formatter configs:
- `.eslintrc*` / `eslint.config.*` — JavaScript/TypeScript linting
- `.prettierrc*` — Code formatting
- `biome.json` — Biome config
- `.editorconfig` — Editor settings
- `rustfmt.toml` — Rust formatting
- `.flake8` / `pyproject.toml [tool.ruff]` — Python linting

### 4. Testing Conventions

Identify test framework and patterns:
- Test file locations (`__tests__/`, `*.test.*`, `*.spec.*`, `tests/`)
- Test runner (`jest`, `vitest`, `pytest`, `cargo test`, `go test`)
- Coverage configuration

### 5. Build & CI

Check build and CI setup:
- `Makefile` / `Justfile` — Build commands
- `.github/workflows/` — CI pipelines
- `Dockerfile` / `docker-compose.yml` — Container config
- `tsconfig.json` — TypeScript configuration

## Output Format

```
## Project Conventions: {project-name}

### Language & Runtime
- Language: [language] [version]
- Runtime: [runtime] [version]
- Package manager: [manager]

### Code Style
- Formatter: [tool] ([config file])
- Linter: [tool] ([config file])
- Key rules: [notable rules]

### Testing
- Framework: [framework]
- Test location: [pattern]
- Run command: [command]

### Build
- Build command: [command]
- CI: [platform]

### Project-Specific Rules
- [Rules from CLAUDE.md that agents must follow]
```
