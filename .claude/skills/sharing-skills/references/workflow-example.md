# Complete Sharing Workflow Example

Here's a complete example of sharing a skill called "async-patterns":

```bash
# 1. Sync with upstream
cd <your-skills-directory>  # e.g., ~/.claude/skills/
git checkout main
git pull upstream main
git push origin main

# 2. Create branch
git checkout -b "add-async-patterns-skill"

# 3. Create/edit the skill
# (Work on skills/async-patterns/SKILL.md)

# 4. Commit
git add skills/async-patterns/
git commit -m "Add async-patterns skill

Patterns for handling asynchronous operations in tests and application code.

Tested with: Multiple pressure scenarios testing agent compliance."

# 5. Push
git push -u origin "add-async-patterns-skill"

# 6. Create PR
gh pr create \
  --repo <YOUR-UPSTREAM-ORG>/<YOUR-UPSTREAM-REPO> \
  --title "Add async-patterns skill" \
  --body "## Summary
Patterns for handling asynchronous operations correctly in tests and application code.

## Testing
Tested with multiple application scenarios. Agents successfully apply patterns to new code.

## Context
Addresses common async pitfalls like race conditions, improper error handling, and timing issues."
```

## After PR is Merged

Once your PR is merged:

1. Sync your local main branch:
```bash
cd <your-skills-directory>  # e.g., ~/.claude/skills/
git checkout main
git pull upstream main
git push origin main
```

2. Delete the feature branch:
```bash
git branch -d "add-${skill_name}-skill"
git push origin --delete "add-${skill_name}-skill"
```

## Troubleshooting

**"gh: command not found"**
- Install GitHub CLI: https://cli.github.com/
- Authenticate: `gh auth login`

**"Permission denied (publickey)"**
- Check SSH keys: `gh auth status`
- Set up SSH: https://docs.github.com/en/authentication

**"Skill already exists"**
- You're creating a modified version
- Consider different skill name or coordinate with the skill's maintainer

**PR merge conflicts**
- Rebase on latest upstream: `git fetch upstream && git rebase upstream/main`
- Resolve conflicts
- Force push: `git push -f origin your-branch`
