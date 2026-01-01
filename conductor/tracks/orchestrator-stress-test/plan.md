# Plan: Orchestrator Stress Test

## Epic: Orchestrator Stress Test

Test parallel worker coordination with Agent Mail and file reservations.

## Track Assignments

| Agent | Beads | File Scope | Dependencies |
|-------|-------|------------|--------------|
| Worker-A | Task A: README | demo/README.md | None |
| Worker-B | Task B: Config | demo/config.json | None |
| Worker-C | Task C: API Stub | demo/api.ts | A, B |
| Worker-D | Task D: API Test | demo/api.test.ts | C |

## Tasks

### Epic: Orchestrator Stress Test

- [ ] **Task A: Create README** - Create demo/README.md documenting the demo project
- [ ] **Task B: Create Config** - Create demo/config.json with project metadata
- [ ] **Task C: Create API Stub** - Create demo/api.ts that reads config (depends on A, B)
- [ ] **Task D: Create API Test** - Create demo/api.test.ts for the API stub (depends on C)
- [ ] **Task E: Verify Results** - Check all files exist, Agent Mail messages logged, beads closed

## Wave Execution

```
Wave 1 (parallel):  A ──┬── B
                        │
Wave 2 (depends A+B):   └── C
                            │
Wave 3 (depends C):         └── D
                                │
Verification:                   └── E
```

## File Contents

### Task A: demo/README.md
```markdown
# Demo Project

Created by orchestrator stress test.

## Files
- config.json - Project configuration
- api.ts - API stub
- api.test.ts - API tests
```

### Task B: demo/config.json
```json
{
  "name": "orchestrator-stress-test",
  "version": "1.0.0",
  "created": "2026-01-01"
}
```

### Task C: demo/api.ts
```typescript
import config from './config.json';

export function getProjectName(): string {
  return config.name;
}

export function getVersion(): string {
  return config.version;
}
```

### Task D: demo/api.test.ts
```typescript
import { getProjectName, getVersion } from './api';

describe('API', () => {
  it('should return project name', () => {
    expect(getProjectName()).toBe('orchestrator-stress-test');
  });

  it('should return version', () => {
    expect(getVersion()).toBe('1.0.0');
  });
});
```

## Success Criteria

- [ ] All 4 workers spawn successfully
- [ ] Wave dependencies respected
- [ ] All files created in demo/
- [ ] Agent Mail coordination verified
- [ ] All beads closed with `completed`
