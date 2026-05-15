/**
 * EC 32 — PR self-weakening.
 *
 * Mitigation: Rule 12 — owners.yaml loaded from BASE branch, not PR head (L5.2).
 *
 * `verdict override` loads sensitive_waiver from the BASE branch via
 * `git show <base>:...owners.yaml`. If the invoking user is NOT in
 * sensitive_waiver at the base, the command exits 1 ("not-authorized").
 * A PR that adds the user to sensitive_waiver cannot self-approve — because
 * the gate reads the base-branch state, not the PR-head state.
 *
 * Positive: user NOT in sensitive_waiver at the base ref → override rejected (exit 1).
 * Negative: user IS in sensitive_waiver at the base ref → override succeeds (exit 0).
 */
import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import os from "node:os";
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

async function setupRepo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-ec32-"));
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
    intent: "EC32 self-weakening test",
    scope: { filesExpected: ["src/**"], filesForbidden: [] },
    doneWhen: [],
    amendments: [],
    createdBy: "ec32-test",
    lockedBy: "ec32-test",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    riskClass: "medium",
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
      "id: ec32-autopilot",
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

async function commitOwnersYaml(
  dir: string,
  sensitiveWaivers: string[],
): Promise<void> {
  const policyDir = join(dir, ".maestro", "policies");
  await mkdir(policyDir, { recursive: true });
  const lines = [
    "policy_approver:",
    "  - admin",
    "ratchet_approver:",
    "  - admin",
    "sensitive_waiver:",
    ...sensitiveWaivers.map((u) => `  - ${u}`),
  ];
  await writeFile(join(policyDir, "owners.yaml"), lines.join("\n"));
  await runCommand(["git", "add", ".maestro/policies/owners.yaml"], dir);
  await runCommand(
    ["git", "commit", "-m", "chore: owners.yaml", "--author", "Test <test@example.com>"],
    dir,
  );
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

/**
 * Request a verdict so there is something to override.
 * Returns the verdict ID.
 */
async function requestVerdictId(dir: string, taskId: string): Promise<string> {
  const result = await runCompiled(
    ["verdict", "request", "--task", taskId, "--json"],
    dir,
  );
  // Any non-crash exit code is acceptable; we just need the verdict id.
  const verdict = expectJson<{ id: string }>(result);
  return verdict.id;
}

// TODO(D-task-rehome): scaffolding uses v1 `task` CLI removed in Phase 5; rewire to v2 `task` verbs
describe.skip("EC 32 — PR self-weakening (Rule 12: base-branch owners.yaml loading)", () => {
  it(
    "positive: user NOT in sensitive_waiver at base ref → verdict override rejected (exit 1)",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "EC32 self-weakening positive");
      await seedContract(dir, taskId);

      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      const verdictId = await requestVerdictId(dir, taskId);

      // Commit owners.yaml WITHOUT the current user in sensitive_waiver.
      // --base HEAD means override reads the just-committed owners file.
      await commitOwnersYaml(dir, ["some-other-user"]);

      const result = await runCompiled(
        [
          "verdict", "override",
          "--task", taskId,
          "--pr", "1",
          "--reason", "EC32 self-promotion attempt",
          "--verdict", verdictId,
          "--base", "HEAD",
        ],
        dir,
      );

      expect(result.exitCode).toBe(1);
      expect(result.stderr + result.stdout).toContain("not-authorized");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "negative: user IS in sensitive_waiver at base ref → verdict override succeeds (exit 0)",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);

      await writeMediumPermissiveAutopilot(dir);

      const taskId = await createTask(dir, "EC32 self-weakening negative");
      await seedContract(dir, taskId);

      // Commit owners.yaml WITH the current user — this is the base commit.
      const currentUser = os.userInfo().username;
      await commitOwnersYaml(dir, [currentUser]);

      // Commit the feature file after the owners file.
      await commitFile(dir, "src/feature.ts", "export const x = 1;\n");

      // Request a verdict (diff from HEAD~1 = feature.ts only)
      const result1 = await runCompiled(
        ["verdict", "request", "--task", taskId, "--base", "HEAD~1", "--json"],
        dir,
      );
      const verdict = expectJson<{ id: string }>(result1);

      // Override with --base HEAD~1 (the owners.yaml commit that includes current user)
      const overrideResult = await runCompiled(
        [
          "verdict", "override",
          "--task", taskId,
          "--pr", "1",
          "--reason", "EC32 authorized override",
          "--verdict", verdict.id,
          "--base", "HEAD~1",
          "--json",
        ],
        dir,
      );

      expect(overrideResult.exitCode).toBe(0);
      const row = expectJson<{ kind: string }>(overrideResult);
      expect(row.kind).toBe("verdict-override");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
