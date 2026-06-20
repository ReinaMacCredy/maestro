# Feature fan-out

WHEN: 2+ ready cards on one feature are independent, and parallel work pays off.

Run one agent per ready card so independent slices land at once. The skill
(maestro-card `feature.md`) decides *whether* to fan out; this is the full HOW
of running it.

## Setup: prove independence first

    maestro card ready <feature>

For each candidate card, read its locked acceptance checks. Two cards are
parallel-safe only when they share neither files nor a dependency edge. If they
touch the same files, either serialize them or isolate each agent in its own
git worktree so commits never collide. The card store is shared too: `claim`
and `complete` write it for every card, so parallel workers in one tree collide
there even when their code files do not -- give each worker its own worktree
(the store is git-tracked, merges back via conflict-handoff) or have them return
proof as data for the conductor to record.

## Dispatch: one agent per card

Each agent owns exactly one card, end to end:

    maestro card claim <id> -> work the card (test-first per work.md) ->
    maestro task complete <id> --summary --claim --proof

- Claude Code: one `Workflow` stage per card; pass the card id and its checks
  as the stage prompt. Use `isolation: 'worktree'` when files or the card-store
  writes overlap -- and `complete` writes the store, so a parallel worker needs
  its own worktree unless it returns proof for the conductor to record.
- Codex: one sub-agent per card, dispatched in parallel; give each its own
  worktree thread when files or store writes overlap (store writes always do).

Give each agent only its card, its checks, and the repo. Do not stream one
agent's progress into another.

## Collect: the conductor verifies and commits

The conductor (not the workers) gathers completions and closes the loop:

    maestro task verify <id>        # per returned card
    git commit                      # each verified slice, on the feature branch

Then run the `qa-slice` pass over the baseline before `feature close`. A card
whose verifier refutes a claim is blocked, not verified:

    maestro task block <id> --reason "<what failed>"

## Stop

All independent cards verified and committed -> qa-slice -> close. Never `accept`
or `close` from inside a worker; those are conductor/human gates.
