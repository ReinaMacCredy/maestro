# Test Suite Performance Optimization Guide

## Overview

This document describes the performance optimizations applied to the Maestro test suite and provides guidance for writing performant tests.

## Current State

- **301 test files** (244 unit, 38 e2e, 19 integration)
- **~3,700+ test cases**
- **Previous timeout**: 300+ seconds (timed out)
- **Target**: <180 seconds (40% reduction)

## Optimizations Applied

### Phase 1: Build and Execution Optimization (COMPLETED)

#### 1. Pre-build CLI Binary (30-45s savings)

**Problem**: Every e2e test file called `buildCompiledCli()` in `beforeAll()`, triggering full TypeScript compilation.

**Solution**: 
- Added `pretest` script to build once before tests run
- Modified `buildCompiledCli()` to check for existing binary (< 5 min old) before rebuilding
- Per-process caching prevents redundant builds within same test process

**Files Modified**:
- `package.json`: Added `pretest: "bun run build"`
- `tests/helpers/run-compiled-cli.ts`: Added freshness check

**Usage**:
```bash
# Automatically builds before running tests
bun test

# Or build explicitly
bun run build && bun test
```

#### 2. Parallel Unit Test Execution (40-60% unit test savings)

**Problem**: Unit tests were running serially by default.

**Solution**: 
- Added `test:unit` script with `--concurrent` flag
- Unit tests are isolated and can safely run in parallel
- E2e tests remain serial to avoid CLI binary contention

**Files Modified**:
- `package.json`: Added `test:unit`, `test:integration`, `test:e2e` scripts

**Usage**:
```bash
# Run unit tests in parallel
bun run test:unit

# Run all tests (unit tests will be parallel)
bun test
```

### Phase 2: Fixture Caching (15-25s savings)

#### 3. Test Repository Fixture Caching

**Problem**: Each e2e test created its own temp directory via `mkdtemp()`, involving:
- OS syscall to create directory
- Git repo initialization (3-5 git commands)
- Maestro initialization (spawns compiled binary)
- Cleanup in `afterEach` (recursive directory deletion)

**Solution**: 
- Created `tests/helpers/test-repo-fixture.ts` with fixture caching
- Create "golden" fixture once per test file, then clone via `cp -r` for each test
- Eliminates N+1 temp directory creation pattern

**Files Created**:
- `tests/helpers/test-repo-fixture.ts`: Fixture caching utilities

**Usage**:

```typescript
import { 
  cloneBasicRepo, 
  cleanupClone, 
  cleanupAllFixtures 
} from "../helpers/test-repo-fixture.js";

describe("my e2e test", () => {
  let testDir: string;

  beforeEach(async () => {
    // Clone the cached fixture (fast)
    testDir = await cloneBasicRepo("my-test");
  });

  afterEach(async () => {
    // Clean up the clone
    await cleanupClone(testDir);
  });

  afterAll(async () => {
    // Clean up all fixtures
    await cleanupAllFixtures();
  });

  it("does something", async () => {
    // testDir is a fresh clone of an initialized repo
    const result = await runCompiled(["status"], testDir);
    expect(result.exitCode).toBe(0);
  });
});
```

**Advanced Usage** (custom fixtures):

```typescript
import { 
  getOrCreateFixture, 
  cloneFixture, 
  cleanupAllFixtures 
} from "../helpers/test-repo-fixture.js";

describe("my test with custom fixture", () => {
  let testDir: string;

  beforeEach(async () => {
    // Get or create a custom fixture
    const fixture = await getOrCreateFixture("with-contract", async (dir) => {
      await initGitRepo(dir);
      await runCommand(["git", "config", "user.email", "test@example.com"], dir);
      await runCommand(["git", "config", "user.name", "Test"], dir);
      await runCompiled(["init"], dir);
      
      // Custom setup: create a task with contract
      await runCompiled(["task", "create", "test task"], dir);
      await runCompiled(["task", "contract", "new", "tsk-aaaaaa"], dir);
      
      await runCommand(["git", "add", "."], dir);
      await runCommand(["git", "commit", "-m", "init with contract"], dir);
    });
    
    // Clone the fixture for this test
    testDir = await cloneFixture(fixture, "my-test");
  });

  afterEach(async () => {
    await cleanupClone(testDir);
  });

  afterAll(async () => {
    await cleanupAllFixtures();
  });

  it("works with contract", async () => {
    // testDir has a task with contract already set up
    const result = await runCompiled(["task", "list"], testDir);
    expect(result.stdout).toContain("tsk-aaaaaa");
  });
});
```

## Writing Performant Tests

### DO: Use Fixture Caching for E2E Tests

```typescript
// ✅ GOOD: Use fixture caching
import { cloneBasicRepo, cleanupClone } from "../helpers/test-repo-fixture.js";

beforeEach(async () => {
  testDir = await cloneBasicRepo("my-test");
});
```

```typescript
// ❌ BAD: Create fresh repo every time
import { mkdtemp } from "node:fs/promises";

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), "maestro-test-"));
  await initGitRepo(testDir);
  await runCompiled(["init"], testDir);
  // ... more setup
});
```

### DO: Parallelize Independent CLI Invocations

