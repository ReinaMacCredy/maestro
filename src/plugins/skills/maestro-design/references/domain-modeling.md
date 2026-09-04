# Domain modeling

Actively build and sharpen the project's domain model as you design: challenge
terms, invent edge-case scenarios, and record the glossary and decisions the
moment they crystallise. Merely reading the glossary for vocabulary is a
one-line habit any skill can do; this reference is for when you are changing
the model, not just consuming it.

## Where it lives

Both artifacts live in the maestro store, so they sit in the recall path every
session already reads:

```
maestro term add <name> "<definition>" [--work <id>]   # record or redefine a term
maestro term list | maestro term show <name>            # the glossary
maestro decision draft "<choice>" --rationale "<why + rejected alternative>" --work <id>
maestro decision lock <id>                              # the recorded decision
maestro search "<word>"                                 # terms, decisions, work, bundles
```

There is no glossary file to create; the first `term add` is the glossary.

## During the session

### Challenge against the glossary

When the user uses a term that conflicts with an existing definition, call it
out immediately. "The glossary defines 'cancellation' as X, but you seem to
mean Y. Which is it?"

### Sharpen fuzzy language

When the user uses vague or overloaded terms, propose a precise canonical
term. "You are saying 'account'. Do you mean the Customer or the User? Those
are different things."

### Discuss concrete scenarios

When domain relationships are being discussed, stress-test them with specific
scenarios. Invent scenarios that probe edge cases and force the user to be
precise about the boundaries between concepts.

### Cross-reference with code

When the user states how something works, check whether the code agrees. If
you find a contradiction, surface it: "The code cancels entire Orders, but you
just said partial cancellation is possible. Which is right?"

### Record terms inline

When a term is resolved, `maestro term add` right there. Do not batch these
up; capture them as they happen. A definition is domain language only: no
implementation details, no file paths, no decisions. Redefining a term is a
new `term add` with the same name; say why in the conversation.

### Lock decisions sparingly

Every settled fork gets a decision, but only offer to lock one for a choice
the user has not asked about when all three are true:

1. **Hard to reverse**: the cost of changing your mind later is meaningful.
2. **Surprising without context**: a future reader will wonder "why did they
   do it this way?"
3. **The result of a real trade-off**: there were genuine alternatives and
   you picked one for specific reasons.

The rationale carries the rejected alternative and the why; a later reversal
is a new decision with `--supersedes <id>`, never an edit.
