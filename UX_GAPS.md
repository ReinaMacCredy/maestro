# Maestro UX Gaps

Use this as a lightweight backlog for out-of-scope Maestro UX gaps noticed
while doing other work. Keep entries short: date, surface, observed friction,
and why it is not part of the current fix.

## 2026-07-04

- Surface: `maestro task setup` harness guidance.
  Observed friction: `.maestro/harness/HARNESS.md` shows
  `maestro task setup --task ... --start`, but the repeatable `--task` shape is
  not obvious enough in the quick path; I first tried a plausible `--step` flag
  and got `unexpected argument '--step'`.
  Later scope: make the harness quick path include a concrete two-row example or
  point directly at the generated CLI reference.
- Surface: global skill sync for unmanaged local skills.
  Observed friction: installing the new binary reported that
  `/Users/reinamaccredy/.maestro/skills/maestro-design/SKILL.md` differs from
  the embedded skill and is not recorded as Maestro-managed. The warning gives a
  blunt move-aside-or-restore remediation, but there is no safe guided diff or
  adopt path for a locally edited skill.
  Later scope: add a guided global-skill reconcile/adopt flow that preserves
  local edits and applies shipped updates intentionally.
- Surface: `maestro feature accept --qa`.
  Observed friction: `maestro feature accept loop-chain-readout-and-trace --qa
  cli --dry-run` failed with `unsupported --qa value 'cli'; only '--qa none' is
  accepted`, even though behavioral CLI work needs a way to name an explicit QA
  surface.
  Later scope: either support typed QA surfaces or make the accepted `--qa none`
  escape hatch and baseline command the only surfaced workflow.
- Surface: `maestro feature prepare` inline task flags.
  Observed friction: `maestro feature prepare ... --task ... --check ...`
  failed with `prepare plan must contain at least one explicit task entry`, while
  generated CLI reference advertises inline `--task`, `--check`, `--covers`,
  `--blocker`, and `--after` flags.
  Later scope: reconcile CLI behavior, help text, and skill references so inline
  task setup either works or is not advertised.
- Surface: concurrent run busy notice.
  Observed friction: task completion printed `[busy]
  019f28bf-02cf-7322-b982-4d2a117a90ac is running the full-suite gate; hold
  heavy runs until it clears`. It explains the contention but gives no exact
  read command to check when the hold has cleared.
  Later scope: add a next-step pointer such as `maestro active` or a session
  status command to busy notices.
