/**
 * Config resolution + UX-fix flow — covers three bugs surfaced by the v0.72.1
 * greenfield-demo run with three parallel teammates working in git worktrees:
 *
 *   1. Worktree config-snapshot bug
 *      `services.config.load(process.cwd())` was called with the worktree's
 *      working tree (no `.maestro/`), so contracts locked from a worktree
 *      captured policy *defaults* instead of the shared-repo `.maestro/config.yaml`.
 *      Fix: route every config-load call through `resolveMaestroProjectRoot`,
 *      which walks via `.git/commondir` to the main worktree.
 *
 *   2. `verdict request` raw stack trace when no contract exists
 *      Bare `throw new Error(...)` produced bunfs-formatted stack traces
 *      to stderr. Fix: throw `MaestroError` with hints — same structured CLI
 *      formatting as every other "missing contract" case.
 *
 *   3. Silent drop of unknown contract-draft YAML keys
 *      Typos like `scope.allowedPaths` (vs the real `scope.filesExpected`)
 *      were silently dropped at parse time. Fix: warn to stderr per unknown
 *      key with did-you-mean hints; warnings are advisory (no exit-code change).
 */
import { afterEach, beforeAll, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  SLOW_CLI_TIMEOUT_MS,
  buildCompiledCli,
  runCompiled,
} from "../helpers/run-compiled-cli.js";
import { runCommand, initGitRepo } from "../helpers/command-runner.js";

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
  readonly configSnapshot?: {
    readonly overlapPolicy?: string;
    readonly strict?: boolean;
  };
  readonly scope: { readonly filesExpected: readonly string[]; readonly filesForbidden: readonly string[] };
  readonly doneWhen: ReadonlyArray<{ readonly id: string; readonly text: string; readonly kind: string }>;
}

interface L1IndexRow {
  readonly id: string;
  readonly taskId: string;
}

async function setupRepo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-l2c-config-"));
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

async function readL1ContractByTaskId(dir: string, taskId: string): Promise<L1Contract> {
  // Always read via the *main* worktree's .maestro dir, even if dir is a worktree.
  const indexPath = join(dir, ".maestro", "tasks", "contracts", "index.jsonl");
  const indexText = await readFile(indexPath, "utf-8");
  const rows: L1IndexRow[] = indexText
    .split("\n")
    .filter((line) => line.trim().length > 0)
    .map((line) => JSON.parse(line) as L1IndexRow);
  const matches = rows.filter((r) => r.taskId === taskId);
  if (matches.length === 0) throw new Error(`No L1 index row for task ${taskId}`);
  const latest = matches[matches.length - 1]!;
  const contractPath = join(dir, ".maestro", "tasks", "contracts", `${latest.id}.json`);
  const raw = await readFile(contractPath, "utf-8");
  return JSON.parse(raw) as L1Contract;
}

async function writeContractYaml(dir: string, intent: string): Promise<string> {
  const yaml = [
    `intent: "${intent}"`,
    `scope:`,
    `  filesExpected:`,
    `    - "src/**"`,
    `  filesForbidden: []`,
    `doneWhen:`,
    `  - text: "task complete"`,
    `    kind: "manual"`,
    "",
  ].join("\n");
  const path = join(dir, "contract.yaml");
  await writeFile(path, yaml);
  return path;
}

