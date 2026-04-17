# Environment Detection and Project State

This reference covers how `maestro:symphony-setup` detects the project state, identifies existing configurations, and handles brownfield (existing setup) vs fresh installations.

## Project State Classification

### Detection Commands

Run these checks at the start of setup to classify the project state:

```bash
# Existing Codex skills
CODEX_SKILLS=$(ls .codex/skills/*/SKILL.md 2>/dev/null | wc -l | tr -d ' ')

# Existing workflow file
WORKFLOW_EXISTS=$([ -f WORKFLOW.md ] && echo "yes" || echo "no")

# Existing Codex config (legacy or current)
CODEX_MD=$([ -f CODEX.md ] || [ -f .codex/CODEX.md ] && echo "yes" || echo "no")

# Agent configs
CLAUDE_MD=$([ -f CLAUDE.md ] && echo "yes" || echo "no")
AGENTS_MD=$([ -f AGENTS.md ] && echo "yes" || echo "no")

# Git state
GIT_REPO=$(git rev-parse --is-inside-work-tree 2>/dev/null && echo "yes" || echo "no")
COMMIT_COUNT=$(git rev-list --count HEAD 2>/dev/null || echo "0")
```

### State Matrix

| Codex Skills | WORKFLOW.md | CODEX.md | Classification | Action |
|-------------|-------------|----------|---------------|--------|
| 0 | No | No | **Fresh** | Full setup (Steps 1-8) |
| 0 | No | Yes | **Foreign config** | Migrate CODEX.md rules to WORKFLOW.md |
| 0 | Yes | No | **Workflow-only** | Install skills, verify workflow |
| 0 | Yes | Yes | **Partial (workflow)** | Install skills, merge CODEX.md into WORKFLOW.md |
| 1-5 | No | * | **Partial (skills)** | Install missing skills, generate WORKFLOW.md |
| 6 | No | * | **Skills-only** | Generate WORKFLOW.md |
| 1-5 | Yes | * | **Partial (both)** | Install missing skills, verify workflow |
| 6 | Yes | No | **Complete** | Verify and report |
| 6 | Yes | Yes | **Complete + legacy** | Verify, offer to remove CODEX.md |

### User Prompts by State

**Fresh**:
```
No existing Symphony configuration detected. Proceeding with full setup.
```

**Partial**:
```
Partial Symphony configuration detected:
- Codex skills: {N}/6 installed ({list of installed})
- WORKFLOW.md: {exists|missing}
- CODEX.md: {exists|not found}

Options:
1. Complete setup -- install missing components, preserve existing
2. Full reset -- remove existing and start fresh
3. Cancel
```

**Complete**:
```
Symphony is already configured:
- All 6 Codex skills installed
- WORKFLOW.md exists

Options:
1. Verify -- run verification checks only
2. Update -- regenerate WORKFLOW.md (preserves skills)
3. Reset -- remove everything and start fresh
4. Cancel
```

## Existing Configuration Migration

### CODEX.md to WORKFLOW.md

If `CODEX.md` exists (at root or `.codex/CODEX.md`), extract relevant content for WORKFLOW.md:

| CODEX.md content | Maps to WORKFLOW.md section |
|-----------------|---------------------------|
| Project description | `{{PROJECT_DESCRIPTION}}` |
| Build/test commands | `{{BUILD_AND_TEST_COMMANDS}}` |
| Coding rules/conventions | `{{PROJECT_RULES}}` |
| Tool restrictions | Project rules section |
| Environment setup | Build commands section |

**Migration steps**:
1. Read CODEX.md content
2. Classify each section (description, commands, rules, other)
3. Map classified content to WORKFLOW.md placeholders
4. Show the mapping to the user for confirmation
5. Generate WORKFLOW.md with the migrated content
6. Offer to archive CODEX.md (rename to `CODEX.md.bak`)

### CLAUDE.md / AGENTS.md Extraction

These files are not replaced -- they coexist with WORKFLOW.md. Extract relevant information for WORKFLOW.md generation:

| Source | What to extract |
|--------|----------------|
| `CLAUDE.md` | Build commands, test commands, package manager preference, coding rules |
| `AGENTS.md` | Agent-specific restrictions, tool usage patterns, workflow phases |

**Do not duplicate**: If a rule exists in CLAUDE.md and would also go into WORKFLOW.md, reference it rather than copying. The agent reads both files.

### Existing Codex Skills

When `.codex/skills/` already has files:

**Inventory check**:
```bash
# List all existing skill directories
ls -d .codex/skills/*/ 2>/dev/null

# Compare against expected Symphony set
for skill in commit debug land linear pull push; do
  if [ -f ".codex/skills/$skill/SKILL.md" ]; then
    echo "[exists] $skill"
  else
    echo "[missing] $skill"
  fi
done
```

