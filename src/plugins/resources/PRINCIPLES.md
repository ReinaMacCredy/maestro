# Principles

This file holds twenty-four judgment rules for code and agent work. They are
rules you steer by, not procedures and not gates. Recipes hold the procedures
(`maestro recipe show <name>`), and WORKFLOW.md holds the method rules.
Nothing in maestro refuses an operation because a principle applies. You do
not invoke a principle. You use its name to steer, and the name points at a
complete rule.

To apply one, find its section through the index below or with
`rg -n '^## ' ~/maestro/PRINCIPLES.md`. Read that whole section before you
apply it. A section that points at WORKFLOW.md adds only what WORKFLOW.md
lacks, so read the rule it points at as well.

A reply closes by naming each principle that changed a decision and the
choice it changed, cited as "**Fix Root Causes** (PRINCIPLES.md)". Cite only
sections you read this session. A citation with no changed decision behind it
is a name-drop, not an application.

Ported from pstack's principles (Lauren Tan, MIT; notice in THIRD_PARTY.md).

**Core**

- **Laziness Protocol.** Refactoring, sizing a diff, or tempted to add abstractions, layers, or signal threading. Bias to deletion and the smallest change that solves the problem.
- **Foundational Thinking.** Before writing logic, when choosing core types and data structures, sequencing scaffold before features, or asking what concurrent actors share.
- **Redesign from First Principles.** Integrating a new requirement into an existing design. Redesign as if it had been foundational from day one.
- **Attack the Premise.** Two or more fixes that share one premise have failed the same gate. Write the premise down and test it directly before you write another fix that assumes it.
- **Subtract Before You Add.** Sequencing an addition, refactor, or rewrite. Remove dead weight first, then build on the simpler base.
- **Minimize Reader Load.** Reviewing or shaping code that's hard to trace. Count layers and hidden state, collapse one-caller wrappers, shrink mutable scope.
- **Outcome-Oriented Execution.** Planned rewrites and migrations with explicit phase boundaries. Converge on the target architecture. Do not preserve throwaway compatibility states.
- **Experience First.** Product, UX, or feature-scope tradeoffs. Choose user delight over implementation convenience.
- **Exhaust the Design Space.** A novel interaction or architectural decision with no precedent. Build two or three competing prototypes and compare before committing.
- **Build the Lever.** Any non-trivial work. Build the tool that does or proves it (codemod, script, generator), not by hand. The tool is the artifact a reviewer reruns.

**Architecture**

- **Model the Domain.** Writing stateful logic, or code that branches a lot or repeats a shape assumption across files. Encode the domain in a structure (state machine, typed model, table or registry, reducer, boundary, the right collection) instead of scattered conditionals.
- **Boundary Discipline.** Wiring validation, error handling, or framework adapters. Guards at system boundaries, trust internal types, keep business logic pure.
- **Type System Discipline.** Designing types or a signature in any typed language. Make illegal states unrepresentable, brand primitives, parse external data at boundaries.
- **Make Operations Idempotent.** Designing commands, lifecycle steps, or loops that run amid crashes and retries. Converge to the same end state.
- **Migrate Callers Then Delete Legacy APIs.** Introducing a new internal API while old callers exist. Migrate and delete in one wave.
- **Separate Before Serializing Shared State.** Concurrent actors might write the same file, branch, key, or object. Eliminate the sharing first.

**Verification**

- **Prove It Works.** After a task, before declaring done. Verify against the real artifact, not a proxy or "it compiles".
- **Fix Root Causes.** Debugging. Trace each symptom back to the first place the system went wrong and fix it there. A guard is a fix only when the state it handles is legitimate.
- **Sequence Work into Verifiable Units.** Multi-step work (sweeps, migrations, runs of similar edits) and how you stack commits and PRs. Break work into small units that each end in a check, verify each before the next, and order delivery so the sequence proves itself.
- **Test Behavior Not Implementation.** Writing, changing, or keeping a test. Assert what the code's users can observe, so the test fails when that behavior breaks and keeps passing when only the internals change.
- **Explain the Number.** Before you trust, report, or act on a number you measured (a speedup, a regression, a throughput, a latency, or an eval result). Find what limits it, and rule out that it measured something other than the work you think.

