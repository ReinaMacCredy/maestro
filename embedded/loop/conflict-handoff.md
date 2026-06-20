# Conflict handoff

WHEN: another session is live and you are about to implement on the same repo.

maestro is passive: it shows peers (`maestro active`, the `[overlap]` /
`[CONFLICT]` / `[busy]` banners) but never runs git, makes a worktree, or links
cards. You drive the whole dance below; maestro only carries the notices.

## Shape

1. See who is live: `maestro active`. At the design-to-implement boundary
   `feature accept` / `prepare` also print a `[worktree]` nudge when a peer is
   live.
2. Isolate -- create your own worktree under the repo's gitignored
   `.maestro/worktree/` and implement there, so two sessions never write the
   same checkout:

       git worktree add .maestro/worktree/<slug> -b <branch>

3. If you will touch a file a peer is also editing, make it visible:

       maestro link add <your-card> <their-card>
       maestro conflict <their-card> "<file>: <why>"

   The peer now sees `[CONFLICT] <you>` in its pre-command banner and holds off
   that file until you clear. Reciprocally, while a `[CONFLICT]` names a file you
   need, work elsewhere until it lifts.
4. Heavy runs serialize themselves: the full-suite gate takes a shared lock, so
   if a peer is mid-gate you will see `[busy]` and your gate waits its turn. Let
   it; do not force a second suite.
5. Merge back when your slice is verified. YOU run git; maestro never does:

       git switch <shared-branch>
       git merge <branch>      # or rebase your branch first, then fast-forward

6. Clear your notice so the peer is unblocked:

       maestro conflict --clear <their-card>

   A stale asserter's notice auto-hides once you drop out of the active union,
   but clear it yourself the moment you resolve.

## When the merge-back itself conflicts

A real git conflict on step 5 means you and the peer changed the same lines.

1. Do NOT clear the maestro notice yet -- the file is still contested.
2. Resolve the git conflict in your worktree: inspect both sides (`git diff`,
   `git log --merge`), keep the union of intended behavior, and re-run the file's
   narrow check (`maestro task verify <id>`) or the suite.
3. If the resolution needs the peer's intent, ask through the link
   (`maestro msg send <their-card> "<question>"`) before overwriting their lines.
4. Only once the merge commit lands and the suite is green do you
   `maestro conflict --clear <their-card>`.

## Stop

Your slice is merged to the shared branch, the suite is green, and every
`maestro conflict` you opened is cleared. Then remove the worktree
(`git worktree remove .maestro/worktree/<slug>`).
