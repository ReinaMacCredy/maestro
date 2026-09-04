---
name: maestro-explore
description: Answer an evidence question without touching production code - research external facts against primary sources, build or iterate a disposable prototype (its own bugfixes included), or baseline current repository behavior before deciding or implementing. Use when the question is answerable by reading, building, or measuring with no user decision required; unsettled decisions belong to maestro-design. Prototypes are throwaway.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-explore

Answer an open question with evidence, without touching production code.
Read-only toward production paths; exploration never authorizes
implementation. Any tier may use it.

Three modes; pick whichever settles the question:

- **research** - documented or external facts: read source, docs, or the web;
  record findings with links so claims stay traceable. Procedure:
  [references/research.md](references/research.md).
- **prototype** - an environment-specific behavior question that reading
  cannot settle: build the smallest throwaway that answers it, outside
  production paths. Prototypes are disposable; only a decision-encoding
  fragment (a schema, a reducer, a type shape) may be inlined into a locked
  decision or the bundle's SPEC.md. Fixing the prototype's own bugs is still
  this mode - a fix routes to `maestro-work` only when the target is
  production code; iterate directly, no re-routing pass per request. When the
  user approves porting, the port is production work: route it by tier
  (`maestro-bundle` tier rule); the prototype never merges as-is. "Ship it
  as-is" waives the rewrite, not the route. A prototype that is deployed or
  relied on in real work is no longer throwaway: say so, and route its next
  change by tier. Procedure: [references/prototype.md](references/prototype.md).
- **baseline** - current repository behavior that must be preserved or
  changed: capture it as runnable commands with observed output, so
  `maestro-verify` can compare later. Phrase each behavior the work must
  preserve as a guard, "the system SHALL CONTINUE TO <existing behavior>", so
  preserved behavior is a checkable claim, not an assumption.

## Where findings land

- A fact that settles a fork becomes a decision:
  `maestro decision draft "<choice>" --rationale "<why, with source link>" --work <id>`,
  then `maestro decision lock <id>`.
- Working evidence for an open work item: `maestro work note <id> "<finding + link>"`;
  in a bundle, also the NOTES.md Current State.
- A baseline the work must preserve: a VERIFY.md scenario in the bundle, or
  the work item's acceptance when there is no bundle.
- No work item and no bundle: deliver the findings in the conversation. Open
  a work item only if the question feeds work that will actually happen; a
  finding alone never opens a bundle.

`maestro search "<topic>"` before any research: a past decision or note may
already hold the answer.