**Delegation**

- **Guard the Context Window.** Context fills up with large outputs, long files, repeated reads, or fan-out planning. Route bulk to subagents and keep summaries in the main thread.
- **Never Block on the Human.** Tempted to ask "should I do X?" on reversible work. Proceed, present the result, and let the human course-correct.

**Meta**

- **Encode Lessons in Structure.** You catch yourself writing the same instruction a second time. Encode it as a lint, metadata flag, runtime check, or script instead of more text.

## Laziness Protocol

Apply when refactoring, sizing a diff, or tempted to add abstractions,
layers, or signal threading. Bias toward deletion and the smallest change that
solves the problem.

- **Consolidate decisions.** Do not repeat the same choice in several places.
  Put it behind one source of truth and pass the result as a simple flag.
- **Minimize the diff.** Make the smallest change that solves the problem.
  Fewer lines beat "elegant" boilerplate.
- **Measure the minimum in concepts.** Minimum means the fewest concepts a
  maintainer meets at the seam, not the fewest lines. A wrapper that hides
  behavior to shorten a diff is a new concept. The smallest change in the
  wrong layer is a second bug.
- **Reach for what the repo already uses.** A helper, type, component, or
  installed dependency beats new code. It also beats a native platform
  feature the repo has an established equivalent for.
- **Fix once.** A bug fix lands once, where every caller routes through.
- **Be lazy about the solution only.** Never be lazy about trust-boundary
  validation, error handling that prevents data loss, security, or anything
  explicitly requested.

## Foundational Thinking

Apply before writing logic, when choosing core types and data structures,
sequencing scaffold and feature work, or asking what concurrent actors share.
Get the data structures right so downstream code becomes obvious.

**Structural decisions** protect option value. **Code-level decisions**
protect simplicity.

**Data structures first.** Get the data shape right before writing logic.
Define core types early, trace every access pattern, and choose structures
that match the dominant paths.

At code level, DRY the structure, not every line. Types and data models
should converge. Three similar statements still beat a premature abstraction.
Prefer explicit over clever. Test behavior and edge cases, not line counts.

**Concurrency corollary.** Before sharing state between actors, ask "what
happens if another actor modifies this concurrently?" If the answer is not
"nothing", isolate.

**Scaffold first.** If something helps every later phase, do it first. Ask
"does every subsequent phase benefit from this existing?" CI, linting, test
infrastructure, and shared types are scaffold. Sequence for option value, with
setup before features and tests before fixes. Keep commits small and
single-purpose.

Each increment should land a coherent abstraction or deepen one that exists.
Do not spread a new capability across callers as special-case coordination.

Subtraction comes before scaffolding. Remove dead code first, then lay
foundations.

## Redesign from First Principles

Apply when integrating a new requirement into an existing design. Do not bolt
the change onto the existing design. Redesign as if the requirement had been
a foundational assumption from the start.

- Read all affected files and understand the current design.
- Ask "if we were writing this from scratch with this new requirement, what
  would we build?"
- Propagate the change through every reference, including types, docs,
  examples, and rationale sections.
- Think about the whole redesign, then deliver it incrementally.

This is the method for preserving option value when integrating changes into
an existing design.

## Attack the Premise

Apply when two or more fixes that share one premise have failed the same
gate. `~/maestro/WORKFLOW.md#recovery-and-verification` owns repeated failure
and the record of the shared assumption. This section adds the direct test.
Write the premise down as the one sentence every failed fix assumed. Then find
an observation that would come out differently if the premise were false,
looking at how the failures are distributed and not only at their size, and
make it before you write another fix.

## Subtract Before You Add

Apply when sequencing an addition, refactor, or rewrite. Remove dead code,
redundant validators, and stub references first, then build on the simpler
base.

When evolving a system, remove complexity first, then build. Default to
subtraction.

Make simplification a continual investment. Leave the design slightly simpler
and more capable than you found it, behind the same or a smaller interface.

- Sequence removal before construction.
- Cut before you polish. Get to the minimum before investing in quality.
- Design for observed usage, not speculative edge cases.
- No speculative validators, parsers, or guards beyond what the spec demands.
- Simplify prompts. Remove redundant instructions and excessive templates.
- When a reference has no novel content, delete it rather than leaving a
  stub.