```typescript
// ✅ GOOD: Parallel read operations
const [evidenceResult, verdictResult, proofResult] = await Promise.all([
  runCompiled(["evidence", "list", "--task", taskId], dir),
  runCompiled(["verdict", "show", "--task", taskId], dir),
  runCompiled(["task", "proof", "--task", taskId], dir),
]);
```

```typescript
// ❌ BAD: Sequential read operations
const evidenceResult = await runCompiled(["evidence", "list", "--task", taskId], dir);
const verdictResult = await runCompiled(["verdict", "show", "--task", taskId], dir);
const proofResult = await runCompiled(["task", "proof", "--task", taskId], dir);
```

### DO: Use afterAll for Cleanup When Possible

```typescript
// ✅ GOOD: Batch cleanup in afterAll
const tempDirs: string[] = [];

afterAll(async () => {
  await Promise.all(tempDirs.map(d => rm(d, { recursive: true, force: true })));
});
```

```typescript
// ❌ BAD: Per-test cleanup (slower on some filesystems)
afterEach(async () => {
  await rm(testDir, { recursive: true, force: true });
});
```

### DO: Reuse Test Data

```typescript
// ✅ GOOD: Create test data once
const testData = { /* ... */ };

beforeAll(() => {
  // Parse or generate test data once
});

it("test 1", () => {
  // Use testData
});

it("test 2", () => {
  // Reuse testData
});
```

```typescript
// ❌ BAD: Recreate test data in every test
it("test 1", () => {
  const testData = { /* ... */ };
  // ...
});

it("test 2", () => {
  const testData = { /* ... */ }; // Duplicate work
  // ...
});
```

### DON'T: Call buildCompiledCli() Unnecessarily

```typescript
// ✅ GOOD: Build once per test file
beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);
```

```typescript
// ❌ BAD: Build in every test
beforeEach(buildCompiledCli);
```

## Measuring Performance

### Baseline Measurement

```bash
# Measure full suite
time bun test

# Measure by type
time bun run test:unit
time bun run test:integration
time bun run test:e2e

# Measure specific file
time bun test tests/e2e/l7-deploy-safety.test.ts
```

### Profiling

```bash
# Run with verbose output to see slow tests
bun test --verbose

# Run specific test file with timing
bun test tests/e2e/l7-deploy-safety.test.ts --verbose
```

## Expected Performance

### Before Optimizations
- Full suite: 300+ seconds (timed out)
- E2e suite: ~180-240 seconds
- Unit suite: ~60-90 seconds

### After Phase 1 Optimizations
- Full suite: ~150-180 seconds (40-50% reduction)
- E2e suite: ~90-120 seconds
- Unit suite: ~30-45 seconds (parallel execution)

### After Phase 2 Optimizations
- Full suite: ~120-150 seconds (60% reduction)
- E2e suite: ~60-90 seconds
- Unit suite: ~30-45 seconds

## Troubleshooting

### Tests Failing After Fixture Caching

**Symptom**: Tests pass individually but fail when run together.

**Cause**: Test is mutating shared fixture state.

**Solution**: Ensure tests only mutate the cloned directory, not the cached fixture.

```typescript
// ✅ GOOD: Mutate the clone
testDir = await cloneBasicRepo("my-test");
await writeFile(join(testDir, "foo.txt"), "bar");

// ❌ BAD: Don't mutate the fixture directly
const fixture = await getBasicRepoFixture();
await writeFile(join(fixture, "foo.txt"), "bar"); // Pollutes fixture!
```

### Binary Not Found Errors

**Symptom**: `ENOENT: no such file or directory, access 'dist/maestro'`

**Cause**: Binary wasn't built before tests ran.

**Solution**: Ensure `pretest` script runs or build manually:

```bash
bun run build && bun test
```

### Slow Test Cleanup

**Symptom**: Tests pass but cleanup takes a long time.

**Cause**: Recursive directory deletion on slow filesystem (Windows, network drives).

**Solution**: Use `afterAll` instead of `afterEach` for cleanup, or rely on OS temp directory cleanup.

## Future Optimizations

### Phase 3: Parallelize Independent CLI Invocations (5-10s savings)
- Audit e2e tests for sequential read operations
- Wrap in `Promise.all()` where safe
- Document dependencies between commands

### Phase 4: Optimize Test Lifecycle Hooks (2-5s savings)
- Move cleanup from `afterEach` to `afterAll` where safe
- Batch directory deletions
- Consider OS-level temp cleanup

### Phase 5: In-Memory Mock Stores (3-8s savings)
- Use in-memory stores for unit tests instead of filesystem
- Cache parsed JSONL/YAML in test setup
- Reduce file I/O in hot paths

## Contributing

When adding new tests:

1. **Use fixture caching** for e2e tests that need initialized repos
2. **Parallelize** independent CLI read operations
3. **Batch cleanup** in `afterAll` when possible
4. **Measure impact** of new tests on suite runtime
5. **Document** any custom fixtures in this file

## References

- Full performance analysis: `/tmp/maestro-test-performance-analysis.md`
- Fixture caching helper: `tests/helpers/test-repo-fixture.ts`
- Compiled CLI helper: `tests/helpers/run-compiled-cli.ts`
