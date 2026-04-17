---
name: maestro:symphony-setup
description: "Set up Symphony orchestration for any repository. Installs Codex skills, generates a customized WORKFLOW.md, configures Linear integration, and verifies the setup. Works with any tech stack."
argument-hint: "<linear-project-slug> [repo-clone-url]"
---

> [!] DEPRECATED: Use `maestro symphony install --linear-project <slug>` instead.
> This skill will be removed in a future release.

# Symphony Setup -- Automated Project Onboarding

Set up OpenAI Symphony orchestration for the current repository so that Codex agents can autonomously pick up Linear issues, implement them, create PRs, and land them.

This skill handles:

- Environment and prerequisite validation
- Brownfield detection (existing configs, partial setups)
- Codex skill installation with conflict resolution
- WORKFLOW.md generation from project context
- Linear integration verification
- End-to-end setup validation

## Arguments

`$ARGUMENTS`

Two arguments:
1. **Linear project slug** (required) -- from the project URL (e.g., `my-project-abc123`). Right-click the project in Linear, copy URL, extract the slug after `/project/`.
2. **Repo clone URL** (optional) -- the git clone URL (e.g., `https://github.com/org/repo.git`). If omitted, detected from `git remote get-url origin`.

If the Linear project slug is missing, ask the user for it. Do not proceed without it.

---

## Step 1: Validate Prerequisites

Check each prerequisite individually. Report all failures at once (do not stop at the first).

### 1a: Environment detection

```bash
uname -s          # Darwin or Linux
echo "$SHELL"     # Shell type (zsh, bash)
git --version     # Git must be available
python3 --version # Required for land_watch.py
```

Record OS and shell for later command compatibility (e.g., `open` vs `xdg-open` for URLs).

### 1b: Required tools

| Tool | Check | Install guidance |
|------|-------|-----------------|
| `git` | `command -v git` | System package manager |
| `codex` | `command -v codex` | `npm install -g @openai/codex` |
| `python3` | `command -v python3` | System package manager (needed for `land_watch.py`) |
| `gh` | `command -v gh` | `brew install gh` or system package manager (needed for PR operations) |

### 1c: Required environment variables

| Variable | Check | How to set |
|----------|-------|------------|
| `LINEAR_API_KEY` | `[ -n "$LINEAR_API_KEY" ]` | Linear Settings --> API --> Personal API keys --> Create key |
| `OPENAI_API_KEY` | `[ -n "$OPENAI_API_KEY" ]` | Required by Codex CLI |

### 1d: Symphony service

Check if Symphony is available:

```bash
ls ~/Code/symphony/bin/symphony 2>/dev/null || echo "not found"
```

If not found, warn but do not block -- the user may run Symphony from a different location or start it later.

### 1e: Repository validation

Confirm the current directory is a git repository with a remote:

```bash
git rev-parse --is-inside-work-tree  # Must be true
git remote get-url origin            # Must return a URL
```

If no remote exists, ask the user for the repo clone URL (second argument).

### Step 1 output

Print a prerequisites summary:

```
Prerequisites Check
===================
[ok] git 2.43.0
[ok] codex 1.2.3
[ok] python3 3.12.1
[ok] gh 2.40.0
[ok] LINEAR_API_KEY set
[ok] OPENAI_API_KEY set
[!!] Symphony not found at ~/Code/symphony/ (non-blocking)
[ok] Git repository with remote: https://github.com/org/repo.git

{N} passed, {M} failed, {K} warnings
```

If any required check fails, print fix instructions and stop. Do not proceed with a broken environment.

---

## Step 2: Detect Project State

Determine whether this is a fresh setup or a brownfield (partial/existing configuration).

See `reference/environment-detection.md` for detection logic, indicators, and migration patterns.

### 2a: Check for existing Symphony/Codex configuration

