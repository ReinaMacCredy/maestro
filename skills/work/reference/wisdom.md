# Wisdom Extraction

After all tasks complete, record learnings to `.maestro/wisdom/{plan-slug}.md`.

---

## Wisdom file template

```markdown
# Wisdom: {Plan Name}

## Conventions Discovered
- ...

## Successful Approaches
- ...

## Failed Approaches to Avoid
- ...

## Technical Gotchas
- ...

## Agent Effectiveness
- [agent-type]: [N/M tasks completed, notes on fit]

## Technology Notes
- [library/framework]: [key findings, gotchas, patterns]
- Only include for technologies not previously seen in wisdom files

## Patterns Captured
- [New test patterns, error handling patterns, API usage patterns from git diff]
```

## Automated pattern capture

After writing the base wisdom file, scan the git diff for this execution to identify:
1. New test patterns (test file structures, assertion styles)
2. New error handling patterns (try/catch, error boundaries)
3. New API usage patterns (client setup, authentication, response handling)

Add discovered patterns to the `## Patterns Captured` section.

## Auto-extract learned skills

**Trigger**: Plan had >= 3 tasks (skip for trivial plans).

After writing the wisdom file:

1. Scan the git diff and `<remember>` tags collected during execution
2. For each candidate learning, apply quality gates:
   - **Non-Googleable**: Would a developer NOT find this in official docs?
   - **Context-specific**: Is it specific to this project/stack/pattern?
   - **Actionable**: Can a future agent act on it immediately?
   - **Hard-won**: Did it require debugging, experimentation, or failure to discover?
3. Learnings that pass all 4 gates → save to `.maestro/skills/learned/{slug}.md`:
   ```yaml
   ---
   name: {slug}
   description: {one-line description}
   triggers: [{keyword1}, {keyword2}]
   source: {plan-name}
   date: {ISO date}
   ---
   ```
4. Learnings that fail any gate → discard (they're already in the wisdom file)

## Worktree handling

If executing in a worktree (handoff has `"worktree": true`), copy the wisdom file back to the main tree so it persists after worktree removal:

```bash
cp "<worktree-path>/.maestro/wisdom/{slug}.md" ".maestro/wisdom/{slug}.md"
```

## Planless mode naming

Derive the file slug from the first 5 significant words of the description (strip articles):
- `/work add retry logic to api client` → `add-retry-logic-to-api.md`
- `/work fix login page redirect bug` → `fix-login-page-redirect-bug.md`
