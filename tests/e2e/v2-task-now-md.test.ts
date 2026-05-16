import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-now-md-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

async function readNowMd(): Promise<string> {
  return readFile(join(tmpDir, ".maestro/tasks/NOW.md"), "utf8");
}

describe("v2 task verbs refresh .maestro/tasks/NOW.md", () => {
  it("task from-spec writes NOW.md with the new draft under Ready to pick up", async () => {
    await runCompiled(["spec", "new", "now-md-demo"], tmpDir);
    const created = await runCompiled(
      ["task", "from-spec", ".maestro/specs/now-md-demo.md"],
      tmpDir,
    );
    expect(created.exitCode).toBe(0);
    const taskId = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1];
    expect(taskId).toBeDefined();

    const md = await readNowMd();
    expect(md).toContain("# NOW");
    expect(md).toContain("## Ready to pick up (1)");
    expect(md).toContain(taskId!);
    expect(md).toContain("## In flight (0)");
  });

  it("task claim moves the task from Ready to pick up into In flight", async () => {
    await runCompiled(["spec", "new", "now-md-claim"], tmpDir);
    const created = await runCompiled(
      ["task", "from-spec", ".maestro/specs/now-md-claim.md"],
      tmpDir,
    );
    const taskId = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1]!;

    const claim = await runCompiled(
      ["task", "claim", taskId, "--agent", "agent-a"],
      tmpDir,
    );
    expect(claim.exitCode).toBe(0);

    const md = await readNowMd();
    expect(md).toContain("## In flight (1)");
    expect(md).toContain("## Ready to pick up (0)");
    expect(md).toContain(taskId);
    expect(md).toContain("Owner: agent-a");
  });

  it("task block moves the task into the Blocked section with its reason", async () => {
    await runCompiled(["spec", "new", "now-md-block"], tmpDir);
    const created = await runCompiled(
      ["task", "from-spec", ".maestro/specs/now-md-block.md"],
      tmpDir,
    );
    const taskId = (created.stdout.match(/^(tsk-\S+)/) ?? [])[1]!;
    await runCompiled(["task", "claim", taskId], tmpDir);

    const blocked = await runCompiled(
      ["task", "block", taskId, "--reason", "waiting on upstream"],
      tmpDir,
    );
    expect(blocked.exitCode).toBe(0);

    const md = await readNowMd();
    expect(md).toContain("## Blocked (1)");
    expect(md).toContain("Reason: waiting on upstream");
    expect(md).toContain("## In flight (0)");
  });
});
