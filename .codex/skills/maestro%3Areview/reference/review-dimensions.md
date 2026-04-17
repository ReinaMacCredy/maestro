# Review Dimensions

Analyze the diff against these 5 dimensions. For each dimension, produce findings with severity labels (`[!!]` blocker, `[!]` major, `[?]` minor, `[.]` nit).

---

## 6.1: Intent Match

Compare implementation against spec.md:

### Checklist

- For each acceptance criterion in spec, verify it's addressed in the code
- Flag any spec requirements that appear unimplemented
- Flag any implemented behavior not in the spec (scope creep)
- Verify edge cases mentioned in the spec are handled

### How to Verify

1. Extract every "MUST", "SHOULD", "SHALL" from the spec
2. For each requirement, find the code that implements it
3. For each requirement, find the test that verifies it
4. Mark: `[ok]` implemented + tested, `[~]` implemented but untested, `[x]` missing

### Severity Rules

- Missing spec requirement: always `[!!]` blocker
- Partial implementation (happy path only, missing error handling): `[!]` major
- Scope creep (extra features not in spec): `[!]` major if it adds complexity, `[?]` minor if harmless
- Spec ambiguity found during review: escalate to NEEDS_DISCUSSION

---

## 6.2: Code Quality

Review against code style guides and general quality.

### By Language

**TypeScript / JavaScript:**
- Consistent use of `const` vs `let` (prefer `const`)
- No `any` without documented reason
- Async/await over raw Promises (unless `.all()` / `.race()` needed)
- Error types narrowed in catch blocks (not `catch (e: any)`)
- Imports organized (externals, then internals, then types)
- No barrel re-exports that pull in unused code

**Python:**
- Type hints on function signatures
- Dataclasses or Pydantic over raw dicts for structured data
- Context managers for resources (files, connections)
- No mutable default arguments
- f-strings over `.format()` or `%`

**Rust:**
- `Result`/`Option` over panics in library code
- No `.unwrap()` outside tests
- Derive traits used appropriately
- Lifetime annotations minimal and correct

**Go:**
- Errors checked immediately after call
- No `_` for error values (except intentional discard with comment)
- Interfaces defined where consumed, not where implemented
- Context propagation in all IO paths

### General Quality Checks

- Function/method < 40 lines (suggest extraction above this)
- Cyclomatic complexity: flag functions with > 5 branches
- Code duplication: flag 3+ identical blocks
- Error handling: consistent pattern within a module
- Naming: verbs for functions, nouns for types, descriptive for variables

**Code style guides are the Law. Violations are `[!]` major severity by default. Only downgrade with explicit written justification in the finding.**

When reporting a style violation, include an explicit diff block:

```diff
- non_compliant_code_here
+ compliant_code_here
```

---

## 6.3: Test Coverage

Assess test quality, not just test existence.

### Coverage Checklist

- [ ] All acceptance criteria have a corresponding test
- [ ] Tests verify behavior, not implementation details
- [ ] Edge cases from spec are tested
- [ ] Error scenarios are tested (invalid input, network failure, permissions)
- [ ] Tests are deterministic (no time-dependent, order-dependent, or flaky patterns)

### What to Flag

| Finding | Severity | Example |
|---------|----------|---------|
| Acceptance criterion with no test | `[!!]` | Spec says "validate email format" but no test for invalid emails |
| Happy path only, no error tests | `[!]` | Tests `createUser(valid)` but not `createUser(duplicate)` |
| Tests mock everything | `[!]` | Unit test mocks the function it's testing |
| Test name doesn't describe behavior | `[?]` | `test('it works')` or `test('test1')` |
| Test setup > 20 lines | `[?]` | Suggests design issue, not just test issue |
| Snapshot tests for logic | `[?]` | Snapshots test serialization, not behavior |
| `skip` / `todo` tests committed | `[!]` | Deferred tests that should block completion |

### Test Quality Signals

**Good test**: Reads like a specification. Name says what happens, body shows setup-act-assert.

**Bad test**: Reads like an implementation log. Heavy mocking, multiple assertions testing internals, name describes code not behavior.

---

## 6.4: Security

Security review scoped to the diff. Not a full audit -- focus on regressions and new attack surface.

### By Category

**Input Validation:**
- All user input validated before use (length, type, format, range)
- File paths validated against traversal (`../`)
- URLs validated against SSRF (no internal/localhost access from user input)
- Numeric inputs checked for overflow/underflow

**Injection:**
- SQL: parameterized queries, no string concatenation
- Command: no `exec`/`spawn` with user input; use argument arrays
- HTML/XSS: output encoding in templates, no `dangerouslySetInnerHTML` with user data
- Template: no user input in template strings evaluated by engines
- LDAP/XPath: parameterized queries

**Authentication / Authorization:**
- Auth checks not removed or weakened
- New endpoints have auth middleware
- Role/permission checks on sensitive operations
- Session/token handling follows project patterns

**Secrets:**
- No hardcoded credentials, API keys, tokens
- No secrets in logs, error messages, or stack traces
- `.env` files not committed
- Secrets accessed via environment variables or secret manager

**Data Handling:**
- PII not logged or exposed in error messages
- Sensitive data encrypted at rest and in transit
- Proper cleanup of temporary files containing sensitive data

### Severity Rules

- Any exploitable vulnerability: `[!!]` blocker
- Missing validation on user input: `[!]` major
- Theoretical vulnerability (requires unlikely conditions): `[?]` minor
- Best practice not followed but no concrete attack vector: `[.]` nit

---

## 6.5: Product Guidelines Compliance

Only run this dimension if `.maestro/context/product-guidelines.md` was loaded.

### Check Areas

- **Branding**: naming, logos, terminology match guidelines
- **Voice and tone**: copy strings, error messages, UI text match voice guidelines
- **UX principles**: interaction patterns, accessibility, flow expectations
- **Accessibility**: ARIA labels, keyboard navigation, contrast ratios (if applicable)
- **Internationalization**: hardcoded strings vs. i18n keys (if i18n is a project concern)

### Severity Rules

- User-facing text violating brand guidelines: `[!]` major
- Accessibility regression (removed ARIA, broken tab order): `[!]` major
- Minor tone mismatch in error messages: `[?]` minor
- Missing i18n key for new string (in i18n project): `[?]` minor

Flag any deviation as a finding with severity appropriate to the impact.
