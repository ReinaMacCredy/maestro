# Grilling

Use this branch when the user asks to grill, stress-test, challenge, sharpen, or
interview a plan or design before building.

## Run The Session

1. Interview relentlessly until there is shared understanding.
2. Walk the design tree branch by branch. Resolve prerequisite decisions before
   dependent ones.
3. Ask exactly one question at a time, then wait for the answer. Do not bundle
   independent questions.
4. Include your recommended answer with each question.
5. If code, docs, Maestro artifacts, command output, or existing decisions can
   answer the question, inspect those instead of asking the user.
6. Convert each settled branch into the normal `maestro decision new` /
   `maestro decision lock` record. Leave unsettled branches as explicit feature
   questions.

Completion criterion: every material branch is settled and locked, left as an
explicit feature question, or blocked on a named missing fact; no batch of
unanswered questions remains.

## Grill With Docs

When the user asks for grilling with docs, or the grilling turns on project
language, bounded contexts, business concepts, glossary, or durable trade-offs,
also use [domain-model.md](domain-model.md).

The docs-backed variant keeps the same one-question-at-a-time grilling rhythm,
but resolved domain terms are captured immediately in feature spec sections, and
Maestro decisions are locked only for hard-to-reverse, surprising trade-offs.
