# Wisdom Extraction — Work Phase

After all tasks complete, record learnings to `.maestro/wisdom/{plan-name}.md`:

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
- [agent-type]: [N/M tasks completed, avg time, notes on fit]
- Example: "build-fixer resolved 3/3 lint tasks quickly"
- Example: "kraken was overkill for single-file config changes"

## Technology Notes
- [library/framework]: [key findings, gotchas, patterns that worked]
- Only include for technologies not previously seen in wisdom files

## Patterns Captured
- [New test patterns, error handling patterns, API usage patterns from git diff]
```

**Automated pattern capture**: After writing the base wisdom file, scan the git diff for this execution to identify:
1. New test patterns (test file structures, assertion styles)
2. New error handling patterns (try/catch, error boundaries)
3. New API usage patterns (client setup, authentication, response handling)

Add any discovered patterns to the `## Patterns Captured` section.

### Auto-Extract Learned Skills

**Trigger**: Plan had >= 3 tasks (skip for trivial plans).

After writing the wisdom file, automatically extract reusable skill files:

1. Scan the git diff for this execution
2. Scan `<remember>` tags collected during execution (from worker output)
3. For each candidate learning, apply quality gates:
   - **Non-Googleable**: Would a developer NOT find this in official docs?
   - **Context-specific**: Is it specific to this project/stack/pattern?
   - **Actionable**: Can a future agent act on it immediately?
   - **Hard-won**: Did it require debugging, experimentation, or failure to discover?
4. Learnings that pass all 4 gates → save to `.claude/skills/learned/{slug}.md` with:
   ```yaml
   ---
   name: {slug}
   description: {one-line description}
   triggers: [{keyword1}, {keyword2}]
   source: {plan-name}
   date: {ISO date}
   ---
   ```
   Followed by the principle and when to apply it.
5. Learnings that fail any gate → discard silently (they're already in the wisdom file)

**If executing in a worktree** (handoff has `"worktree": true`): Copy the wisdom file back to the main tree so it persists after worktree removal:

```bash
cp "<worktree-path>/.maestro/wisdom/{plan-name}.md" ".maestro/wisdom/{plan-name}.md"
```

Where `<worktree-path>` is the `worktree_path` value from the handoff JSON.