```bash
# Existing Codex skills
ls .codex/skills/*/SKILL.md 2>/dev/null

# Existing workflow file
ls WORKFLOW.md 2>/dev/null

# Existing Codex config
ls CODEX.md .codex/CODEX.md 2>/dev/null

# Existing agent configs (may contain relevant rules)
ls CLAUDE.md AGENTS.md 2>/dev/null
```

### 2b: Classification and routing

| State | Indicators | Action |
|-------|-----------|--------|
| **Fresh** | No `.codex/`, no `WORKFLOW.md` | Proceed with full setup (Steps 3-8) |
| **Partial** | `.codex/skills/` exists but incomplete, or `WORKFLOW.md` missing | Offer to complete the setup -- install missing skills, generate missing files |
| **Complete** | All skills present + `WORKFLOW.md` exists | Ask: Update / Verify / Cancel |
| **Foreign config** | `CODEX.md` exists but no `.codex/skills/` | Offer migration: extract rules from `CODEX.md` into `WORKFLOW.md` |

For **Partial** and **Complete** states, ask the user before overwriting any existing files.

### 2c: Existing file inventory

If existing Codex skills are found, list them and compare against the expected set (commit, debug, land, linear, pull, push). Identify:

- Missing skills (will be installed)
- Extra skills (will be preserved)
- Modified skills (warn, ask before overwriting)

---

## Step 3: Gather Project Context

Read the project's existing documentation and configuration to populate the WORKFLOW.md template. This step produces a structured context object used in Step 5.

### 3a: Documentation sources

Read these files (whichever exist), in priority order:

| File | What to extract |
|------|----------------|
| `CLAUDE.md` | Coding conventions, build commands, project rules, tech stack |
| `AGENTS.md` | Agent-specific guidelines, tool restrictions |
| `README.md` | Project name, description, purpose, architecture overview |
| `CODEX.md` | Existing Codex configuration (migration source) |
| `package.json` / `Cargo.toml` / `pyproject.toml` / `go.mod` | Tech stack, dependencies, scripts |
| `.github/workflows/*.yml` | CI commands, test commands, lint commands |

### 3b: Git metadata

```bash
git remote get-url origin                    # Repo URL
git rev-parse --abbrev-ref HEAD              # Primary branch (main/master)
basename $(git rev-parse --show-toplevel)    # Project directory name
git log --oneline -5                         # Recent activity (brownfield indicator)
```

### 3c: Build/test command detection

Search for build and test commands in this priority order:

1. **Explicit in CLAUDE.md/AGENTS.md** -- highest priority, use as-is
2. **Package manifest scripts** -- `package.json` scripts, `Makefile` targets, `Cargo.toml` profiles
3. **CI configuration** -- GitHub Actions steps, commands in workflow files
4. **Convention** -- infer from tech stack (e.g., `bun test` for Bun projects, `cargo test` for Rust)

See `reference/environment-detection.md` for tech stack detection heuristics and default commands per stack.

### 3d: Structured output

Assemble the collected data into this structure (used in Step 5):

```
project_name:        <from README or directory name>
project_description: <2-3 sentences from README or ask user>
repo_url:            <from git remote or argument>
primary_branch:      <main or master>
build_commands:      <list of build/test/lint commands>
project_rules:       <list of coding rules and conventions>
tech_stack:          <languages, frameworks, tools>
```

If any required field cannot be determined, ask the user. Do not leave placeholders in the final output.

---

## Step 4: Install Codex Skills

Copy the Symphony skill templates into `.codex/skills/` in the target repository.

See `reference/codex-skills-guide.md` for detailed documentation on what each skill does, how they relate to each other, and customization patterns.

### 4a: What Codex skills are

Codex skills are markdown instruction files that the Codex CLI loads to guide agent behavior during specific operations. They live in `.codex/skills/<name>/SKILL.md` and are automatically available to the agent during Symphony sessions.

The Symphony skill set provides six skills that form an autonomous development workflow:

