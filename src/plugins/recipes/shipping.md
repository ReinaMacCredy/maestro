# Shipping

Use this recipe when the owner's explicit words ask to land a named PR or
stack. It verifies each PR independently, lands only the verified run from
the root, and then keeps its hands off the stack. It begins where
`maestro recipe show babysit` ends. Ported from pstack's shipping,
autopilot-full and autopilot-stack playbooks (Lauren Tan, MIT; notice in
THIRD_PARTY.md).

You own what lands. Merge is an owner gate under every grant
(`~/maestro/WORKFLOW.md#authorization-boundaries`). Neither `policy-auto-pr`
nor an owner's commit and PR grant authorizes it. Only the owner's explicit
words for this stack do.

## Steps

1. Confirm the owner's words to land this stack. Quote them in a work note
   before the first merge, with `maestro work note <id> "land: <owner quote>"`.
   Without them, stop at merge-ready and report.
2. Verify each PR independently before it lands. `gh` is the forge CLI.
   Never require Graphite (`gt`). One verifier per PR, a fresh subagent that
   did not write the code, runs the change the way a user would through the
   repository's verify-<repo> skill (`maestro recipe show verification-skill`),
   parent against head. It posts `PASS`, `PASS+NOTES` or `FAIL` on that PR.
   Only a verdict from an agent that did not write the code counts, because
   an author checks what it meant to build. Green CI and an approving bot
   review are not verdicts, because neither runs the change as a user would.
3. Land only the contiguous verified run rooted at the bottom. Walk up from
   the lowest unmerged PR and stop at the first one without `PASS` or
   `PASS+NOTES`. The PRs below it are the run, and the top one is the
   ceiling. A verified PR above an unverified one waits, because it cannot
   land without the code below it. A PR whose branch neither holds nor needs
   that code can move onto trunk and land on its own. It needs a fresh
   verdict first, because its old one ran with that code beneath it and
   trunk does not have it.
4. Re-check that each verdict still describes the code. A verdict covers a
   patch, and a chain rebase moves every PR above it to a new SHA without
   failing any check. Compare `git patch-id` at the verdict SHA with the
   current head before you trust an older verdict.
   - When the patches differ only in tests, docs or lint config, a lane's
     result stands if what it ran builds the same at both SHAs apart from
     noise. Noise is a difference that also shows up between two builds of
     the verdict SHA, or an embedded commit SHA. Report each kind of noise
     with its files, so a reader can check the call.
   - The change itself still gets fresh checks and a review, because the old
     verdict never saw it.
   - A lane with no build output, such as a dev server, cannot show this.
     Rerun it, and re-verify anything else that drifted.
5. Land bottom-up, one PR at a time. Always merge the run's lowest unmerged
   PR, with `gh pr merge <pr> --squash`.
   - A PR whose base is still its parent's branch merges into that branch,
     not trunk.
   - Read the PR back with `gh pr view <pr> --json state,mergedAt,baseRefName`,
     because a merge command can report success when a merge queue only
     queued the PR, and a requested auto-merge is not a merge. A landed PR
     reads merged with trunk as its base.
   - Arm `--auto` only on a PR whose verdict covers its current head,
     because auto-merge lands whatever head is ready.
   - After a merge lands, retarget the PR whose base was the merged branch
     to trunk (`gh pr edit <pr> --base <trunk>`), move its patch onto the
     merge result, and push with
     `git push --force-with-lease=<branch>:<sha you last saw>`. Do this only
     on a branch this run alone pushes. A force-push to a shared branch
     pauses for the owner.
   - The verdict covers the patch, not its commits, so the patch-id rule of
     step 4 decides whether it survives. Merge that PR once its checks pass
     and its verdict survives.
6. Leave the stack alone while it lands. The move in step 5 is the only
   change to a branch or base, because a merge lands whatever its branch and
   base hold at that moment, verdict or not. Watch with
   `gh pr checks <pr> --watch` and re-read with
   `gh pr view <pr> --json state,mergedAt,baseRefName` each time the watch
   returns, until the ceiling reads merged. A run that must outlive the
   session uses an external driver (`maestro recipe show unattended`). A
   base that moves to trunk when its parent merges is expected, not damage
   to repair. If a merge stalls, find out why before you change anything.
7. Stop at the ceiling. Extending the run takes new verdicts, so it is a new
   pass from step 2.

## Queues

A queue is a set of PRs built at once, one PR owner per PR, under a delivery
grant. A PR owner is an agent; the owner is the human whose words gate a
merge. The root is the SLP Lead or the main chat that dispatched the PR
owners. Fan-out discipline (brief with standing orders, rolling window,
retry by failure mode, pilot unit) is `maestro recipe show fan-out`.

- The owner's gates come first. Landing any queue PR still needs the
  owner's words for that queue (step 1). Items the owner names as theirs and
  PRs outside the queue stop at merge-ready. A request to state the plan is
  not a go.
