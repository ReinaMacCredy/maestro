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
  to reject mutation.
- Independent phases may run concurrently only because their write roots are
  disjoint. Dependency edges remain sequential.
- `duration_ms` and cache status are written only to the performance JSONL log.
  They never enter plan, phase, artifact, checkpoint, or publication identity.
- Publication first seals one immutable content-addressed release and then
  atomically replaces one pointer file. A failed proof phase cannot move the
  pointer.

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
