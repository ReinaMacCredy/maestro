# Cross-cutting layers are universally importable

The `cross_cutting` list in `docs/architecture.yaml` declares layers that act as composition roots. In v2, the only entry is `providers`, where ports are wired to their default adapters.

The architecture-lint runner exempts cross-cutting layers from the forward-only `layer-order` rule in **both directions**:

- Any layer may import from a cross-cutting layer (no rank check).
- A cross-cutting layer may import from any layer (it is the wiring point).

Only the internal forward-only chain between non-cross-cutting layers (`types -> config -> repo -> service -> runtime -> ui`) is enforced. The `passive_harness` rule still applies to cross-cutting layers; "universally importable" is a layer-order exemption, not a free pass on forbidden patterns.

Rejected: enforce forward-only on `providers` too (defeats the composition-root role — it must import from runtime/ui to wire them); model `providers` as a normal high-rank layer (forces every non-`providers` consumer to also outrank what `providers` outranks, which collapses the graph).
