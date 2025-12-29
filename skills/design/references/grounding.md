# Grounding — DEPRECATED

> ⚠️ **This system has been replaced by the Research Protocol.**
> 
> All grounding functionality is now handled by parallel research agents.

## Redirect

**New location:** [conductor/references/research/protocol.md](../../conductor/references/research/protocol.md)

## Quick Reference

| Old Grounding | New Research |
|---------------|--------------|
| Sequential tools (Grep → finder → web) | Parallel sub-agents |
| Tiered intensity (Light/Mini/Standard/Full) | Always full agent dispatch |
| `/ground <query>` | `/research <query>` |
| Grounding at phase transitions | Research at all integration points |

## Migration

### Old Command
```
/ground how to create Stripe customer
```

### New Command
```
/research how to create Stripe customer
```

## Integration Points

Research runs automatically at:

| Point | Trigger | Agents |
|-------|---------|--------|
| `ds` start | DISCOVER phase | Locator + Pattern |
| DEVELOP → DELIVER | Phase transition | All 4 agents |
| `/conductor-newtrack` | Pre-spec | All 5 agents |

## Documentation

- [Research Protocol](../../conductor/references/research/protocol.md)
- [Agents](../../conductor/references/research/agents/)
- [Hooks](../../conductor/references/research/hooks/)

---

## Legacy Files (Archived)

The following files are kept for reference but no longer used:

- [tiers.md](grounding/tiers.md) - Old tier definitions
- [router.md](grounding/router.md) - Old cascading logic
- [cache.md](grounding/cache.md) - Old caching (now handled by research)
- [sanitization.md](grounding/sanitization.md) - Moved to research protocol
- [impact-scan-prompt.md](grounding/impact-scan-prompt.md) - Now part of Impact agent
