# Project Instructions
@AGENTS.md

## Quick Reference

### Build and verify
```bash
bun run build && ./dist/maestro --version
bun run release:local          # rebuild + install to PATH
```

### TUI preview (agent-friendly)
```bash
maestro mission-control --preview --size 120x40 --format plain
maestro mission-control --preview all --size 120x40 --format plain
maestro mission-control --render-check --size 120x40
bun tui:dev --screen all --size 120x40
```

### Test
```bash
bun test                       # full suite
bun test tests/unit/tui/       # TUI unit tests only
```

### Mission bundles
```bash
maestro bundle export <missionId> --out ./review.mission.tar.gz
maestro bundle export <missionId> --redact memory,prompts --json
maestro bundle inspect ./review.mission.tar.gz --json
```

### Conventions
- Conventional Commits: `feat(scope):`, `fix(scope):`, `refactor(scope):`
- Bump version for every behavior change (minor=feature, patch=fix, major=breaking)
- Verify against `./dist/maestro`, not `maestro` on PATH, unless specifically testing the installed binary
