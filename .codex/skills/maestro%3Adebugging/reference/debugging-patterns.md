# Debugging Patterns Reference

Concrete workflows, tool commands, and decision trees for systematic debugging.

## Decision Tree: Symptom to Approach

Given a symptom, start with the highest-probability approach. Exhaust it before moving down.

```
SYMPTOM: Runtime error / exception
  --> Read the full stack trace (not just the message)
  --> Trace backward from throw site to caller
  --> Check: is the value null/undefined where it shouldn't be?
  --> Check: is the type wrong (string where number expected)?
  --> Check: is the timing wrong (async not awaited)?

SYMPTOM: Type error (compile-time)
  --> Read the FULL error, including "expected X, got Y"
  --> Find the type definition for X
  --> Find what produces Y
  --> The mismatch IS the bug -- don't cast to silence it

SYMPTOM: Test failure
  --> Read expected vs actual output carefully
  --> Is the test wrong or the code wrong? (Check test assumptions)
  --> Run test in isolation: does it pass alone?
  --> If passes alone, fails in suite: shared state or ordering issue
  --> Check: did the test ever pass? (git log on test file)

SYMPTOM: Performance issue
  --> Measure first, don't guess the bottleneck
  --> Profile: is it CPU, memory, I/O, or network?
  --> Find the hot path (single function consuming >50% time)
  --> Check: N+1 queries, unbounded loops, missing pagination
  --> Check: are you doing work inside a render/hot loop?

SYMPTOM: Integration failure (API, service, CI)
  --> Verify each component works in isolation
  --> Add logging at every component boundary
  --> Check: auth, headers, content-type, request format
  --> Check: environment variables, config differences
  --> Check: network (DNS, firewall, proxy, TLS)

SYMPTOM: Build failure
  --> Read the FIRST error (later errors are often cascading)
  --> Check: missing dependency, version mismatch
  --> Check: import path wrong, circular dependency
  --> Clean build artifacts and retry before deeper investigation
  --> Compare: does it build on another machine / in CI?

SYMPTOM: Flaky / intermittent failure
  --> Cannot debug what you cannot reproduce
  --> Add timestamps and sequence logging
  --> Check: race condition, shared mutable state, timing dependency
  --> Check: test pollution (global state, file system, ports)
  --> Run in loop: `for i in {1..50}; do bun test <file> || break; done`
```

## Phase 1: Reproduce -- Tool Commands

The goal is a single command that reliably triggers the bug.

### Read the error

```bash
# Get the full error with context -- don't truncate
bun test path/to/failing.test.ts 2>&1 | tail -80

# For runtime errors, get the full stack
NODE_OPTIONS='--stack-trace-limit=50' bun run script.ts

# For build errors, get the FIRST error (ignore cascade)
bun run build 2>&1 | head -30
```

### Reproduce consistently

```bash
# Run the specific failing test in isolation
bun test path/to/failing.test.ts --bail

# Run with verbose output
bun test path/to/failing.test.ts --verbose

# Reproduce a flaky test (run until failure)
for i in {1..50}; do echo "Run $i"; bun test path/to/failing.test.ts || break; done

# Reproduce with clean state
rm -rf node_modules/.cache dist/ .turbo/
bun install && bun test path/to/failing.test.ts
```

### Check recent changes

```bash
# What changed since last green build?
git log --oneline -20
git diff HEAD~3 -- src/

# What files changed that could affect this module?
git log --oneline --all -- path/to/affected/module/

# Diff between working commit and broken commit
git diff <good-commit>..<bad-commit> -- src/

# Binary search for the breaking commit
git bisect start
git bisect bad HEAD
git bisect good <known-good-commit>
# Then run your reproduction command at each step
git bisect run bun test path/to/failing.test.ts
```

## Phase 2: Isolate -- Tool Commands

Narrow from "something is broken" to "THIS line is the cause."

### Trace data flow

```bash
# Find where a value originates (search for assignments)
# ripgrep: find all assignments to the variable
rg 'myVariable\s*=' --type ts -n

# ast-grep: find structural patterns (function calls, assignments)
sg run -p '$X = fetchData($$$ARGS)' --lang typescript

# Find all callers of a function
rg 'brokenFunction\(' --type ts --files-with-matches
sg run -p 'brokenFunction($$$ARGS)' --lang typescript

# Find where an error is thrown
rg 'throw new.*ErrorMessage' --type ts -n -C 3
```

### Isolate component

