# Three layers — kernel (mechanism), policy (plugins), recipes (plugins) — on a self-written Cordis-inspired plugin runtime

Mentor critique of the Rust maestro (Demonthorn, 2/8/26): the architecture is
fine but opinionated rules were fused into the core; lean mode is an escape
hatch that proves the missing extension point ("cung cấp một default policy
mạnh nhưng không coi nó là kernel"). We adopt "mechanism, not policy" as the
structural rule: the kernel carries only mechanism (store, event log, readiness
projection, CLI dispatch, plugin loader) and holds no opinion about proof, QA,
TDD, research gates, or workflow. Every opinionated capability — default policy,
design workflow, playbook, QA, loop recipes, watch, teams — is a plugin that can
be installed, disabled, or removed per repo. The plugin runtime is written
in-house, borrowing the Cordis paradigm (context as service registry, `inject`
declared dependencies, typed events with emit/waterfall/parallel/serial dispatch,
registrations as reversible effects with disposers, config-driven loader with
per-entry `disabled`); gates are waterfall listeners that may short-circuit.

## Considered Options

- Depend on the `cordis` npm package (as deepseek-harness does): rejected by
  user — keep full control of the kernel, no framework lock-in; the paradigm is
  adopted, the dependency is not.
- Keep the fused architecture and add more mode flags (lean, `--lane light`,
  `--qa none`): rejected — that is the escape-hatch accretion this rewrite
  exists to end. Disabling a plugin replaces every mode flag.
- Generic item store instead of card/task/feature/decision entities: rejected —
  the mentor called the domain model sound; re-modeling it is speculative
  abstraction. Entities stay first-class in the kernel; only the rules about
  them move out.

## Consequences

- The kernel source must never reference policy concepts (proof, QA, TDD,
  research); that boundary is enforced by a VERIFY grep check, not convention.
- There is no lean mode in the new system. Its use cases are served by
  disabling or swapping policy plugins.