- Each PR owner is an SLP Peer seat (`maestro recipe show slp`) whose brief
  carries the grant; it delegates code edits like any seat
  (`maestro recipe show fan-out`). It builds, opens its PR per
  `maestro recipe show opening-a-pr`, proves the change on the running
  product, and babysits to green per `maestro recipe show babysit`. Each
  branch has one writer. The root never pushes a fix itself; it replaces a
  stuck PR owner instead.
- A PR owner opens its PR early, so CI and review bots run beside the work.
  The code is ready only after the simplification pass and
  `maestro recipe show no-comments`. The PR owner then rebases onto current
  trunk, runs the repository's checks for the touched paths on the committed
  head, and reports the code-ready head before self-proof. It reports every
  later head, and merge-ready once self-proof, CI and babysit are done. Fix
  rounds stay on that base unless trunk conflicts or breaks CI, because each
  rebase restarts CI.
- Assume a PR owner can die mid-run. It pushes soon after starting and after
  every unit worth keeping, and keeps its trail in work notes on its item.
  Whoever spawns a subagent records its id, state and a stuck-after runtime
  in a work note, so the root can probe it. A PR owner force-pushes only its
  own branch, with `--force-with-lease` pinned to the SHA it last pushed or
  took over. On any push it did not make, it stops and reports to the root.
- The root verifies each PR before its PR owner may land it. A full fan-out
  of independent verifiers runs once, at the code-ready head. Another runs
  only for new behavior, a redesign or a large new diff. A rebase, a
  conflict resolution or a minor fix gets one verifier at the new head.
  Verifiers re-run the gates, prove the load-bearing behavior on the running
  product through verify-<repo>, and audit the diff. No verdict is clean
  without that live lane, or before the root audits the receipts in the
  merge-ready report. The root sends every proven finding to the PR owner in
  one fix-forward, with a red test across every site with the same defect.
  A defect trunk shows without the PR is recorded against trunk and does not
  gate the PR. One the PR copies or newly reaches is the PR's own.
- Merge prep starts only after the owner's words to land. The PR owner
  rebases onto current trunk, reports the new head, and waits for green CI.
  Green CI alone never authorizes a merge. Right before merging, it fetches
  trunk and asks whether trunk's changes can affect the patch. It rebases
  and waits for CI when the code that would land was never tested, as with a
  conflict, a file both sides changed, or a change to which checks run. A
  lockfile both sides changed needs no rebase when a frozen install passes
  on the merge result and the PR's packages resolve there as on the head.
  When trunk changed a caller of the patch or something the patch depends
  on, it runs the patch's checks on the merge result and merges on a pass.
  If trunk would force a third rebase past merge prep, it holds and reports
  to the root, which sequences the colliding work.
- Independent PRs run in parallel off trunk, never stacked, so each lands on
  its own. Deliver a stack for the owner to land instead when the owner
  wants review before landing, the work is sequenced or coupled, or merge
  authority is withheld. In a stack, a PR owner branches from the item it
  depends on, pushes only its own branch, reports each tip and its intended
  parent, and reports stack-ready instead of merge-ready. It never rebases
  onto another branch, retargets, merges, closes a PR or arms auto-merge.
  Only the root rebases or retargets a link
  (`gh pr edit <pr> --base <parent-branch>`), appending each clean PR in
  dependency order. Until landing starts, the root absorbs drift bottom-up
  with the merge-prep test above, then applies the patch-id rule link by
  link. Once landing starts, nobody writes to the chain except the retarget
  in step 5. A mid-landing problem goes through the root to the owner, and
  any fix waits for the owner's go.
- Anything that loosens a gate, such as a raised budget or an allowlist
  entry, needs a verifier's proof and then the root's fresh countersign. A
  disabled or soft-failed check always comes out. Never give or bypass an
  approval the forge enforces.
- Audit the queue on a cadence through SLP attention (`maestro attention`)
  or an external driver (`maestro recipe show unattended`). Judge each agent
  by its side effects, not its status text. An agent is stuck when it
  errors, cannot start turns, or shows nothing new past its stuck-after
  runtime. One waiting on the root or the owner is not stuck, so answer it.
  Replace a stuck agent at once. Drift such as an unreported head or an
  unrecorded agent is a gap in the record, and its PR owner records it.
- Stand down at once on the owner's stop. A hold reaches every PR owner as a
  zero-writes order. Each reports where it stands and keeps its brief until
  the owner releases it.

## Reply

The verified run and its ceiling, each PR's verdict and who produced it,
what landed and how you confirmed it, and what the next gap needs. For a
queue, open with how many things need the owner, and give each action item
a recommendation and what happens with no answer. Cover each PR's state and
verdict, what merged, each countersign and why, and the open owner gates,
with a link to each PR and the stack root and tip.
