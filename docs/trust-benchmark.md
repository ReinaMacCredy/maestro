# Trust Benchmark

`tests/e2e/trust-benchmark/` is a compiled-binary regression corpus that exercises edge-case mitigations end-to-end. The seed contains 9 of 32 scenarios from the master edge-case list. The corpus grows demand-driven — one PR per new scenario.

## Corpus contents

9 seed scenarios, each covering one edge case from the master list in ROADMAP.md:

| File | Edge case | Mitigation under test |
|------|-----------|----------------------|
| `ec05-out-of-scope.test.ts` | EC 5 (out-of-scope harmless) | Trust Verifier scope check at L2.3 |
| `ec06-generated-drift.test.ts` | EC 6 (generated drift) | Generated-file parity at L2.3 |
| `ec09-sensitive-path.test.ts` | EC 9 (sensitive path) | `forbidden_paths` + `sensitive-paths.yaml` |
| `ec12-security-thin.test.ts` | EC 12 (security thin) | Threat-model required predicate at L4 |
| `ec22-amendment-creep.test.ts` | EC 22 (amendments hide creep) | Amendment-budget rules 3–7 at L2 |
| `ec23-proof-not-tied.test.ts` | EC 23 (proof not tied) | ProofMap at L3.5 |
| `ec27-rebase-squash.test.ts` | EC 27 (rebase/squash) | Tree-SHA verdict identity at L5.3 |
| `ec31-decision-authority.test.ts` | EC 31 (decision authority) | `owners.yaml.deploy_approver` at L7.9 |
| `ec32-self-weakening.test.ts` | EC 32 (PR self-weakening) | Rule 12 base-branch reading at L5.2 |

EC 26 (cross-task conflict, L8.1) lives in `tests/e2e/l8-cross-task-conflict.test.ts` rather than this directory because it depends on infrastructure that shipped separately.

## How to run

```bash
bun test tests/e2e/trust-benchmark/
```

Each test file calls `buildCompiledCli` in `beforeAll`, which compiles `dist/maestro` on the first run. Subsequent runs in the same process reuse the cached binary.

To run a single scenario:

```bash
bun test tests/e2e/trust-benchmark/ec05-out-of-scope.test.ts
```

To run only the trust-benchmark corpus as part of the full E2E suite:

```bash
bun test tests/e2e/
```

## How to add a new scenario

### 1. Name the file

Use the pattern `ec<NN>-<slug>.test.ts` where `<NN>` is the edge-case number from the master list in ROADMAP.md. For example, for edge case 14 (policy weakening):

```
tests/e2e/trust-benchmark/ec14-policy-weakening.test.ts
```

### 2. Fixture pattern

Every scenario follows the same setup sequence:

```typescript
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildCompiledCli } from "../../helpers/run-compiled-cli.js";
import { runCommand } from "../../helpers/command-runner.js";

const runCompiled = await buildCompiledCli();

describe("EC <NN>: <short description>", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "ec<NN>-"));
    // Initialize a git repo
    await runCommand("git", ["init"], { cwd: dir });
    await runCommand("git", ["config", "user.email", "test@test.com"], { cwd: dir });
    await runCommand("git", ["config", "user.name", "Test"], { cwd: dir });
    // Initialize maestro
    await runCompiled(["init"], { cwd: dir });
    // Write fixtures specific to this scenario
    // ...
  });

  it("mitigation fires when trigger is present", async () => {
    // Write the triggering fixture
    // Drive the relevant verb
    // Assert the expected outcome (non-zero exit, specific error, evidence row, etc.)
  });

  it("mitigation does not fire when trigger is absent", async () => {
    // Write a clean fixture (no trigger)
    // Drive the same verb
    // Assert the clean-path outcome (zero exit, expected output)
  });
});
```

The `runCompiled` function takes a string array of CLI arguments and an options object with `cwd`. It returns `{ exitCode, stdout, stderr }`.

### 3. Assertion guidelines

Each scenario must include:

- **One positive assertion** — the mitigation fires when the trigger condition is present. Assert the specific exit code (`exitCode !== 0`), a JSON field, or an Evidence record that confirms the mitigation activated.
- **One negative assertion** — the mitigation does not fire when the trigger is absent. Assert the clean-path exit code and that no spurious Evidence rows were written.

Both can be separate `it(...)` blocks or two `expect(...)` calls within the same block. Do not combine them in a way that makes it unclear which condition was tested.

For Evidence-based assertions, prefer inspecting the JSON output (`--json` flag) over parsing text output.

### 4. Use helpers from the test infrastructure

Available helpers:

```typescript
// Build (or reuse cached) compiled binary
import { buildCompiledCli } from "../../helpers/run-compiled-cli.js";

// Run an arbitrary shell command in a directory
import { runCommand } from "../../helpers/command-runner.js";
```

`buildCompiledCli` returns a `runCompiled(args, opts)` function bound to the compiled binary path.

### 5. Keep scenarios isolated

Each test should create its own temp directory. Do not share state between `it(...)` blocks. Use `beforeEach` (not `beforeAll`) for fixture setup unless the setup is genuinely idempotent and read-only.

## Honest framing

The corpus is 9 of 32 edge cases — a seed, not a complete benchmark. The "20+ scenarios" vision in the original roadmap is explicitly not blocked on this slice. Nine well-tested scenarios beat twenty hand-waved ones. New scenarios are added when:

1. A regression is found in a shipped feature (add the scenario, fix the bug).
2. A new mitigation ships (add the scenario as part of the same PR).
3. A team member identifies a gap in coverage from a real incident.

The corpus is not intended to be complete coverage of all possible edge cases. It is a targeted regression guard against the mitigations that have already shipped.
