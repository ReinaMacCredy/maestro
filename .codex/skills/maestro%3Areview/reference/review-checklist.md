# Review Checklist by Concern Area

Use this checklist during Step 6 (Review Dimensions). Check each item that applies to the diff. Not every item applies to every review -- skip sections for unchanged concern areas.

---

## Correctness

- [ ] All spec acceptance criteria are implemented
- [ ] Edge cases handled: empty input, null, undefined, zero, negative, max values
- [ ] Error paths return/throw appropriate errors (not silent fallbacks)
- [ ] Async operations have proper error handling (no unhandled promise rejections)
- [ ] State transitions are valid (no impossible states reachable)
- [ ] Race conditions considered for concurrent operations
- [ ] Resource cleanup on all exit paths (files, connections, timers)
- [ ] Boundary conditions: off-by-one, empty collections, single-element collections
- [ ] Type narrowing is correct (no unsafe casts hiding real type mismatches)
- [ ] Return values are used by callers (no silently dropped results)

## Security

- [ ] User input validated at every entry point (API, CLI, file, env)
- [ ] No SQL/command/template injection vectors
- [ ] No hardcoded secrets, API keys, passwords, tokens
- [ ] No secrets in log output or error messages
- [ ] Auth/authz checks present on new endpoints or operations
- [ ] File path operations sanitized against traversal
- [ ] Crypto: using standard libraries, not custom implementations
- [ ] Dependencies: no known vulnerabilities in new dependencies
- [ ] CORS/CSP headers appropriate (web applications)
- [ ] Rate limiting on new public endpoints

## Performance

- [ ] No N+1 queries (database calls in loops)
- [ ] No repeated parsing of the same data
- [ ] Large collections: pagination or streaming, not load-all
- [ ] No blocking operations on main thread / event loop
- [ ] Caching: appropriate use, proper invalidation
- [ ] No memory leaks (event listeners, subscriptions, timers cleaned up)
- [ ] String building: no quadratic concatenation in loops
- [ ] Regex: no catastrophic backtracking patterns
- [ ] File I/O: buffered, not byte-at-a-time
- [ ] Network: batch requests where possible, avoid waterfalls

## Style & Consistency

- [ ] Naming follows project conventions (casing, prefixes, verbs/nouns)
- [ ] Error handling matches nearby code patterns
- [ ] Logging level and format matches project patterns
- [ ] Import organization matches project patterns
- [ ] No style drift from surrounding code
- [ ] Comments explain "why", not "what"
- [ ] No AI-generated restating comments
- [ ] Magic numbers/strings extracted to named constants
- [ ] Consistent use of language features (async/await vs promises, etc.)

## Testing

- [ ] Each acceptance criterion has a test
- [ ] Tests verify behavior, not implementation
- [ ] Error cases tested (invalid input, failures, timeouts)
- [ ] Tests are deterministic (no time/order/network dependencies)
- [ ] Test names describe the behavior being tested
- [ ] No skipped tests committed without explanation
- [ ] Test data is minimal and intentional (not copy-pasted fixtures)
- [ ] Mocks are minimal (only external dependencies, not internal logic)
- [ ] Assertions are specific (not just "no error thrown")

## Architecture & Design

- [ ] Changes follow existing patterns in the codebase
- [ ] No premature abstraction (abstractions have 2+ concrete uses)
- [ ] Module boundaries respected (no reaching into internal implementation)
- [ ] Dependencies flow in one direction (no circular references)
- [ ] Public API surface is minimal (private by default)
- [ ] Configuration separated from logic
- [ ] No feature flags or modes added "for future use"

## Documentation & API

- [ ] Public functions/methods have doc comments (if project convention)
- [ ] Breaking API changes documented
- [ ] New configuration options documented
- [ ] Error messages are actionable (user knows what to do)
- [ ] Migration path provided for breaking changes

---

## Quick Reference: Severity by Checklist Failure

| Area | Typical severity |
|------|-----------------|
| Correctness: spec requirement missing | `[!!]` blocker |
| Correctness: edge case unhandled | `[!]` major |
| Security: exploitable vulnerability | `[!!]` blocker |
| Security: missing input validation | `[!]` major |
| Performance: N+1 query | `[!]` major |
| Performance: unoptimized but functional | `[?]` minor |
| Style: project convention violated | `[!]` major (style guides are law) |
| Style: subjective improvement | `[.]` nit |
| Testing: untested acceptance criterion | `[!!]` blocker |
| Testing: missing error case test | `[!]` major |
| Architecture: premature abstraction | `[?]` minor |
| Documentation: missing doc comment | `[.]` nit |
