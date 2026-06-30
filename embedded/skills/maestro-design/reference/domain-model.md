# Domain Model

Use this branch when a design fork depends on project language, bounded
contexts, business concepts, or a plan that should be challenged against the
existing domain model.

## Find The Maestro Domain Record

During current-state mapping, use Maestro's native search engine before generic
repository search:

```sh
maestro grep "<topic>"
maestro grep "<topic> corpus:memory"
maestro grep "<topic> corpus:source"
```

Then read the Maestro artifacts it points at:

- `.maestro/cards/<feature>/spec.md` is the editable design record. Use
  sections such as `Language`, `Relationships`, and `Flagged ambiguities` for
  domain terms.
- `.maestro/cards/<feature>/handoff.md` is the clean continuation index after
  `maestro feature finalize`.
- `.maestro/cards/<feature>/notes.md` carries dated context and corrections.
- `maestro decision list --feature <id>` and `maestro decision show <id>` hold
  locked rulings and rejected alternatives.
- `maestro grep "<topic> corpus:memory"` finds reusable language and precedent
  across prior work.

Do not create `CONTEXT.md`, `CONTEXT-MAP.md`, or `docs/adr/` from this Maestro
skill. Code and ordinary docs are evidence, not the domain record of authority.

## Run The Fork

1. Walk the design tree one fork at a time. Ask one question, wait for the
   answer, then continue.
2. If code, docs, Maestro artifacts, or command output can answer the question,
   inspect them instead of asking the user.
3. Give each question a recommended answer.
4. Challenge terminology immediately when the user uses a term that conflicts
   with feature spec language, locked decisions, memory, or code evidence.
5. Sharpen vague or overloaded language into one canonical term.
6. Stress-test relationships with concrete scenarios that probe boundaries,
   edge cases, and cardinality.
7. Cross-reference user claims with code. If code contradicts the stated model,
   surface the contradiction and ask which source should change.
8. Lock decisions through the normal `maestro decision new` /
   `maestro decision lock` path once the fork is settled.

Completion criterion: every material domain term used by the design has an
existing Maestro-backed definition, a new feature-spec entry, or an explicit
unresolved feature question; every settled domain fork is reflected in the
feature decision record.

## Update Maestro Artifacts

When a term is resolved, update the feature spec immediately. Do not batch
glossary updates. Keep the language domain-facing, not implementation-facing.
Use:

```sh
maestro feature spec <id> --section "Language" --append "<entry>"
maestro feature spec <id> --section "Relationships" --append "<entry>"
maestro feature spec <id> --section "Flagged ambiguities" --append "<entry>"
```

Use this shape:

```md
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

## Lock Durable Rulings Sparingly

Use `maestro decision new` and `maestro decision lock` only when all three are
true:

- hard to reverse
- surprising without context
- the result of a real trade-off

Keep the decision context small:

```md
# {Short title of the decision}

{1-3 sentences: context, what was decided, and why.}
```

Record rejected alternatives in the lock context when they prevent future
re-litigation. If a locked decision is contradicted later, supersede it; never
edit or unlock the old Decision record.