```bash
# Test a single module in isolation
bun test path/to/module.test.ts --bail

# Test with mocked dependencies (if integration test is failing)
# Create a minimal reproduction script:
cat > /tmp/repro.ts << 'REPRO'
import { brokenFunction } from './src/module';
const result = brokenFunction({ input: 'test' });
console.log('Result:', JSON.stringify(result, null, 2));
REPRO
bun run /tmp/repro.ts

# Check if the issue is in your code or a dependency
# Test with pinned dependency version
bun add exact-package@known-good-version
bun test path/to/failing.test.ts
```

### Add diagnostic instrumentation

For multi-component systems, add temporary logging at each boundary:

```typescript
// TEMPORARY DEBUG -- remove after fixing
console.log('[DEBUG boundary:api-handler] input:', JSON.stringify(req.body));
console.log('[DEBUG boundary:service-layer] received:', JSON.stringify(data));
console.log('[DEBUG boundary:db-query] query:', query, 'params:', params);
console.log('[DEBUG boundary:db-result] rows:', rows.length, 'first:', rows[0]);
```

Run once, read the output, identify which boundary the data corrupts at. Then remove all `[DEBUG boundary:` lines.

```bash
# Find and remove all debug instrumentation after fixing
rg '\[DEBUG boundary:' --type ts --files-with-matches
# Then remove those lines
```

## Phase 3: Fix -- Tool Commands

One change, one hypothesis, one test.

### Create the failing test

```bash
# Write a minimal test that captures the bug
cat >> path/to/module.test.ts << 'TEST'

test('rejects input when X is missing (regression for #123)', () => {
  const result = processInput({ x: undefined });
  expect(result.error).toBe('X is required');
});
TEST

# Verify it fails (MUST fail before you fix)
bun test path/to/module.test.ts --bail
# Expected: FAIL
```

### Apply the fix

```bash
# Make the SMALLEST change that addresses root cause
# Edit the file, then verify:
bun test path/to/module.test.ts --bail
# Expected: PASS

# Verify no regressions
bun test
# Expected: all PASS
```

### Verify the fix holds

```bash
# Run full test suite
bun test

# Run type check
bun run build

# For flaky bugs: run multiple times
for i in {1..20}; do bun test path/to/module.test.ts || { echo "FAILED on run $i"; break; }; done
```

## Phase 4: Verify -- Tool Commands

Prove the fix is correct and complete.

```bash
# 1. All tests pass
bun test

# 2. Build succeeds
bun run build

# 3. No type errors
# (covered by build for TypeScript projects)

# 4. Linting passes
bun run lint 2>&1 | head -20

# 5. The specific reproduction case works
bun test path/to/module.test.ts --verbose

# 6. Check for similar patterns elsewhere (prevent sibling bugs)
# "Did I fix just one instance of a pattern that exists in 5 places?"
rg 'the-broken-pattern' --type ts --files-with-matches
sg run -p 'brokenPattern($$$ARGS)' --lang typescript
```

## Pattern: Runtime Errors

### Null/Undefined Access

**Symptom:** `TypeError: Cannot read properties of undefined (reading 'x')`

**Isolate:**
```bash
# Find where the object is constructed
rg 'createThing\(' --type ts -n -C 3
# Check: is the property optional? Is it conditionally set?
sg run -p 'createThing({ $$$PROPS })' --lang typescript
```

**Common root causes:**
- Optional property accessed without guard
- Async data not yet loaded (race condition)
- Object shape changed but callers not updated
- Destructuring with wrong property name

**Fix pattern:**
```typescript
// BAD: silencing the symptom
const value = obj?.x ?? 'default';  // Hides the real bug

// GOOD: fix at source -- ensure obj.x is always set
function createThing(input: Input): Thing {
  if (!input.x) throw new Error('x is required for createThing');
  return { x: input.x, /* ... */ };
}
```

### Unhandled Promise Rejection

**Symptom:** `UnhandledPromiseRejection` or silent failure

**Isolate:**
```bash
# Find unhandled promises
sg run -p '$FUNC($$$ARGS).then($HANDLER)' --lang typescript
# These are missing .catch() -- but the real fix is usually await + try/catch

# Find fire-and-forget async calls
rg '^\s+\w+\(' --type ts  # Look for calls without await
```

**Fix pattern:**
```typescript
// BAD: fire-and-forget
saveData(result);

// GOOD: await and handle
try {
  await saveData(result);
} catch (err) {
  log.error('Failed to save result', { err, result });
  throw err;  // or handle gracefully
}
```

## Pattern: Type Errors

**Symptom:** `Type 'X' is not assignable to type 'Y'`

**Isolate:**
```bash
# Find the type definition
rg 'type Y\b|interface Y\b' --type ts -n

# Find what produces the value
rg 'functionThatReturnsX\(' --type ts -n -C 3

# Check: was the type recently changed?
git log --oneline -10 -- path/to/types.ts
```

