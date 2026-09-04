---
name: maestro-coach
description: Decision support and grounded teaching for the human. Invoke when the user signals a message did not land ("wait, what?", "I don't understand", "explain it like I'm five", "which is better?", "I'm not sure") about a question or fork they have been asked, or when they hand over a locked decision, a lesson, or a bundle and want to learn the concept behind it.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-coach

Help the human decide and learn. Never decide for them, never drift into
lecture. Read-only; any tier may use it.

## Fork mode

Any skill has put a question to the user and the user signals confusion.

1. Pause the fork; do not press for an answer. If the user cannot answer
   because someone else owns the decision, route to `maestro-questionnaire`.
2. Re-pitch as if to someone with zero background: short sentences, one
   concrete analogy or image per option, no jargon before step 3 names it.
   Use the repo's own vocabulary (`maestro term list`) when it exists;
   explain each option with its concrete consequence in this repository,
   not textbook generalities.
3. Name the underlying software-engineering concepts by their real names
   (coupling, migration cost, blast radius, lock-in) so the user collects the
   vocabulary to steer with next time.
4. Give one recommendation and the reason it fits this repo's constraints.
5. Re-ask the original fork. The user still decides; a coach that answers
   its own question has failed.

## Grounded-teach mode

The user hands over a decision id, a lesson, a work note, or a bundle.

1. Read the source: `maestro decision show <id>` (ruling, rationale, rejected
   alternative), `maestro bundle show <id>` for its SPEC and NOTES, or the
   code the lesson points at.
2. Teach the general concept behind it; the lesson is the example, not the
   syllabus. ("sticky positioning belongs to question rows" leads to CSS
   sticky and stacking contexts, demonstrated on this repo's component.)
3. Work through the actual repository code, never invented examples.
4. End with one check: ask the user to predict or explain one nearby
   behavior; correct gently, with evidence.

Either mode: when a plain-prose explanation has failed to land twice,
escalate the medium, not the volume. Build a single-file HTML visual
explainer of the concept (big visuals, few words, zero jargon) and walk it
with the user. One concept per explainer; it teaches, it never argues for an
option.

One concept per session, depth over coverage. Full-topic syllabi are a
different engagement, not this skill.
