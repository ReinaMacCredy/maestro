# Report Template and Post-Review Protocol

## Report Format

```
## Review Report: {track_description}

**Track**: {track_id}
**Commits**: {sha_list}
**Files changed**: {count}

### Summary
{1-2 sentence overall assessment}

## Automated Check Results
- [ok] TypeScript: 0 errors
- [!]  ESLint: 3 errors in 2 files (2 auto-fixable)
- [ok] Tests: 47 passed, 0 failed
- [--] Format: not configured

## Verification Checks
- [ ] **Intent Match**: [Yes/No/Partial] - {comment}
- [ ] **Style Compliance**: [Pass/Fail] - {comment}
- [ ] **Test Coverage**: [Yes/No/Partial] - {comment}
- [ ] **Test Results**: [Passed/Failed] - {summary}
- [ ] **Security**: [Pass/Fail] - {comment}
- [ ] **Product Guidelines**: [Pass/Fail/N/A] - {comment}

### Intent Match
- [ok] {criterion met}
- [!] {criterion not fully met}: {explanation}

### Code Quality
{findings with severity labels}

For each violation include a diff block:
```diff
- old_code
+ new_code
```

### Test Coverage
{findings}

### Security
{findings}

### Product Guidelines
{findings, or "N/A -- no product-guidelines.md found"}

### Suggested Fixes
1. [severity] {fix description} -- {file}:{line}
   ```diff
   - old_code
   + new_code
   ```
2. [severity] {fix description} -- {file}:{line}

### Verdict
{PASS | PASS WITH NOTES | NEEDS CHANGES}
```

### Verdict Definitions

| Verdict | Meaning | When to use |
|---------|---------|-------------|
| **PASS** | Implementation is correct and complete | Zero blockers, zero majors, minors are trivial |
| **PASS WITH NOTES** | Correct but has improvement opportunities | Zero blockers, zero majors, has minors worth noting |
| **NEEDS CHANGES** | Must be fixed before merging | Has blockers or majors that affect correctness/security |

---

## Auto-fix Protocol

If the verdict is PASS WITH NOTES or NEEDS CHANGES:

Ask the user: "Apply auto-fixes for the suggested changes?"
Options:
- **Yes, apply fixes** -- Make the suggested changes automatically
- **No, manual only** -- I'll handle fixes myself
- **Show me each fix** -- Review and approve each fix individually
- **Complete Track (ignore warnings)** -- Mark track complete without fixing warnings

### Auto-Fixable vs. Human-Judgment Boundary

Not every finding can be safely auto-fixed. Use this classification:

#### Auto-fixable (apply without asking)

These are mechanical transformations with zero behavior change risk:

| Category | Examples |
|----------|----------|
| **Formatting** | Indentation, trailing whitespace, line length, semicolons |
| **Linter auto-fix** | `eslint --fix`, `ruff format`, `cargo fmt` |
| **Import ordering** | Sorting, grouping, removing unused imports |
| **Type narrowing** | Adding explicit types where inference is clear |
| **Dead code removal** | Removing clearly unreachable code, unused variables flagged by compiler |
| **Comment cleanup** | Removing AI-generated restating comments (e.g., `// increment counter` above `counter++`) |

#### Semi-auto (show diff, ask for approval)

These are likely correct but could change behavior in edge cases:

| Category | Why it needs review |
|----------|-------------------|
| **Error handling changes** | Changing fallback to throw could break callers |
| **Simplification refactors** | Inlining or extracting could change scope/closure behavior |
| **Removing try/catch** | Swallowed errors might be intentional in some contexts |
| **Renaming** | Could affect serialization, API contracts, or external references |
| **Adding validation** | New validation could reject previously-accepted input |

#### Human-only (never auto-fix)

These require understanding intent, architecture, or user-facing impact:

| Category | Why it can't be automated |
|----------|--------------------------|
| **Missing spec requirements** | Requires implementing new functionality |
| **Architecture changes** | Different abstractions, patterns, module boundaries |
| **Security fixes** | Risk of breaking auth flows, introducing new vulnerabilities |
| **Test additions** | Tests require understanding expected behavior |
| **API changes** | Consumers must be updated |
| **Performance fixes** | Require profiling to validate improvement |
| **Business logic changes** | Require product understanding |

### Auto-fix Execution

If auto-fix accepted: apply changes, run tests, commit:

```bash
# Apply linter auto-fixes first
{lint_command} --fix {changed_files}
{format_command} --write {changed_files}

# Apply reviewer-suggested fixes
# (edit files as specified in the review findings)

# Verify nothing broke
CI=true {test_command}
{typecheck_command}

# Commit
git add {changed_files}
git commit -m "fix(review): apply review fixes for track {track_id}"
```

**If tests fail after auto-fix**: Revert the auto-fix, report the failure, and reclassify the finding as human-only.

After committing, capture the new commit SHA and update `plan.md` with a new section:

```markdown
## Review Fixes

| Fix | Severity | Category | Commit |
|-----|----------|----------|--------|
| {fix_description} | {severity} | auto/semi/manual | {commit_sha} |
```

Write this section to `.maestro/tracks/{track_id}/plan.md` appended after existing content.

---

## Post-Review Cleanup

After the review is complete (verdict delivered and any fixes applied):

Ask the user: "Review complete. What would you like to do with this track?"
Options:
- **Archive** -- Move track to .maestro/archive/
- **Delete** -- Remove track files entirely
- **Keep** -- Leave track as-is for further work
- **Skip** -- Do nothing

- **Archive**: Move `.maestro/tracks/{track_id}/` to `.maestro/archive/{track_id}/`
- **Delete**: Remove `.maestro/tracks/{track_id}/` entirely
- **Keep** / **Skip**: No file changes
