# Report Template and Post-Review Protocol

## Report Format

```
## Review Report: {track_description}

**Track**: {track_id}
**Commits**: {sha_list}
**Files changed**: {count}

### Summary
{1-2 sentence overall assessment}

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
{findings with severity: [!] critical, [?] suggestion}

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
1. {fix description} -- {file}:{line}
   ```diff
   - old_code
   + new_code
   ```
2. {fix description} -- {file}:{line}

### Verdict
{PASS | PASS WITH NOTES | NEEDS CHANGES}
```

---

## Auto-fix Protocol

If the verdict is PASS WITH NOTES or NEEDS CHANGES:

Ask the user: "Apply auto-fixes for the suggested changes?"
Options:
- **Yes, apply fixes** -- Make the suggested changes automatically
- **No, manual only** -- I'll handle fixes myself
- **Show me each fix** -- Review and approve each fix individually
- **Complete Track (ignore warnings)** -- Mark track complete without fixing warnings

If auto-fix: apply changes, run tests, commit:
```bash
git add {changed_files}
git commit -m "fix(review): apply review fixes for track {track_id}"
```

After committing, capture the new commit SHA and update `plan.md` with a new section:

```markdown
## Review Fixes

| Fix | Commit |
|-----|--------|
| {fix_description} | {commit_sha} |
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
