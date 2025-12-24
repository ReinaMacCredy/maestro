# LEARNINGS: changelog-cicd

## Summary
Added GitHub Actions CI/CD for automated changelog generation and version bumping using git-cliff and conventional commits.

## Key Learnings

### Patterns
- **Conventional Commits Versioning:** feat: → minor, fix: → patch, feat!: or BREAKING CHANGE: → major
- **Automated Version Bumping:** CI parses commit messages to determine version increment
- **Skip CI:** Add [skip ci] to commit message to bypass automation

### Commands
- `git cliff` - Generate changelog from conventional commits
- Version in .claude-plugin/plugin.json and marketplace.json is auto-bumped

### Gotchas
- docs: and chore: commits don't bump version (changelog only)
- Skill versions in SKILL.md frontmatter are manually updated (not automated)
- Use env block for COMMITS variable to prevent shell interpretation issues

## Linked Commits
- df7e2e8 feat: add GitHub Actions CI/CD for changelog and versioning
