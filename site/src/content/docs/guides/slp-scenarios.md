---
title: SLP scenarios
description: Six cases seen from the owner's seat, from what you type to what your session does with it and what it reports back.
---

You never write a dispatch envelope by hand. You type to a session the way you
always have, short or long, and the session decides whether it is the Lead of
a repository or the Supervisor of the room, whether one session is enough, and
when to open lanes. This page shows six cases from your seat: what you type,
how the session reads it, what it does, and what comes back. The role
contracts are in [Roles](/concepts/roles/) and the lane mechanics in
[Lanes](/concepts/lanes/).

## Where you start is who you talk to

The role of the session you are typing to comes from where it was started, not
from anything you say.

| You start an agent in | You are talking to | It reads |
|---|---|---|
| a repository working tree | the Lead of that repository | the repo `AGENTS.md` maestro block and the hook brief |
| `~/maestro`, after `hm` | the Supervisor, your embodiment across projects | `IDENTITY.md`, `OWNER.md`, then `maestro brief` |

You never talk to a Peer. Peers are panes the Lead opens with a stored
dispatch; they take orders from that contract and return to the Lead, and the
Lead reports to you. If you find yourself copying answers between panes, you
are playing Lead by hand.

## How the Lead reads a prompt

Before opening anything, the Lead turns your prompt into one sentence:

```text
For <person or system>, reach <observable outcome> inside <boundary>,
because <impact>, without <excluded effect>.
```

If it cannot write that sentence, the request is still fog and it opens a
scout or decision lane before touching code. Then it scores five questions, 0
to 2 each: is a wrong framing expensive; is the change hard to reverse; is
the domain new to you; could an independent judgment really disagree; is your
attention split. Score 0-2 means it does the work itself, 3-5 one delivery
Peer, 6-8 several independent lanes, 9-10 the room. You do not score anything;
knowing the rule tells you why the session answered the way it did, and lets
you override it in a sentence ("do it yourself", "get a second opinion").

## Case 0: the first session in the room

After `maestro install` in your first repository, `~/maestro` exists with
`IDENTITY.md`, `AGENTS.md`, `lane.md`, and `shellrc` (managed, rewritten on
every install or update) and `OWNER.md` (yours, written once, never
overwritten). `OWNER.md` starts as a template with questions under four
headings; the room's first job is to ask them.

**You type**, after `hm` and starting an agent in `~/maestro`:

```text
hi, first time here
```

**The Supervisor reads it as**: `OWNER.md` still holds unanswered questions,
so this session is an interview, and the brief comes after.

**What you get back** is the questions, one group at a time:

```text
OWNER.md is still the template, so I'll ask before I read the brief.
Environment first: where does code live, which terminal workspace manager opens panes, which harnesses run here, and is there anything that must never run on this machine?
```

You answer in plain words. Stable facts go into `OWNER.md`; anything that
could change later is recorded as a decision in the room store, so a reversal
supersedes it instead of leaving two preferences side by side:

```text
Written to OWNER.md: code under ~/Code, Herdr for panes, Claude Code and Codex, no rm -rf, no git checkout --.
Recorded as room decisions: d1 answer in Vietnamese, terse, no emoji; d2 push, tag, publish, deploy, delete, spend always ask; d3 implementation details, sequencing, test strategy, retries, and opening lanes are mine to decide.
Brief: one repository registered, ~/Code/example, nothing needs attention.
```

From then on every room session reads the answers instead of asking. To
change one later, say so; the Supervisor drafts the replacement with
`--supersedes` and locks it.

## Case 1: a fix you could check yourself

**You type**, in the repository:

```text
the README still points at the old install path, fix it
```

**The Lead reads it as**: for a new user, the README names `install.sh`,
inside one file, so the install works first time, without touching anything
else. Score 0: small, reversible, and you can check it by reading one line.

**What it does**: the change itself. A one-line diff needs no record; a change
that needs a claim and proof to outlive the session gets a work item:

