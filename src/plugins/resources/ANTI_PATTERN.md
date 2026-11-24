# Anti-patterns

Eight named failure shapes of multi-agent work: the first seven from the
owner's giáo án walk of 2026-09-16 (room d138), the eighth, parachute, from the
SLP article the owner adopted on 2026-09-28
(https://vhlam.com/article/agent-orchestration-multi-agent-slp). They are
vocabulary, not gates: nothing in maestro refuses an operation because a name
below fits. A filed lesson names the pattern it shows when one fits, so
corrections that rhyme collect under one word instead of eight descriptions.

Each entry gives the definition, what it looks like in maestro, and the
evidence from this project - or "not yet seen here" when there is none.

## balloon

A seat's scope grows past the brief it accepted, one reasonable-looking step at
a time, until the work it returns is not the work that was authorized.

In maestro: a Peer that finds a second defect while fixing the first and fixes
both; a Lead that widens an item instead of opening new work; a return whose
diff is larger than its objective.

evidence: the Peer mandate exists because of it - `resources/slp/peer.md` says
"a different scope is a different request, so work you notice beyond this one
is `maestro work note <work-id> \"<what you found>\"` and never part of the
change, and a request you cannot meet at the scope it set comes back with the
blocker named, never quietly narrowed". The shared contract's companion rule is
that an objective and its acceptance never change: changed scope requires new
work.

## brake

A seat stops early and hands the work back with the hard part undone, so the
decision it was holding travels up instead of being made where it belongs.

In maestro: a `--blocked` note on a technical fork the seat's own mandate says
it decides; a return that reports a gate as unrunnable without naming the
property it could not prove.

evidence: room lesson l20 - five contracts named "the suite green under
DEVELOPER_DIR" as their acceptance clause, a later ruling made that mechanism
unavailable to every seat, and because acceptance never changes all five became
unacceptable however good the work was. The shared contract now requires an
acceptance clause the receiving seat can discharge, and the exact words for
reporting a property unproven.

## pre-solve

The root seat solves the problem itself and hands down the solution as a brief,
so the seat below executes a plan it never checked and no second judgment is
ever applied to the design.

In maestro: a Hub or Lead brief that specifies the diff rather than the
outcome and its acceptance; a review brief that fixes the hypothesis, the
criterion and the answer shape in advance ("check whether A is right, look at
X only, do not reopen Y, answer PASS or FAIL in five bullets"), so the reviewer
can find faults in A but never ask whether A is the thing to build.

evidence: not yet seen here as a recorded failure. The Hub's g11 brief is the
allowed small-task form: seven findings specified in advance, each small enough
that the specification *is* the outcome (the `w734 F1`..`F7` commits). The
pattern begins where the task is large enough to have a design and the brief
settles it anyway.

## weak scout

A weak or cheap model is given owned work - work that writes files, returns,
or is accepted - and its output is trusted because the seat is trusted.

In maestro: opening a seat on a smaller model to save quota; running a
writing sub-agent below the strongest model (code is written on Opus);
passing a sub-agent's report through without reviewing its diff.

evidence: room d130 and d132 - every seat of the cleanup and improver
generations runs on Opus, never Fable, because a refactor Lead and the doctrine
edits every seat reads warrant the strongest model; the Codex-quota pressure
that prompted the question did not move the ruling. Room d135 drew the line by
capability instead (sub-agents only scout); the pstack port (d860, d868) moved
it back to the model and the review: a sub-agent writes on the strongest
model, named in the call, and the seat or main chat that holds the item reviews every diff.

## polling waste

State is read on a timer, or from whatever surface is nearest, instead of being
pushed by the operation that changes it - so the reader spends turns on
unchanged state and eventually acts on something that was never a record.

In maestro: re-running `maestro status` in a loop to see whether a Peer
returned; reading a pane's transcript for a work state.

evidence: the g6 phantom record - four times since g6 a Hub read a seat pane
and saw a line such as "return w730" in the input box before the store held any
such record, the harness's own prompt suggestion mistaken for a push. Every
`work add`, `return`, `accept` and `note --rework` wakes the counterpart, and
since g11 finding F7 the shared contract says pane text is not a push and a
work state exists only when `maestro status <work-id>` shows it.

## dual ownership

Two seats hold the same scope at the same time, so each one's work is correct
alone and the pair is a conflict nobody owns.

In maestro: two threads writing the same file.

evidence: the concurrency rule exists because of it - WORKFLOW.md requires that
"concurrent threads write only disjoint, exclusively owned paths", one work
item per tracked thread, and `work start` prints an `[overlap]` banner when
another session holds a sibling work item. The
0.107 Rust line shipped the same rule as the warm-file overlap banner with the
cross-worktree active union (Hub memory,
`project_same-branch-conflict-prevention-shipped`, 2026-06-17).

## priority by label

Work is ordered by a badge - P0, P1, urgent - instead of by what unblocks what
and where the leverage is, so the label becomes the argument and the real
ordering is never stated.

In maestro: a queue sorted by tag rather than by dependency; an item argued for
by its letter rather than by what waits on it.

evidence: not yet seen here, and maestro has no priority field by design. Work
carries an objective, an assignee and a state; ordering lives in the objective
text ("write the acceptance condition and any ordering into the objective") and
in what a seat is actually blocked on.

## parachute

A design choice made by one seat travels down later briefs as if it were a
requirement, so every later seat improves the choice and none may ask whether
it should exist; the owner's goal narrows into optimizing a solution someone
picked.

In maestro: an objective that states a mechanism an earlier item chose as a
constraint with no source behind it; an item that answers "still too slow" by
making the chosen mechanism faster; a fix that raises a limit (a history size,
a timeout, a retry count) instead of asking why the limit is reached. The
guard is the shared contract's brief rule - an objective tells the goal, its
sourced constraints and any chosen design apart - and the `--blocked`
objection a seat records when it can show the premise is false.

evidence: not yet seen here as a recorded failure. The article's own cases are
a bicycle given a braking parachute that each later agent made lighter or
larger, and a replication history raised past 128 packets when the defect was
that a packet dropping out of history was read as the client having
acknowledged its state.
