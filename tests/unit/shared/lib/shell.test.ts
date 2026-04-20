import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";

interface ShellMemorySample {
  readonly iteration: number;
  readonly rssMb: number;
  readonly heapMb: number;
  readonly externalMb: number;
}

async function runShellProbe(script: string): Promise<readonly ShellMemorySample[]> {
  const repoRoot = join(import.meta.dir, "..", "..", "..", "..");
  const proc = Bun.spawn(["bun", "-e", script], {
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  expect(exitCode).toBe(0);
  expect(stderr.trim()).toBe("");
  return JSON.parse(stdout) as readonly ShellMemorySample[];
}

function growthMb(
  samples: readonly ShellMemorySample[],
  key: "rssMb" | "heapMb" | "externalMb",
): number {
  if (samples.length < 2) return 0;
  return Math.max(0, samples[samples.length - 1]![key] - samples[0]![key]);
}

describe("shell exec helpers", () => {
  it("keeps memory bounded across repeated command execution", async () => {
    const samples = await runShellProbe(`
      import { execArgv } from "./src/shared/lib/shell.ts";

      const cwd = process.cwd();
      const samples = [];
      for (let iteration = 1; iteration <= 15; iteration += 1) {
        await execArgv(["git", "status", "--porcelain"], { cwd });
        if (global.gc) global.gc();
        const memory = process.memoryUsage();
        samples.push({
          iteration,
          rssMb: Math.round(memory.rss / 1024 / 1024),
          heapMb: Math.round(memory.heapUsed / 1024 / 1024),
          externalMb: Math.round(memory.external / 1024 / 1024),
        });
      }

      console.log(JSON.stringify(samples));
    `);

    expect(samples).toHaveLength(15);
    expect(growthMb(samples, "rssMb")).toBeLessThan(8);
    expect(growthMb(samples, "externalMb")).toBeLessThan(4);
  });

  it("returns a timeout result instead of leaking a hanging child", async () => {
    const samples = await runShellProbe(`
      import { execArgv } from "./src/shared/lib/shell.ts";
      const result = await execArgv(["sleep", "2"], { timeout: 100 });
      console.log(JSON.stringify([{
        iteration: 1,
        rssMb: result.exitCode,
        heapMb: 0,
        externalMb: 0,
      }]));
    `);

    expect(samples[0]?.rssMb).toBe(124);
  });

  describe("runLoggedCommand", () => {
    let tempDir: string;

    beforeEach(async () => {
      tempDir = await mkdtemp(join(tmpdir(), "maestro-shell-log-"));
    });

    afterEach(async () => {
      await rm(tempDir, { recursive: true, force: true });
    });

    it("captures child output into the requested log file", async () => {
      const { runLoggedCommand } = await import("@/shared/lib/shell.js");
      const logPath = join(tempDir, "output.log");

      const result = await runLoggedCommand(
        ["bash", "-lc", "echo hello && echo world 1>&2"],
        { cwd: tempDir, logPath, wait: true },
      );

      expect(result.exitCode).toBe(0);
      const log = await readFile(logPath, "utf8");
      expect(log).toContain("hello");
      expect(log).toContain("world");
    });
  });
});
