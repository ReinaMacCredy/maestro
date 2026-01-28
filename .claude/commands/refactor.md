---
description: Intelligent refactoring workflow with test coverage verification and atomic commits
allowed-tools: Bash, Read, Write, Edit, Glob, Grep, Task
context: fork
agent: atlas-kraken
model: sonnet
---

# Refactor Workflow

Systematic refactoring with test coverage and atomic commits.

## Target

$ARGUMENTS

## Workflow

### 1. Analysis Phase

- Identify refactoring scope
- Map affected files
- Check existing test coverage
- Document current behavior

### 2. Test Coverage Check

```bash
# Run existing tests with coverage
npm test -- --coverage
# or: pytest --cov
```

If coverage is insufficient, write tests for current behavior FIRST.

### 3. Refactor Incrementally

For each logical change:
1. Make the change (single, focused modification)
2. Run tests (verify nothing broke)
3. Commit (atomic commit with clear message)

```bash
npm test && git add -p && git commit -m "refactor(scope): description"
```

### 4. Verification

- All tests pass
- No regressions
- Code quality improved
- Performance maintained

## Principles

1. **Never refactor without tests** - Write them first if missing
2. **One logical change per commit** - Makes reverting easy
3. **Preserve behavior** - Refactoring doesn't change what code does
4. **Run tests after every change** - Catch regressions immediately

## Delegation

For large refactors, the orchestrator delegates to:
- `atlas-kraken` - TDD implementation
- `atlas-code-reviewer` - Quality verification
- `atlas-explore` - Find affected code paths

---

## References

- [Atlas SKILL.md](../skills/atlas/SKILL.md)

**Spawns**: atlas-kraken
**Uses Skills**: atlas, git-master
