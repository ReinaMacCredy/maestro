# Domain-Driven Design

Synthesized from Eric Evans (*Domain-Driven Design*), Vaughn Vernon
(*Implementing Domain-Driven Design*), Alberto Brandolini (Event Storming), and
Martin Fowler (Anemic Domain Model). The concepts are restated for maestro's
design flow; no text is copied from those sources.

DDD lets the model of the business domain drive the code. Its payoff is real but
narrow, so this reference is gate-first on purpose: **most cards do NOT need DDD
-- run the fitness gate below and stop at the first NO.** Reach for DDD only when
a card survives the gate; otherwise design it the ordinary way and move on.

This runs at maestro-design's **"Map the current state"** step -- the gate is
part of mapping, not a separate ceremony. Record the verdict as a decision
(`maestro decision new --lock`) so the next session does not re-litigate it.

## Fitness gate (run first, stop at the first NO)

Most cards stop at G1 or G2. That is the point: the gate's job is to say "not
this one" cheaply.

- **G1 -- genuine behavioral complexity?** Real rules, invariants, and state
  transitions, not just create/read/update/delete over fields? NO -> this is
  CRUD / a transaction script; design it directly, DDD here is the anemic-model
  trap. **Stop.**
- **G2 -- in the core, differentiating part?** The part that makes the product
  worth building, versus supporting or generic work? NO -> keep it simple and
  spend modeling effort on the core instead. **Stop.**
- **G3 -- a real domain language to capture?** Terms that experts (or the
  existing spec/code) already use precisely? NO -> there is nothing to make
  ubiquitous. **Stop.**
- **G4 -- multiple infrastructure implementations to swap?** More than one real
  backing store / transport / external system the same logic must run against?
  Often NO in a single-process CLI, often YES in a multi-service or monorepo
  system. **Drives the infra tier below** (does not switch DDD off).
- **G5 -- distributed across teams or deploy units?** Separate teams or
  separately-deployed services owning pieces of this domain? Often NO for one
  local tool, YES across services. **Drives bounded contexts as real
  boundaries** (does not switch DDD off).

G1 and G2 gate whether you model at all. G4 and G5 do not turn DDD on or off --
they size how much infrastructure ceremony is warranted.

## Strategic core (when G1 + G2 pass)

This is the heart of DDD; Evans publicly regretted leading with the tactical
patterns. Do this work first, during mapping.

- **Ubiquitous language.** Name things in the spec the way the domain names
  them, and reuse exactly those names in code, tests, and decisions -- no
  translation layer between conversation and code. Capture the glossary in the
  spec: `maestro feature spec <id> --section "Ubiquitous language" --append`.
- **Subdomain classification.** Split the area into *core* (your differentiator
  -- model it deeply), *supporting* (needed, not special -- keep it plain), and
  *generic* (buy / borrow / copy -- do not model it). Effort goes to the core.
- **Event-storming-lite.** Walk the flow as **events -> commands -> policies**
  to surface aggregates and boundaries, right at the Map step:
  - the domain **events** (past tense: "card accepted", "proof recorded"),
  - the **commands** that cause them, and the actor behind each,
  - the **policies** ("whenever X, then Y") linking events to new commands.

  Events that always change together point at an **aggregate**; a seam where the
  same word means different things points at a **context boundary**. Record the
  map in the spec or `notes.md`.

## Tactical patterns: two gate-driven tiers

Tactics are named here at the *decision* level -- when the design calls for them.
The actual code-writing hands off to maestro-card (test-first). This is not a
how-to-implement catalog.

- **Type-patterns -- warranted by G1 + G2.** When the domain is complex and core,
  model its invariants in types so illegal states do not compile:
  - value objects as **newtypes** (a `CardId`, not a bare `String`),
  - invariants via **enums + exhaustive match** and **type-state**, so a
    transition the domain forbids is unrepresentable,
  - aggregates modeled as types that own their consistency rules.
- **Infra ceremony -- warranted only when G4 / G5 are YES.** Repository / port
  seams, a domain-event bus, bounded-contexts-as-deploy-units. **Skip these when
  G4 / G5 are NO** -- a single-process local tool has no second backing store to
  swap and no cross-deploy boundary, so the seam is pure overhead. But this is
  **not "always skip"**: a multi-service or monorepo system (G4 / G5 = YES)
  genuinely wants the repository seam and the explicit context boundary, and
  omitting it there is the opposite mistake.

### The DDD-lite / anemic-domain-model trap (a sequencing failure)

Distinct from the infra decision above. The trap is **reaching for tactical
patterns before, or without, the strategic and language work** -- entities and
aggregates with no ubiquitous language, behavior drained into "service"
procedures wrapped around dumb data bags. That recreates the anemic domain
model: all the cost of the patterns, none of the benefit. The rule is **order,
not avoidance** -- do the language + subdomain + event-storming work first, then
let types carry the model. Jumping straight to aggregates is the failure,
whatever the gate said.

## Hand-off to maestro-card

ddd.md decides the model -- the language, the boundaries, which tier of patterns
the gate warrants. Writing the code is maestro-card's job, test-first. The Rust
bridge is shared vocabulary only: "value object" -> newtype, "invariant" ->
enum + exhaustive match / type-state, "aggregate" -> a type that owns its rules.
Carry the ubiquitous language across the hand-off so test names and interfaces
match the domain's words (maestro-card's `tdd.md` already asks for this).

## Worked examples

**PASSES the gate -- multi-service / monorepo scopes (a core domain).**
- G1 yes (scope resolution has real rules), G2 yes (a differentiator), G3 yes
  ("scope", "service", "project" are precise terms), G4 yes (per-service
  stores), G5 yes (services deploy separately). -> full strategic + tactical,
  and infra ceremony is warranted.
- Strategic: glossary fixes "scope" vs "project"; subdomains split (scope
  resolution = core, file IO = generic); event storming surfaces a
  "ScopeResolved" event and a Scope aggregate.
- Tactical: `ScopeId` / `ServiceId` newtypes, resolution states as an enum; and
  because G4 / G5 = YES, a per-service repository seam is warranted.
- Hand to maestro-card to implement test-first, using exactly those names.

**STOPS at the gate -- add a `--json` flag to `maestro list` (mechanical).**
- G1 no: it shapes existing data into JSON, with no new rules or invariants.
- -> Stop at G1. No language work, no subdomains, no aggregates, no repository.
  Design it as the small mechanical change it is; pulling in DDD here would be
  the anemic-model trap.
- Record the verdict: `maestro decision new --lock --decision "DDD gate: NO at
  G1, mechanical output-shaping, transaction script"`.

## Stop

- Do not run tactical patterns without the language + strategy work first (the
  anemic trap).
- Do not treat "skip infra ceremony" as universal -- it is conditional on
  G4 / G5.
- Do not model supporting or generic subdomains as if they were core.
- Do not implement from this reference; hand the code-writing to maestro-card.
