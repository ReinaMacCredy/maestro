# Maestro proof engine accelerator

This package is a side-by-side orchestration engine for expensive vNext proof
chains. It does not replace a stage's independent validators or change what a
proof means. It removes avoidable reruns around those validators.

## Safety contract

- `run` is the default cache mode. A checkpoint can resume only the same exact
  seal token. A new seal reexecutes the phase.
- `content` is opt-in for deterministic immutable work, such as a sealed
  predecessor reconstruction. It must not be used where the contract requires
  fresh execution for every certification.
- Phase identity binds the exact phase definition, declared inputs, dependency
  outputs, executable bytes and probe, environment, target/profile/mutant
  literals, and the run token when applicable.
- A cache hit rehashes the complete checkpoint payload. Corrupt, substituted,
  or same-identity/different-result checkpoints fail closed.
- Commands receive isolated phase output and temporary roots. Bound inputs,
  tools, completed dependency outputs, and completed phase outputs are rehashed
  to reject mutation. File/tree inputs and executable bytes are first copied
  into one immutable run-owned binding tree, and commands receive only those
  pinned paths. Live input or tool substitution cannot change executed bytes.
- Ordinary trees reject every symlink. A declared `symlink_tree` admits only
  exact, content-bound links that resolve inside the same root, preserves those
  links in the frozen run binding, and rejects escaping, broken, or cyclic links.
- The plan/run-token pair has one exclusive cache-root lock. A concurrent retry
  waits for the active execution and then observes its completion marker rather
  than executing the same seal twice.
- Independent phases may run concurrently only because their declared write
  roots are disjoint. Command templates expose only the phase root, declared
  inputs, and declared dependency outputs; there is no run-root placeholder.
  This is capability narrowing for trusted, immutable proof commands, not an OS
  sandbox against a hostile executable. Each phase also receives an isolated
  non-canonical temporary root, which is removed before the output manifest and
  checkpoint are sealed. Dependency edges remain sequential.
- `duration_ms` and cache status are written only to the performance JSONL log.
  They never enter plan, phase, artifact, checkpoint, or publication identity.
- Publication first seals one immutable content-addressed release and then
  atomically replaces one pointer file. A failed proof phase cannot move the
  pointer.
- Each run binds the pointer preimage. A post-pointer crash resumes without
  republishing, while an older run fails closed if a newer seal advanced the
  pointer.
- One cache-root completion marker is authoritative for a seal token. An
  interrupted run has no marker and may resume its exact checkpoints; a
  completed token cannot be replayed from a different run directory.
- Each checkpoint also has a separate immutable cache-root binding to the full
  checkpoint tree. Rewriting and internally resealing a checkpoint cannot
  substitute it for the externally bound tree.
- A stage may place its exact proof-engine and snapshot adversarial tests in a
  run-scoped preflight phase and bind that deterministic receipt into consensus
  before expensive validators execute.
- Tool identity binds exact executable bytes and an exact probe under the proof
  environment. Stage adapters must additionally bind any runtime closure that
  they relocate or claim to make hermetic. Stage 5 materializes and binds its
  Rust compiler driver and target libraries; its host Python and Ruby standard
  libraries remain an explicit host-runtime trust boundary and are not claimed
  to be copied or OS-sandboxed by this engine.

`InputBinding.path_identity="resolved"` is the conservative default. The
`content` path identity mode is allowed only after the caller proves that the
command cannot observe or emit the physical input location.

## Stage 4 adoption boundary

Keep the current Stage 4 builder, Python validator, and Ruby verifier as the
parity oracle until Stage 4 has reached its committed boundary. The first
adapter should use:

1. one run-scoped builder phase;
2. one run-scoped Python reexecution phase;
3. one run-scoped Ruby reexecution phase;
4. separate Cargo target and temporary roots for Python and Ruby only after
   binary-receipt parity has been proven;
5. content caching only for immutable predecessor chains whose complete sealed
   input closure is in the key.

A stage that claims fresh predecessor reexecution cannot use that exception;
its predecessor phase must remain run-scoped and execute once for every new seal.

The existing engine can be removed only after A/B output parity, all compiled
mutant rejections, cache invalidation, interrupted-run resume, corrupt-cache
rejection, concurrent-run isolation, and failure-publication tests pass with
the same canonical receipts.

## Focused verification

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest \
  tools.vnext_contracts.proof_engine.test_engine -v
PYTHONDONTWRITEBYTECODE=1 mypy --strict --explicit-package-bases \
  tools/vnext_contracts/proof_engine
PYTHONDONTWRITEBYTECODE=1 pyright tools/vnext_contracts/proof_engine
```
