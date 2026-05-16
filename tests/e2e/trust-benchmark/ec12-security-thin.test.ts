/**
 * EC 12 — Security-thin change (threat-model required predicate).
 *
 * Mitigation: Threat-model required predicate at L4 (compute-risk.ts).
 *
 * When derivedRiskClass === "critical" AND matchedSignal === "diff-intersects-sensitive-security"
 * AND no threat-model Evidence row exists, a "threat-model-required" reason is emitted.
 *
 * Positive: critical security-path diff without a threat-model → threat-model-required reason
 *           in the HUMAN verdict.
 * Negative: same setup WITH a threat-model Evidence row → threat-model-required reason absent.
 */
import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../../helpers/run-compiled-cli.js";
import { runCommand } from "../../helpers/command-runner.js";

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

const tempDirs: string[] = [];

afterEach(async () => {
  for (const d of tempDirs.splice(0)) {
    await rm(d, { recursive: true, force: true });
  }
});

// Path to the minimal threat-model fixture used by L4 tests.
const TM_FIXTURE = join(import.meta.dir, "..", "..", "fixtures", "threat-models", "minimal.json");

async function setupRepo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-ec12-"));
  await initGitRepo(dir);
  await runCommand(["git", "config", "user.email", "test@example.com"], dir);
  await runCommand(["git", "config", "user.name", "Test"], dir);
  const init = await runCompiled(["init"], dir);
  if (init.exitCode !== 0) throw new Error(`maestro init failed: ${init.stderr}`);
  await runCommand(
    ["git", "commit", "--allow-empty", "-m", "init", "--author", "Test <test@example.com>"],
    dir,
  );
  return dir;
}

async function createTask(dir: string, title: string): Promise<string> {
  const r = await runCompiled(["task", "q", title], dir);
  if (r.exitCode !== 0) throw new Error(`task q failed: ${r.stderr}`);
  const id = r.stdout.trim();
  if (!id.match(/^tsk-[0-9a-f]{6}$/)) throw new Error(`Unexpected task id: "${id}"`);
  return id;
}

async function seedContract(dir: string, taskId: string): Promise<void> {
  const contractDir = join(dir, ".maestro", "contracts", taskId);
  await mkdir(contractDir, { recursive: true });
  const contract = {
    schemaVersion: 2,
    id: `c-${taskId.slice(-6)}`,
    taskId,
    repoRoot: ".",
    status: "locked",
    createdAt: "2026-01-01T00:00:00.000Z",
    lockedAt: "2026-01-01T00:00:01.000Z",
    intent: "EC12 threat-model test",
    scope: { filesExpected: ["src/auth/**"], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "ec12-test",
    lockedBy: "ec12-test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    // Agent proposes low; derived will be critical via sensitive-security signal → Rule 1 raises.
    riskClass: "low",
    amendmentBudget: {
      maxAmendments: 4,
      maxPathsPerAmendment: 3,
      forbiddenAmendmentPaths: [],
    },
  };
  await writeFile(join(contractDir, "v1.json"), JSON.stringify(contract, null, 2));
}

async function writeMediumPermissiveAutopilot(dir: string): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  await writeFile(
    join(policyDir, "autopilot.yaml"),
    [
      "kind: autopilot",
      "id: ec12-autopilot",
      'version: "1"',
      "auto_merge_allowed:",
      "  low: true",
      "  medium: true",
      "  high: false",
      "  critical: false",
      "required_witness_level:",
      "  low: agent-claimed-locally",
      "  medium: agent-claimed-locally",
      "  high: witnessed-by-maestro",
      "  critical: witnessed-by-maestro",
    ].join("\n"),
  );
}

async function writeSensitivePaths(dir: string, globs: string[]): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  const lines = ["paths:", ...globs.map((g) => `  - "${g}"`)];
  await writeFile(join(policyDir, "sensitive-paths.yaml"), lines.join("\n"));
}

async function commitFile(dir: string, relPath: string, content = "// test\n"): Promise<void> {
  const fullPath = join(dir, relPath);
  await mkdir(join(fullPath, ".."), { recursive: true });
  await writeFile(fullPath, content);
  await runCommand(["git", "add", relPath], dir);
  await runCommand(
    ["git", "commit", "-m", `chore: ${relPath}`, "--author", "Test <test@example.com>"],
    dir,
  );
}

// TODO(D-task-rehome): scaffolding uses v1 `task` CLI removed in Phase 5; rewire to v2 `task` verbs
describe.skip("EC 12 — security-thin (threat-model required predicate at L4)", () => {
  it(
    "positive: critical security diff without threat-model → threat-model-required reason in HUMAN verdict",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);
      await writeSensitivePaths(dir, ["src/auth/**"]);

      const taskId = await createTask(dir, "EC12 no-threat-model");
      await seedContract(dir, taskId);

      // Touch security-relevant path → derivedRiskClass=critical, signal=diff-intersects-sensitive-security
      await commitFile(dir, "src/auth/login.ts", "export function login() {}\n");

      const result = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      // critical → HUMAN (exit 2)
      expect(result.exitCode).toBe(2);
      const verdict = expectJson<{
        decision: string;
        effectiveRiskClass: string;
        reasons: Array<{ category: string; code: string }>;
      }>(result);
      expect(verdict.decision).toBe("HUMAN");
      expect(verdict.effectiveRiskClass).toBe("critical");

      const tmReason = verdict.reasons.find((r) => r.code === "threat-model-required");
      expect(tmReason).toBeDefined();
      expect(tmReason!.category).toBe("policy");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "negative: critical security diff WITH a threat-model Evidence row → threat-model-required reason absent",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);
      await writeSensitivePaths(dir, ["src/auth/**"]);

      const taskId = await createTask(dir, "EC12 with-threat-model");
      await seedContract(dir, taskId);

      await commitFile(dir, "src/auth/login.ts", "export function login() {}\n");

      // Record a threat-model evidence row
      const tmResult = await runCompiled(
        [
          "evidence", "record",
          "--task", taskId,
          "--kind", "threat-model",
          "--threat-model-file", TM_FIXTURE,
        ],
        dir,
      );
      expect(tmResult.exitCode).toBe(0);

      const result = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );

      // Still HUMAN (critical always requires human review per Rule 4),
      // but threat-model-required reason must be absent.
      const verdict = expectJson<{
        decision: string;
        reasons: Array<{ category: string; code: string }>;
      }>(result);
      expect(verdict.decision).toBe("HUMAN");

      const tmReason = verdict.reasons.find((r) => r.code === "threat-model-required");
      expect(tmReason).toBeUndefined();
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
