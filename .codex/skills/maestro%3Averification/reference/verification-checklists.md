# Verification Checklists Reference

Concrete checklists for each verification type. Use these as lookup tables when running verification at any scope (task, phase, feature).

## Build Verification

Confirms the code compiles and produces valid artifacts.

### Checklist

- [ ] Build command exits 0
- [ ] No compilation errors in output
- [ ] No unresolved imports or missing modules
- [ ] Output artifacts exist (dist/, build/, .js files)
- [ ] Artifact sizes are reasonable (no empty bundles, no 10x size increase)
- [ ] No new deprecation warnings (compare to pre-change build output)
- [ ] Source maps generated (if expected)

### Commands

```bash
# TypeScript/Bun
bun run build

# TypeScript/Node
npm run build

# Rust
cargo build --release

# Go
go build ./...

# Python (if applicable)
uv run python -m py_compile src/main.py

# Java
./gradlew build
```

### How to Read Build Output

| Output | Meaning | Action |
|--------|---------|--------|
| `exit 0`, no errors | Build passes | Proceed |
| `exit 0`, warnings | Build passes but investigate warnings | Check if warnings are new |
| `exit non-zero`, errors | Build fails | Fix errors before anything else |
| `exit 0`, but artifacts missing | Silent failure | Check build config, output paths |

### Common False Positives

- **Cached build**: Old artifacts pass checks even though source has errors. Fix: clean build (`rm -rf dist && bun run build`).
- **Partial build**: Only changed files recompiled, missing transitive errors. Fix: full rebuild.
- **Warning-as-error disabled**: Build "passes" with warnings that should be errors. Fix: check build config for strict mode.

### Common False Negatives

- **Unrelated build error**: Pre-existing error blocks your verification. Fix: document pre-existing error, verify it exists without your changes (`git stash && bun run build`).
- **Environment-specific failure**: Works locally, fails in CI (or vice versa). Fix: check Node/Bun version, OS differences, env vars.

## Test Verification

Confirms behavior is correct and regressions are caught.

### Checklist

- [ ] Test command exits 0
- [ ] All tests pass (exact count: N passed, 0 failed)
- [ ] No test errors (errors differ from failures: errors = test could not run)
- [ ] No skipped tests that should be running
- [ ] New behavior has new tests
- [ ] Test output is clean (no unexpected console.log, no unhandled promise warnings)
- [ ] Test names describe behavior, not implementation

### Commands

```bash
# Full suite
bun test

# Targeted (changed files only -- for speed, not as a substitute)
bun test src/parser.test.ts

# With coverage
bun test --coverage

# Grep for specific tests
bun test --grep "retry"

# Python
uv run pytest
uv run pytest tests/test_parser.py -v

# Rust
cargo test
cargo test -- --test-threads=1  # if tests share state

# Go
go test ./...
go test -v -run TestRetry ./pkg/retry/
```

### How to Read Test Output

| Output | Meaning | Action |
|--------|---------|--------|
| `N passed, 0 failed` | All tests pass | Report exact count |
| `N passed, M failed` | Some tests fail | Fix ALL failures before proceeding |
| `N passed, M skipped` | Some tests skipped | Investigate why. Skipped is not passed. |
| `N passed, 0 failed` but warnings | Tests pass with noise | Investigate warnings. Clean output matters. |
| Test errors (not failures) | Tests could not execute | Fix test infrastructure before re-running |

### Failure Triage

```
Test fails
  |
  +--> Is it YOUR test (newly written)?
  |     |
  |     +--> YES: Your implementation is wrong. Fix code, not test.
  |     |
  |     +--> NO: Is it a test for code YOU changed?
  |           |
  |           +--> YES: Your change broke existing behavior.
  |           |         Decide: is the old behavior correct?
  |           |         - YES: Revert your change, rethink approach
  |           |         - NO: Update test to match new correct behavior
  |           |
  |           +--> NO: Is it a pre-existing failure?
  |                 |
  |                 +--> YES: Document it. Not your problem, but don't make it worse.
  |                 |
  |                 +--> NO: Your change has an unexpected side effect.
  |                           Investigate the coupling.
```

### Common False Positives

- **Test passes but tests the mock**: Mock returns expected value regardless of implementation. Fix: use real code where possible.
- **Test passes but assertion is wrong**: `expect(result).toBeDefined()` passes for wrong values. Fix: assert specific values.
- **Test passes but only in isolation**: Test depends on execution order or shared state. Fix: run tests in random order, isolate state.

### Common False Negatives

- **Flaky test**: Fails intermittently due to timing, state, or external deps. Follow the Flaky Test Protocol in SKILL.md.
- **Environment-dependent test**: Fails on different OS, timezone, or locale. Fix: mock environment-specific values.
- **Test timeout**: Slow test hits timeout limit but logic is correct. Fix: increase timeout or optimize test.

## Integration Verification

