# Cross-CLI Support

Mission-specific notes for expanding Mission Control beyond Claude/Codex-only operational behavior.

---

## Goals

- Keep `src/domain/agents.ts` as the human-readable source of supported hosts.
- Normalize host/session/event metadata before it reaches core usecases.
- Preserve existing Claude/Codex behavior while adding Droid and other supported hosts in scope.

## Expectations

- Unsupported host context should degrade cleanly.
- Host-specific hook/event gaps should not block normal command execution.
- Session detection must be testable through fixtures/env-driven scenarios rather than requiring real live sessions only.

## Verification Notes

- Add regression coverage for existing detection paths before extending them.
- Prefer fixture-driven tests for host-specific source-path/session parsing.
- Keep operator-facing output host-agnostic wherever possible.
