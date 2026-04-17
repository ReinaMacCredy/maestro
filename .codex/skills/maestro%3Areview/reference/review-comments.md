# Review Comments: Examples and Patterns

How to write review findings that are actionable, specific, and calibrated.

---

## Anatomy of a Good Finding

```
[severity] file:line - What is wrong (concrete) + Why it matters (impact) + How to fix (specific)
```

Every finding has three parts:
1. **What**: The specific issue, citing code
2. **Why**: The concrete consequence if not fixed
3. **How**: A recommended fix, ideally with a diff block

---

## Good vs. Bad Review Comments

### Correctness

<Good>
```
[!!] src/api/users.ts:47 - `findUser` returns `undefined` on miss but caller
at routes.ts:23 destructures the result without null check. This crashes with
TypeError on any non-existent user lookup.

Recommended fix:
```diff
- const { name, email } = await findUser(id);
+ const user = await findUser(id);
+ if (!user) return res.status(404).json({ error: 'User not found' });
+ const { name, email } = user;
```
```
Cites both sides of the bug (producer + consumer), explains the crash scenario, provides a fix.
</Good>

<Bad>
```
The error handling could be improved here.
```
No file, no line, no specific issue, no fix. Reviewer would need to re-review to act on this.
</Bad>

---

### Security

<Good>
```
[!!] src/api/files.ts:31 - User-supplied `filename` parameter passed directly to
`path.join(uploadDir, filename)`. An attacker can use `../../etc/passwd` to read
arbitrary files. The endpoint has no auth middleware either.

Recommended fix:
```diff
- const filePath = path.join(uploadDir, req.params.filename);
+ const safeName = path.basename(req.params.filename);
+ const filePath = path.join(uploadDir, safeName);
```
Also add auth middleware to this route.
```
Names the attack, explains how to exploit it, provides a fix for the immediate issue and flags the related auth gap.
</Good>

<Bad>
```
This might have security implications. Consider adding validation.
```
"Might" is not a finding. "Consider" is not actionable.
</Bad>

---

### Performance

<Good>
```
[!] src/services/orders.ts:89 - `getOrderDetails` runs one DB query per line item
inside a loop (N+1). For an order with 50 items, this fires 51 queries.

Recommended fix: batch-fetch line items with a single `WHERE order_id = ?` query
and join in memory, or use a SQL JOIN in the original query.
```
Quantifies the impact ("51 queries"), identifies the pattern ("N+1"), offers two concrete alternatives.
</Good>

<Bad>
```
This could be more performant.
```
Could be. Or could be fine. No data, no suggestion.
</Bad>

---

### Style / YAGNI

<Good>
```
[?] src/utils/format.ts:12 - `formatCurrency` is called once (in invoice.ts:34).
Single-use utility functions in a shared utils file add indirection without reuse
benefit. Inline it at the call site.

```diff
- // utils/format.ts (delete file)
- export function formatCurrency(amount: number): string {
-   return `$${amount.toFixed(2)}`;
- }

- // invoice.ts
- import { formatCurrency } from '../utils/format';
- const display = formatCurrency(total);
+ // invoice.ts
+ const display = `$${total.toFixed(2)}`;
```
```
Verifies the single call site claim, explains the principle, shows the concrete simplification.
</Good>

<Bad>
```
I wouldn't have written it this way. Consider refactoring.
```
Personal preference framed as review feedback. No specific issue identified.
</Bad>

---

### AI Slop / De-Slop

<Good>
```
[?] src/config/loader.ts:15-22 - This try/catch swallows the JSON parse error and
returns an empty config object. The existing codebase pattern (see loader.ts:45,
parser.ts:12) is to let parse errors propagate. This silent fallback was likely
added by an AI assistant as defensive coding, but it masks config file typos.

```diff
- try {
-   return JSON.parse(raw);
- } catch {
-   return {};
- }
+ return JSON.parse(raw);
```
```
Identifies the AI pattern, references existing codebase conventions, explains the real-world impact (masked typos).
</Good>

<Bad>
```
This looks like AI-generated code. Please rewrite.
```
"Looks like AI" is not a technical finding. The issue is the silent error swallowing, regardless of who wrote it.
</Bad>

---

### Testing

<Good>
```
[!] tests/auth.test.ts - Tests cover successful login and invalid password, but
the spec (spec.md section 3.2) requires account lockout after 5 failed attempts.
No test exercises the lockout behavior.

Add a test like:
```typescript
test('locks account after 5 failed login attempts', async () => {
  for (let i = 0; i < 5; i++) {
    await login({ user: 'test', pass: 'wrong' });
  }
  const result = await login({ user: 'test', pass: 'correct' });
  expect(result.error).toBe('Account locked');
});
```
```
Traces finding back to the spec, names the missing behavior, provides a concrete test.
</Good>

<Bad>
```
Tests should be more comprehensive.
```
No information about what is missing, what the spec requires, or what to add.
</Bad>

---

## Anti-Patterns in Review Comments

### Do Not

| Anti-pattern | Problem | Instead |
|--------------|---------|---------|
| "Consider..." | Not actionable, reviewer isn't committed to the suggestion | "Change X to Y because Z" or don't mention it |
| "This could be better" | Vague, no direction | Specify what "better" means and provide a fix |
| "I would have done it differently" | Personal preference, not a defect | Only flag if the current approach has a concrete downside |
| "Nit: ..." on 10+ items | Noise drowns signal | Batch nits into one item: "Formatting: 12 instances, auto-fixable with `eslint --fix`" |
| Restating the code | Wastes reader's time | State the issue, not the code's behavior |
| "LGTM" with no detail | Provides no verification evidence | At minimum: "PASS. Intent match verified against spec sections 1-4. All 12 tests pass." |
| Asking questions as findings | Findings assert; questions belong in NEEDS_DISCUSSION | If uncertain, label as "Uncertain" with what evidence would confirm |

### Calibration Rules

- **One finding per issue.** Don't split a single problem into 3 nits.
- **One fix per finding.** If a finding requires 2 unrelated changes, split it.
- **Batch mechanical issues.** "12 trailing whitespace violations" is one finding, not 12.
- **Positive findings matter.** Include at least one concrete positive observation. "Good use of discriminated unions for state machine" is more useful than "Code looks clean."
- **Uncertainty is fine; vagueness is not.** "Uncertain: this might cause a race condition if `processQueue` is called concurrently -- needs verification with a concurrent test" is a good finding. "There might be issues" is not.

---

## Severity Calibration Examples

### Blocker `[!!]` -- Must fix, blocks merge

- Spec requirement not implemented
- Null pointer crash on common path
- SQL injection via user input
- Auth check removed from endpoint
- Test suite fails
- Data loss on error path

### Major `[!]` -- Must fix or provide written justification

- Error path returns wrong status code
- Missing test for spec requirement
- N+1 query in hot path
- Style guide violation
- Swallowed exception hides failures
- Race condition under realistic load

### Minor `[?]` -- Should fix, doesn't block

- Missing edge case test (non-critical path)
- Naming inconsistency
- Missing doc comment on public function
- Slightly complex code that could be simplified
- Log message at wrong level

### Nit `[.]` -- Optional, take it or leave it

- Formatting preference within acceptable range
- Comment wording
- Variable name alternative that's equally valid
- Import ordering (when no linter enforces it)
- Subjective architectural preference with no concrete downside