| Skill | Purpose | Depends on |
|-------|---------|-----------|
| `linear` | Query and update Linear issues via GraphQL | Symphony app-server (provides `linear_graphql` tool) |
| `commit` | Create well-formed git commits from staged changes | None |
| `push` | Push commits and create/update PRs | `commit` (creates the commits to push) |
| `land` | Monitor PR status and squash-merge when green | `push` (creates the PR), `linear` (updates issue status) |
| `pull` | Pull upstream changes and resolve merge conflicts | None |
| `debug` | Diagnose stuck Symphony/Codex runs via logs | None (diagnostic tool) |

### 4b: Install the skills

The templates are bundled at `reference/codex-skills/`. Create the directory structure and copy:

```bash
mkdir -p .codex/skills/{commit,debug,land,linear,pull,push}
```

Copy each skill file from `reference/codex-skills/` to `.codex/skills/`:

```
reference/codex-skills/commit/SKILL.md  -->  .codex/skills/commit/SKILL.md
reference/codex-skills/debug/SKILL.md   -->  .codex/skills/debug/SKILL.md
reference/codex-skills/land/SKILL.md    -->  .codex/skills/land/SKILL.md
reference/codex-skills/land/land_watch.py --> .codex/skills/land/land_watch.py
reference/codex-skills/linear/SKILL.md  -->  .codex/skills/linear/SKILL.md
reference/codex-skills/pull/SKILL.md    -->  .codex/skills/pull/SKILL.md
reference/codex-skills/push/SKILL.md    -->  .codex/skills/push/SKILL.md
```

### 4c: Conflict resolution

If `.codex/skills/` already has files:

1. **Identical content** -- skip silently
2. **Different content** -- show a diff summary and ask:
   - **Overwrite** -- replace with Symphony template
   - **Keep existing** -- preserve the current file
   - **Merge** -- show both versions side by side, let user choose sections
3. **Extra skills** (not in the Symphony set) -- preserve them, do not touch

### 4d: Make land_watch.py executable

```bash
chmod +x .codex/skills/land/land_watch.py
```

### 4e: Verification

```bash
# Confirm all expected files exist
ls .codex/skills/commit/SKILL.md \
   .codex/skills/debug/SKILL.md \
   .codex/skills/land/SKILL.md \
   .codex/skills/land/land_watch.py \
   .codex/skills/linear/SKILL.md \
   .codex/skills/pull/SKILL.md \
   .codex/skills/push/SKILL.md
```

Print install summary: `{N} skills installed, {M} skipped (existing), {K} conflicts resolved`.

---

## Step 5: Generate WORKFLOW.md

Use the template at `reference/WORKFLOW.md.template` as the base. Replace placeholders with the context gathered in Step 3.

See `reference/workflow-customization.md` for detailed guidance on each section and customization options.

### 5a: Placeholder replacement

| Placeholder | Source | Example |
|---|---|---|
| `{{PROJECT_SLUG}}` | First argument (Linear project slug) | `my-project-abc123` |
| `{{REPO_CLONE_URL}}` | Second argument or `git remote get-url origin` | `https://github.com/org/repo.git` |
| `{{PROJECT_NAME}}` | Step 3d `project_name` | `My Project` |
| `{{PROJECT_DESCRIPTION}}` | Step 3d `project_description` | 2-3 sentence summary of the project |
| `{{BUILD_AND_TEST_COMMANDS}}` | Step 3d `build_commands` | Formatted as a markdown list under a `Build and validation commands:` header |
| `{{PROJECT_RULES}}` | Step 3d `project_rules` | Formatted as a markdown list under a `Rules:` header |

### 5b: Quality checks before writing

Before writing WORKFLOW.md, verify:

1. No unreplaced `{{...}}` placeholders remain
2. The `project_description` is substantive (not just the project name repeated)
3. At least one build/test command is included
4. Project rules are formatted as a markdown list