Confirms that multiple components work together correctly after merging or combining changes.

### Checklist

- [ ] All unit tests pass (baseline)
- [ ] Integration test suite passes
- [ ] API contracts honored (request/response shapes match)
- [ ] Database migrations run cleanly (if applicable)
- [ ] Cross-module imports resolve correctly
- [ ] No circular dependencies introduced
- [ ] Event/message contracts match between producer and consumer
- [ ] Configuration values consistent across modules
- [ ] Error handling works across module boundaries (errors propagate correctly)

### Commands

```bash
# Run integration tests specifically
bun test --grep "integration"
bun test tests/integration/

# Check for circular dependencies (TypeScript)
npx madge --circular src/

# Check import resolution
bun run build  # catches unresolved imports

# API contract verification (if using typed clients)
bun test tests/api/

# Database migration (if applicable)
bun run db:migrate
bun run db:migrate:status
```

### Integration Points to Verify

| Integration point | What to check | How to check |
|-------------------|---------------|--------------|
| Module A calls Module B | B's API unchanged or A updated | Build + unit tests for both |
| Shared types/interfaces | All consumers use updated type | `grep` for type name, verify usage |
| Event emitter/listener | Event shape matches listener expectation | Integration test with real event |
| Config consumed by multiple modules | All modules read same keys | Grep config keys, verify consistency |
| Database schema + queries | Queries match current schema | Run migrations, then test suite |
| CLI command + handler | Command parsing matches handler signature | E2E test of command |

### Post-Merge Integration Checks

After merging two or more worktrees:

```bash
# 1. Full build (catches import/type mismatches)
bun run build

# 2. Full test suite (catches behavior mismatches)
bun test

# 3. Check for conflict artifacts
grep -r "<<<<<<" src/
grep -r "=======" src/
grep -r ">>>>>>>" src/
# Expected: 0 matches

# 4. Check for duplicate declarations (common merge artifact)
# If two worktrees both added the same import or function
bun run build  # usually catches these as redeclaration errors
```

## Regression Verification

Confirms that existing behavior is preserved after changes.

### Checklist

- [ ] All pre-existing tests still pass
- [ ] New regression test written for the specific bug/behavior
- [ ] Regression test verified with red-green cycle (see maestro:tdd)
- [ ] Edge cases from the original bug report tested
- [ ] Related functionality spot-checked (nearby code paths)
- [ ] Performance not degraded (if applicable)
- [ ] No new warnings in areas not touched by the change

### Red-Green Cycle for Regression Tests

This is the definitive proof that a regression test actually tests what it claims.

```bash
# 1. Write the regression test
# 2. Run it -- must PASS (bug is now fixed)
bun test tests/regression/empty-email.test.ts
# Expected: PASS

# 3. Revert the fix
git stash  # or manually revert the fix

# 4. Run the test -- must FAIL
bun test tests/regression/empty-email.test.ts
# Expected: FAIL (proves test catches the bug)

# 5. Restore the fix
git stash pop  # or re-apply the fix

# 6. Run the test -- must PASS again
bun test tests/regression/empty-email.test.ts
# Expected: PASS

# All three results required:
# PASS (fix works) --> FAIL (test catches bug) --> PASS (fix restored)
# Missing any step = unverified regression test
```

### Before/After Comparison

For changes where regression risk is high:

```bash
# 1. Capture baseline (before your changes)
git stash
bun test 2>&1 | tee /tmp/test-before.txt
git stash pop

# 2. Capture current (after your changes)
bun test 2>&1 | tee /tmp/test-after.txt

# 3. Compare
diff /tmp/test-before.txt /tmp/test-after.txt
# Expected: only your new tests appear as additions
# Unexpected: existing tests changing status
```

### Regression Risk Assessment

| Change type | Regression risk | What to verify |
|-------------|----------------|----------------|
| Bug fix | Medium | Red-green cycle, nearby code paths |
| Refactoring | High | All existing tests, behavior preservation |
| New feature | Low (for existing code) | Existing tests still pass |
| Dependency update | High | Full suite, API compatibility |
| Performance optimization | Medium | Correctness tests + benchmark comparison |
| Schema/API change | Very high | All consumers, migration path, rollback |

### Common False Positives (Regression Looks Broken But Is Not)

- **Test order dependency**: Test fails when run after new tests but passes alone. Fix: isolate test state.
- **Snapshot mismatch**: Snapshots outdated, not regression. Fix: update snapshots deliberately.
- **Timezone/locale change**: Test assumes specific timezone. Fix: mock time in tests.

### Common False Negatives (Regression Exists But Tests Miss It)

- **No test for the regressed behavior**: Test suite has a gap. Fix: add regression test.
- **Test too coarse**: Tests check happy path, regression is in edge case. Fix: add edge case tests.
- **Mock hides regression**: Real dependency changed but mock returns old values. Fix: integration test with real dependency.
