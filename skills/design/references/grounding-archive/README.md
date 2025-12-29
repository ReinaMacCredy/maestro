# Grounding Archive — DEPRECATED

> ⚠️ **These files are archived. The grounding system has been replaced by the Research Protocol.**

## New Location

All verification functionality is now in: `skills/conductor/references/research/`

## Archived Files

| File | Replacement |
|------|-------------|
| `tiers.md` | Research always runs at full capacity |
| `router.md` | Parallel agents replace cascading |
| `cache.md` | Session memory in research protocol |
| `sanitization.md` | Handled by research agents |
| `impact-scan-prompt.md` | Now the Impact agent |
| `schema.json` | Research output format |

## Why Archived

The tiered grounding system had these issues:
- Sequential execution was slow
- Skip conditions led to missed context
- Tiered intensity was inconsistent

The new research protocol:
- Always spawns parallel agents
- No skip conditions
- Faster due to parallelization
- More comprehensive coverage

## Reference

See [conductor/references/research/protocol.md](../../../conductor/references/research/protocol.md) for the new system.
