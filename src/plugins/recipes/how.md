# How

Use this recipe to answer "how does X work" before changing it. It covers a
code walkthrough, a subsystem's runtime flow, an onboarding mental model, and
a placement question ("where should this live", "which package owns this",
"is this the right layer"). The result is the explanation a senior engineer needs
to start working in the area, not annotated source. For the forces that gave
the code its shape, use `maestro recipe show why`. This recipe also holds the
research rules for every fact an explanation rests on, inside the repository
or outside it. Ported from pstack's how skill (Lauren Tan, MIT; notice in
THIRD_PARTY.md).

## Steps

1. Search the store with `maestro search "<term>"` before reading anything
   else. A past decision, note or archived bundle may already hold the answer.
   Cite what you use and re-verify fast-drifting facts live.
2. Assess complexity. If the scope is ambiguous, state your reading and
   continue. The owner can redirect.
   - **Simple.** One module, a small utility, or a narrow question such as
     "how does function X work". Spawn no explorers. Spawn one subagent under
     the `explainer` profile that explores and explains in one pass, using the
     explainer brief below without the explorer findings. Go to step 5.
   - **Complex.** A subsystem across several files or services, a
     cross-cutting feature, or a full architectural overview. Go to step 3.
   - When in doubt, take the simple path.
3. Explore (complex only). Split the question into 2 to 4 angles, each a
   distinct slice of the subsystem. Spawn every explorer at once, one subagent
   under the `explorer` profile per angle, each with the explorer brief below
   and its angle filled in.
4. Synthesize (complex only). Once every explorer has returned, spawn one
   subagent under the `explainer` profile with the explainer brief and every
   explorer's findings filled in.
5. Present the explainer's output. Light edits for clarity or for context from
   the conversation are fine. Do not substantially rewrite it.
6. Land the findings as "Where findings land" below says.

## Research rules

These hold for every claim in the explanation and for any fact looked up
along the way.

- Read primary sources. Official docs, source code, specs and first-party
  APIs count. A secondary write-up of them does not. Follow every claim back
  to the source that owns it.
- Cite each claim's source next to the claim.
- Never fabricate a link, citation or reference. Link only what you read in
  this session.
- A small fact needed mid-task to keep going (an API detail, a version quirk)
  needs no ceremony. Look it up inline against primary sources, record the
  finding and its link with `maestro work note <id> "<finding + link>"`, and
  move on.
- Research long enough to block you goes to a background subagent under the
  `explorer` profile. It writes its findings to one Markdown file in the
  folder `maestro topic path <slug>` prints, citing each claim's source.

## Where findings land

- A fact is evidence, not a decision. When it settles a durable choice,
  record that choice per `~/maestro/WORKFLOW.md#decisions-and-readiness`
  with `maestro decision draft "<choice>" --rationale "<why, with source link>" --work <id>`,
  then `maestro decision lock <id>`.
- Working evidence for an open work item is a work note with its link.
- Current behavior the next change must preserve is a VERIFY.md scenario in
  the bundle, or the work item's acceptance when there is no bundle.
- A long-form artifact (a report, an inventory, a subagent's answer, a
  capture) goes in the topic folder per
  `~/maestro/WORKFLOW.md#concurrency-and-git`, linked from the work note.
- With no work item and no bundle, deliver the findings in the conversation.
  Open a work item only when the question feeds work that will happen. A
  finding alone never opens a bundle.

## Explorer brief

Fill in the question and the angle.

```text
Other explorers are investigating different slices of the same subsystem in
parallel. Do not try to cover everything. Focus on your assigned angle and go
deep. You are read-only.

Question: {QUESTION}
Your angle: {EXPLORATION_ANGLE}

Return these sections:
- Components found. The key types, services, classes and abstractions. For
  each, the name, the file path and one sentence on what it does.
- Flow. The execution flow step by step. For each step, the function that
  runs, its file, what it does and what it calls next, with the data that
  passes between steps.
- Files read. Every file you read, so the explainer can reference them.
- Boundaries. Where this subsystem connects to the rest of the codebase. Its
  inputs and outputs.
- Non-obvious things. Anything surprising, historically motivated, or easy to
  get wrong. Things that look like they work one way but work another.
- Open questions. Anything you could not fully trace. Be honest about gaps.
```

## Explainer brief

Fill in the question and, on the complex path, every explorer's findings.

```text
You are writing an architectural explanation for a senior engineer. You are
read-only.

Question: {QUESTION}
Explorer findings: {EXPLORER_FINDINGS_ALL}

The explorers each traced a different angle of the same subsystem. Their
findings overlap and may contradict. Reconcile them. Merge overlapping
descriptions, resolve each contradiction by checking the code yourself, and
combine the slices into one picture. You may read the codebase to check a
detail or fill a gap. The explorers did the work, so do not re-explore from
scratch. On the simple path there are no findings, so explore the code
yourself first.

Write so a senior engineer new to this area walks away with a working mental
model, enough to start working in it with confidence.

Use these sections, dropping any that do not apply:
- Overview. One or two paragraphs. What this is, what it does, why it
  exists. A reader can stop here and decide whether to keep reading.
- Key concepts. The types, services or abstractions needed to follow the
  rest. Brief definitions, not exhaustive.
- How it works. The core and the longest section. What triggers it, what
  happens step by step, where data goes, where the decision points are. Use
  prose, not pseudocode. Name files and functions so the reader knows where
  to look. Skip large code blocks unless a snippet is essential. When several
  components talk to each other or data changes through stages, add a
  diagram. Use mermaid for structured flows and ASCII for simple
  relationships. A diagram must clarify, not decorate. If prose covers the
  flow, skip it.
- Where things live. A short file and directory map, only what someone needs
  to start working here.
- Gotchas. Non-obvious behavior, history, pitfalls. Skip it when there is
  nothing worth naming.

Style:
- Concrete language. Write "UserService calls AuthClient.refresh()", not "the
  service delegates to the client".
- When something is complex, explain why it is complex.
- When something is simple, do not pad it.
- Use an analogy only when one helps.
- Name the explorers' open questions and gaps instead of hiding them.
```

## Reply

The explanation in the five sections above, then where its findings landed
(the decision id, the work note, the topic file, or "conversation only").
