# Why

Use this recipe for "why does X work this way", "why did we pick Y", design
rationale, regressions, postmortems, and thresholds that came from data. It
investigates the motivation behind code. `maestro recipe show how` answers
what the code does and how it runs; this recipe answers what forces gave it
its shape. It returns a cited, confidence-graded read, and a
Preserve / Change / Avoid / Risk set when a change follows. Ported from
pstack's why skill (Lauren Tan, MIT; notice in THIRD_PARTY.md).

Work as a careful, cautious and precise investigator. Be honest about what
you know and what you infer. Every claim sits in one of the confidence tiers
below, and the synthesizer follows them.

## Steps

1. Understand the target and the question. The target is usually a chunk of
   code, a pattern, a feature or a named design decision. The question is
   usually a design rationale, a tradeoff, a motivating edge case, an
   external constraint, dead code, or a broad history sweep. If the target is
   vague, make your best guess from the conversation (open files, recent
   edits, what was just discussed), state it in one sentence, and proceed.
2. Establish the code anchor before spawning anyone. Collect file paths and
   line ranges, key symbols, the last commits touching the target, PR numbers
   from merge subjects (the `(#1234)` pattern), and linked ticket ids. Run
   `maestro search "<symbol or feature>"` too. A locked decision with its
   rationale is direct evidence. The anchor goes to every investigator and to
   the synthesizer.

   ```bash
   git blame -L <start>,<end> <file>
   git log --follow -p -- <file>
   git log --oneline -20 -- <file>
   git log -1 --format=%B <commit>
   gh pr view <number> --json title,body,author,createdAt,mergedAt,labels,closingIssuesReferences,comments,reviews
   ```

3. Map the evidence categories. Default to the full parallel investigation.
   The goal is a complete coverage map, not a minimal one. Document the null;
   never skip the search. Source control is always available through git and
   `gh`. Each other category runs only when the harness exposes a matching
   connector (an MCP server). Classify each connector by its name, server
   instructions, tool names and resource descriptions. A connector that fits
   two categories goes to the one matching its primary evidence; record the
   ambiguous ones in the coverage map.
4. Spawn the investigators. Spawn one subagent under the `investigator`
   profile per category with a connector, all at once, each owning exactly
   one connector. Never ask one investigator to cover two. Each brief is the
   investigator brief below, filled in, plus that category's row from the
   evidence table, the code anchor and the original question. When the target
   looks defensive (null checks, retries, timeout handling, rate limiting,
   feature flags, egress guards, out-of-memory handlers), add the incident row
   so each investigator runs incident queries inside its own source.
5. Skip a category only with a written reason, which goes into Sources
   Consulted. Two reasons are valid.
   - No connector exists for it in this harness. That is a gap, not a choice.
     "Real-time team chat skipped. No matching connector, so the conversational
     record was not searchable."
   - The source is provably irrelevant, not just probably. "Error tracking
     skipped. The target is a build-time script with no runtime path."
   - A single trivial commit whose PR already holds the complete answer may be
     answered inline, only after confirming every category search would be
     redundant. Say so. This is rare.
6. Synthesize. Spawn one subagent under the `explainer` profile as the
   synthesizer. Give it every investigator's findings including nulls, every
   skipped category with its reason, the code anchor, the original question,
   the confidence tiers and the synthesizer brief below. The brief's sections
   replace the explainer's default ones.
7. Present the synthesizer's output. Light edits for clarity or for context
   from the conversation are fine. Never rewrite the confidence language.
8. When the question comes before a change to this code, turn the findings
   into a Preserve / Change / Avoid / Risk set for planning the change, placed
   after Sources Consulted.

## Evidence categories

Each category is best at one shape of "why". Use the "best at" column to know
what to expect back, to name a gap when a category returns nothing, and, in
the rare provably irrelevant case, to justify a skip.

| Category | Best at | Look for |
|---|---|---|
| Source control (git, `gh`, code comments, tests). Always run. | Rationale captured at implementation and review time | History through renames, pickaxe (`git log -S '<text>'`, `git log -G '<regex>'`), one commit's diff (`git show <hash>`), PR bodies and review threads, TODO and FIXME near the target, ADRs (`rg -l -i 'architecture.decision' --glob '*.md'`), tests whose names encode the motivating edge case, files changed in the same commits, changelog entries, ticket ids in messages |
| Issue or ticket tracker | The product or business forcing function, strongest when the why is outside engineering | Tickets linked from commits and PRs first, read with every comment; keyword search with several phrasings; the parent of a sub-issue; project docs and specs; labels and milestones |
| Long-form documents | Design rationale written before it became code | Searches by feature name, symbols, author handles, error strings; the full page, not the preview; "alternatives considered" sections; child pages and backlinks; design-review meeting notes; the finalized or newest draft |
| Real-time team chat | Deliberation that never reached a doc, most valuable when the paper trail is thin | The PR author's messages around the merge date; searches for the PR URL or `/pull/<number>`; the error string; whole threads, never single messages. Direct messages are usually not searchable; name that gap |
| Infrastructure observability | Runtime reality the code reacts to (timeouts, retries, rate limits, circuit breakers) | The owning services first; dashboards and monitors whose threshold matches the code's; metrics around the change date; time-bounded log searches by service and tag; spans for slow or failing paths; incidents near the change date |
| Error or exception tracking | The exceptions and error trajectories behind defensive or corrective code | Issues matching the exception classes, function names, messages and file the target handles; first seen and last seen against the ship date; affected releases; a full event whose stack passes through the target; releases near the commit date |
| Product analytics warehouse | Product and data reality behind flag-gated code, experiments, migrations, and "where did this number come from" | Usage before and after the merge; the distribution (median, p99, max) of the guarded value before the change; experiment exposure by variant; query history for migrations and backfills. Probe table names before trusting them and time-bound every query, about 30 days on each side of the ship date |
| Incident and postmortem (add when the target looks defensive) | The incident a defensive change answered | Tickets labelled incident, severity, postmortem action item or reliability; incident chat channels around the add date; commits such as "fix for incident", "add defensive check", or a revert followed by a re-apply |