## Minimize Reader Load

Apply when reviewing or shaping code that is hard to trace.

Maintainability is the work a reader must do to understand code. Track two
axes. **Layers to trace** are the indirections between the question and the
answer. **State to hold** is the hidden or mutable context the reader must
keep in their head. LOC, cyclomatic complexity, and "clean architecture" are
proxies. Reader load is the thing that matters. The two axes are independent.
A flat file with 50 globals can be as hard to reason about as a 6-layer
adapter stack. Guard both. This is the human analog of **Guard the Context
Window**. Working memory is finite for readers too.

- **Collapse layers** that cost more than they save, such as wrappers with
  one caller, adapters with no second implementation, and speculative
  indirection that was never needed. Inline them.
- **Make adjacent layers change the abstraction.** A layer that repeats the
  same methods and arguments adds reader load without compression. Collapse
  pass-through layers.
- **Demand interface compression.** A broad interface that hides little
  complexity makes readers learn both the interface and the implementation.
  Prefer boundaries that hide meaningful decisions.
- **Shrink state scope.** Prefer pure functions (returns over mutations),
  locals over fields, fields over module state, and module state over
  globals. Derive instead of sync.
- **Name the invariant at the boundary,** not in every consumer, so the
  reader learns it once.
- Before adding a layer or a piece of state, ask whether it reduces reader
  load elsewhere by at least as much.

**The test.** Can a new reader answer "where does X come from?" and "what can
change X?" in under 30 seconds? If not, cut layers or cut state.

## Outcome-Oriented Execution

Apply during planned rewrites and migrations with explicit phase boundaries.
Optimize for the intended, verifiable end state rather than preserving smooth
intermediate states.

**Why.** Keeping every intermediate step fully stable often creates temporary
compatibility code that becomes long-lived debt. Converge on the target
architecture and prove correctness at explicit verification boundaries.

**Core rule.**
- Prioritize end-state integrity over transitional stability.
- Intermediate breakage is acceptable when it is planned, scoped, and
  reversible.

**Guardrails.**
- Use this for planned rewrites and migrations with explicit phase
  boundaries.
- Declare where temporary breakage is acceptable.
- Keep high-signal checks for actively touched areas while migrating.
- Require full static and runtime verification at plan completion.

## Experience First

Apply when product, UX, or feature-scope tradeoffs come up. When
implementation convenience conflicts with user delight, choose delight.

- Every feature, control, and option must be justified.
- Ship less, ship better. A polished experience with three features beats a
  rough one with ten.
- Prototype before committing. Design decisions are cheaper in throwaway HTML
  than in production code.
- Get the details right, including transitions, alignment, spacing, feedback,
  and error states.
- Tighten the core loop. Every feature should serve the central workflow or
  get out of the way.

The user is whoever consumes the work. For a UI that is the end user. For a
library or an internal API it is the colleague who imports it. The engineer
who maintains the code next is a user too. Weigh their experience the same
way, and explain impact from their perspective.

Foundations should serve the experience. **Foundational Thinking** governs
the sequence of work. This principle governs the target.

## Exhaust the Design Space

Apply when facing a novel UI interaction or architectural decision with no
precedent in the codebase. Build two or three competing prototypes and compare
them side by side before committing. It also applies to product design
decisions where user experience depends on feel, not logic.

## Build the Lever

Apply to any non-trivial work, not just bulk work, including edits,
migrations, analyses, and checks. Build the tool that does the work or proves
it instead of working by hand. That tool is a codemod, a script, a generator,
or the brief your subagents follow. The tool is the artifact a reviewer can
rerun.

A codemod, generator, or script does the work the same way every time and
reruns for free. The tool is one artifact a reviewer can read and rerun to
check the work. Hand-done changes can only be re-verified by redoing them. A
deterministic script turns "trust me" into "run this".

Default to building the lever. Skip it only when the task is trivial, a couple
of obvious edits you can see at a glance.

- Do the first unit by hand to learn the recipe, then build the tool. Prove it
  by rerunning it on that unit and diffing against your hand-done version.
  Make the lever safe to rerun.