**Common root causes:**
- Type was updated but callers weren't
- Import from wrong module (same name, different type)
- Generic type parameter not propagated
- Union type needs narrowing

**Fix pattern:** Don't cast. Find the mismatch source and align it.

## Pattern: Test Failures

### Test Passes Alone, Fails in Suite

**Symptom:** `bun test file.test.ts` passes, `bun test` fails

**Isolate:**
```bash
# Find which other test causes pollution
# Run pairs of tests until you find the polluter
bun test fileA.test.ts file.test.ts --bail
bun test fileB.test.ts file.test.ts --bail
# The one that makes it fail is the polluter
```

**Common root causes:**
- Global state modified and not cleaned up
- Module-level side effects (singleton, cache)
- Environment variables set by another test
- File system or database state leaked

**Fix pattern:**
```typescript
// Add cleanup in the polluting test
afterEach(() => {
  // Reset whatever global state was modified
  globalCache.clear();
  process.env.NODE_ENV = 'test';
});
```

### Snapshot Mismatch

**Symptom:** `Snapshot mismatch` or `toMatchSnapshot` failure

**Isolate:**
```bash
# View the diff
bun test file.test.ts --verbose
# Check: is the change intentional?
```

**Decision:** If the change is intentional (you changed output format), update snapshot. If unintentional, the code change is the bug.

```bash
# Intentional change: update snapshot
bun test file.test.ts --update-snapshots

# Unintentional: revert code change and investigate
git diff path/to/source.ts
```

## Pattern: Performance Issues

**Isolate:**
```bash
# Profile with timing
time bun run script.ts

# Add timing instrumentation
cat > /tmp/perf-wrapper.ts << 'PERF'
const start = performance.now();
import { heavyFunction } from './src/module';
const mid = performance.now();
const result = heavyFunction(testInput);
const end = performance.now();
console.log(`Import: ${mid - start}ms`);
console.log(`Execute: ${end - mid}ms`);
PERF
bun run /tmp/perf-wrapper.ts
```

**Common root causes and fixes:**

| Root Cause | Detection | Fix |
|-----------|-----------|-----|
| N+1 queries | N identical queries in logs | Batch/join query |
| Unbounded loop | Loop count grows with data size | Add limit, paginate |
| Synchronous I/O in hot path | Blocking call in profiler | Make async or cache |
| Redundant computation | Same function called N times with same args | Memoize |
| Large payload serialization | JSON.stringify on large object | Serialize only needed fields |

## Pattern: Integration Failures

**Isolate by boundary:**

```bash
# 1. Check: does the service respond at all?
curl -s -o /dev/null -w '%{http_code}' http://localhost:3000/health

# 2. Check: is auth working?
curl -s -H "Authorization: Bearer $TOKEN" http://localhost:3000/api/test

# 3. Check: is the request format correct?
curl -s -X POST -H 'Content-Type: application/json' \
  -d '{"test": true}' http://localhost:3000/api/endpoint | jq .

# 4. Check: environment variables
env | grep -E 'API_|DATABASE_|SERVICE_' | sed 's/=.*/=***/'
```

**Common root causes:**

| Symptom | Likely Cause | Check |
|---------|-------------|-------|
| 401/403 | Auth token expired/wrong | Token format, expiry, audience |
| 404 | Wrong URL or route not registered | Base URL, path prefix, trailing slash |
| 500 | Server-side exception | Server logs, not client code |
| Timeout | Service down or network issue | DNS, firewall, port binding |
| CORS error | Missing headers | Server CORS config, preflight |
| JSON parse error | Response is HTML (error page) | Check Content-Type header |

## Anti-Patterns: What NOT to Do

### The Shotgun Fix
Making 5 changes at once "to be safe." If it works, you don't know which change fixed it. If it fails, you've made the problem worse.

**Instead:** One change. Test. One change. Test.

### The Cast/Suppress Fix
```typescript
// NEVER do this
const result = brokenFunction() as any;
// @ts-ignore
const value = thing.prop;
```
This silences the compiler, not the bug. The bug will resurface at runtime.

### The Defensive Null Check
```typescript
// NEVER do this as a "fix"
if (obj && obj.prop && obj.prop.sub) {
  // use obj.prop.sub
}
```
If `obj.prop` should always exist, the fix is ensuring it exists at creation, not checking everywhere it's used.

### The "Works on My Machine" Close
If it works locally but fails in CI/production, the difference IS the bug. Compare: Node version, env vars, file system, network, timing, concurrency.
