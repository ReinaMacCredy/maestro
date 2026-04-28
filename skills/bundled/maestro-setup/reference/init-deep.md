<!-- Vendored from upstream init-deep skill. Edit upstream first, then re-vendor. -->

# Init Deep

Generate hierarchical AGENTS.md files. Root + complexity-scored subdirectories.

## Usage

```text
$init-deep                      # Update mode: modify existing + create new where warranted
$init-deep --create-new         # Read existing -> remove all -> regenerate from scratch
$init-deep --max-depth=2        # Limit directory depth (default: 3)
```

Treat `$init-deep` as the Codex skill equivalent of the upstream `/init-deep`
command.

---

## Workflow (High-Level)

1. **Discovery + Analysis** (concurrent)
   - Fire background explore agents immediately
   - Main session: bash structure + LSP codemap + read existing AGENTS.md
2. **Score & Decide** - Determine AGENTS.md locations from merged findings
3. **Generate** - Root first, then subdirs in parallel
4. **Review** - Deduplicate, trim, validate

## Critical

Maintain a task list for all phases. Mark `in_progress` -> `completed` in
real time.

Use these phases:

- discovery
- scoring
- generate
- review

---

## Phase 1: Discovery + Analysis (Concurrent)

Mark `discovery` as `in_progress`.

### Fire Background Explore Agents Immediately

Do not wait. These run while the main session works.

Launch all of these immediately as read-only explorer-style agents. Prefer
`Codex Spark` for them when the runtime supports it.

1. Explore project structure
   Prompt:
   `Project structure: PREDICT standard patterns for detected language -> REPORT deviations only`

2. Find entry points
   Prompt:
   `Entry points: FIND main files -> REPORT non-standard organization`

3. Find conventions
   Prompt:
   `Conventions: FIND config files (.eslintrc, pyproject.toml, .editorconfig) -> REPORT project-specific rules`

4. Find anti-patterns
   Prompt:
   `Anti-patterns: FIND 'DO NOT', 'NEVER', 'ALWAYS', 'DEPRECATED' comments -> LIST forbidden patterns`

5. Explore build and CI
   Prompt:
   `Build/CI: FIND .github/workflows, Makefile -> REPORT non-standard patterns`

6. Find test patterns
   Prompt:
   `Test patterns: FIND test configs, test structure -> REPORT unique conventions`

### Dynamic Agent Spawning

After the bash analysis, spawn additional explore agents based on project scale.

| Factor | Threshold | Additional Agents |
|--------|-----------|-------------------|
| Total files | >100 | +1 per 100 files |
| Total lines | >10k | +1 per 10k lines |
| Directory depth | >=4 | +2 for deep exploration |
| Large files (>500 lines) | >10 files | +1 for complexity hotspots |
| Monorepo | detected | +1 per package/workspace |
| Multiple languages | >1 | +1 per language |

Measure scale first:

```bash
total_files=$(find . -type f -not -path '*/node_modules/*' -not -path '*/.git/*' | wc -l)
total_lines=$(find . -type f \( -name "*.ts" -o -name "*.py" -o -name "*.go" \) -not -path '*/node_modules/*' -exec wc -l {} + 2>/dev/null | tail -1 | awk '{print $1}')
large_files=$(find . -type f \( -name "*.ts" -o -name "*.py" \) -not -path '*/node_modules/*' -exec wc -l {} + 2>/dev/null | awk '$1 > 500 {count++} END {print count+0}')
max_depth=$(find . -type d -not -path '*/node_modules/*' -not -path '*/.git/*' | awk -F/ '{print NF}' | sort -rn | head -1)
```

Examples of extra agent prompts:

- `Large file analysis: FIND files >500 lines, REPORT complexity hotspots`
- `Deep modules at depth 4+: FIND hidden patterns, internal conventions`
- `Cross-cutting concerns: FIND shared utilities across directories`

### Main Session: Concurrent Analysis

While background agents run, the main session does:

#### 1. Bash Structural Analysis

```bash
# Directory depth + file counts
find . -type d -not -path '*/\.*' -not -path '*/node_modules/*' -not -path '*/venv/*' -not -path '*/dist/*' -not -path '*/build/*' | awk -F/ '{print NF-1}' | sort -n | uniq -c

# Files per directory (top 30)
find . -type f -not -path '*/\.*' -not -path '*/node_modules/*' | sed 's|/[^/]*$||' | sort | uniq -c | sort -rn | head -30

# Code concentration by extension
find . -type f \( -name "*.py" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.go" -o -name "*.rs" \) -not -path '*/node_modules/*' | sed 's|/[^/]*$||' | sort | uniq -c | sort -rn | head -20

# Existing AGENTS.md / CLAUDE.md
find . -type f \( -name "AGENTS.md" -o -name "CLAUDE.md" \) -not -path '*/node_modules/*' 2>/dev/null
```

#### 2. Read Existing AGENTS.md

For each existing file found:

- read it
- extract key insights, conventions, and anti-patterns
- keep those findings in working notes

If `--create-new`: read all existing first, preserve the useful context, then
remove and regenerate.

#### 3. LSP Codemap (if available)

Use LSP when available:

- inspect likely entrypoints such as `src/index.ts` or `main.py`
- search for key symbols such as `class`, `interface`, and `function`
- check reference centrality for top exports

