import { describe, expect, it } from "bun:test";
import { BunProcessRunner } from "@/v2/repo/bun-process-runner.adapter.js";

describe("BunProcessRunner", () => {
  it("captures stdout from a successful command", async () => {
    const runner = new BunProcessRunner();
    const result = await runner.run("printf hello");
    expect(result.stdout).toBe("hello");
    expect(result.exitCode).toBe(0);
  });

  it("captures stderr and a non-zero exit code", async () => {
    const runner = new BunProcessRunner();
    const result = await runner.run("printf nope >&2; exit 7");
    expect(result.stderr).toBe("nope");
    expect(result.exitCode).toBe(7);
  });

  it("respects cwd", async () => {
    const runner = new BunProcessRunner();
    const result = await runner.run("pwd", { cwd: "/" });
    expect(result.stdout.trim()).toBe("/");
  });
});
