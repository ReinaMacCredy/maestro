# .github

## Purpose
GitHub configuration for CI/CD automation, issue management, and repository governance.

## Key Directories

| Directory | Purpose |
|-----------|---------|
| workflows/ | CI/CD workflow definitions |
| actions/ | Custom GitHub Actions (setup-git-cliff) |
| ISSUE_TEMPLATE/ | Structured issue forms |

## Key Files

| File | Purpose |
|------|---------|
| workflows/release.yml | Automated versioning and changelog on push to main |
| workflows/validate.yml | PR validation (version sync, changelog preview) |
| CODEOWNERS | Defines code ownership for review requirements |
| dependabot.yml | Automated dependency updates |
| labels.yml | Repository label definitions |
| PULL_REQUEST_TEMPLATE.md | PR description template |
| SECURITY.md | Security policy and reporting |

## Patterns

- Conventional Commits: Versioning derived from commit prefixes (feat!, feat:, fix:)
- PR Labels Override: Labels (release:major/minor/patch/skip) take priority over commits
- git-cliff: Changelog generation from commit history
- Structured Issues: YAML forms for bug reports and feature requests

## Versioning Strategy

Priority order for version bumps:
1. PR labels: release:major, release:minor, release:patch, release:skip
2. Conventional commits: feat!: (major), feat: (patch), fix: (patch)

## Dependencies

- External: git-cliff (changelog), GitHub Actions runtime
- Internal: plugin.json and marketplace.json must stay in sync

## Notes for AI Agents

- Never manually edit CHANGELOG.md - it's auto-generated
- Use conventional commit format for automatic versioning
- Blank issues are disabled - use templates or Discussions
- Check validate.yml for required PR checks before merging