- Use a codemod or script for edits, a generator for repetitive files, a
  dump-to-sqlite query for analysis, and a rerunnable check for verification.
- A deterministic lever beats fan-out. If the tool can process every unit in
  one pass, run it yourself. Do not fan out delegates to hand-apply what a
  script can do.
- When you fan work out to subagents, write the lever as the one brief they
  all read, holding the recipe, the verification contract, and the
  do-not-touch fences. In maestro that is the dispatch envelope and its
  standing orders. Keep it outside the
  delegates' write scope so they cannot quietly edit the contract.
- Applying this principle produces a file. If you cited it and there is no
  codemod, script, generator, or delegate brief in the work, you did not apply
  it.
- When the work outlives the session, the lever lands with the change.

The bar is triviality, not repetition. A one-off still earns a lever when the
lever is what makes the work checkable. Per the **Laziness Protocol**, build
the smallest script that does or proves the job, never a framework.

Distinct from **Encode Lessons in Structure**, which makes a recurring
instruction a durable guardrail. This is throughput and reviewability on the
work in front of you. For scripting the verification itself, see **Prove It
Works**.

## Model the Domain

Apply when writing stateful logic, or when code branches a lot or repeats a
shape assumption across files.

Encode the real domain in a data structure instead of scattering it across
conditionals. A structure that matches the domain makes invalid states
unrepresentable and deletes branches. Choosing it at write time is cheap.
Recovering it later reads as a refactor and gets deferred.

Reach for:

- A state machine instead of scattered booleans, phases, or lifecycle checks.
- A typed model instead of loose parameters or repeated shape assumptions.
- A map, registry, lookup table, or discriminated union instead of branching
  spread across files.
- A reducer or command/event model instead of ad hoc state mutations.
- A module organized around one body of domain knowledge instead of a
  sequence such as load, validate, transform, and save. Execution order is not
  ownership.
- A small module boundary that gathers repeated behavior, ownership, or
  invariants.
- A queue, cache, index, graph, tree, or normalized collection where the
  access pattern calls for it.
- Any other structure that fits. When none fits, work out what the code must
  never allow and how the data gets read, then find the structure that encodes
  exactly that.

Do not force an abstraction. Prefer boring code if the current shape is
already clear, local, and unlikely to grow. Be skeptical of an abstraction
that adds indirection without removing branches, duplicated rules, invalid
states, or lifecycle risk.

The sign that you skipped this is a new feature that grows an existing if/else
chain by one more branch, or a second boolean that must stay in sync with the
first. Temporal decomposition is another sign. Phase-named modules repeat the
same domain rules across steps.

## Boundary Discipline

Apply when wiring validation, error handling, or framework adapters.
Concentrate guards at system boundaries (CLI, config, network, external APIs).
Trust internal types and keep business logic in pure functions.

Place validation, type narrowing, and error handling at system boundaries.
Trust internal code unconditionally. Business logic lives in pure functions.
The shell is thin and mechanical.

**Why.** Scattered validation is noisy, redundant, and gives a false sense of
safety. Keep logic out of framework wiring so it can be tested without the
framework.

**The pattern.**
- **At boundaries** (CLI args, config files, external APIs, network
  protocols), validate, return errors, and handle defensively.
- **Inside the system**, use typed data and error propagation with no
  re-validation. Trust the types.
- **Across the boundary**, expose domain concepts, not the boundary's private
  representation. Keep general-purpose mechanism inside and special-purpose
  policy at the edge.

Validation and error handling:

- Validate config at parse time (the boundary), not inside business logic.
- Parse raw data into domain types at the boundary.
- Do not re-export transport, storage, framework, or wire types through the
  public interface.
- No redundant nil checks deep in call chains if the boundary already
  validated.

Code organization:

- Business logic lives in pure functions with no framework dependencies.
- Parse functions are pure transforms from raw bytes to typed state.
- Prompt construction takes structured state in and returns a string.
- Scoring and assessment are pure transforms from state to results.

**The tests.**
- "Is this data crossing a system boundary right now?" If not, validation is
  redundant.
- "Can this be a pure function that the shell just calls?" If yes, extract it.

