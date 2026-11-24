# Fan-out

Use this recipe when N parallel subagents cover separate slices, race the
same brief, or both, and you return one report. It also holds the discipline
for program-scale work. SLP stays the frame for that work. A Lead or Peer
seat in a pane fans out inside its own seat, the maestro store holds every
record, and this recipe never creates a second coordination model. Ported from
pstack's swarm skill and orchestrate playbook (Lauren Tan, MIT; notice in THIRD_PARTY.md).

Three rules carry the rest.

- Completions are queue events, not interrupts.
- Every spawn and every resume carries the standing orders verbatim.
- The brief is the product. A vague brief fails quietly, because a worker
  cannot ask you a question.

Ceremony scales with the program. On cheap, near-identical units, collapse
it as each step says.

## Roles

- You, the seat, own the program; writers produce the code. You frame,
  write briefs, drain returns, decide, and report. Conflicted merges and code
  changes are units with briefs. Landing a verified unit mechanically (a
  fast-forward or clean merge of a worker's branch per
  `maestro recipe show worktree`) is bookkeeping you may do yourself.
- Read-only slices run under the `fan-out-worker` profile. Writing slices run
  under the `delegate` profile, each in its own worktree or on disjoint owned
  paths, one writer per worktree or branch per
  **Separate Before Serializing Shared State** (PRINCIPLES.md). Prefer fewer,
  broader workers.
- A unit that needs a separate verifier gets one on a different model family
  from its worker.
- A brief uses the dispatch envelope's fields (maestro-work, Dispatch). The
  seat keeps the work item and its lease; a subagent brief opens no work item.
- Work too large for one seat to drain is a team. The Lead adds Peers per
  `maestro recipe show slp`, and each Peer fans out inside its own seat.
  There is no nested coordinator subagent.
- Workers never rebase, retarget or merge. A stack operation is a unit with a
  brief, run by one agent per stack. Babysitting a stack follows
  `maestro recipe show babysit`, and PR merges stay owner gates.

## Steps

