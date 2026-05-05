/**
 * L2 bridge flow — end-to-end coverage for the L1 ↔ L2 contract seam.
 *
 * Pre-fix bug: `task contract new --from <yaml> && task contract lock` wrote
 * to the L1 store (.maestro/tasks/contracts/c-XXXXXX.json) but never to the
 * L2 versioned store (.maestro/contracts/<taskId>/vN.json). All trust-substrate
 * readers (task verify, plan check, verdict request, contract show, merge auto,
 * ci verify) read from L2, so the documented workflow returned "no contract".
 *
 * Three parallel teammates worked around it by hand-copying L1 → L2 v1.json.
 *
 * Fix (commit 1 of this branch): write-through mirror at the use-case layer +
 * read-time backfill for legacy L1-only contracts.
 *
 * This suite locks the seam from both directions:
 *   - Every L1 transition that lands in active state mirrors to L2 (tests 1, 6–9, 13).
 *   - Every L2 reader resolves a freshly-locked contract without manual seeding (tests 2–5).
 *   - Pre-fix repos with L1-only state are backfilled on first L2 read (test 10).
 *   - Drafts and discarded contracts are deliberately invisible to L2 (tests 11–12).
 *
 * These tests would have failed loudly against `main` before the bridge fix shipped.
 */
import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  expectJson,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";
import { runCommand } from "../helpers/command-runner.js";

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

const tempDirs: string[] = [];

afterEach(async () => {
  for (const d of tempDirs.splice(0)) {
    await rm(d, { recursive: true, force: true });
  }
});

interface L1Contract {
  readonly id: string;
  readonly taskId: string;
  readonly status: string;
  readonly intent: string;
  readonly scope: { readonly filesExpected: readonly string[]; readonly filesForbidden: readonly string[] };
  readonly doneWhen: ReadonlyArray<{ readonly id: string; readonly text: string; readonly kind: string; readonly met?: boolean }>;
  readonly amendments: ReadonlyArray<unknown>;
}

interface L1IndexRow {
  readonly id: string;
  readonly taskId: string;
  readonly status: string;
}

interface SetupRepoOptions {
  /** When true, write a .maestro/config.yaml that allows overlapping contracts.
   *  Needed for tests that lock multiple contracts in one repo. */
  readonly allowOverlap?: boolean;
}

async function setupRepo(options: SetupRepoOptions = {}): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-l2b-bridge-"));
  await initGitRepo(dir);
  await runCommand(["git", "config", "user.email", "test@example.com"], dir);
  await runCommand(["git", "config", "user.name", "Test"], dir);
  const init = await runCompiled(["init"], dir);
  if (init.exitCode !== 0) throw new Error(`maestro init failed: ${init.stderr}`);
  if (options.allowOverlap === true) {
    const configPath = join(dir, ".maestro", "config.yaml");
    await writeFile(configPath, "contracts:\n  overlapPolicy: annotate\n");
  }
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

interface ContractYamlOptions {
  readonly intent?: string;
  readonly filesExpected?: readonly string[];
  readonly filesForbidden?: readonly string[];
  readonly criteria?: readonly string[];
}

async function writeContractYaml(dir: string, opts: ContractYamlOptions = {}): Promise<string> {
  const intent = opts.intent ?? "L2 bridge flow test";
  const filesExpected = opts.filesExpected ?? ["src/**"];
  const filesForbidden = opts.filesForbidden ?? [];
  const criteria = opts.criteria ?? ["task complete"];
  const yaml = [
    `intent: "${intent}"`,
    `scope:`,
    `  filesExpected:`,
    ...filesExpected.map((p) => `    - "${p}"`),
    `  filesForbidden:`,
    ...filesForbidden.map((p) => `    - "${p}"`),
    `doneWhen:`,
    ...criteria.flatMap((c) => [`  - text: "${c}"`, `    kind: "manual"`]),
    "",
  ].join("\n");
  const path = join(dir, "contract.yaml");
  await writeFile(path, yaml);
  return path;
}

/**
 * Drive the documented workflow: `task contract new --from <yaml>` then `lock`.
 * This is the path that was broken pre-fix — it intentionally does not
 * hand-write v1.json. The whole point of these tests is that this works
 * end-to-end without manual L2 seeding.
 */
