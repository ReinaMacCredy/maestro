# Ports + default adapters + setup recipes

Maestro defines three ports the three reference diagrams require:

- `ObservabilityPort` (LogQL / PromQL / TraceQL query shape, scoped per worktree)
- `ArchitectureRules` schema (yaml the architecture-lints read, declaring layered domains and forward-only dependencies)
- `PrinciplesSchema` (markdown format with rule, rationale, scan-command, fix-recipe; consumed by `gc slop-cleanup`)

Maestro ships default adapters for the article's reference stack (Vector + VictoriaLogs/Metrics/Traces; the Types -> Config -> Repo -> Service -> Runtime -> UI layering with `Providers` as the single cross-cutting boundary). `maestro setup` wires consumer projects to defaults. Consumers can override adapters per project. Maestro's own repo dogfoods its defaults.

Rejected: ship full stack (too opinionated); document only (defeats the harness premise); ports only with no defaults (every consumer reinvents the wheel).