1. Frame.
   - State the done predicate as something countable ("all 40 slices
     returned PASS, each ledger-verified at the `artifact` layer or better")
     and the artifact or report the fan-out returns.
   - Quantify the scope in units, rough effort, and the wall-clock and token
     budget. If one agent could finish inside that budget, it is not a
     program. Work directly with plain workers where they help, verify
     inline, land as you go, and skip steps 2 and 3 and the ledger. The brief,
     return and aggregate rules still apply.
   - Choose the shape. Partition into slices, race N workers on identical
     briefs, or mix both. A partition gets a coverage matrix that lists every
     required slice, and each one needs a result. A race or mix declares its
     selection rule (first pass, rank all, or best-of) before spawning.
   - Set N from the owner or derive it from the shape. N is total workers,
     not the in-flight cap. For a model race, name each arm's model up front.
   - A contested decomposition or a one-way door goes through
     `maestro recipe show arena` before the pilot.
   - Present the framing once. Reversible preparation proceeds without
     waiting.
2. Write the standing orders before any spawn. Keep them in one file in the
   folder `maestro topic path <slug>` prints, as numbered lines with one
   constraint each (model policy, verification bar, forbidden paths,
   escalation policy). Record the file's path in a work note. Paste it
   verbatim into every brief and every resume, because directives decay
   across resumes and each dropped one costs the owner a turn. When you catch
   yourself restating an instruction, append it as a line before you act,
   per **Encode Lessons in Structure** (PRINCIPLES.md).
3. Pilot. Push one unit through the whole path (brief, worker, verification,
   ledger note, landing). The pilot falsifies the brief template, the verify
   command and the unit size while that costs one agent instead of fifty.
   Fix the contract from pilot evidence before any fan-out. Scale the pilot
   to the unit. On cheap, near-identical units the first unit is the pilot,
   run as a normal unit with its verify command inline, and fan-out starts
   when it lands.
4. Fan out on a rolling window. Spawn in the background up to the in-flight
   cap, about ten or what one drain can process, and refill as children
   finish. Never run
   blocking batches; each batch pays for its slowest child. Recompute ready
   units after each drain. Relay upstream reports into downstream briefs.
   Sibling communication goes upward only. Audit one sampled brief per wave
   while the wave runs, never as a gate in front of it. A failing brief stops
   the next refill and fixes the template, not just that brief.
5. Drain. A completion is a queue event. Note it and return to what you were
   doing. Every writing slice's diff is reviewed before it lands: read it
   yourself, or, past what one drain can read, hand it to a verifier unit on
   a different model family and read that verdict. A diff nobody reviewed
   never lands. Drain in batches at the end of a critical section (writing
   a brief, a landing, a conflict decision, a ledger note) and before an
   owner report. Each drain classifies every return as landed, needs-verify,
   failed, zombie or noise, writes the ledger notes, and spawns the next wave
   in one message. A drain ends with three lines that give the counts against
   the predicate, what changed, and the open gates.
6. Aggregate. Read the terminal returns. Drop a return that does not record
   the SHAs and method its brief names, and respawn that worker once. After a
   second miss, record a gap. A gap is not a pass. If a worker drops out,
   proceed with N-1 and note the dropout. For coverage, every required slice
   needs a result, so a dropped slice stays a gap. For a race, apply the selection rule
   declared in step 1. Account for every spawned child as arrived, respawned,
   or its scope explicitly absorbed. Silently redoing a missing child's work
   hides both the spend and the coverage gap. Never paste raw worker output.
   Keep a compact result table, one-line evidenced issues, and explicit gaps
   and dropouts.
7. Land continuously. Integration starts with the first verified unit and
   runs beside the remaining waves, never as a final phase. A unit is done
   only when its output lives outside the worker, as a merged branch per the
   worktree recipe or findings in the store, with its ledger note written.
   Commits, pushes and PRs follow the grant in
   `maestro recipe show opening-a-pr`. By about 70 percent of the context or
   token budget, stop spawning and land what is verified.
8. Close. Drain the last returns. Reconcile every spawned agent to a terminal
   state (done, abandoned, zombie reconciled). Confirm the predicate on the
   real artifact. Confirm every landed unit has a ledger verdict for its
   current head SHA. Encode recurring corrections into the standing orders
   or the brief template. The work notes and `maestro trace <id>` are the
   postmortem.

## The brief

Every brief stands alone. A field you cannot fill is a unit you have not
scoped yet, and a missing field means you do not spawn.

```text
Objective: <one sentence, the outcome, executable by a stranger with no chat access>
Owned scope: <paths this unit may write and its exclusive worktree or branch, or none>
Excluded scope: <paths it may not touch; no PR retargets or merges, no rebase, no force-push, no fixes outside scope; unit-specific bans>
Mutation: <no-write | write-bounded: paths>
Stop condition: <the done boundary; the timebox, on whose expiry the worker returns partial findings and stops>
Lane: <scout for a read-only slice | delivery for a writing slice>
Evidence required: <checkable acceptance, one per line; exact verify commands or the repository's verify-<repo> skill; known gotchas; the exact SHAs and method when it verifies or measures commits>
Context: <file and PR pointers; upstream reports pasted in full when this unit depends on them, because workers cannot see siblings>
Report: <PASS | ISSUES | BLOCKED with evidence; branch; head SHA; method; what you actually ran; deviations; suggested follow-ups>
Standing orders: <the standing-orders file, pasted verbatim>
```

- The timebox lives in Stop condition. Forbidden actions live in Excluded
  scope and Mutation.
- Size the brief to the unit. A one-command unit collapses to a short
  paragraph that still names the objective, excluded scope, mutation, the
  verify command and the report shape.
- A measurement brief names the method (sample count, what one sample is,
  order). The worker records the SHAs and the method in its return.
- A worker that can prove a defect reports ISSUES and lists every issue it
  can prove, not only the first.
- A dependency is a context relay, not just an ordering. Undeclared upstream
  context makes the worker guess.
- Never resume-chain a brief. Respawn fresh with consolidated scope (the
  original brief, every later directive, the prior report and branch).

## Verification ledger

- Scale verification to the unit. When verification is one cheap command,
  the worker runs it and reports the output, and you spot-check receipts. A
  dedicated verifier subagent is for units whose verification is expensive,
  judgment-laden or high in blast radius. A verifier whose whole product is
  rerunning one command is ceremony.
- The ledger is work notes on the program's work item, one per verdict, keyed
  by unit and head SHA, written as
  `maestro work note <id> "ledger: <unit> <head-sha> <verdict> <method>"`.
- A verdict names the evidence layer it reached in policy-layer's words
  (`source`, `artifact`, `installed`, `live`, `journey`), or it is
  `verifier-blocked` or `verifier-failed`.
- CI green is an input to a verdict, not a verdict. Behavioral work needs
  more than `source`. `verifier-blocked` is not a pass; respawn when the
  environment heals. `verifier-failed` gets a fix unit, not a re-verify. A
  worker may self-report, and a verifier's note overrides it on the same key.
  A new head SHA voids the verdict, so re-verify after any restack. The
  ledger answers "was this verified", not memory.

## Liveness and failure

- Never resume an agent to check on it, because a resume restarts an idle
  agent. Probe read-only through the ledger notes, the worker's branch and
  worktree, `gh`, and the harness's own task status.
- A silent death gets a postmortem note naming the unit, the failure mode,
  the last evidence and the options. Replan on evidence as it arrives. Never
  wait for full quiescence.
- Retry by failure mode. Cap hit or out of memory, respawn with smaller
  scope. Network drop, retry as is. Tool error, retry on another model.
  Unknown, retry once. After two failures, abandon the unit, record it with
  `maestro work note <id> "failed: <unit> <mode> <evidence>"`, and replan
  around it.
- A zombie that returns late reconciles against the current ledger and
  branches before anything is accepted. Salvage its unique findings through a
  fresh unit, never a blind merge.
- When continued spawning would produce garbage across the program (bad
  upstream output, broken acceptance, dead infrastructure), write a stop line
  at the top of the standing orders, let in-flight work finish, fix the
  cause, and clear the line.
- Bound your own retries the same way. After a few consecutive tool aborts,
  stop. Write a `checkpoint:` work note with what is done, where it lives and
  the exact command to resume, then end the run.
- After a session restart, harness subagents are gone and committed or
  pushed work is not. Re-read the standing orders and the ledger notes,
  reattach work by branch, and respawn fresh with consolidated scope.

## Escalation

These reach the owner, batched into the next report rather than one by one.

- Irreversible actions (force-push to shared branches, deploys, deletions,
  closing someone else's PR).
- Product or preference calls that no experiment settles.
- A standing order that contradicts observed reality.
- A program-level dead end that survived a replan.

Record each as
`maestro work note <id> "<question, options, default if no answer>" --blocked`
and route work around it. Retries, rebase mechanics, CI flake triage,
review-thread triage, format fixes, scope the brief already forbids (refuse
and continue) and "should I keep going" never reach the owner. When in doubt,
act and log. Mid-run discoveries fix only what blocks the predicate.
Everything else is a finding for the owner, because at this fan-out a small
scope leak multiplies into changes nobody asked for.

## Reply

At checkpoints and at close, the reply carries these items.

- The predicate and the count against it, taken from the ledger notes.
- What each track landed, and the verdict summary.
- The result table, one-line issues, gaps and dropouts, and the selection
  rule when a race ran.
- What was abandoned and why.
- Gates waiting on the owner, which are the only asks.
- The work id for `maestro trace`, and PR links when PRs exist.

Numbers come from the ledger, not from narrative.