If any check fails, ask the user to provide the missing information.

### 5c: Write the file

Write the result to `WORKFLOW.md` in the repository root.

If `WORKFLOW.md` already exists:
- Show a diff summary of changes
- Ask: **Overwrite** / **Backup and overwrite** (saves to `WORKFLOW.md.bak`) / **Cancel**

### 5d: Validate YAML front matter

The WORKFLOW.md template begins with YAML front matter. Confirm it parses correctly:

```bash
head -20 WORKFLOW.md | grep -c "^---"  # Should be 2 (opening and closing)
```

---

## Step 6: Configure Linear Integration

Verify the Linear project is accessible and has the required custom statuses.

See `reference/linear-setup.md` for detailed Linear configuration guidance.

### 6a: Verify Linear project access

Use the Linear MCP or API to confirm the project slug resolves:

```
Query: project by slug "{{PROJECT_SLUG}}"
Expected: project name, team, URL
```

If the project is not found, ask the user to verify the slug. Common issues:

- Slug includes extra URL path segments
- Project is in a different workspace
- API key lacks access to the project's team

### 6b: Check required custom statuses

Symphony requires three non-standard Linear issue statuses. Check if they exist in the team's workflow:

| Status name | Required state type | Purpose |
|-------------|-------------------|---------|
| `Rework` | Started | Agent needs to address review feedback |
| `Human Review` | Started | PR is ready for human review |
| `Merging` | Started | PR is approved, agent is merging |

Query the team's workflow states and check for each required status.

### 6c: Report and guide

If all statuses exist:

```
[ok] Linear project "{project_name}" accessible
[ok] Status "Rework" (started) exists
[ok] Status "Human Review" (started) exists
[ok] Status "Merging" (started) exists
```

If any are missing, provide step-by-step creation instructions:

```
[!!] Missing Linear statuses. Create them in Team Settings:

1. Open Linear --> Settings --> Teams --> {team_name} --> Workflow
2. Under "Started" states, click "Add status"
3. Create each missing status:
   - Name: "Rework"       | Type: Started | Description: "Agent addressing review feedback"
   - Name: "Human Review"  | Type: Started | Description: "PR ready for human review"
   - Name: "Merging"       | Type: Started | Description: "Agent merging approved PR"
4. Save changes

After creating the statuses, re-run this step to verify.
```

---

## Step 7: Verify Setup

Run a structured verification of the entire setup. Each check is pass/fail with specific recovery guidance.

### 7a: File integrity checks

| Check | Command | Pass condition | Recovery |
|-------|---------|---------------|----------|
| Codex skills directory | `ls -d .codex/skills/` | Directory exists | Re-run Step 4 |
| All 6 skill directories | `ls .codex/skills/{commit,debug,land,linear,pull,push}/SKILL.md` | All 6 exist | Re-run Step 4 for missing skills |
| land_watch.py | `ls .codex/skills/land/land_watch.py` | File exists | Copy from reference |
| land_watch.py executable | `test -x .codex/skills/land/land_watch.py` | Executable bit set | `chmod +x` |
| WORKFLOW.md | `ls WORKFLOW.md` | File exists | Re-run Step 5 |
| WORKFLOW.md no placeholders | `grep -c '{{' WORKFLOW.md` | Returns 0 | Re-run Step 5 |
| WORKFLOW.md front matter | `head -1 WORKFLOW.md` | Starts with `---` | Check template rendering |

### 7b: Environment checks

| Check | Command | Pass condition | Recovery |
|-------|---------|---------------|----------|
| LINEAR_API_KEY | `[ -n "$LINEAR_API_KEY" ]` | Non-empty | Set in shell profile |
| OPENAI_API_KEY | `[ -n "$OPENAI_API_KEY" ]` | Non-empty | Set in shell profile |
| codex CLI | `command -v codex` | Found | `npm install -g @openai/codex` |
| python3 | `command -v python3` | Found | Install via system package manager |
| gh CLI | `command -v gh` | Found | `brew install gh` |

