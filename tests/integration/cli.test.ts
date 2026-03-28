import { describe, expect, it } from "bun:test";

const CLI = ["bun", "run", "src/index.ts"];

async function run(
  ...args: string[]
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  const proc = Bun.spawn([...CLI, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    cwd: process.cwd(),
  });
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  const exitCode = await proc.exited;
  return { stdout: stdout.trim(), stderr: stderr.trim(), exitCode };
}

describe("CLI integration", () => {
  it("prints version", async () => {
    const { stdout, exitCode } = await run("--version");
    expect(exitCode).toBe(0);
    expect(stdout).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("prints help with all commands", async () => {
    const { stdout, exitCode } = await run("--help");
    expect(exitCode).toBe(0);
    expect(stdout).toContain("init");
    expect(stdout).toContain("handoff ");
    expect(stdout).toContain("handoff-pickup");
    expect(stdout).toContain("handoff-dig");
    expect(stdout).toContain("status");
    expect(stdout).toContain("doctor");
  });

  it("doctor --json returns structured output", async () => {
    const { stdout, exitCode } = await run("doctor", "--json");
    expect(exitCode).toBe(0);
    const checks = JSON.parse(stdout);
    expect(Array.isArray(checks)).toBe(true);
    expect(checks.length).toBeGreaterThan(0);
    expect(checks[0]).toHaveProperty("name");
    expect(checks[0]).toHaveProperty("status");
  });

  it("status --json returns structured output", async () => {
    const { stdout, exitCode } = await run("status", "--json");
    expect(exitCode).toBe(0);
    const status = JSON.parse(stdout);
    expect(status).toHaveProperty("cassAvailable");
    expect(status).toHaveProperty("gitAvailable");
  });

  it("handoff fails without required options", async () => {
    const { exitCode } = await run("handoff");
    expect(exitCode).not.toBe(0);
  });

  it("handoff --dry-run outputs plan without writing", async () => {
    const { stdout, exitCode } = await run(
      "handoff",
      "--sitrep", "test sitrep",
      "--quickstart", "run tests",
      "--dry-run",
    );
    expect(exitCode).toBe(0);
    const data = JSON.parse(stdout);
    expect(data.dryRun).toBe(true);
    expect(data.sitrep).toBe("test sitrep");
  });

  it("handoff-pickup --list returns empty when no handoffs", async () => {
    const { stdout, exitCode } = await run("handoff-pickup", "--list");
    expect(exitCode).toBe(0);
  });
});