Pitfalls that hold across categories:

- Correlation is not causation. A spike before a change and calm after is
  suggestive. Check what else landed in the same window.
- Instrumented is not caused. A metric or event shows someone cared enough to
  measure it, not that the code exists because of it. Pair it with a commit
  or PR citation. A chart shows its maker's framing, not the reason for a
  line of code.
- A step in event volume may be a new event starting to log, not a change in
  behavior.
- Missing data from the relevant window (retention cliff, renamed metric, a
  table created later) is a gap, not a null result. Name it.
- An item you cannot access, or a connector that fails to authenticate, is a
  gap. Stop and report it. Never make up findings.
- Specs and tickets go stale and drift from the code. Check dates against the
  ship date, read the whole history, and name any divergence.
- Boilerplate "why" sections and casual chat jokes are not decisions. Look
  for considered, specific text.
- Follow closed-as-duplicate chains back to the canonical ticket.
- An AI-generated root-cause summary inside a tool is a hypothesis, not
  evidence.
- For a quantitative source, return the exact query, the time window and a
  compact numeric summary (counts, percentiles, first and last seen). Never
  dump raw rows.

## Confidence tiers

Code shows what it does, not why it exists. The why lives in commits, PRs,
tickets, docs and conversations, all incomplete and sometimes contradictory.
Every claim sits in exactly one tier, and the tier sets its section and its
phrasing.