async function lockContractViaCli(
  dir: string,
  taskId: string,
  opts: ContractYamlOptions = {},
): Promise<void> {
  const yamlPath = await writeContractYaml(dir, opts);
  const newRes = await runCompiled(["task", "contract", "new", taskId, "--from", yamlPath], dir);
  if (newRes.exitCode !== 0) throw new Error(`task contract new failed: ${newRes.stderr}`);
  const lockRes = await runCompiled(["task", "contract", "lock", taskId], dir);
  if (lockRes.exitCode !== 0) throw new Error(`task contract lock failed: ${lockRes.stderr}`);
}

async function readL1ContractByTaskId(dir: string, taskId: string): Promise<L1Contract> {
  const indexPath = join(dir, ".maestro", "tasks", "contracts", "index.jsonl");
  const indexText = await readFile(indexPath, "utf-8");
  const rows: L1IndexRow[] = indexText
    .split("\n")
    .filter((line) => line.trim().length > 0)
    .map((line) => JSON.parse(line) as L1IndexRow);
  // Latest row for this taskId
  const matches = rows.filter((r) => r.taskId === taskId);
  if (matches.length === 0) throw new Error(`No L1 index row for task ${taskId}`);
  const latest = matches[matches.length - 1]!;
  const contractPath = join(dir, ".maestro", "tasks", "contracts", `${latest.id}.json`);
  const raw = await readFile(contractPath, "utf-8");
  return JSON.parse(raw) as L1Contract;
}

async function readL2VersionFile(dir: string, taskId: string, version: number): Promise<L1Contract> {
  const path = join(dir, ".maestro", "contracts", taskId, `v${version}.json`);
  const raw = await readFile(path, "utf-8");
  return JSON.parse(raw) as L1Contract;
}

async function listL2Versions(dir: string, taskId: string): Promise<readonly string[]> {
  const taskDir = join(dir, ".maestro", "contracts", taskId);
  if (!existsSync(taskDir)) return [];
  const names = await readdir(taskDir);
  return names.filter((n) => /^v\d+\.json$/.test(n)).sort();
}

function l2VersionExists(dir: string, taskId: string, version: number): boolean {
  return existsSync(join(dir, ".maestro", "contracts", taskId, `v${version}.json`));
}

/**
 * Single source of truth for "L1 and L2 stayed in sync." Used wherever a test
 * mutates state via L1 verbs and expects L2 to reflect the latest state.
 */
async function assertL1AndL2Match(dir: string, taskId: string, expectedVersion: number): Promise<void> {
  const l1 = await readL1ContractByTaskId(dir, taskId);
  const l2 = await readL2VersionFile(dir, taskId, expectedVersion);
  expect(l2.id).toBe(l1.id);
  expect(l2.taskId).toBe(l1.taskId);
  expect(l2.status).toBe(l1.status);
  expect(l2.intent).toBe(l1.intent);
  expect(l2.scope.filesExpected).toEqual(l1.scope.filesExpected);
  expect(l2.scope.filesForbidden).toEqual(l1.scope.filesForbidden);
  expect(l2.doneWhen).toEqual(l1.doneWhen);
  expect(l2.amendments).toEqual(l1.amendments);
}