## Type System Discipline

Apply when designing types, reviewing a function signature, or writing code
in any statically-typed language. Make illegal states unrepresentable, brand
semantic primitives, parse external data at boundaries, refuse to lie to the
compiler, exhaust variants, and derive from authoritative schemas.

Make the compiler reject impossible states, mismatched primitives, and
unhandled variants. Prefer defining errors and special cases out of existence
over proliferating handlers. For a language's own syntax and idioms, read
`maestro recipe show style-<lang>` (for TypeScript,
`maestro recipe show style-typescript`).

- **Make illegal states unrepresentable.** Model variants as sum types
  (discriminated unions, enums with payloads, sealed classes), not as a bag of
  optional fields where contradictory combinations compile.
  `{ completed: boolean; completedAt?: Date }` admits `completed: true` with
  no date. Derive the boolean from `completedAt`, or model
  `{ kind: 'open' } | { kind: 'done'; at: Date }`. If you can write a comment
  explaining when a combination of fields is valid, split it into a sum type.
- **Types are constructions, not restrictions.** Build the type up from the
  values you want instead of carving them out of a looser type with checks. A
  non-empty list is a head plus a rest. A valid time range is a start plus a
  duration. Choose the shape that cannot build the illegal value and expose
  the interface callers need on top.
- **Brand semantic primitives.** `UserId` and `OrderId` are strings
  underneath but should not be interchangeable. Give each its own type, such
  as a branded intersection in TypeScript or a newtype elsewhere. Validate
  once at creation, trust the type downstream. If two arguments share a
  primitive type but mean different things, brand them.
- **External data is untyped until parsed.** RPC payloads, JSON, IPC
  messages, CLI args, config files, environment variables, and database rows
  all count. Have a parse function at every boundary that turns unstructured
  input into the typed model. **Boundary Discipline** says where to put
  validation.
- **Don't lie to the type system.** Casts, unsafe coercions, and assertion
  functions that bypass the compiler are latent runtime crashes. If the
  compiler can't prove a fact, prove it (validate, narrow, refine the model)
  or accept that the cast is a hazard. Trace each `any`, `as`, or
  `assertNotNull` to the boundary and validate there.
- **Exhaustive matching is the compiler's job.** A match on a sum type must
  fail compilation when a new variant is added without handling, so the
  compiler tells the next agent where to add a case. Use your language's
  idiom, such as a `never`-typed binding in TypeScript, an unannotated `match`
  in Rust, `-Wincomplete-patterns` in Haskell, or sealed-class match
  exhaustiveness in Kotlin.
- **Derive types from authoritative schemas.** When a protocol buffer,
  OpenAPI spec, GraphQL schema, database migration, design-token file, or
  another file owns a shape, derive from it instead of hand-rolling a parallel
  type. See **Encode Lessons in Structure**.
- **Strengthen a type only where partiality appears.** A runtime assertion,
  null check, or "this should never happen" throw marks the place a type is
  too weak. Push that check up into the type. Then stop. The type system's job
  is to track the cases each use site must handle, not to describe the data as
  precisely as possible. Prefer total functions. `sum` of an empty list is 0,
  so it takes the plain list. `head` of an empty list has no answer, so it
  demands the non-empty one. If nothing would otherwise panic, keep the plain
  type.

## Make Operations Idempotent

Apply when designing commands, lifecycle steps, or processing loops that run
amid crashes, restarts, and retries. Converge to the same end state regardless
of partial prior runs.

Design operations so they converge to the correct state regardless of how
many times they run or where the previous run stopped. Every state-mutating
operation should answer two questions. What happens if this runs twice? What
happens if the previous run crashed halfway?

**Why.** Commands, lifecycle operations, and processing loops run where
crashes, restarts, and retries are normal. If partial state changes the next
run's outcome, every restart becomes a debugging session.

**The idea.** Derive each step from the state that exists, not from what the
previous run meant to do. Read the current state, compare it to the goal, and
apply only the difference. A step written this way is safe to repeat, because
a repeat finds nothing left to do.

