---
name: maestro-improve
description: Turn filed lessons into the smallest doctrine edit. Use when a Lead assigns improvement work for one target: group pending lessons, make one evidence-linked commit per target, return it for independent challenge, and process each accepted or rejected lesson without deleting history.
review-date: 2026-11-29
---
<!-- maestro-skill-version: dev -->

# maestro-improve

Use when a Lead assigns improvement work on one target to a Peer. The target is
the only parameter; a separate Peer challenges the returned candidate before
acceptance.

A correction that stays in a transcript is spent when that session ends. The
lesson record is what survives, and doctrine is the only thing a future session
actually reads. This skill is the one place the two are joined: it turns filed
corrections into the smallest edit that would have prevented them.

## The parameter

The target names the doctrine a lesson corrects: a recipe section, the SLP
Workspace Pack in `src/plugins/resources/SLP.md`, a Hub template, a
`skills/maestro-*` file, or a repository's Workspace Protocol. Where each one
lives, and what an edit to it costs, is in
[references/targets.md](references/targets.md).

## The loop

```sh
maestro lesson list --project <project>     # pending only, by design
maestro lesson show <id>                    # the gap, the expectation, the why
```

- Group the pending lessons by target. Two lessons on one rule are one edit,
  not two, and the second one is usually what tells you which reading of the
  rule was ambiguous.
- Per group, propose the **smallest edit** that would have prevented what
  happened. Doctrine is read under load, so a sentence that removes an
  ambiguity beats a paragraph that adds a procedure. If the existing text
  already says it, the lesson is a rejection, not an edit.
- One commit per target group, on a branch, with the **evidence ids** of every
  lesson in the group in the message. The ids are how a later reader gets from
  the rule back to the incident that shaped it.
- Mark each lesson processed by pointing at that commit:

```sh
maestro lesson process <id> --commit <sha>
```

The improver never deletes a lesson and never edits one. Processing is a
pointer, so the record of what was corrected stays readable after the doctrine
it corrected has changed again.

## Rejecting a lesson

A lesson can be wrong: the rule already covers it, the correction misread it,
or two lessons disagree and only one survives. Answer it where it lives and
mark it processed in the same command:

```sh
maestro lesson process <id> --answer "<why this produced no edit>"
```

The answer is for whoever filed it. "Out of scope" is not an answer; name the
text that already covers it, or the reading that was wrong.

## The replay gate

Doctrine has golden scenarios: a script of maestro commands and the transcript
it produced. In this repository they are `tests/scenarios/<name>.script` and
`<name>.golden`, replayed by `bun test tests/scenario-golden.test.ts`.

Run the replay after every edit. An edit is accepted only when it still matches
the golden set, or matches the change a lesson explicitly expected. A replay
that drifts in a way no lesson asked for is a regression in the doctrine, not an
improvement, and it goes back before the work return. When a lesson did ask for the
change, re-record with `MAESTRO_GOLDEN_UPDATE=1 bun test
tests/scenario-golden.test.ts` and put the new golden in the same commit, so the
diff shows the behaviour that changed next to the sentence that changed it.

A doctrine edit no scenario covers is an edit nothing can falsify: add the
scenario in the same commit rather than leaving the rule unwatched.

## Filing new lessons

Reading a pile of corrections is the best moment to notice one nobody filed.
File it (`maestro lesson file`) rather than folding it silently into an edit:
the next improver run needs the same evidence trail this one had.

## Return

Return the bounded candidate through the normal SLP work operation:

```sh
maestro work return <work-id> "candidate: <branch and commits>; lessons: <ids and stores>; proof: <replay>; residual risk: independent challenge pending"
```

Name every lesson processed, answered, or left pending in the return body. A
lesson in another store is processed by that store's holder after acceptance;
do not mutate it from the wrong project. The Lead assigns a separate challenge
Peer, reconciles the two returns, and accepts only after the challenge and
replay support the candidate. Improving doctrine and approving it alone is the
loop this separation exists to break.

## Red flags

| The thought | The reality |
|---|---|
| "While I am in this file I will also tidy..." | Every changed line traces to a lesson, or the challenge Peer cannot tell an edit from an opinion. |
| "These five lessons all point at the same mess; I will rewrite the section" | A rewrite loses the reading that was ambiguous. Fix the ambiguity, keep the section. |
| "This lesson is wrong, I will just leave it pending" | Pending means unread. Answer it and mark it processed, so it stops counting toward the next threshold. |
| "The scenarios changed because my edit is better" | An unrequested replay drift is a regression until a lesson says otherwise. |
| "I will land the branch since it is obviously right" | The challenge Peer and reviewer boundary are the point; a self-approved doctrine edit is one model marking its own work. |
