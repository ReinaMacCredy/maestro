# Grilling

Interview the user until you reach a shared understanding. Map the effort as
a **design tree**: every decision branches into the decisions that hang off
it.

Work the tree in **rounds**. The **frontier** is every decision whose
prerequisites are already settled — the questions you can ask _now_ without
guessing at answers you haven't heard yet. Ask the frontier in plain prose,
one decision at a time when the forks are heavy, or as a short numbered list
when they are independent and light. Then wait for the user's answers before
the next round.

For each question:

- Lead with **what this does** — the consequence of the decision in this
  repo, before any option list.
- Present the options as prose, each with its concrete trade-off. When a
  layout or structure is easier to see than to describe, include a small
  ASCII sketch per option.
- End with a `my rec:` line — your recommended answer and the one-line why.
- No emoji, no batching questions that depend on each other's answers. When
  the harness offers a question card, use it: one decision per card, "what
  this does" first, the sketch per option, the `my rec:` line on every fork.
- Record each answer the moment it lands:
  `maestro decision draft "<choice>" --rationale "<why + rejected alternative>" --work <id>`
  then `maestro decision lock <id>`. An answer to a fork is a decision to
  record, never an implementation order, even when the chosen option is
  itself an artifact (a script, a schema, a prototype); building starts only
  on an explicit request.

Each round the user answers reshapes the tree — settled decisions push the
frontier outward and unblock questions that depended on them. Recompute the
frontier and ask the next round. A question whose answer depends on another
question still open in this round belongs to a _later_ round, not this one.

Finding _facts_ is your job, never the user's. When a frontier question needs
a fact from the environment (filesystem, tools, docs), dispatch a sub-agent
to find it — don't ask the user for anything you could look up yourself.
Don't block on it: a running exploration is an unsettled prerequisite, so
only the questions downstream of it wait for the sub-agent to report — ask
the rest of the frontier now. The _decisions_ are the user's — put each to
them and wait.

The session is done when the frontier is empty: every branch of the design
tree visited, nothing left silently assumed. Do not act on it until the user
confirms you have reached a shared understanding.
