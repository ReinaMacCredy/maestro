---
name: maestro:user-testing-validator
description: "Run user testing validation during mission checkpoints. Determines testable assertions, sets up test environment, spawns flow validators, and synthesizes results."
argument-hint: "[<milestone>|--all] [--flows <flow1,flow2>]"
---

# User Testing Validator

Run user testing validation during mission checkpoints. Determines testable assertions, sets up test environment, spawns flow validators, and synthesizes results.

## Arguments

`$ARGUMENTS`

- `<milestone>`: Validate a specific milestone (e.g., `bootstrap`, `core`)
- `--all`: Validate all milestones
- `--flows`: Comma-separated list of specific flows to test

---

## When to Use This Skill

Use during Mission Control validation checkpoints when:
- Features have user-facing behavior
- CLI commands or UI flows need verification
- Integration with external systems must be tested
- End-to-end workflows need validation
- `maestro:worker-base` instructs final validation

---

## Required Skills

- `maestro:worker-base` - For startup/cleanup procedures
- `maestro:scrutiny-validator` - Often runs in sequence (code review first, then user testing)

---

## Work Procedure

### Step 1: Load Validation State and Contract

Check current validation status:

```bash
maestro validation show
```

Read `validation-contract.md` to identify testable assertions:
- Each feature's `fulfills` field references contract assertions
- Assertions define expected behavior to verify

### Step 2: Determine Testable Assertions

From the validation contract, identify assertions that:
- Are user-visible (CLI output, file changes, behavior)
- Can be tested via command execution
- Have clear pass/fail criteria

Example assertions:
```markdown
- **VAL-CLI-001**: `maestro mission list` outputs valid JSON with `--json`
- **VAL-FEAT-003**: Feature handoff creates worker artifact directory
- **VAL-INT-005**: Integration test passes with temp git repo
```

### Step 3: Set Up Test Environment

**For CLI testing:**
1. Create temporary git repository
2. Initialize Mission Control: `maestro mission create`
3. Set up required services (check `.maestro/bootstrap/services.yaml`)
4. Start services and verify healthchecks

**For UI testing:**
1. Start development server
2. Initialize browser automation session
3. Navigate to test entry point

**For API testing:**
1. Start API server
2. Verify endpoint availability
3. Prepare test data

### Step 4: Spawn Flow Validators

For each identified assertion:

1. **Determine validation surface**:
   - CLI command → Execute and verify output
   - File system → Verify file exists with expected content
   - Service → Verify healthcheck passes
   - Integration → Run integration test

2. **Spawn user-testing-flow-validator** subagent:
   - Pass assertion ID and description
   - Pass test environment details
   - Request structured test result

### Step 5: Execute Flow Tests

Each flow validator executes:

```typescript
interface FlowTest {
  assertionId: string;
  description: string;
  command?: string;        // CLI command to run
  expectedOutput?: string; // Expected stdout/stderr pattern
  expectedFiles?: string[]; // Files that should exist
  expectedExitCode?: number; // Expected exit code (default 0)
}
```

Example flow execution:
```bash
# Test CLI JSON output
cd /tmp/test-repo
maestro mission list --json | jq '.missions'
# Verify: Returns valid JSON array

# Test file creation
maestro feature prompt feat-001 --mission m1 --out /tmp/p.md
# Verify: File /tmp/p.md exists and contains required sections
```

### Step 6: Synthesize Results

Collect all flow validator results:

```typescript
interface UserTestingSynthesis {
  milestone: string;
  assertionsTested: number;
  assertionsPassed: number;
  assertionsFailed: number;
  flows: Array<{
    flowName: string;
    status: "passed" | "failed" | "skipped";
    assertions: string[];
    duration: number;
  }>;
  blockers: Array<{
    assertionId: string;
    flowName: string;
    expected: string;
    actual: string;
  }>;
}
```

### Step 7: Update Validation State

Record results:

```bash
maestro validation update --milestone <name> --status <passed|failed>
```

**Next step**: return results to `maestro:conduct`. If status is `passed` and scrutiny validation also passed, the conductor proceeds to `maestro milestone seal`. If status is `failed`, the conductor re-opens the affected features for worker re-dispatch. Do not seal the milestone yourself -- that is the conductor's gate.

### Step 8: Produce Validation Report

**Human format:**
```
=== User Testing Validation: <milestone> ===

Flows Tested: 3
Assertions Passed: 6/8

Flow Results:
✓ mission-creation-flow (2 assertions)
✗ feature-prompt-flow (1 failed)
  - VAL-FEAT-003: Worker artifact not created
    Expected: .maestro/missions/{id}/workers/...
    Actual: Directory missing

Status: FAILED (1 blocking issue)
```

**JSON format** (with `--json`):
```json
{
  "milestone": "core",
  "summary": {
    "flowsTested": 3,
    "assertionsPassed": 6,
    "assertionsTotal": 8
  },
  "flows": [...],
  "blockers": [...],
  "status": "failed"
}
```

---

## Validation Surfaces

| Surface | Testing Approach |
|---------|-----------------|
| **CLI Commands** | Execute via Bun subprocess, verify output |
| **File System** | Verify file existence, read and validate content |
| **HTTP APIs** | curl/http client requests, verify response |
| **UI/Browser** | Browser automation, screenshot verification |
| **Database** | Query verification, state checks |
| **Services** | Healthcheck endpoints, status commands |

---

## Test Environment Setup

**CLI Testing Pattern:**
```typescript
// Create temp repo
const tempDir = await mkdtemp(join(tmpdir(), 'test-'));
await exec(`git init ${tempDir}`);

// Run command and capture
const result = await exec(`cd ${tempDir} && maestro mission list --json`);
const output = JSON.parse(result.stdout);

// Verify
assert(Array.isArray(output.missions));
```

**Service Testing Pattern:**
```typescript
// Start service
await exec('bun run src/index.ts server &');
await sleep(1000); // Wait for startup

// Healthcheck
const health = await fetch('http://localhost:3000/health');
assert(health.status === 200);
```

---

## Related Commands

| Command | Purpose |
|---------|---------|
| `maestro validation show` | Show current validation state |
| `maestro validation update` | Update validation status |
| `maestro checkpoint save` | Save checkpoint after testing |
| `maestro checkpoint list` | List saved checkpoints |

---

## Integration with Scrutiny

User testing runs after scrutiny validation:

```
Feature Complete → Scrutiny Review → User Testing → Milestone Seal
                      ↓                    ↓
                 Blockers?              Blockers?
                      ↓                    ↓
              Return to fix          Return to fix
```

Both must pass before `maestro milestone seal` succeeds.

---

## Best Practices

1. **Test real user flows** - Not just unit tests, actual command sequences
2. **Use temp environments** - Never test in production repos
3. **Clean up after tests** - Remove temp files, stop services
4. **Document expected behavior** - Clear assertions enable clear failures
5. **Save checkpoint on success** - Mark validation milestone