If LSP is unavailable, rely on explore agents plus code inspection.

### Collect Background Results

After the main-session analysis is done, collect all explore-agent results and
merge:

- bash findings
- LSP findings
- existing AGENTS findings
- explorer findings

Mark `discovery` as `completed`.

---

## Phase 2: Scoring & Location Decision

Mark `scoring` as `in_progress`.

### Scoring Matrix

| Factor | Weight | High Threshold | Source |
|--------|--------|----------------|--------|
| File count | 3x | >20 | bash |
| Subdir count | 2x | >5 | bash |
| Code ratio | 2x | >70% | bash |
| Unique patterns | 1x | Has own config | explore |
| Module boundary | 2x | Has index.ts or `__init__.py` | bash |
| Symbol density | 2x | >30 symbols | LSP |
| Export count | 2x | >10 exports | LSP |
| Reference centrality | 3x | >20 refs | LSP |

### Decision Rules

| Score | Action |
|-------|--------|
| Root (`.`) | ALWAYS create |
| >15 | Create AGENTS.md |
| 8-15 | Create if distinct domain |
| <8 | Skip (parent covers) |

### Output

Produce a concrete location list before writing:

```text
AGENTS_LOCATIONS = [
  { path: ".", type: "root" },
  { path: "src/hooks", score: 18, reason: "high complexity" },
  { path: "src/api", score: 12, reason: "distinct domain" }
]
```

Mark `scoring` as `completed`.

---

## Phase 3: Generate AGENTS.md

Mark `generate` as `in_progress`.

## Critical File Writing Rule

If `AGENTS.md` already exists at the target path:

- edit it in place
- preserve useful handwritten content unless the user explicitly asked for
  `create-new`

If it does not exist:

- create a new file

Never blindly overwrite an existing file.

### Root AGENTS.md (Full Treatment)

```markdown
# PROJECT KNOWLEDGE BASE

**Generated:** {TIMESTAMP}
**Commit:** {SHORT_SHA}
**Branch:** {BRANCH}

## OVERVIEW
{1-2 sentences: what + core stack}

## STRUCTURE
```text
{root}/
├── {dir}/    # {non-obvious purpose only}
└── {entry}
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|

## CODE MAP
{From LSP - skip if unavailable or project <10 files}

| Symbol | Type | Location | Refs | Role |
|--------|------|----------|------|------|

## CONVENTIONS
{ONLY deviations from standard}

## ANTI-PATTERNS (THIS PROJECT)
{Explicitly forbidden here}

## UNIQUE STYLES
{Project-specific}

## COMMANDS
```bash
{dev/test/build}
```

## NOTES
{Gotchas}
```

Quality gates:

- 50 to 150 lines
- no generic advice
- no obvious info the agent can infer directly from the tree

### Subdirectory AGENTS.md (Parallel)

Generate subdirectory files in parallel after the location list is finalized.

For each child location:

- generate 30 to 80 lines max
- never repeat parent content
- include:
  - `OVERVIEW` (1 line)
  - `STRUCTURE` (if more than 5 subdirs)
  - `WHERE TO LOOK`
  - `CONVENTIONS` (if different from parent)
  - `ANTI-PATTERNS` (if local and real)
  - explicit parent and child hierarchy block

Wait for all child generations to finish.

Mark `generate` as `completed`.

---

## Phase 4: Review & Deduplicate

Mark `review` as `in_progress`.

For each generated file:

- remove generic advice
- remove parent duplicates
- trim to the size limits
- verify telegraphic style

### Add the hierarchy block manually

Use one managed-style block per file:

```md
<!-- AGENTS-HIERARCHY:START -->
## AGENTS Hierarchy
Parent:
- [../AGENTS.md](../AGENTS.md)

Children:
- [packages/ui/AGENTS.md](packages/ui/AGENTS.md)
- [packages/api/AGENTS.md](packages/api/AGENTS.md)

Managed by `init-deep`. Edit outside this block.
<!-- AGENTS-HIERARCHY:END -->
```

For a root file, use:

- `Parent:`
- `- none (root)`

If there are no children, use:

- `Children:`
- `- none`

There is no helper script for this. Update the links directly in the file
body.

### Validate the hierarchy

Spot-check the final hierarchy:

- root index points at every included child `AGENTS.md`
- each child points to its nearest parent
- nested external projects are separate subtrees
- no ignored or generated folder got an `AGENTS.md` by accident
- no child just restates the parent

Mark `review` as `completed`.

---

## Final Report

Use this exact structure:

```text
=== init-deep Complete ===

Mode: {update | create-new}

Files:
  [OK] ./AGENTS.md (root, {N} lines)
  [OK] ./src/hooks/AGENTS.md ({N} lines)

Dirs Analyzed: {N}
AGENTS.md Created: {N}
AGENTS.md Updated: {N}

Hierarchy:
  ./AGENTS.md
  └── src/hooks/AGENTS.md
```

---

## Anti-Patterns

- Static agent count: vary agents based on project size and depth
- Sequential execution: discovery should parallelize explore agents with the
  main analysis
- Ignoring existing files: always read existing AGENTS.md first, even with
  `create-new`
- Over-documenting: not every directory needs AGENTS.md
- Redundancy: child never repeats parent
- Generic content: remove anything that applies to all projects
- Verbose style: telegraphic or die
