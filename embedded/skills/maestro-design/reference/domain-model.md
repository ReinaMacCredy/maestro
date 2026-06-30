# Domain Model

Use this branch when a design fork depends on project language, bounded
contexts, business concepts, or a plan that should be challenged against the
existing domain model.

## Find The Domain Record

During current-state mapping, also look for existing domain documentation:

- Root `CONTEXT-MAP.md` means the repo has multiple contexts; read it and use
  the context path that matches the topic.
- Root `CONTEXT.md` means the repo has one context.
- `docs/adr/` at the root holds system-wide decisions.
- Context-local `docs/adr/` holds context-specific decisions.

Create files lazily. If no `CONTEXT.md` exists, create it only when the first
domain term is resolved. If no `docs/adr/` exists, create it only when the first
ADR qualifies.

## Run The Fork

1. Walk the design tree one fork at a time. Ask one question, wait for the
   answer, then continue.
2. If code, docs, Maestro artifacts, or command output can answer the question,
   inspect them instead of asking the user.
3. Give each question a recommended answer.
4. Challenge terminology immediately when the user uses a term that conflicts
   with existing `CONTEXT.md` language.
5. Sharpen vague or overloaded language into one canonical term.
6. Stress-test relationships with concrete scenarios that probe boundaries,
   edge cases, and cardinality.
7. Cross-reference user claims with code. If code contradicts the stated model,
   surface the contradiction and ask which source should change.
8. Lock decisions through the normal `maestro decision new` /
   `maestro decision lock` path once the fork is settled.

Completion criterion: every material domain term used by the design has either
an existing definition, a new resolved definition, or an explicit unresolved
feature question; every settled domain fork is reflected in the feature
decision record.

## Update CONTEXT.md

When a term is resolved, update the matching `CONTEXT.md` immediately. Do not
batch glossary updates. Keep it domain-facing, not implementation-facing.

Use this shape:

```md
# {Context Name}

{One or two sentence description of what this context is and why it exists.}

## Language

**Canonical Term**:
One-sentence definition of what it is.
_Avoid_: old alias, ambiguous synonym

## Relationships

- A **Canonical Term** belongs to exactly one **Other Term**

## Example dialogue

> **Dev:** "Question using the canonical terms?"
> **Domain expert:** "Answer that clarifies the boundary."

## Flagged ambiguities

- "ambiguous word" was used to mean both **Term A** and **Term B**; resolved:
  they are distinct concepts.
```

Rules:

- Pick one canonical word and list aliases to avoid.
- Define what the concept is in one sentence.
- Include only project-domain concepts, not general programming terms.
- Express relationships and cardinality where obvious.
- Add flagged ambiguities when a term was overloaded or corrected.

## Offer ADRs Sparingly

Offer an ADR only when all three are true:

- hard to reverse
- surprising without context
- the result of a real trade-off

ADRs live in `docs/adr/` or the matching context's `docs/adr/`. Number them by
scanning for the highest existing `NNNN-*.md` and incrementing it. Keep the file
small:

```md
# {Short title of the decision}

{1-3 sentences: context, what was decided, and why.}
```

Optional sections such as `Status`, `Considered Options`, or `Consequences`
belong only when they carry useful future context.