Anything a run leaves behind, such as a lock, a temporary file, a
half-written record, or a claimed job, must be recognizable as stale by the
next run. Judge staleness by evidence that stays true wherever the previous
run could have stopped, not only in the common case. A marker that the next
run cannot judge turns every crash into manual cleanup. For example, write to
a temporary path and rename it into place, or compare artifacts by content
instead of by creation order.

**The test.**
1. What happens if this runs twice in a row?
2. What happens if the previous run crashed at every possible point?
3. Does re-execution converge to the same end state?

If any answer is "it depends on what state was left behind", the operation
needs a reconciliation step.

## Migrate Callers Then Delete Legacy APIs

Apply when introducing a new internal API while old callers still exist. When
the new API is the right design, migrate callers and remove the old API in the
same refactor wave instead of preserving compatibility layers.

**Rule.**
- Do not keep legacy API paths only because internal callers still exist.
- Inventory callers, migrate them, and delete the old API immediately.
- Treat temporary adapters as exceptional and time-boxed, not default
  architecture.
- Update tests to assert the new contract, and delete tests that only protect
  pre-refactor implementation details.

**When this applies.**
- No external users depend on backward compatibility.
- The project can absorb coordinated breaking changes.
- The new API is part of a simplification or refactor initiative.

Keeping both old and new APIs creates dual-path complexity, slows cleanup, and
makes the codebase feel append-only.

## Separate Before Serializing Shared State

Apply when concurrent actors might write to the same file, branch, key, or
state object. `~/maestro/WORKFLOW.md#concurrency-and-git` owns the default,
which is to eliminate the shared write target. Serialize access structurally
(lockfiles, sequential phases, a single-writer actor, atomic compare-and-swap)
only when one shared writer is a real invariant. Treat "we need a lock" as a
design smell to check, and remember that instructions and conventions are not
concurrency control.

## Prove It Works

Apply after a task, before declaring done. Verify against the real artifact
(run the feature, read the actual value, inspect the diff), not a proxy, a
self-report, or "it compiles". The rules live in
`~/maestro/WORKFLOW.md#recovery-and-verification` and
`~/maestro/WORKFLOW.md#completion-and-delivery`, and the repository's
`verify-<repo>` skill (`maestro recipe show verification-skill`) drives the
real product. The strongest proof is a deterministic script that reruns the
same comparison, kept as an artifact a reviewer can rerun instead of trusting
your word.

## Fix Root Causes

Apply when debugging. Trace each symptom back to the first place the system
went wrong and fix it there. A guard is a fix only when the state it handles
is legitimate.

Fix the fault, not the place where it showed up. The symptom is where a wrong
state became visible. The root cause is the first place the system did
something it should not do, or failed to handle a state it should handle.

**Why.** A symptom fix hides the evidence and leaves the fault free to appear
somewhere else. Each one makes the system harder to reason about, because the
code now carries a workaround for a bug that nobody named.

**Pattern.**
- Reproduce first.
- Ask "why" at each step until the answer is a wrong decision in the code,
  not a consequence of an earlier one.
- When stuck, instrument. Read the actual error and the actual state. Do not
  guess.
- Ask whether the state that triggered the symptom is legitimate. If the
  system may be in that state, the code that broke on it is the root cause,
  and handling the state there is the fix. If the state should never exist, a
  check where it broke hides the bug. Find what produced the state. Guards,
  retries, swallowed errors, clamped values, and discarded state are the usual
  ways to hide a bug.
- When something fails only after a restart, suspect persisted state first,
  such as config files, caches, lock files, and serialized state.
- Check for the pattern, not just the instance. Find every place with the
  same fault and fix them all.
- If a workaround needs a paragraph-long comment to justify it, the code is
  wrong. Fix the code, not the comment.
- If the deciding fact took a deep dig to find, propose the cheap, always-on
  signal that would have shown it at first look.

## Sequence Work into Verifiable Units

Apply to multi-step work (sweeps, migrations, runs of similar edits) and to
how you stack commits and PRs. Order work as a sequence of small units, each
ending in a state you can check, and don't advance until the current one is
green.

**Why.** A break caught at the unit that caused it is cheap to localize. A
break caught after a batch is buried, and you have already built further on a
broken base. Sequencing those same units into a delivery a reviewer can
replay turns "trust me" into "watch it go red, then green."

