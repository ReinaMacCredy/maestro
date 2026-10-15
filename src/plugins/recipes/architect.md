# Architect

Use this recipe for non-trivial work where jumping to code would lock in the
wrong shape. Sketch types, function signatures, class shapes and module
boundaries with `not implemented` bodies and pseudocode, synthesize across
several candidate designs, then fill in code against the chosen sketch. It
shapes code inside an authorized slice; a fork the owner must settle belongs
to maestro-design. Ported from pstack's architect skill (Lauren Tan, MIT;
notice in THIRD_PARTY.md).

## Steps

1. Ground. Build a real mental model of every system the new code touches.
   Run `maestro recipe show how` over the relevant subsystems and produce the
   explanation it prescribes. Naming a file is not grounding. If the design
   redefines ownership or layering, also run `maestro recipe show why` on the
   existing shape, so its rationale becomes a constraint and not a guess.
   Skip this step only when the work is greenfield with no surrounding system
   to integrate.
2. Sketch. Run `maestro recipe show arena` with the design-sketch task and
   the step 1 grounding, giving each `arena-runner` the runner brief below.
   - Design it twice. Require at least two structurally distinct candidates
     before synthesis, even when the first looks sufficient. This is
     **Exhaust the Design Space** (PRINCIPLES.md) made concrete. Whole-shape
     alternatives count; point fixes inside one shape do not.
   - Screen every candidate against the red flags below. Assume the next
     contributor is an agent that sees only the files it opened, copies the
     nearest example, and takes the shortest path that compiles. Prefer the
     design where a change that looks right from one file is right for the
     whole repo.
   - Compare viable candidates on interface depth. Prefer the design that
     hides more complexity behind a smaller, simpler public interface. A rich
     interface can keep call chains short by concentrating capability instead
     of scattering it across layers.
   - Arena returns one synthesized design package. Its synthesis note fills
     the rationale's "Synthesis decision" section.
3. Agree (opt-in). By default, go straight to step 4 with the synthesized
   design. There is no owner checkpoint unless the invoker asks for one
   ("with checkpoint", "stop and show me before implementing"); then show the
   synthesized design and pause for sign-off.
   - The synthesis may land as its own scaffold commit under the commit grant
     (`maestro recipe show opening-a-pr`), the "scaffold first" mode of
     **Foundational Thinking** (PRINCIPLES.md). Planned, scoped breakage
     during fill-in is fine per **Outcome-Oriented Execution** (PRINCIPLES.md).
   - For adversarial pressure before implementing, run the review gate on the
     sketch (`~/maestro/WORKFLOW.md#per-tool-adapters`).
   - Owner pushback on the shape, at a checkpoint or later, is step 1
     evidence. Re-ground and re-run step 2 before writing more code.
   - A durable, hard-to-reverse shape choice becomes a decision per
     `~/maestro/WORKFLOW.md#decisions-and-readiness`, with the rationale's
     tradeoffs and alternatives as its rationale and rejected options.
     Otherwise the rationale stays in a work note.
4. Implement against the sketch. A `delegate` subagent replaces
   `not implemented` bodies with code and pseudocode with logic; you review
   its diff. The sketch is the contract. A deviation from it is a signal to
   report, not friction to absorb silently. When a function needs a
   parameter the sketch did not anticipate, ask whether the sketch was wrong,
   the requirement was missed, or the implementation is overreaching.
5. Scrap when the architecture is wrong. If implementation keeps producing
   friction the sketch cannot absorb, throw the sketch out. Do not bolt fixes
   onto a wrong design, per **Redesign from First Principles** and
   **Fix Root Causes** (PRINCIPLES.md). The signal is a pattern, not single
   instances. The tells are these.
   - The same shape of workaround appears repeatedly across unrelated code.
   - Several unrelated edge cases each need a special-case branch.
   - Types need escape hatches to compile (`any`, casts, optional fields that
     are always set in practice).
   - The "we need a lock" reflex appears where the sketch said the state was
     not shared.
   - Callers must know the abstraction's internal rules to use it.
   - Two or more independent step 4 deviations share one shape.

   Use judgment. A few edge cases do not condemn an architecture, and
   complexity in the data is not complexity in the design. When you scrap,
   run `maestro recipe show how` over what has been built, redesign as if the
   new constraints were day-one assumptions, subtract before adding per
   **Subtract Before You Add** (PRINCIPLES.md) so the new sketch is smaller
   than the old one before it grows, and return to step 2.

## Runner brief