1. **Direct.** An author wrote the reason. Examples are a PR description, a
   ticket, a code comment ("clamp to 100 because the upstream API rejects
   larger values"), a design doc, or the author's chat message. Phrase it
   with confidence, in present tense, cited. "This exists because X."
2. **Supported.** Several indirect items converge, though none states it.
   The PR title says "improve performance", the ticket is labelled perf, and
   the nearby commits touch the same hot path. Phrase it with confidence but
   as derived, citing each item. "The evidence points strongly to X."
3. **Inferred.** A reasonable reading that nothing explicitly supports. It is
   your interpretation, not a fact from the record. Phrase it hedged with the
   chain visible. "Given A and B, C seems likely because D."
4. **Speculative.** A plausible hypothesis on thin evidence, where other
   explanations fit as well. Mark it as a guess. "One possibility is X, but
   we have no direct evidence." It usually belongs in Competing Hypotheses.
5. **Unknown.** You looked and could not find out. Say exactly what you
   searched. "We searched the ticket tracker for A and B, read the 6 PRs that
   touched this file since 2023, and searched the repo for the threshold
   value. None gave a rationale."

Phrasing rules:

- Confidence words need a citation right beside them and never carry an
  inference. They are "because", "the reason is", "was designed to", "fixes",
  "addresses", "solves", and "the team decided".
- Hedges mark interpretation. Use "appears to", "seems to", "likely",
  "suggests", "is consistent with", "one reading is", "plausibly", "may have
  been", "the evidence points toward".
- Avoid "obviously", "clearly", "of course", "just" ("it's just for
  performance"), "I think" and "I believe". Write "the evidence suggests".

Reasoning rules:

- Do not rationalize. Code that makes sense today may have been written for
  reasons that no longer hold, or were wrong at the time. Do not assume a
  consistent pattern was intended when it may be copy-paste. Absence of
  evidence is not evidence of absence.
- Treat a hypothesis inside the question ("I assume it's for performance?")
  as one candidate. Check it independently. Confirm it with citations or say
  what the evidence supports instead.
- When sources contradict, show both with citations. Do not pick the tidier
  story. The owner makes the call.
- An honest "we don't know" is a valuable result. Name each gap concretely
  with the question, the sources searched, what you searched for, and what
  came back. A confident guess in place of a gap misleads, because the
  reader acts on it.

## Investigator brief

Fill in the placeholders and append the category row (and the incident row
for defensive code).

```text
You are investigating the history and motivation behind a piece of code.
Other investigators search other sources in parallel, and a separate
synthesizer combines the findings. Go deep on your assigned source only.
Gather evidence accurately; do not write prose answers. You never write
files, commit, or change external state.

Question: {QUESTION}
Target files: {FILES_WITH_LINE_RANGES}
Key symbols: {SYMBOLS}
Commits touching this code, newest first: {COMMIT_LIST}
PR numbers from commit messages: {PR_NUMBERS}
Ticket ids from commits or PR bodies: {TICKET_IDS}
Your source: {SOURCE_NAME}
{CATEGORY_ROW}

Loop:
1. Cast a wide net first, then narrow to specific items.
2. Read each PR, ticket, doc or thread in full. The key evidence is often in
   a comment, a subtask or a follow-up.
3. Follow links inside your source only. Record a reference into another
   source under Additional leads for that source's investigator. Chasing it
   yourself duplicates work.
4. Quote verbatim with the exact location (PR number, ticket id, URL, commit
   hash, file and line), so a reader can confirm the claim in seconds.
5. Record what you searched, not only what you found. Write queries
   verbatim. An empty search is a finding.
6. Resist the story. When items disagree, record both. When three line up and
   a fourth contradicts them, the contradiction is the most interesting
   finding.

Before calling a finding strong, ask whether you would expect to see it if
your current reading were wrong. Never invent. A partial finding is labelled
partial.

Discipline:
- Mechanics are not motivation. A commit changing a limit from 50 to 100
  shows the change, not the reason.
- Claim intent only where an author stated it. Code style is an observation,
  not evidence of intent.
- Keep ambiguity visible. Do not collapse it to look decisive.
- Evidence about feature Y does not answer a question about feature X.
- Gather, do not conclude. The synthesizer does the reasoning.

Return these sections:
- Source. Which source you investigated.
- What I searched. Queries run, items opened, places looked.
- Direct evidence. For each item, the quote, where it is from, author and
  date, and one sentence on how it bears on the question.
- Indirect evidence. For each item, what it is, where it is from, what a
  careful reader might infer and through which chain, and any alternative
  reading.
- Contradictions. Items that disagree, with both citations.
- Gaps. What you searched for and did not find, with the query and range.
- Additional leads. Pointers for another source's investigator.
```

## Synthesizer brief

```text
You are answering a "why" question about a piece of code from findings that
investigators gathered in separate sources. Produce a confidence-weighted,
cited account that says what the evidence supports and what it does not. You
are read-only. You may spot-check a citation by reading the code or calling a
connector, but never write files, commit, or change external state.

Question: {QUESTION}
Target files: {FILES_WITH_LINE_RANGES}
Key symbols: {SYMBOLS}
Investigator findings: {ALL_INVESTIGATOR_FINDINGS}
Sources not searched, with reasons: {SKIPPED_SOURCES_WITH_REASONS}
Confidence tiers and phrasing rules: {TIERS}

Instructions:
1. Read every finding and weigh the evidence yourself. Investigators gathered
   raw material, not conclusions.
2. Merge overlaps. Several investigators citing one PR, ticket or doc get one
   reference.
3. Show contradictions. Never pick one side silently.
4. Calibrate each claim to its tier and phrase it to match. A claim with no
   evidence goes to the gaps.
5. Spot-check any citation you doubt. Do not propagate errors.
6. Do not overreach. Leave an open question open.

Write exactly these sections:
- The question. One or two sentences.
- The code in question. Paths, line ranges, key symbols, two or three lines.
- What we found. "[Direct] <claim>. Source: <citation>. <quote>" and
  "[Supported] <claim>. Evidence: <each item and what it adds>".
- What we can reasonably infer. "[Inferred] <hedged claim>. Reasoning:
  <evidence and inference step>". Skip when there is nothing to infer.
- Competing hypotheses. For each, the hypothesis, the evidence for, and the
  evidence against or missing. Skip when one answer is clear.
- What we don't know. Unanswered questions, searches that returned nothing,
  unavailable sources and why, people who would likely know.
- Sources consulted. One line per category, including empty and skipped ones
  with the reason ("Not searched. No matching connector in this harness.").
  Source control is never skipped.
- Confidence summary. One or two sentences on overall confidence.

Check before returning, and revise on any failure:
1. Every claim in What we found carries a citation, or it moves to Inferred
   or Competing hypotheses.
2. Phrasing matches the tier. Direct claims may say "because"; inferred ones
   may not.
3. Every contradiction you noticed is shown.
4. What we don't know names specific gaps. An empty one is suspicious.
5. Any hypothesis in the question was checked, not rubber-stamped.
6. No claim cites code as evidence of its own intent. Code is mechanics.
7. The tone matches the evidence. A confident answer on weak evidence is the
   failure this recipe exists to prevent.

The value of this output is its honesty, not its authority. Optimize for
being useful, not for looking decisive.
```

## Failure mode

Recency bias. The latest commit is not authoritative. The current shape is
often the accretion of many earlier decisions. Trace back.

## Reply

The synthesizer's sections with the confidence language intact, then the
Preserve / Change / Avoid / Risk set when a change follows.