**Content comparison for existing skills**:
```bash
# Check if an existing skill matches the Symphony template
diff .codex/skills/commit/SKILL.md reference/codex-skills/commit/SKILL.md 2>/dev/null
```

**Conflict resolution rules**:
1. **Identical** -- skip (no action needed)
2. **Modified** -- ask user: overwrite, keep, or show diff
3. **Extra skills** (not in Symphony set) -- preserve, report in summary
4. **Missing skills** -- install from templates

## Tech Stack Detection

Detect the project's tech stack from manifest files to auto-populate build commands.

### Detection Heuristics

| File | Tech Stack | Default Build Commands |
|------|-----------|----------------------|
| `package.json` + `bun.lockb` | TypeScript/Bun | `bun install`, `bun run build`, `bun test`, `bun run lint` |
| `package.json` + `package-lock.json` | TypeScript/Node (npm) | `npm install`, `npm run build`, `npm test`, `npm run lint` |
| `package.json` + `yarn.lock` | TypeScript/Node (yarn) | `yarn install`, `yarn build`, `yarn test`, `yarn lint` |
| `package.json` + `pnpm-lock.yaml` | TypeScript/Node (pnpm) | `pnpm install`, `pnpm build`, `pnpm test`, `pnpm lint` |
| `Cargo.toml` | Rust | `cargo build`, `cargo test`, `cargo clippy` |
| `pyproject.toml` + `uv.lock` | Python (uv) | `uv sync`, `uv run pytest`, `uv run ruff check` |
| `pyproject.toml` + `poetry.lock` | Python (poetry) | `poetry install`, `poetry run pytest`, `poetry run ruff check` |
| `requirements.txt` | Python (pip) | `pip install -r requirements.txt`, `pytest`, `ruff check` |
| `go.mod` | Go | `go build ./...`, `go test ./...`, `golangci-lint run` |
| `mix.exs` | Elixir | `mix deps.get`, `mix compile`, `mix test`, `mix credo` |
| `build.gradle` / `build.gradle.kts` | Java/Kotlin (Gradle) | `./gradlew build`, `./gradlew test` |
| `pom.xml` | Java (Maven) | `./mvnw compile`, `./mvnw test` |
| `Gemfile` | Ruby | `bundle install`, `bundle exec rake test`, `bundle exec rubocop` |
| `composer.json` | PHP | `composer install`, `composer test`, `./vendor/bin/phpstan` |
| `Package.swift` | Swift | `swift build`, `swift test` |
| `Makefile` | Any (custom) | Parse targets: `make build`, `make test`, `make lint` |

### Multi-stack detection

Projects may use multiple tech stacks. Detect all present manifests and merge their commands:

```bash
# Check for common manifest files
for f in package.json Cargo.toml pyproject.toml go.mod mix.exs build.gradle pom.xml Gemfile; do
  [ -f "$f" ] && echo "[found] $f"
done
```

### Package.json script extraction

For JavaScript/TypeScript projects, read available scripts:

```bash
# Extract script names from package.json
cat package.json | python3 -c "import json,sys; scripts=json.load(sys.stdin).get('scripts',{}); [print(f'  - bun run {k}') for k in scripts]"
```

Use detected scripts instead of defaults when available. Prioritize:
- `build` or `compile` script
- `test` or `test:unit` script
- `lint` or `check` script
- `typecheck` or `tsc` script

## OS and Shell Compatibility

### Command differences by OS

| Operation | macOS (Darwin) | Linux |
|-----------|---------------|-------|
| Open URL | `open <url>` | `xdg-open <url>` |
| Clipboard copy | `pbcopy` | `xclip -selection clipboard` |
| File watcher | `fswatch` | `inotifywait` |
| Process list | `ps aux` | `ps aux` (same) |

### Shell considerations

| Shell | Config file | Notes |
|-------|------------|-------|
| zsh | `~/.zshrc` | Default on macOS since Catalina |
| bash | `~/.bashrc` / `~/.bash_profile` | Default on most Linux |
| fish | `~/.config/fish/config.fish` | Non-POSIX syntax |

Environment variables (LINEAR_API_KEY, OPENAI_API_KEY) should be set in the user's shell config file. Suggest the correct file based on detected shell:

```bash
SHELL_NAME=$(basename "$SHELL")
case "$SHELL_NAME" in
  zsh)  echo "Add to ~/.zshrc:" ;;
  bash) echo "Add to ~/.bashrc:" ;;
  fish) echo "Add to ~/.config/fish/config.fish: (use 'set -x' syntax)" ;;
esac
```
