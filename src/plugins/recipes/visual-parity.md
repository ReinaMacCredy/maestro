# Visual parity

Use this recipe when two UI implementations must match pixel for pixel, or
when a styling system migrates with no visible change. You own pixel-exact
equivalence. The baseline is the spec, and you never touch it. An image diff
decides equivalence, not your eye. Ported from pstack's visual-parity playbook
(Lauren Tan, MIT; notice in THIRD_PARTY.md).

## Steps

1. Establish the baseline before any migration. It is a visual regression
   harness that screenshots the current component across its states, plus the
   target when you match two implementations. No baseline, no parity claim.
   The baseline is a blocking prerequisite, never a follow-up.
2. State the anti-shortcut clauses and hold them. No harness edits, no
   baseline edits, no component restructuring to make a diff pass. If the
   baseline looks wrong, stop and ask the owner (WORKFLOW.md Per-tool
   adapters). Never edit it.
3. Migrate one component at a time. Shared components migrate first, as a
   blocking phase. After that, parallelize across worktrees with one owner per
   component (`maestro recipe show worktree`),
   per **Separate Before Serializing Shared State** (PRINCIPLES.md). A
   subagent under the `delegate` profile writes each migration, and you review
   its diff.
4. Verify each component against its baseline with an image diff, driven
   through the repository's verify-<repo> skill
   (`maestro recipe show verification-skill`). A nonzero diff is a fail.
   Investigate the pixel delta and repeat the component until its diff is
   zero. An unattended run repeats through an external driver
   (`maestro recipe show unattended`).
5. Deliver: `maestro recipe show opening-a-pr`.
   - Deliver per component or per safe batch.

## Reply

The components migrated, the diff result for each, the baseline harness
location, and what is left.
