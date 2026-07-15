---
candidate_only: true
runtime_activation: false
runtime_registration: false
logical_resource: skills/maestro/SKILL.md
---
# Maestro

Use exactly one read path per acquisition:

1. Before a Store, request `BootstrapNoRecipeV1` and accept only its exact
   bootstrap fact view. Store absence alone never implies Setup.
2. With an active Store, request the complete 30-option
   `DiscoverSelectionContextV1`, copy one exact option into `ProjectV1`, and
   accept only a Packet whose nonzero `PacketRecipeBindingV1` reproduces that
   request, one sealed Frontier, all selected component provenance, and the
   composed Advice.
3. Derive one fresh Release-bound `JobRouteV1`. Load its one job and no method
   or Recipe. Resolve the optional typed Capability closure and post-route
   Job/Recipe admission separately. Never invent a fallback or private map.
4. Invoke only the Packet-advertised Action or Ceremony through the typed CLI
   Operation envelope. `maestro_cli_search` discovers a known command shape;
   it does not choose an Operation.

After a complete Selected route and same-Release load, the caller may form one
ephemeral `SkillActivationCandidateV1`. Routing and loading publish nothing.
Only a separately advertised and freshly authorized `PublishObservation`
Action may publish it; Ambiguous, Blocked, Refused, stale, mixed-Release,
over-budget, or partial closures form no candidate. Activation never grants
authority, currentness, liveness, resume state, lifecycle truth, or next action.

Discard the route after every Result, bounded read, mutation, expiry, or basis
change. This Resource owns no route table, Recipe body, lifecycle, authority,
Recommendation, retry, cursor, persistence, or hidden state.
