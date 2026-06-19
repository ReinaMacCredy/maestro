# Loop until done

WHEN: the work amount is unknown up front (an audit/sweep you cannot count in advance).

Keep working until a *query* says you are done, not until you feel finished. The
skill (maestro-card `work.md`, Triage And Loops) states the rule; this is the
full HOW.

## The stop condition is a query, not a feeling

Stop only when one of these holds:

    maestro card ready     # empty -> no claimable work remains
    K discovery sweeps in a row return zero new findings

Pick K up front (2-3 is typical). A single empty sweep is not enough for
open-ended discovery; the tail is where the easy findings have run out but real
ones remain.

## Each round

1. Discover: sweep for new findings (one pass, or a parallel multi-angle sweep
   when one search angle will miss things).
2. Capture immediately: turn each new finding into a card *before* working it,
   so it survives context loss.

       maestro card create "<finding>" -t <task|bug>

3. Work the ready cards: `claim -> work -> task complete --proof -> task verify`.
4. Re-check the stop condition. Not met -> loop.

- Claude Code: a `while` loop in a `Workflow` script that re-checks `ready` /
  new-findings each round and exits when the query is satisfied.
- Codex: repeat the sweep-mint-work-recheck cycle; track the consecutive-dry
  count across rounds.

## Stop

`ready` empty, or K consecutive dry sweeps. Log what bounded the loop (dry,
cap, or budget) so a silent truncation never reads as full coverage.