### 7c: Integration checks

| Check | Command | Pass condition | Recovery |
|-------|---------|---------------|----------|
| Git remote matches WORKFLOW.md | Compare `git remote get-url origin` with value in WORKFLOW.md | Match | Update WORKFLOW.md or git remote |
| Primary branch exists | `git rev-parse --verify {branch}` | Valid ref | Check branch name in WORKFLOW.md |
| Linear project accessible | Query Linear API with project slug | Returns project | Check slug, API key, permissions |

### 7d: Verification summary

```
Setup Verification
==================
File integrity:    {N}/7 passed
Environment:       {N}/5 passed
Integration:       {N}/3 passed

Overall: {PASS|FAIL} ({total_passed}/{total_checks})
```

If all checks pass, proceed to Step 8. If any fail, print the recovery steps and stop.

---

## Step 8: Report and Next Steps

Print a comprehensive setup summary with clear next steps.

```
Symphony Setup Complete
=======================

Repository:       {repo_name}
Linear project:   {project_slug}
Primary branch:   {primary_branch}
Skills installed: commit, debug, land, linear, pull, push
WORKFLOW.md:      created
Linear statuses:  {verified|manual setup needed}

Verification: {total_passed}/{total_checks} checks passed

Next steps:

1. Commit the new files:
   git add .codex/skills/ WORKFLOW.md
   git commit -m "feat: add Symphony workflow and Codex skills"

2. Start Symphony:
   cd ~/Code/symphony
   ./bin/symphony /path/to/WORKFLOW.md --port 4000

3. Create a test issue in Linear (assign to the project)
   - Symphony will pick it up and start working

4. Monitor the first run:
   - Watch the Symphony terminal for status updates
   - Check Linear issue status transitions
   - Use the debug skill if a run gets stuck

Troubleshooting:
- Symphony not picking up issues?  Check LINEAR_API_KEY and project slug
- Codex errors?                    Check OPENAI_API_KEY and codex version
- PR creation fails?               Check gh auth status
- land_watch.py fails?             Check python3 and gh CLI availability
- Stuck in "In Progress"?          Use the debug skill to inspect logs
```

---

## Notes

- The `linear` skill requires Symphony's `linear_graphql` app-server tool -- it only works during active Symphony sessions. Outside of Symphony, use Linear MCP or the web UI.
- The `land` skill uses `land_watch.py` to monitor PR status asynchronously. It polls GitHub for CI status, review comments, and head changes. Requires Python 3 and the `gh` CLI.
- The `debug` skill is for troubleshooting Symphony/Codex log issues -- not needed for normal operation but invaluable for diagnosing stuck runs.
- Symphony depends on non-standard Linear issue statuses (`Rework`, `Human Review`, `Merging`). These must be added manually in Linear Team Settings --> Workflow. See `reference/linear-setup.md`.
- The WORKFLOW.md template includes a ticket state machine that drives all agent behavior. See `reference/workflow-customization.md` for customization guidance.

---

## Relationship to Other Skills

| Skill | Relationship |
|-------|-------------|
| `maestro:setup` | General project onboarding. Run `maestro:setup` first for project context, then `maestro:symphony-setup` for Symphony-specific configuration. |
| `maestro:implement` | Implementation workflow. After Symphony setup, agents use the Codex skills (not maestro:implement) for autonomous work. |
| `maestro:review` | Code review. Symphony's `land` skill handles automated review monitoring; `maestro:review` is for human-driven review. |
| `maestro:debugging` | General debugging. The `debug` Codex skill is Symphony-specific; `maestro:debugging` covers broader debugging workflows. |

Symphony setup is complementary to maestro setup. Maestro manages human-orchestrated multi-agent workflows; Symphony manages fully autonomous single-agent issue resolution. They can coexist in the same repository.
