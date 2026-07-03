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