// TODO(D-task-rehome): scaffolding uses v1 `task` CLI removed in Phase 5; rewire to v2 `task` verbs
describe.skip("L2C config resolution + UX fixes", () => {
  it(
    "1. contract locked from a git worktree captures the main repo's shared config",
    async () => {
      // Main repo with a shared config that opts into annotate-overlap.
      const main = await setupRepo();
      tempDirs.push(main);
      const sharedConfigPath = join(main, ".maestro", "config.yaml");
      await writeFile(sharedConfigPath, "contracts:\n  overlapPolicy: annotate\n");

      // Spin up a worktree on a side branch — the worktree path has NO `.maestro/`.
      const worktree = await mkdtemp(join(tmpdir(), "maestro-l2c-wt-"));
      tempDirs.push(worktree);
      // git worktree add prefers a non-existent dir; remove the mkdtemp-created one first.
      await rm(worktree, { recursive: true, force: true });
      const wtAdd = await runCommand(
        ["git", "worktree", "add", "-b", "wt-branch", worktree, "HEAD"],
        main,
      );
      if (wtAdd.exitCode !== 0) throw new Error(`git worktree add failed: ${wtAdd.stderr}`);

      // Drive the documented flow from inside the worktree.
      const taskId = await createTask(main, "config-resolution test");
      const yamlPath = await writeContractYaml(worktree, "from-worktree");
      const newRes = await runCompiled(
        ["task", "contract", "new", taskId, "--from", yamlPath],
        worktree,
      );
      expect(newRes.exitCode).toBe(0);
      const lockRes = await runCompiled(["task", "contract", "lock", taskId], worktree);
      expect(lockRes.exitCode).toBe(0);

      // The L1 contract lives in the *main* worktree's .maestro/ — read it from there.
      const l1 = await readL1ContractByTaskId(main, taskId);
      expect(l1.status).toBe("locked");
      // Critical assertion: the lock-time configSnapshot reflects the SHARED config,
      // not the contract-loader's defaults.
      expect(l1.configSnapshot?.overlapPolicy).toBe("annotate");

      // Cleanup the worktree before the parent dir is rm-rf'd by afterEach.
      await runCommand(["git", "worktree", "remove", "--force", worktree], main);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "2. unknown contract-draft YAML keys reject by default; --allow-unknown-keys downgrades to warnings",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const taskId = await createTask(dir, "unknown-keys test");

      // Mix top-level and nested unknown keys, including two with rename hints
      // (`allowedPaths` -> filesExpected, `forbiddenPaths` -> filesForbidden).
      const yaml = [
        `intent: "draft with typos"`,
        `scope:`,
        `  filesExpected:`,
        `    - "src/**"`,
        `  allowedPaths:`,
        `    - "src/extra/**"`,
        `  forbiddenPaths:`,
        `    - ".env"`,
        `  unknownNonsense: "ignored"`,
        `doneWhen:`,
        `  - text: "task complete"`,
        `    kind: "manual"`,
        `mysteryField: "ignored"`,
        "",
      ].join("\n");
      const yamlPath = join(dir, "contract.yaml");
      await writeFile(yamlPath, yaml);

      // Default: strict — typos that produce half-initialized contracts are
      // rejected. The error message names every offending key so the user
      // doesn't play whack-a-mole on retries.
      const strictRes = await runCompiled(
        ["task", "contract", "new", taskId, "--from", yamlPath],
        dir,
      );
      expect(strictRes.exitCode).not.toBe(0);
      const strictMsg = `${strictRes.stderr}${strictRes.stdout}`;
      expect(strictMsg).toContain("scope.allowedPaths");
      expect(strictMsg).toContain("did you mean 'filesExpected'");
      expect(strictMsg).toContain("scope.forbiddenPaths");
      expect(strictMsg).toContain("did you mean 'filesForbidden'");
      expect(strictMsg).toContain("scope.unknownNonsense");
      expect(strictMsg).toContain("mysteryField");

      // Opt-in fallback: `--allow-unknown-keys` keeps the old warn-and-ignore
      // behavior for users who actually want to attach free-form keys.
      const lenientRes = await runCompiled(
        ["task", "contract", "new", taskId, "--from", yamlPath, "--allow-unknown-keys"],
        dir,
      );
      expect(lenientRes.exitCode).toBe(0);
      expect(lenientRes.stderr).toContain("scope.allowedPaths");
      expect(lenientRes.stderr).toContain("did you mean 'filesExpected'");
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "3. verdict request emits a structured MaestroError when no contract exists (no raw stack trace)",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      // Real-shaped task id, but no contract was ever created for it.
      const r = await runCompiled(["verdict", "request", "--task", "tsk-aaaaaa"], dir);
      // Non-zero exit — exact code is not load-bearing here.
      expect(r.exitCode).not.toBe(0);
      // The user-facing message must be present.
      expect(r.stderr).toContain("No contract found for task tsk-aaaaaa");
      // Hints must be present (the MaestroError formatter prints them).
      expect(r.stderr).toContain("contract new tsk-aaaaaa");
      expect(r.stderr).toContain("contract lock tsk-aaaaaa");
      // No bunfs/runtime stack trace leaks should appear.
      expect(r.stderr).not.toContain("bunfs");
      expect(r.stderr).not.toMatch(/\bat \S+ \(.*\.ts:\d+/);
    },
    SLOW_CLI_TIMEOUT_MS,
  );

  it(
    "4. policy check emits a structured MaestroError when no contract exists",
    async () => {
      const dir = await setupRepo();
      tempDirs.push(dir);
      const r = await runCompiled(["policy", "check", "--task", "tsk-bbbbbb"], dir);
      expect(r.exitCode).not.toBe(0);
      expect(r.stderr).toContain("No contract found for task tsk-bbbbbb");
      expect(r.stderr).toContain("contract new tsk-bbbbbb");
      expect(r.stderr).toContain("contract lock tsk-bbbbbb");
      expect(r.stderr).not.toContain("bunfs");
    },
    SLOW_CLI_TIMEOUT_MS,
  );
});
