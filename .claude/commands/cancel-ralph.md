---
description: Cancel active Ralph Loop autonomous execution
allowed-tools: Bash
---

# Cancel Ralph

Stop the currently active Ralph Loop.

## Usage

```
/cancel-ralph
```

## How It Works

1. Run `./scripts/cancel-ralph.sh`
2. Clears `.atlas/ralph-loop.local.md` state file
3. In-flight agents complete their current task but no new tasks are delegated

## Response

```
[OK] Ralph loop cancelled
  - State cleared: .atlas/ralph-loop.local.md
  - No further autonomous execution will occur

To resume work manually, use `/atlas-work`.
```

---

## References

- [Atlas SKILL.md](../skills/atlas/SKILL.md)

**Related**: `/ralph-loop` (starts the loop this cancels)
**Uses Skills**: atlas
