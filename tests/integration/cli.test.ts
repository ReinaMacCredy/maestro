import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";

const CLI = [
  "bun",
  "run",
  join(import.meta.dir, "..", "..", "src", "index.ts"),
];

let tmpDir: string;

async function run(
  args: string[],
  cwd = process.cwd(),
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = Bun.spawn([...CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    cwd,
  });
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  const exitCode = await proc.exited;
  return { stdout: stdout.trim(), stderr: stderr.trim(), exitCode };
}

async function initGitRepo(cwd: string): Promise<void> {
  const init = Bun.spawn(["git", "init", "-b", "main"], {
    cwd,
    stdout: "pipe",
    stderr: "pipe",
  });
  await init.exited;
}

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-cli-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("CLI integration", () => {
  it("prints version", async () => {
    const { stdout, exitCode } = await run(["--version"]);
    expect(exitCode).toBe(0);
    expect(stdout).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("prints help with all commands", async () => {
    const { stdout, exitCode } = await run(["--help"]);
    expect(exitCode).toBe(0);
    expect(stdout).toContain("init");
    expect(stdout).toContain("handoff ");
    expect(stdout).toContain("handoff-pickup");
    expect(stdout).toContain("handoff-dig");
    expect(stdout).toContain("note");
    expect(stdout).toContain("status");
    expect(stdout).toContain("doctor");
  });

  it("doctor --json returns structured output", async () => {
    const { stdout, exitCode } = await run(["doctor", "--json"]);
    expect(exitCode).toBe(0);
    const checks = JSON.parse(stdout);
    expect(Array.isArray(checks)).toBe(true);
    expect(checks.length).toBeGreaterThan(0);
    expect(checks[0]).toHaveProperty("name");
    expect(checks[0]).toHaveProperty("status");
  });

  it("status --json returns structured output", async () => {
    const { stdout, exitCode } = await run(["status", "--json"]);
    expect(exitCode).toBe(0);
    const status = JSON.parse(stdout);
    expect(status).toHaveProperty("cassAvailable");
    expect(status).toHaveProperty("gitAvailable");
  });

  it("handoff with no flags creates auto-generated handoff", async () => {
    const { exitCode, stdout } = await run(["handoff", "--json"]);
    expect(exitCode).toBe(0);
    // Should contain auto-generated sitrep with branch info
    expect(stdout).toContain("Branch:");
  });

  it("handoff --dry-run outputs plan without writing", async () => {
    const { stdout, exitCode } = await run([
      "handoff",
      "--sitrep", "test sitrep",
      "--quickstart", "run tests",
      "--dry-run",
    ]);
    expect(exitCode).toBe(0);
    const data = JSON.parse(stdout);
    expect(data.dryRun).toBe(true);
    expect(data.sitrep).toBe("test sitrep");
  });

    it("handoff --list returns table when handoffs exist", async () => {
      const { exitCode } = await run(["handoff", "--list"]);
      expect(exitCode).toBe(0);
    });

    it("handoff --prompt includes stored instructions from latest pending handoff", async () => {
      await initGitRepo(tmpDir);

      const create = await run([
        "handoff",
        "--skip-session",
        "--sitrep", "test sitrep",
        "--quickstart", "run tests",
        "--instructions", "Deploy to staging first",
        "--json",
      ], tmpDir);
      expect(create.exitCode).toBe(0);

      const prompt = await run(["handoff", "--prompt", "codex"], tmpDir);
      expect(prompt.exitCode).toBe(0);
      expect(prompt.stdout).toContain("Your instructions: Deploy to staging first");
    });

    it("note writes a note and note --list returns it", async () => {
      await initGitRepo(tmpDir);

    const create = await run(
      ["note", "--content", "Remember the branch state", "--json"],
      tmpDir,
    );
    expect(create.exitCode).toBe(0);
    const createdNote = JSON.parse(create.stdout);
    expect(createdNote.content).toBe("Remember the branch state");
    expect(createdNote.git_branch).toBe("main");

    const list = await run(["note", "--list", "--json"], tmpDir);
    expect(list.exitCode).toBe(0);
    const notes = JSON.parse(list.stdout);
    expect(notes).toHaveLength(1);
    expect(notes[0]!.content).toBe("Remember the branch state");
  });
});