```text
You are producing one candidate design in architect's parallel exploration.
Run `maestro recipe show architect` and read it in full first. That is the
workflow you are inside. You write only inside your own output path.

Task: {TASK}
Grounding: {GROUNDING_PATHS}
Output path: {OUTPUT_PATH}

Produce a candidate design package. It holds a type sketch, function
signatures, a module map, and a prose rationale in the rationale shape of
that recipe. The orchestrator compares candidates on these axes to pick a
base.

- Caller's usage first. Write the README-style usage and two or three real
  call sites before the types, then derive the type sketch from them. The
  usage is the spec. Reconcile the sketch to the usage, never the reverse.
- Data structures first. Get the core types right and the code becomes
  obvious. Trace each dominant access pattern through the structure. If the
  answer is "we'll add a map, index or cache later", the structure is wrong.
- Interface depth. Weigh the capability hidden behind the public interface
  against the size of that interface. Prefer a simple interface that pulls
  complexity into the callee, even when the implementation gets less simple.
  Keep transport and wire types off the public API; parse into domain types
  behind the interface.
- Shared state. If two actors might both write, ask what happens. If the
  answer is not "nothing", default to per-actor state merged at the read
  boundary (Separate Before Serializing Shared State).
- Visible boundaries. Use `not implemented` errors for bodies, TODO
  pseudocode for tricky logic, and doc comments stating intent and
  invariants. A reader traces data from input to output by types and
  signatures alone.
- Invariants in types. Hard-to-misuse types beat runtime checks, which beat
  prose comments (Encode Lessons in Structure).
- Validate at boundaries and trust types inside (Boundary Discipline).
  Business logic is pure functions; the shell stays thin.
- One source of truth per invariant. Derive instead of syncing.
- Idempotent state transitions where they apply (Make Operations
  Idempotent). Ask what happens if the operation runs twice or crashes
  halfway.
- Short call chains. If tracing the flow takes more than three files,
  flatten the hierarchy (Laziness Protocol, Minimize Reader Load).

You are one of several parallel runners. Produce the best design you can.
Do not hedge toward the others. Differences between candidates are the
signal used to pick a base and graft. Converging on a safe-looking middle
defeats the exploration.
```

## Red flags

Screen every candidate before synthesis. A red flag is a reason to revise or
reject the shape.

- **Shallow module.** A large interface hiding little complexity. Callers
  coordinate several methods for one operation, public options expose
  internal stages, or learning the interface does not spare the caller from
  learning the implementation. A deep module concentrates capability behind
  one interface; a deep call chain scatters it across layers. Do not confuse
  the two.
- **Information leakage.** Several modules depend on the same internal
  decision, so changing it takes coordinated edits. Public re-exports of
  transport or wire types are leakage. Keep storage schemas, framework
  objects and protocol details private.
- **Temporal decomposition.** Modules organized by execution order (load,
  validate, transform, save) instead of the knowledge they own, repeating one
  representation across several boundaries. Group code around the decisions
  it protects.
- **Pass-through method.** A method forwards the same arguments to another
  method of the same shape. Remove it, or move the responsibility to the
  module that can complete the operation. Keep a forwarding boundary only
  when it adds policy, adaptation or a distinct abstraction.
- **Split ownership.** More than one module writes the same state or keeps
  its own copy. Give each piece of state one owner; others read it or ask
  the owner to change it.
- **Two ways to do one task.** An agent copies whichever way it finds first,
  so every way keeps gaining callers. Keep one way, move callers off the
  others, and delete them in the same change.
- **Importable internals.** A caller can import a module's internals, so an
  agent does and they become interface. Make an import from outside fail the
  build.
- **Hand-synced list.** Two or more places list the same items. Keep one list
  and derive the others, or make the build fail when they disagree.

## Rationale

One page that ships beside the type sketch, in sentence-case headings with no
boilerplate.

- **Problem.** What we are trying to do, and what about the existing system
  makes the shape non-obvious. Name the constraints step 1 found.
- **Usage (caller's view).** Written first. The README or quickstart a
  consumer reads, plus two or three realistic call sites. The type sketch is
  derived from it, and the two agree.
- **Shape.** Data structures first, then how data flows through the
  signatures. The load-bearing decisions, which invariants live in types,
  where validation lives, what the system deliberately does not do. Judge
  interface depth explicitly. Cite the principle behind each decision by
  name without restating it.
- **Synthesis decision.** From arena. Which candidate became the base and
  why, what came from each other candidate, and what was rejected and why.
- **Tradeoffs accepted.** One bullet each, in the form "we accept X in
  exchange for Y". Name anything a later reader might mistake for an
  oversight.
- **Alternatives considered.** Required. At least one concrete alternative
  shape with one line on why it lost, judged on interface depth. One is fine
  when the constraints forced the answer ("this was the only viable shape
  because..."). Not other runners' candidates, and not flavors of the same
  shape.
- **Open questions and risks.** Phrased as questions, so the owner's answer
  is the resolution.
- **Next implementation step.** One sentence. The first thing to build
  against the sketch.

## Reply

The caller's usage first and the type sketch derived from it. For a small
change, one file of new types and signatures; for larger work, a module map
plus type definitions. The rationale beside it, and the decision id when the
shape choice was recorded.
