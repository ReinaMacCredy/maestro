---
name: maestro-questionnaire
description: Turn a decision the user cannot fully answer into a Markdown questionnaire for the one person who can - filled in async, or together over a meeting. Adapted from Matt Pocock's to-questionnaire.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-questionnaire

Turn something the user cannot answer alone into a **questionnaire**: a
Markdown document they hand to one person to fill in async, or fill out
together over a meeting. The recipient holds knowledge the user lacks; the
questionnaire pulls it out of them. Read-only toward code; any tier may use it.

**Grill the send, not the subject.** Interview the user only about the
_send_, which they can always answer: who it goes to, and what they need
back. The questions in the document then target the **gap** between what the
recipient knows and what the user needs.

1. **Who is it going to?** Ask, in one exchange, the recipient's role,
   expertise, and relationship to the user. This fixes the questionnaire's
   tone and how much context it must carry. Done when you know who the
   recipient is and what they know that the user does not.

2. **What do you need back?** Ask, in one exchange, the specific decisions
   or facts the user cannot resolve alone and needs from this person. Done
   when you have a concrete list of what the user must walk away able to do
   or decide.

3. **Write the questionnaire.** Draft questions aimed at the gap from steps
   1 and 2, following the Document structure below. When an active bundle
   owns the decision, write it to
   `.maestro/bundle/<bundle-id>/QUESTIONNAIRE-<slug>.md`, record the wait as
   `maestro work note <id> "blocked: questionnaire <slug> sent to <role>"`,
   and set the bundle's NOTES.md Next Action to the return of the answers;
   otherwise write `questionnaire-<slug>.md` in the current directory. Report
   the path. Done when the file exists and every item the user named in step
   2 is covered by a question.

When the answers come back, they are settled forks: record each as
`maestro decision draft "<answer>" --rationale "<why, per <recipient>>" --work <id>`
then `maestro decision lock <id>`, and clear the Next Action. The filled
questionnaire is the evidence the decision cites.

## Document structure

Frame the document as a **discovery questionnaire**: the user lacks context,
the recipient holds it. Order questions most-important-first, since async
means you may only get one pass, and group them under `##` headings by theme
once there are more than a handful. Write it using the template below.

<questionnaire-template>

# <Questionnaire title>

**Purpose:** why this questionnaire exists and the decision riding on it.

**From:** <the user> **To:** <the recipient> **How your answers will be
used:** <where they go>

## Context

One paragraph orienting a recipient who was not in the user's head. Enough to
answer well, not a page.

## How to answer

Deadline and rough effort. Partial answers and "I don't know" are useful;
flag anything you are unsure of rather than skipping it.

## <Theme heading>

One `##` section per theme. Under each, its questions, most-important-first.
Every question is one idea, never compound, with an answer stub directly
beneath, and a one-line _why this matters_ only where the question could be
misread or invite a throwaway answer.

<question-example>
### What load is the system expected to handle at launch?

_Why this matters: it decides whether we provision for burst traffic now or
defer it._

>
</question-example>

## Anything else?

A closing catch-all: anything we did not ask that we should know?

</questionnaire-template>
