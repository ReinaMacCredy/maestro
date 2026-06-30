# Deepening Candidate

Use this branch after `maestro-audit` surfaces architecture deepening
opportunities and the user picks one to explore.

## Vocabulary

Use these architecture words exactly:

- **Module**: anything with an interface and an implementation.
- **Interface**: everything callers must know to use the module correctly.
- **Implementation**: what sits inside the module.
- **Depth**: leverage at the interface.
- **Deep**: small interface with substantial implementation behind it.
- **Shallow**: interface nearly as complex as the implementation.
- **Seam**: where the module's interface lives.
- **Adapter**: a concrete implementation at a seam.
- **Leverage**: capability per unit of interface.
- **Locality**: change, bugs, knowledge, and verification concentrate in one
  place.

Avoid substituting component, service, API, signature, boundary, layer, or
wrapper when one of the terms above is meant.

## Design Loop

1. Start from the chosen candidate, its files/modules, the audit report, feature
   spec language, locked decisions, notes, memory, and relevant source evidence.
2. Classify dependencies:
   - in-process: test directly through the new interface
   - local-substitutable: test with the local stand-in
   - remote but owned: define a port and use production plus in-memory adapters
   - true external: inject the port and test with a mock adapter
3. Grill the candidate through constraints, dependency edges, what sits behind
   the seam, what remains outside, and which tests survive.
4. Apply the deletion test: if deleting the module only moves complexity, it was
   shallow; if deletion spreads complexity across callers, it is earning depth.
5. When alternative interfaces matter, use a design-it-twice pass: ask 3+ fresh
   peers for radically different interfaces, then compare by depth, locality,
   and seam placement.
6. Lock the chosen shape as normal `maestro decision` records, then author
   acceptance criteria and affected areas.

Completion criterion: the selected design names the module, interface, seam,
dependency strategy, adapters if any, tests that move to the interface, tests to
delete, rejected alternatives, and any feature-spec, decision, or memory updates
needed to keep future reviews from re-suggesting rejected work.
