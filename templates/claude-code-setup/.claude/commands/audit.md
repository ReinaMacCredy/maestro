---
description: Example command that audits files against project standards
allowed-tools: Read, Grep, glob
argument-hint: <file-or-directory>
---

# Audit Code

## Target

Audit: $ARGUMENTS

## Instructions

1. Read the target file(s) or directory
2. Check for common issues:
   - Missing type annotations
   - Unused imports
   - TODO comments
   - Console.log statements
3. Report findings with file paths and line numbers
4. Offer to fix issues if appropriate

## Output Format

```
# Audit: {path}

## Issues Found

- {file}:{line} - {issue description}
- {file}:{line} - {issue description}

## Summary

- Total issues: {count}
- Files checked: {count}

## Suggestions

[Actionable recommendations]
```
