# Review Dimensions

Analyze the diff against these 5 dimensions.

---

## 6.1: Intent Match

Compare implementation against spec.md:
- For each acceptance criterion in spec, verify it's addressed in the code
- Flag any spec requirements that appear unimplemented
- Flag any implemented behavior not in the spec (scope creep)

## 6.2: Code Quality

Review against code style guides and general quality:
- Naming conventions consistent with project style
- Function/method size and complexity
- Code duplication
- Error handling patterns
- Proper use of language idioms

**Code style guides are the Law. Violations are High severity by default. Only downgrade severity with explicit written justification in the finding.**

When reporting a style violation, include an explicit diff block showing the required change:

```diff
- non_compliant_code_here
+ compliant_code_here
```

## 6.3: Test Coverage

Assess test quality:
- Are all acceptance criteria covered by tests?
- Do tests verify behavior (not implementation details)?
- Are edge cases from spec tested?
- Are error scenarios tested?

## 6.4: Security

Basic security review:
- Input validation at boundaries
- No hardcoded secrets or credentials
- SQL/command injection prevention
- XSS prevention (for web code)
- Auth/authz checks where appropriate

## 6.5: Product Guidelines Compliance

Only run this dimension if `.maestro/context/product-guidelines.md` was loaded.

Check the implementation against product guidelines:
- Branding rules (naming, logos, terminology)
- Voice and tone (copy strings, error messages, UI text)
- UX principles (interaction patterns, accessibility, flow expectations)

Flag any deviation as a finding with severity appropriate to the impact.
