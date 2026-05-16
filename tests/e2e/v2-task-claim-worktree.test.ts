import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";
import { runCommand } from "../helpers/command-runner.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

async function seedHeavySpec(dir: string, slug: string): Promise<string> {
  const specPath = join(dir, ".maestro/specs", `${slug}.md`);
  await mkdir(join(dir, ".maestro/specs"), { recursive: true });
  await writeFile(
    specPath,
    `---
slug: ${slug}
acceptance_criteria:
  - x
non_goals: []
risk_class: low
mode: heavy
work_type: spec-slice
---

# ${slug}
`,
    "utf8",
  );
  return specPath;
}

async function seedLightSpec(dir: string, slug: string): Promise<string> {
  const specPath = join(dir, ".maestro/specs", `${slug}.md`);
  await mkdir(join(dir, ".maestro/specs"), { recursive: true });
  await writeFile(
    specPath,
    `---
slug: ${slug}
acceptance_criteria:
  - x
non_goals: []
risk_class: low
mode: light
work_type: spec-slice
---

# ${slug}
`,
    "utf8",
  );
  return specPath;
}

async function makeCommittedRepo(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "maestro-v2-wt-"));
  await initGitRepo(dir);
  await runCommand(["git", "config", "user.email", "test@example.com"], dir);
  await runCommand(["git", "config", "user.name", "test"], dir);
  await writeFile(join(dir, "README.md"), "# test\n", "utf8");
  await runCommand(["git", "add", "."], dir);
  await runCommand(["git", "commit", "-m", "init"], dir);
  return dir;
}

beforeEach(async () => {
  tmpDir = await makeCommittedRepo();
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
  // Clean up sibling worktree dirs that git created during the test.
  // Pattern: <repoName>-tsk-* alongside the repo dir.
});

describe("maestro task claim --worktree (v2)", () => {
  it("creates a sibling worktree when claiming a heavy-mode spec", async () => {
    const specPath = await seedHeavySpec(tmpDir, "heavy-demo");
    const created = await runCompiled(["task", "from-spec", specPath], tmpDir);
    expect(created.exitCode).toBe(0);
    const taskId = created.stdout.split(/\s+/)[0]!;

    const claim = await runCompiled(["task", "claim", taskId], tmpDir);
    expect(claim.exitCode).toBe(0);
    expect(claim.stdout).toContain("worktree");

    const flag = await readFile(
      join(tmpDir, ".maestro/worktrees", `${taskId}.json`),
      "utf8",
    );
    const record = JSON.parse(flag) as { task_id: string; path: string; branch: string };
    expect(record.task_id).toBe(taskId);
    expect(record.branch.startsWith("feat/")).toBe(true);
    const wtStat = await stat(record.path);
    expect(wtStat.isDirectory()).toBe(true);
    // Clean up the sibling worktree dir.
    await rm(record.path, { recursive: true, force: true });
  });

  it("does not create a worktree for light-mode specs", async () => {
    const specPath = await seedLightSpec(tmpDir, "light-demo");
    const created = await runCompiled(["task", "from-spec", specPath], tmpDir);
    const taskId = created.stdout.split(/\s+/)[0]!;
    const claim = await runCompiled(["task", "claim", taskId], tmpDir);
    expect(claim.exitCode).toBe(0);
    expect(claim.stdout).not.toContain("worktree");
    const dir = join(tmpDir, ".maestro/worktrees");
    let exists = true;
    try {
      await stat(dir);
    } catch {
      exists = false;
    }
    if (exists) {
      const list = await runCompiled(["task", "get", taskId, "--json"], tmpDir);
      // Either the dir doesn't exist or there's no record for this task.
      const flagPath = join(dir, `${taskId}.json`);
      try {
        await stat(flagPath);
        throw new Error(`unexpected worktree flag at ${flagPath}`);
      } catch (err) {
        if ((err as NodeJS.ErrnoException).code !== "ENOENT") {
          // Re-throw unexpected errors.
          if (!(err as Error).message.startsWith("unexpected")) throw err;
          throw err;
        }
      }
      expect(list.exitCode).toBe(0);
    }
  });

  it("--skip-worktree bypasses worktree creation for heavy specs", async () => {
    const specPath = await seedHeavySpec(tmpDir, "skip-demo");
    const created = await runCompiled(["task", "from-spec", specPath], tmpDir);
    const taskId = created.stdout.split(/\s+/)[0]!;
    const claim = await runCompiled(
      ["task", "claim", taskId, "--skip-worktree"],
      tmpDir,
    );
    expect(claim.exitCode).toBe(0);
    expect(claim.stdout).not.toContain("worktree");
  });
});
