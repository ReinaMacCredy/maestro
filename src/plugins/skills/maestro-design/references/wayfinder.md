# Wayfinding

A loose idea has arrived: too big for one agent session, and wrapped in fog.
The way from here to the **destination** is not visible yet. Wayfinding finds
that way instead of charging at the destination. It charts the way as a
**map** in the maestro store, then works its **decision tickets** (questions
whose resolution is a decision, not slices of a build to execute) one at a
time until the route is clear.

The destination varies per effort, and naming it is the first act of
charting. It might be a SPEC to hand off, a decision to lock before planning
starts, or a change made in place like a data-structure migration.

## Plan, don't do

Wayfinding is **planning** by default: each ticket resolves a decision, and
the map is done when nothing is left to decide before someone goes and does
the thing. The pull to just do the work is usually the signal you have reached
the edge of the map and it is time to hand off. Absent an explicit override
in the map's notes, produce decisions, not deliverables.

## The map in maestro terms

| Wayfinding | maestro |
|---|---|
| the map | a parent work item, `maestro work add "<destination>" --kind idea --acceptance "<what reaching the end looks like>"` |
| a ticket | a child, `maestro work add "<question>" --parent <mapId> --kind research\|idea\|task` |
| blocking | `--blocked-by <ticketId>` on the child (repeatable) |
| the frontier | `maestro ready`: open, unblocked children |
| claim | `maestro work start <ticketId>`; the lease is the claim |
| resolve | `maestro decision draft ... --work <ticketId>`, `decision lock`, then `maestro work done <ticketId> --claim "<answer>" --proof "<evidence>"` |
| decisions so far | `maestro work show <mapId>` lists the closed children; `maestro bundle show` renders their decisions |
| not yet specified | `maestro work note <mapId> "fog: <suspected question>"` |
| out of scope | `maestro work cancel <ticketId> --reason "beyond the destination: <why>"` |

Refer to tickets by their title in everything the human reads; ids ride
inside, they never stand in for a name.

## Ticket types

Every ticket is either **HITL** (worked with a human who speaks for
themselves) or **AFK** (driven by the agent alone). A HITL ticket only
resolves through that live exchange; the agent never stands in for the
human's side of it.

- **research** (AFK): a fact outside the working directory that a decision
  waits on. Resolved by `maestro-explore` in research mode.
- **prototype** (HITL): raise the fidelity of the discussion with a cheap,
  concrete artifact to react to, via `maestro-explore` prototype mode; link
  the artifact from the ticket's note.
- **grilling** (HITL): conversation. The default case; always with
  [grilling.md](grilling.md) and [domain-modeling.md](domain-modeling.md).
- **task** (HITL or AFK): manual work that must happen before a decision can
  be made (signing up for a service, provisioning access, moving data so its
  shape can be seen). The one type that does rather than decides; it earns
  its place by unblocking a decision. The answer records what was done and
  the facts later tickets depend on.

## Fog of war

The map is deliberately incomplete: do not chart what you cannot yet see. The
test between fog and ticket is whether you can state the question precisely
now, not whether you can answer it now. Ticket when the question is sharp,
even if blocked; fog note when you cannot phrase it that sharply. Resolving a
ticket clears the fog ahead of it; graduate whatever is now specifiable into
fresh tickets and drop the fog note.

Work beyond the destination is out of scope, not fog: cancel it with the
reason and leave it out of the decisions list. It returns only if the
destination is redrawn, and then as a fresh effort.

## Invocation

Never resolve more than one ticket per session, except research tickets.

### Chart the map

1. **Name the destination.** Grill with the domain model in hand until the
   destination is one or two sentences; it becomes the parent's acceptance.
2. **Map the frontier.** Grill again, breadth-first: fan out across the whole
   space, surfacing the open decisions and the first steps takeable now. If
   this surfaces no fog and the whole journey fits one session, you do not
   need a map: stop and ask the user how they would like to proceed.
3. **Create the parent**, then the tickets you can specify now as children;
   wire `--blocked-by` edges once the ids exist. Fog goes into parent notes.
4. **Fire the research agents** for each research ticket, in parallel.
5. Stop. Charting is one session's work; it resolves nothing by hand.

### Work through the map

1. `maestro work show <mapId>` for the low-resolution view, then
   `maestro ready` for the frontier.
2. Choose the ticket: the one the user named, else the first frontier ticket.
   `maestro work start` it before any work.
3. Resolve it, zooming into related or closed tickets on demand.
4. Record the resolution as a locked decision and close the ticket with the
   answer as its claim.
5. Add newly surfaced tickets, graduate fog, cancel anything the answer shows
   sits beyond the destination, and update tickets the decision invalidates.

Other sessions may be working unblocked tickets in parallel; the lease on a
ticket is what keeps two sessions off the same question.