```sh
maestro work add "README names install.sh" --kind task --atomic-reason "one-line doc fix" --acceptance "README install line matches scripts/install.sh"
maestro work start w40
maestro work done w40 --claim "docs: README names install.sh" --proof "source: rg -n 'install.sh' README.md"
```

**What you get back**:

```text
Fixed README.md:12; w40 closed. Proof: rg -n 'install.sh' README.md shows the new line. Not committed.
```

No pane, no dispatch. If you had wanted it routed through a lane anyway, the
contract would have been longer than the diff.

## Case 2: a bounded change while you are busy

**You type**:

```text
add a --since flag to audit so old entries are filtered out. test first. I'm in meetings all afternoon, don't push anything
```

**The Lead reads it as**: for an operator reading the audit log, entries older
than a date are omitted, inside one verb and its tests, because long logs are
unreadable, without a push. Score 4: one moving scope that touches real code,
you are not watching, and a dependency (the store's date format) might turn
up. One delivery Peer, with the Lead keeping integration and review.

**What it does**: records the work, releases its own lease so the Peer can take
it, opens a pane, stores the contract, and sends it as the Peer's first prompt:

```sh
maestro work add "audit --since omits older entries" --kind task --atomic-reason "one verb, one flag" --acceptance "entries before the date are omitted"
maestro work release w41
herdr tab create --workspace w1 --cwd ~/Code/example --label lanes --no-focus
herdr agent start lane-since --kind claude --pane <pane-id>
maestro dispatch open w41 --lane delivery --objective "audit --since <date> omits entries older than the date, test-first" --owned-scope "src/plugins/audit.ts, tests/audit.test.ts" --excluded-scope "store schema, other verbs, docs" --mutation "write-bounded to the owned scope; no commit, no push" --stop-condition "new test green and full suite passes, or BLOCKED" --evidence-required "source: bun test tests/audit.test.ts with counts" --pane <pane-id> --target-session <session-id>
```

The Peer's prompt is the stored contract with the role line in front; your
"test first" and "don't push" are in it because the Lead carried them over:

```text
You are lane-since, a delivery lane for dispatch x12 on work w41 in ~/Code/example (branch main, HEAD 1a2b3c4d).
First `maestro dispatch show x12`, `maestro dispatch accept x12`, then `maestro work start w41`.
Objective: `example audit --since <date>` omits entries older than the date. Write the failing test first.
Owned scope: src/plugins/audit.ts and tests/audit.test.ts. Excluded: the store schema, other verbs, docs.
Mutation: write-bounded to the owned scope. No commit, no push; the Lead integrates.
Stop condition: the new test is green and the full suite passes, or you are BLOCKED. Then file exactly once:
maestro handback file x12 --status DONE --claim "<what is now true>" --proof "source: bun test tests/audit.test.ts <pass/fail>" --assumptions "<or None>" --residual-risks "<or None>" --incidental-findings "<or None>"
and go idle. A bug outside the scope goes in --incidental-findings, not in the diff.
```

The Lead waits for the pane to go `working`, then for any terminal state, and
does not prompt the Peer again in between. The Peer returns once, as a claim:

```text
h7 x12 DONE
claim: audit --since omits entries older than the date; 2 new tests
proof: source: bun test tests/audit.test.ts 14 pass 0 fail; bun test 336 pass 0 fail
assumptions: dates are ISO-8601 in the store
residual risks: None
incidental findings: audit --json prints the date in local time, not UTC
```

The Lead runs the falsifier itself, commits, closes w41 with its own claim,
closes the pane, and turns the incidental finding into a card rather than
letting it leak into the diff.

**What you get back**, when you look up from your meetings:

```text
w41 closed: audit --since omits older entries, 2 tests, suite 336/0, committed as 5e6f7a8b, not pushed.
One incidental finding from the lane, filed as w42: audit --json prints local time instead of UTC. Not fixed; your call whether it matters.
```

Had the Peer returned `DEPENDENCY_REQUEST` because the store's date column
needed a change, the Lead would have opened that as its own work item with its
own owner and told you the flag is waiting on it, not widened x12 quietly.

## Case 3: a decision you do not want made blindly

**You type**, a longer one:

```text
I'm worried about session identity. when the OS reuses a pid, a new process could inherit a dead session's lease and we'd never know. I don't know this part of the code well and I don't want a random first answer. figure out the options properly, no code yet, and tell me what you'd pick and why
```

**The Lead reads it as**: for the runtime, a dead session never inherits a
live lease and a live one is never declared dead by a pid collision, inside
the session model, because a wrong scheme is expensive to reverse, without
writing code. Score 7: irreversible, new to you, more than one defensible
answer. A council of two decision lanes with one neutral brief, sealed until
both return.

**What it does**: opens one work item, two panes (here one Claude, one Codex),
and two dispatches with the same wording:

```sh
maestro work add "Session identity after pid reuse" --kind task --atomic-reason "one decision, no code" --acceptance "a locked decision with a falsifier"
herdr pane split --pane <pane-id> --direction right --cwd ~/Code/example --no-focus
herdr agent start decision-a --kind claude --pane <pane-a>
herdr agent start decision-b --kind codex --pane <pane-b>
maestro dispatch open w47 --lane decision --objective "recommend how a session proves it is still the same process after the OS reuses its pid" --owned-scope "src/kernel/sessions.ts and its tests, read only" --excluded-scope "any code change" --mutation "no-write" --stop-condition "options, recommendation, and falsifier written" --evidence-required "source: note path with mechanism per option" --pane <pane-a> --target-session <session-a>
maestro dispatch open w47 --lane decision --objective "recommend how a session proves it is still the same process after the OS reuses its pid" --owned-scope "src/kernel/sessions.ts and its tests, read only" --excluded-scope "any code change" --mutation "no-write" --stop-condition "options, recommendation, and falsifier written" --evidence-required "source: note path with mechanism per option" --pane <pane-b>
```

The brief is neutral: all the facts, none of the Lead's preference. Your
sentence "I don't want a random first answer" became "do not assume":

```text
You are decision-a, a decision lane for dispatch x20 on work w47 in ~/Code/example (branch main, HEAD 1a2b3c4d).
First `maestro dispatch show x20` and `maestro dispatch accept x20`. Do not run `maestro work start`; this lane does not write. A sibling lane holds the same question; do not read its notes.
Decision question: how should a session prove it is still the same process after the operating system reuses its pid?
Outcome: a dead session never inherits a live lease, and a live session is never declared dead by a pid collision.
Constraints already binding: SQLite store; no daemon or background process (decision d5); sessions are recorded by harness hooks at SessionStart.
Evidence bundle: src/kernel/sessions.ts, tests/sessions.test.ts, decision d9 (lease liveness).
Do not assume: that a pid alone identifies a session, or that the answer must be a schema change.
Return: each option with its mechanism and trade-offs, one recommendation, and the observation that would change it, written to <note path>. Then file exactly once:
maestro handback file x20 --status DONE --claim "<recommendation in one sentence>" --proof "source: <note path>" --assumptions "..." --residual-risks "..." --incidental-findings "..."
and go idle.
```

While one lane is still out, the other's return is sealed: `handback show`
refuses it and `handback list w47` marks it `[SEALED]`, so the Lead cannot
lean on the first view. When both are in, the Lead compares them with its own
reading and records the ruling and the dissent:

```sh
maestro decision draft "sessions carry a start-time incarnation; a pid match without a matching incarnation is dead" --rationale "both lanes reject pid-only identity; A proposes an incarnation stamp, B a heartbeat row; a heartbeat needs a writer that d5 forbids" --work w47
maestro decision lock d31
maestro work note w47 "dissent from x21: a heartbeat would also detect a hung process; deferred, not rejected"
```

**What you get back**:

```text
Council on w47 returned, both lanes.
Both reject pid-only identity. A: stamp each session with its process start time (incarnation); a pid match without the stamp is dead. B: a heartbeat row refreshed by the hook; needs a writer we forbade in d5.
Locked d31: incarnation stamp. Dissent recorded: B's heartbeat also catches hung processes; parked, not rejected.
Falsifier that would change it: a platform where process start time is not readable from the hook.
Nothing implemented. Say the word and it becomes a delivery lane on a new work item.
```

If the two lanes had contradicted each other on a fact rather than a
preference, the Lead would have opened a second generation: one new dispatch
per lane, quoting the other's handback verbatim with one targeted question,
answered by handback (`DONE` with a `CONFIRM` claim, `CHALLENGE`, or
`REOPEN_REQUEST`). Lanes never talk to each other; you would have seen the
reconciled result, not the debate.

## Case 4: you do not trust what just landed

**You type**:

```text
a lot landed this week and I haven't read it. before we tag, get someone to try and break the dispatch code. real bugs only, I don't want a list of nitpicks
```

**The Lead reads it as**: for the release, the SLP runtime since the last tag
holds its own invariants, inside the dispatch and attention plugins, because a
broken seal or lease is a governance bug, without any fix in the same breath.
Score 6 on the "independent judgment" axis: the code exists, and what is
missing is someone whose job is to break it with no authority to repair it.
One or more challenge lanes, no-write, returning findings rather than patches.

**What it does**: a challenge dispatch whose prompt names the invariants to
attack and separates what is trusted from what is still a claim. "Real bugs
only" became "a repro is required for high":

```sh
maestro work add "Review the dispatch runtime since v0.108.0" --kind task --atomic-reason "one review, one note" --acceptance "findings with severity, file:line, and a repro for each high"
maestro dispatch open w51 --lane challenge --objective "find where the SLP runtime since v0.108.0 breaks its own invariants" --owned-scope "src/plugins/dispatch.ts, attention.ts, policy-dispatch.ts and their tests, read only" --excluded-scope "fixes, redesign, install or update" --mutation "no-write" --stop-condition "review note written with counts by severity" --evidence-required "source: note path; each high finding carries a repro or failing-test sketch" --pane <pane-id> --target-session <session-id>
```

```text
You are swarm-review, a challenge lane for dispatch x64 on work w51 in ~/Code/example (branch main, HEAD 1a2b3c4d).
First `maestro dispatch show x64` and `maestro dispatch accept x64`. Do not run `maestro work start`; the Lead holds w51.
Scope: the SLP runtime as it changed since tag v0.108.0. Read `git log --oneline v0.108.0..HEAD` and the diffs of the dispatch, attention, and policy-dispatch commits, with their tests.
Review like a skeptical second engineer: invariants (one handback per dispatch, council sealed by first return, lease ownership on file), transaction boundaries between concurrent lanes, id parsing (h1 versus h10), error envelopes that name the next command, and anything a test asserts by string that the code could drift from. Read `maestro recipe show slp` so doctrine and code are compared.
Trusted: the store schema and the passing suite at HEAD. Still a claim: everything the commit messages say.
Deliver <note path>: for each finding, severity, file:line, the commit that introduced it, what goes wrong and when, a repro or failing-test sketch (required for high), and the smallest fix. End with counts by severity and what you checked and found sound.
No code edits, no commits, no push, no install or update. Then file exactly once:
maestro handback file x64 --status DONE --claim "<counts and the one finding that matters most>" --proof "source: <note path>; <N> findings cite file:line and commit" --assumptions "..." --residual-risks "..." --incidental-findings "..."
and go idle.
```

A lane that finds nothing returns `DONE` with a claim that says so and lists
what it probed. A suspicion it could not reproduce stays `UNKNOWN` rather than
being rounded up. A lane that finds the premise itself wrong returns
`CHALLENGE`. Three lanes at once on the same work (review, simplify, bug hunt)
form a council and stay sealed until all three are in.

**What you get back** is a triage, not the raw note:

```text
Challenge lane on w51 returned: 4 high, 3 medium, 0 low.
Fixed now, each with a test, committed, not pushed: sealed council readable through handback list --json; a no-write lane could take the work lease; a dispatch opened before the handback could clear HANDBACK_UNREVIEWED.
Cards, too deep for tonight: w52 cancel has no authority check; w53 one-handback-per-dispatch is not a store constraint.
Rejected with reason in the work note: the lane wants the handback to release the work lease; d21 says the Lead reclaims, and it stands.
Suite 339/0. Tag when you say.
```

## Case 5: you are away and several projects are moving

**You type**, in the room after `hm`:

```text
I'm out until tomorrow morning. three repos have work running. keep an eye on them. anything that's my call waits for me, anything a Lead can answer, ask the Lead. don't freeze anyone
```

**The Supervisor reads it as**: a lease for the day: observe, ask, advise;
relay nothing in your name that you have not said; no recovery lease, so no
freeze and no Lead replacement. Score 9: long horizon, split attention, more
than one Lead.

**What it does**: reads the brief, which computes attention across every
registered repository at read time; nothing is pushed into the room. A
packet keeps observation, hypothesis, and question apart:

```sh
maestro brief
```

```text
attention REPEATED_FAILURE work w47 (~/Code/example)
  observed: three work done attempts in 40 minutes, each rejected by policy-proof
  evidence: event log entries for w47; holder role Lead; route room
  unknown: whether the proof layer is wrong or the falsifier itself is failing
  question: what did the third attempt change from the second?
  smallest action: stop retrying; record the failure episode before a fourth attempt
  human decision needed: no
```

The Supervisor answers it with a question into the Lead's pane, provenance
first, and never by editing the project or prompting a Peer:

```sh
herdr agent prompt <lead-agent> "[from supervisor][ask w47] Three proof rejections in 40 minutes. What changed between the second and third attempt, and what is the smallest new information a fourth attempt would need?"
```

The wrong version of that message is "the Lead is doing proof wrong, tell it
to use the suite": observation, verdict, and command in one line, over the
Lead's head.

**What you get back**, the next morning, is the room's notebook rather than a
transcript:

```text
Overnight, three repositories.
example: w47 stalled on proof; asked the Lead what changed between attempts; it found the falsifier itself was wrong, fixed, w47 closed. No decision needed.
site: quiet.
tools: the Lead drafted d12 (drop the Windows build) and it is waiting for you. human decision needed: yes. Nothing else touched.
```

You answer d12 in a sentence; the Supervisor relays it by locking the draft in
that project's store, the one write it makes there:

```sh
maestro decision lock d12
```

## Reading a report

Every case ends with a claim coming back to you. Behind it, the Lead ran the
same check before writing the report, and you can ask for any line of it:

1. The actor and dispatch match: the handback is on the dispatch that was sent, from the session that accepted it.
2. The candidate is stable: the claim names a commit, not "the current tree".
3. The writes stayed inside the lease.
4. The required checks ran on that candidate, and the proof names its layer: `source:`, `artifact:`, `installed:`, `live:`.
5. Independence fits the claim: a review that read the author's notes first is not independent.
6. Unknowns and residual risks are still visible, not rounded to `DONE`.
7. No external effect happened that you did not grant.

Completion is reported only to the layer that was proved. "source: PASS at
1a2b3c4d; installed: BLOCKED, an active agent prevents restart; live: NOT
TESTED" is an honest report; "done" is not.

## What it looks like when it goes wrong

- The Supervisor edits a file, prompts a Peer, or accepts a candidate: it has become a second Lead, and the Peer now has two authority paths.
- The Lead opens lanes before it can write the one-sentence problem: you get three reports it cannot reconcile.
- The Lead implements a material change and accepts it alone: `work done` with a proof nobody else ran.
- A decision brief says "show that X is best": the Peer is a function call, and the council added nothing.
- A second lane is briefed after the first returned, or two lanes share a scratch file: independence is theatre, and the seal was pointless.
- `DONE` is treated as done: the work closes without anyone running the falsifier.
- A Case 1 change goes through a Case 2 lane: the contract is longer than the diff.
