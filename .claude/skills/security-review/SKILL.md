---
name: security-review
description: Deep security analysis delegated to the security-reviewer agent. Checks auth, injection, secrets, dependencies, and reports with severity ratings.
argument-hint: "[<files or feature area> | --diff [range]]"
allowed-tools: Read, Grep, Glob, Bash, Task, TeamCreate, TeamDelete, SendMessage, AskUserQuestion
disable-model-invocation: true
---

# Security Review — Delegated Security Analysis

> Spawn a `security-reviewer` agent to perform deep security analysis of code. Reports findings with severity ratings and file:line evidence. Read-only — no code changes.

## Arguments

- `<files or feature area>` — Specific files or area to review (e.g., `src/auth/`, `api routes`)
- `--diff [range]` — Review recent changes (default: `HEAD~1..HEAD`)
- No argument — Review all uncommitted and staged changes

## Hard Rules

- **Read-only**: This skill does NOT modify code. It produces a report.
- **Delegate to agent**: All security analysis is performed by the `security-reviewer` agent.
- **Evidence required**: Every finding must reference a specific file and line number.

## Workflow

### Step 1: Determine Scope

Based on the argument:

**Specific files/area:**
1. Resolve the paths via Glob
2. Pass the file list to the reviewer

**`--diff [range]`:**
1. Run `git diff {range} --name-only` to get changed files
2. Run `git diff {range}` to get the full diff
3. Pass both to the reviewer

**No argument:**
1. Check for uncommitted changes: `git diff --name-only` and `git diff --cached --name-only`
2. If no changes: `git diff HEAD~1..HEAD --name-only`
3. Pass changed files to the reviewer

Record the scope for the report header.

### Step 2: Create Team and Delegate

```
TeamCreate(team_name: "security-review", description: "Security analysis of {scope description}")
```

Spawn the `security-reviewer` agent with a task describing:
- Files to review (full paths)
- Diff output (if applicable)
- Any specific concerns from the user's request

Wait for the agent to complete its review and produce a structured report.

### Step 3: Dependency Audit

Run ecosystem-specific audit in parallel with the agent review:

```bash
# JavaScript/TypeScript
if [[ -f "package.json" ]]; then
  bun audit 2>/dev/null || npm audit 2>/dev/null || echo "SKIP: No audit tool available"
fi

# Python
if [[ -f "requirements.txt" ]] || [[ -f "pyproject.toml" ]]; then
  pip audit 2>/dev/null || echo "SKIP: pip-audit not installed"
fi

# Go
if [[ -f "go.mod" ]]; then
  govulncheck ./... 2>/dev/null || echo "SKIP: govulncheck not installed"
fi

# Other ecosystems
echo "SKIP: No supported audit tool detected"
```

### Step 4: Synthesize Report

Combine the agent's findings with dependency audit results into a unified report:

```markdown
## Security Review Report

**Scope**: `{scope description}`
**Reviewed**: {current date}

---

### Verdict: SECURE | CONCERNS | CRITICAL

### Severity Summary
- Critical: N
- High: N
- Medium: N
- Low: N

### Findings

#### [CRITICAL|HIGH|MEDIUM|LOW] Finding Title
- **File**: `path/to/file.ts:42`
- **Category**: [Authentication|Authorization|Injection|Secrets|Dependencies|Data Exposure|Configuration]
- **Description**: What the vulnerability is
- **Impact**: What an attacker could do
- **Recommendation**: How to fix it
- **Evidence**: Relevant code snippet or pattern

### Dependency Audit
- Status: [PASS|CONCERNS|SKIP]
- Details: [audit output summary]

### Files Reviewed
- `path/to/file.ts` — [summary]

### Recommendations Priority
1. [Most critical fix first]
2. [Next priority]
```

### Step 5: Cleanup

```
TeamDelete(reason: "Security review complete")
```

**TeamDelete cleanup**: If TeamDelete fails, fall back to: `rm -rf ~/.claude/teams/{team-name} ~/.claude/tasks/{team-name}`

## Severity Definitions

| Severity | Criteria |
|----------|----------|
| **Critical** | Exploitable vulnerability with high impact (RCE, auth bypass, data breach) |
| **High** | Significant vulnerability requiring attacker effort (stored XSS, SQL injection with limited scope) |
| **Medium** | Vulnerability with mitigating factors (reflected XSS, information disclosure) |
| **Low** | Best practice violation or hardening opportunity (missing headers, verbose errors) |

## When to Use

- Before merging a PR with auth/security changes
- After adding new API endpoints or user input handling
- Periodic security audit of critical modules
- After adding new dependencies

## When NOT to Use

- For code quality review → use `/review` instead
- For test coverage analysis → use `/review` or `/ultraqa`
- If you need code changes → this is read-only; fix issues manually after review