**Execution.** In a sweep, migration, or any run of similar edits, verify each
change before starting the next. Each unit is a before/after bracket, with a
known-good state, one change, the check, then the next unit. Rebase onto clean
trunk first so every check measures against the real baseline. When a lever
does the edits, the per-unit check is nearly free. Run it anyway.

**Delivery.** Stack commits and PRs in the order that proves the work to a
reviewer. The canonical shape is the failing test first, then the fix on top.
Other story orders are a subtraction before the reshape, a baseline capture
before the treatment, and the scaffold before the feature. Each commit lands
on its own and the sequence reads as an argument. When a commit, push, or PR
may happen is set by `~/maestro/WORKFLOW.md#concurrency-and-git`.

For Full-tier work the units live in the bundle SPEC's `## Units` contract
(`maestro recipe show multi-phase-plan`).

It complements **Prove It Works**, which keeps each check real, and **Build
the Lever**, which makes the per-unit check cheap.

## Test Behavior Not Implementation

Apply when you write, change, or keep a test. Assert what the code's users can
observe. `~/maestro/WORKFLOW.md#testing-discipline` owns the rule, including
the undefined-import falsifier. This section adds two imagined edits to judge
each test against.

- **Break the behavior.** Make the code return a wrong value, skip an effect,
  or do the effect twice. A test that still passes proves nothing.
- **Change the internals.** Keep every observable result the same and
  restructure the code underneath. A test that now fails is pinned to the
  implementation.

Keep a test that catches the first edit and survives the second. Rewrite or
delete the rest.

## Explain the Number

Apply before you trust, report, or act on a number you measured, such as a
speedup, a regression, a throughput, a latency, or an eval result. Find what
limits it, and rule out that it measured something other than the work you
think.

- **Ask "why not double?"** Name the resource or code path that bounds the
  result, such as a core, a lock, the disk, the network, or the load generator
  itself. Get it from a profile or from system counters taken during a run,
  then map it to source. A guess from reading the code is not a limiter. If
  you cannot say why the number is not twice as good, you do not know what you
  measured.
- **List what else the number could be measuring, and rule out each one with
  evidence.** The usual suspects are errors, skipped or cached work, code that
  never ran, an untuned side (one left on default settings), noise, and a
  piece too small to matter end to end.
- **Keep the evidence with the number.** Put the run count, the spread, and
  the limiter in a work note (`maestro work note <id> "..."`) or a linked
  artifact.

For a performance number, run each side at least five times alternating
under production settings, count errors, confirm the work ran, and name the
limiter. For an eval result, ask the same of
the trials. Did every run do the task, does the gap hold across trials and
models, and does the scenario matter?

You skipped this when the evidence behind a number has no run count, no
spread, or no named limiter, or when the time saved is larger than the time
the changed piece took.

Distinct from **Prove It Works**, which checks that an output is real.

## Guard the Context Window

Apply when context is filling up with large outputs, long files, repeated
reads, or fan-out planning.

**Why.** Context overflow degrades reasoning quality, creates compression
artifacts, and halts progress.

**Pattern.**
- **Route bulk to subagents.** Keep their summaries in the main thread, not
  raw payloads.
- **Size phases and cap scope.** Limit files per phase, set turn budgets, and
  account for mechanism costs.

## Never Block on the Human

Apply when tempted to ask "should I do X?" on reversible work. Do X, explain
why, and let the human course-correct. `~/maestro/WORKFLOW.md#autonomy` owns
the rule. `~/maestro/WORKFLOW.md#decisions-and-readiness` owns what the agent
decides and when it asks. `~/maestro/WORKFLOW.md#authorization-boundaries`
owns which actions still pause for the owner.

## Encode Lessons in Structure

Apply when you catch yourself writing the same instruction a second time, or
notice a recurring correction. Encode the rule as a lint, metadata flag,
runtime check, or script instead of more text. Answer a doctrine lesson at the
highest rung that works: architecture, then types, then a lint or check, then
a test, and doctrine text last. If the lesson needs judgment and
cannot be encoded, make the instruction more prominent and add an example of
the failure mode. If the fix is structural, use only the structural fix. The
instruction is the symptom.
