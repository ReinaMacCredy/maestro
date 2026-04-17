# TDD Workflow Protocol

## Red-Green-Refactor Cycle

### Red: Write Failing Tests

1. **Identify test scope** from task description and spec requirements
2. **Create test file** following project conventions:
   - Python: `tests/test_{module}.py` or `{module}_test.py`
   - TypeScript/JavaScript: `{module}.test.ts` or `__tests__/{module}.test.ts`
   - Go: `{module}_test.go` (same package)
   - Rust: `#[cfg(test)] mod tests` or `tests/{module}.rs`
3. **Write tests** that define expected behavior:
   - Test the public API, not implementation details
   - Cover happy path, edge cases, and error scenarios
   - Use descriptive test names that read as specifications
4. **Run tests** -- they MUST fail
   - If tests pass: the behavior already exists. Investigate before proceeding.
   - If tests don't compile: fix syntax, but don't implement the feature yet
5. **Do NOT proceed** until tests fail for the right reason

### Green: Implement to Pass

1. Write the **minimum code** to make all failing tests pass
2. Do NOT add features beyond what tests require
3. Do NOT refactor yet -- just make it work
4. Run tests -- they MUST pass
5. If tests fail:
   - Debug the implementation (not the tests)
   - Max 3 fix attempts
   - If still failing: halt and ask user

### Refactor: Clean Up

1. With passing tests as a safety net, improve:
   - Variable and function names
   - Code structure and readability
   - Remove duplication
   - Extract common patterns
2. Run tests after each refactoring step
3. Tests MUST still pass after refactoring
4. Skip if the implementation is already clean

## Coverage Commands by Language

| Language | Command | Coverage Tool |
|----------|---------|---------------|
| Python | `CI=true pytest --cov={module} --cov-report=term-missing` | pytest-cov |
| TypeScript (Jest) | `CI=true npx jest --coverage --collectCoverageFrom='{pattern}'` | Jest built-in |
| TypeScript (Vitest) | `CI=true npx vitest run --coverage` | c8/v8 |
| Go | `go test -coverprofile=coverage.out ./... && go tool cover -func=coverage.out` | go test |
| Rust | `cargo tarpaulin --out Stdout` | tarpaulin |
| JavaScript (Node) | `CI=true node --experimental-vm-modules node_modules/.bin/jest --coverage` | Jest |

## Test Command Discovery

To find the project's test command:
1. Check `package.json` scripts (`test`, `test:unit`, `test:coverage`)
2. Check `pyproject.toml` (`[tool.pytest]`, `[tool.coverage]`)
3. Check `Makefile` (`test`, `coverage` targets)
4. Check CI config (`.github/workflows/*.yml`, `.gitlab-ci.yml`)
5. Fall back to language defaults

## Anti-patterns

| Don't | Do Instead |
|-------|-----------|
| Write tests after implementation | Write tests first (Red phase) |
| Write tests that test implementation details | Test behavior and public API |
| Skip the Red phase | Always verify tests fail first |
| Write all tests at once | Write one test, make it pass, repeat |
| Ignore failing tests | Fix immediately or mark as known issue |
| Test private methods directly | Test through the public interface |