describe("L2 contract bridge — end-to-end seam coverage", () => {
  it(
    "1. lock writes both stores",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-1 lock writes both");

      await lockContractViaCli(dir, taskId);

      // L1 file exists at .maestro/tasks/contracts/<id>.json
      const l1 = await readL1ContractByTaskId(dir, taskId);
      expect(l1.status).toBe("locked");
      // L2 v1.json exists and matches L1 field-for-field
      expect(l2VersionExists(dir, taskId, 1)).toBe(true);
      await assertL1AndL2Match(dir, taskId, 1);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "2. plan check resolves a freshly-locked contract without manual seeding",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-2 plan check");

      await lockContractViaCli(dir, taskId);

      // plan-check expects a YAML plan with intendedFiles + proofSet + riskClass.
      const planPath = join(dir, "plan.yaml");
      await writeFile(
        planPath,
        [
          "intendedFiles:",
          "  - src/feature/x.ts",
          "proofSet: []",
          "riskClass: low",
          "",
        ].join("\n"),
      );
      const r = await runCompiled(
        ["plan", "check", "--task", taskId, "--plan-file", planPath, "--json"],
        dir,
      );
      // plan check always exits 0; what matters is it didn't bail with "no contract"
      expect(r.exitCode).toBe(0);
      const combined = r.stdout + " " + r.stderr;
      expect(combined).not.toMatch(/no contract found|cannot find contract|undefined contract/i);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "3. task verify resolves a freshly-locked contract without manual seeding",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-3 task verify");

      await lockContractViaCli(dir, taskId);

      const r = await runCompiled(["task", "verify", "--task", taskId, "--json"], dir);
      expect(r.exitCode).toBe(0);
      const combined = r.stdout + " " + r.stderr;
      expect(combined).not.toMatch(/no contract proposed/i);
      const parsed = expectJson<{ findings: readonly unknown[] }>(r);
      expect(Array.isArray(parsed.findings)).toBe(true);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "4. verdict request resolves a freshly-locked contract without manual seeding",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-4 verdict request");

      await lockContractViaCli(dir, taskId);

      const r = await runCompiled(["verdict", "request", "--task", taskId, "--json"], dir);
      // Verdict request exits 0/1/2/3 (PASS/FAIL/HUMAN/BLOCK). The bug we're
      // catching is when it exits non-zero with "no contract" rather than a
      // legitimate decision — assert no "no contract" error in either stream.
      expect(r.exitCode).toBeGreaterThanOrEqual(0);
      expect(r.exitCode).toBeLessThanOrEqual(3);
      const combined = r.stdout + " " + r.stderr;
      expect(combined).not.toMatch(/no contract found|run 'maestro contract amend' first/i);
      const parsed = expectJson<{ decision: string }>(r);
      expect(["PASS", "FAIL", "HUMAN", "BLOCK"]).toContain(parsed.decision);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "5. contract show (L2 reader) returns the locked contract",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-5 contract show");

      await lockContractViaCli(dir, taskId, { intent: "test-5 specific intent" });

      const r = await runCompiled(["contract", "show", "--task", taskId, "--json"], dir);
      expect(r.exitCode).toBe(0);
      const parsed = expectJson<{ status: string; taskId: string; intent: string }>(r);
      expect(parsed.status).toBe("locked");
      expect(parsed.taskId).toBe(taskId);
      expect(parsed.intent).toBe("test-5 specific intent");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "6. contract amend → L2 history shows v1+v2 in order",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-6 amend history");

      await lockContractViaCli(dir, taskId);

      // Amend via the L2 verb (which we now backfill correctly)
      const amend = await runCompiled(
        [
          "contract", "amend",
          "--task", taskId,
          "--add-path", "tests/**",
          "--reason", "test-6 expanding scope",
          "--json",
        ],
        dir,
      );
      expect(amend.exitCode).toBe(0);
      const amendResult = expectJson<{ newVersion: number; amendmentId: string }>(amend);
      expect(amendResult.newVersion).toBe(2);

      // L2 history shows v1 + v2
      const versions = await listL2Versions(dir, taskId);
      expect(versions).toEqual(["v1.json", "v2.json"]);

      const history = await runCompiled(["contract", "history", "--task", taskId, "--json"], dir);
      expect(history.exitCode).toBe(0);
      const rows = expectJson<readonly { status: string; amendments: readonly unknown[] }[]>(history);
      expect(rows.length).toBeGreaterThanOrEqual(2);
      expect(rows[0]!.status).toBe("locked");
      expect(rows[1]!.status).toBe("amended");
      expect(rows[1]!.amendments.length).toBeGreaterThan(0);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "7. criteria add via L1 propagates to L2",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-7 criteria add");

      await lockContractViaCli(dir, taskId, { criteria: ["original criterion"] });

      const baseShow = await runCompiled(["contract", "show", "--task", taskId, "--json"], dir);
      const before = expectJson<{ doneWhen: ReadonlyArray<{ text: string }> }>(baseShow);
      const beforeCount = before.doneWhen.length;

      const add = await runCompiled(
        ["task", "contract", "criteria", "add", taskId, "newly added criterion"],
        dir,
      );
      expect(add.exitCode).toBe(0);

      const afterShow = await runCompiled(["contract", "show", "--task", taskId, "--json"], dir);
      const after = expectJson<{ doneWhen: ReadonlyArray<{ text: string }> }>(afterShow);
      expect(after.doneWhen.length).toBe(beforeCount + 1);
      expect(after.doneWhen.some((c) => c.text === "newly added criterion")).toBe(true);

      // L1 and L2 reflect the same state at the latest version
      const versions = await listL2Versions(dir, taskId);
      await assertL1AndL2Match(dir, taskId, versions.length);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "8. criteria mark met via L1 propagates to L2",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-8 criteria mark");

      await lockContractViaCli(dir, taskId, { criteria: ["criterion to mark"] });
      const initialShow = await runCompiled(["contract", "show", "--task", taskId, "--json"], dir);
      const initial = expectJson<{ doneWhen: ReadonlyArray<{ id: string; text: string }> }>(initialShow);
      const criterionId = initial.doneWhen[0]!.id;

      const mark = await runCompiled(
        ["task", "contract", "criteria", "mark", taskId, criterionId, "--met"],
        dir,
      );
      expect(mark.exitCode).toBe(0);

      const afterShow = await runCompiled(["contract", "show", "--task", taskId, "--json"], dir);
      const after = expectJson<{ doneWhen: ReadonlyArray<{ id: string; met?: boolean }> }>(afterShow);
      const target = after.doneWhen.find((c) => c.id === criterionId);
      expect(target).toBeDefined();
      expect(target?.met).toBe(true);

      const versions = await listL2Versions(dir, taskId);
      await assertL1AndL2Match(dir, taskId, versions.length);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "9. criteria remove via L1 propagates to L2",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-9 criteria remove");

      await lockContractViaCli(dir, taskId, {
        criteria: ["keeper criterion", "removable criterion"],
      });
      const initialShow = await runCompiled(["contract", "show", "--task", taskId, "--json"], dir);
      const initial = expectJson<{ doneWhen: ReadonlyArray<{ id: string; text: string }> }>(initialShow);
      const targetId = initial.doneWhen.find((c) => c.text === "removable criterion")!.id;

      const remove = await runCompiled(
        ["task", "contract", "criteria", "remove", taskId, targetId],
        dir,
      );
      expect(remove.exitCode).toBe(0);

      const afterShow = await runCompiled(["contract", "show", "--task", taskId, "--json"], dir);
      const after = expectJson<{ doneWhen: ReadonlyArray<{ id: string; text: string }> }>(afterShow);
      expect(after.doneWhen.some((c) => c.id === targetId)).toBe(false);
      expect(after.doneWhen.some((c) => c.text === "keeper criterion")).toBe(true);

      const versions = await listL2Versions(dir, taskId);
      await assertL1AndL2Match(dir, taskId, versions.length);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "10. legacy L1-only contract is backfilled to L2 on first read",
    async () => {
      // Simulates a pre-fix repo: L1 contract exists, no L2 mirror exists,
      // and the user just upgraded to a build with the bridge fix. The first
      // L2 read (here `task verify`) should backfill v1.json from L1.
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-10 legacy backfill");

      // Hand-write only the L1 store (mimics pre-fix repo state).
      const contractId = `c-${taskId.slice(-6)}`;
      const legacyContract = {
        schemaVersion: 2,
        id: contractId,
        taskId,
        repoRoot: ".",
        status: "locked",
        createdAt: "2026-01-01T00:00:00.000Z",
        lockedAt: "2026-01-01T00:00:01.000Z",
        intent: "legacy locked contract from a pre-fix repo",
        scope: { filesExpected: ["src/**"], filesForbidden: [] },
        doneWhen: [{ id: "dw-aaaaaa", text: "still passes", kind: "manual" }],
        amendments: [],
        createdBy: "legacy-test",
        lockedBy: "legacy-test",
        configSnapshot: {
          strict: false,
          overlapPolicy: "annotate",
          rebaseFallback: "best-effort",
          staleReclaimContractPolicy: "inherit",
        },
      };
      const l1Dir = join(dir, ".maestro", "tasks", "contracts");
      await mkdir(l1Dir, { recursive: true });
      await writeFile(join(l1Dir, `${contractId}.json`), JSON.stringify(legacyContract, null, 2));
      await writeFile(
        join(l1Dir, "index.jsonl"),
        JSON.stringify({ id: contractId, taskId, status: "locked", at: "2026-01-01T00:00:01.000Z" }) + "\n",
      );

      // Confirm L2 is empty BEFORE the read.
      expect(l2VersionExists(dir, taskId, 1)).toBe(false);

      // Trigger an L2 read — should backfill v1.json transparently.
      const r = await runCompiled(["task", "verify", "--task", taskId, "--json"], dir);
      expect(r.exitCode).toBe(0);
      const combined = r.stdout + " " + r.stderr;
      expect(combined).not.toMatch(/no contract proposed/i);

      // After the read, L2 v1.json now exists and matches L1.
      expect(l2VersionExists(dir, taskId, 1)).toBe(true);
      const l2 = await readL2VersionFile(dir, taskId, 1);
      expect(l2.id).toBe(contractId);
      expect(l2.taskId).toBe(taskId);
      expect(l2.status).toBe("locked");
      expect(l2.intent).toBe(legacyContract.intent);
      expect(l2.scope.filesExpected).toEqual(legacyContract.scope.filesExpected);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "11. draft contract is NOT mirrored to L2",
    async () => {
      // The bridge intentionally only mirrors active states (locked, amended,
      // fulfilled, broken). Drafts must stay invisible to the trust substrate.
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "test-11 draft not mirrored");

      const yamlPath = await writeContractYaml(dir, { intent: "draft intent" });
      const newRes = await runCompiled(
        ["task", "contract", "new", taskId, "--from", yamlPath],
        dir,
      );
      expect(newRes.exitCode).toBe(0);

      // L1 has a draft contract; L2 should be empty.
      const l1 = await readL1ContractByTaskId(dir, taskId);
      expect(l1.status).toBe("draft");
      const versions = await listL2Versions(dir, taskId);
      expect(versions).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "12. discarded draft does not write to L2 and does not corrupt prior L2 state",
    async () => {
      // Discard only applies to drafts, so this verifies the negative path:
      // we lock taskA (mirrors v1 to L2), then create a fresh draft on taskB,
      // discard it, and confirm neither operation polluted L2 for taskA, and
      // taskB has nothing in L2. Two contracts in one repo — annotate policy.
      const dir = await setupRepo({ allowOverlap: true });
      tempDirs.push(dir);

      const taskA = await createTask(dir, "test-12 keeper task A");
      await lockContractViaCli(dir, taskA, { intent: "task A — locked" });
      const aVersionsBefore = await listL2Versions(dir, taskA);
      expect(aVersionsBefore).toEqual(["v1.json"]);

      const taskB = await createTask(dir, "test-12 disposable task B");
      const yamlPath = await writeContractYaml(dir, { intent: "disposable draft" });
      const newRes = await runCompiled(
        ["task", "contract", "new", taskB, "--from", yamlPath],
        dir,
      );
      expect(newRes.exitCode).toBe(0);

      const discardRes = await runCompiled(["task", "contract", "discard", taskB], dir);
      expect(discardRes.exitCode).toBe(0);

      // taskA's L2 store unchanged.
      const aVersionsAfter = await listL2Versions(dir, taskA);
      expect(aVersionsAfter).toEqual(["v1.json"]);
      const aL2 = await readL2VersionFile(dir, taskA, 1);
      expect(aL2.intent).toBe("task A — locked");
      // taskB has nothing in L2 (draft was never mirrored, discard didn't write either).
      const bVersions = await listL2Versions(dir, taskB);
      expect(bVersions).toEqual([]);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "13. two tasks: bridge is per-task, contents do not cross-pollinate",
    async () => {
      // Two locked contracts in one repo — flip overlapPolicy to annotate so
      // L1 doesn't reject the second lock for scope overlap. This test is
      // about the bridge, not overlap detection.
      const dir = await setupRepo({ allowOverlap: true });
      tempDirs.push(dir);

      const taskA = await createTask(dir, "test-13 task A");
      const taskB = await createTask(dir, "test-13 task B");

      await lockContractViaCli(dir, taskA, {
        intent: "task A specific intent",
        filesExpected: ["src/featureA/**"],
      });
      await lockContractViaCli(dir, taskB, {
        intent: "task B specific intent",
        filesExpected: ["src/featureB/**"],
      });

      // Both tasks have their own v1.json, no cross-contamination.
      expect(l2VersionExists(dir, taskA, 1)).toBe(true);
      expect(l2VersionExists(dir, taskB, 1)).toBe(true);

      const aL2 = await readL2VersionFile(dir, taskA, 1);
      const bL2 = await readL2VersionFile(dir, taskB, 1);
      expect(aL2.taskId).toBe(taskA);
      expect(bL2.taskId).toBe(taskB);
      expect(aL2.intent).toBe("task A specific intent");
      expect(bL2.intent).toBe("task B specific intent");
      expect(aL2.scope.filesExpected).toEqual(["src/featureA/**"]);
      expect(bL2.scope.filesExpected).toEqual(["src/featureB/**"]);
      expect(aL2.id).not.toBe(bL2.id);

      await assertL1AndL2Match(dir, taskA, 1);
      await assertL1AndL2Match(dir, taskB, 1);
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
