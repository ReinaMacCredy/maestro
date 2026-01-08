# CI/CD Version Migration Plan

## Execution Order

All tasks are sequential (touch same CI/CD files):

```
Wave 1 (Parallel P1):
├── my-workflow:3-0gzh: Reset plugin.json + marketplace.json
├── my-workflow:3-32a2: Update cliff.toml tag_pattern
└── my-workflow:3-cmkc: Update release.yml bump semantics

Wave 2 (Sequential P2):
├── my-workflow:3-zgby: Archive CHANGELOG.md
└── my-workflow:3-jc21: Create fresh CHANGELOG.md

Wave 3 (Sequential P3):
├── my-workflow:3-093g: Create v0.5.0 tag
└── my-workflow:3-1m89: Update AGENTS.md docs
```

## File Scope

| Task | Files |
|------|-------|
| 0gzh | `.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json` |
| 32a2 | `cliff.toml` |
| cmkc | `.github/workflows/release.yml` |
| zgby | `CHANGELOG.md` → `CHANGELOG-legacy.md` |
| jc21 | `CHANGELOG.md` (new) |
| 093g | Git tag + GitHub release |
| 1m89 | `AGENTS.md` |

## Ready to Execute

Start with: `ci` or `/conductor-implement`
