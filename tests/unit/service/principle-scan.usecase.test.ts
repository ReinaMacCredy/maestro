import { describe, expect, it } from "bun:test";
import {
  parseScanLine,
  principlesScan,
} from "@/service/principle-scan.usecase.js";
import type { Principle } from "@/types/principle.js";
import type { PrinciplesStorePort } from "@/repo/principles-store.port.js";
import type {
  ProcessRunResult,
  ProcessRunnerPort,
} from "@/repo/process-runner.port.js";

function principle(slug: string, scan_command = `scan-${slug}`): Principle {
  return {
    slug,
    rule: "r",
    rationale: "y",
    scan_command,
    fix_recipe: "f",
  };
}

function memStore(principles: Principle[]): PrinciplesStorePort {
  return {
    list: async () => principles,
    get: async (slug) => principles.find((p) => p.slug === slug),
    write: async () => {},
  };
}

function runnerFor(map: Record<string, ProcessRunResult>): ProcessRunnerPort {
  return {
    run: async (cmd) => map[cmd] ?? { stdout: "", stderr: "no-mock", exitCode: 0 },
  };
}

describe("parseScanLine", () => {
  it("parses file:line:message", () => {
    const f = parseScanLine("p", "src/foo.ts:42: bad thing happened");
    expect(f.file).toBe("src/foo.ts");
    expect(f.line).toBe(42);
    expect(f.message).toBe("bad thing happened");
    expect(f.kind).toBe("violation");
  });

  it("keeps raw message when no file:line prefix", () => {
    const f = parseScanLine("p", "freeform output");
    expect(f.file).toBeUndefined();
    expect(f.line).toBeUndefined();
    expect(f.message).toBe("freeform output");
    expect(f.kind).toBe("violation");
  });
});

describe("principlesScan", () => {
  it("returns no findings when every scan exits 0", async () => {
    const ps = [principle("a"), principle("b")];
    const report = await principlesScan({
      principlesStore: memStore(ps),
      processRunner: runnerFor({
        "scan-a": { stdout: "", stderr: "", exitCode: 0 },
        "scan-b": { stdout: "", stderr: "", exitCode: 0 },
      }),
      repoRoot: "/repo",
    });
    expect(report.principlesScanned).toBe(2);
    expect(report.findings).toEqual([]);
  });

  it("collects violations from stdout when scan exits non-zero", async () => {
    const ps = [principle("a")];
    const report = await principlesScan({
      principlesStore: memStore(ps),
      processRunner: runnerFor({
        "scan-a": {
          stdout: "src/x.ts:10: bad\nsrc/y.ts:3: also bad\n",
          stderr: "",
          exitCode: 1,
        },
      }),
      repoRoot: "/repo",
    });
    expect(report.findings).toHaveLength(2);
    expect(report.findings[0]).toMatchObject({
      principle_slug: "a",
      file: "src/x.ts",
      line: 10,
      kind: "violation",
    });
    expect(report.findings[1]).toMatchObject({
      file: "src/y.ts",
      line: 3,
    });
  });

  it("emits scan-error finding when exitCode != 0 and stdout is empty", async () => {
    const ps = [principle("a")];
    const report = await principlesScan({
      principlesStore: memStore(ps),
      processRunner: runnerFor({
        "scan-a": { stdout: "", stderr: "command not found", exitCode: 127 },
      }),
      repoRoot: "/repo",
    });
    expect(report.findings).toHaveLength(1);
    expect(report.findings[0]?.kind).toBe("scan-error");
    expect(report.findings[0]?.message).toContain("exit 127");
    expect(report.findings[0]?.message).toContain("command not found");
  });

  it("emits scan-error when scan_command is empty", async () => {
    const ps = [principle("a", "   ")];
    const report = await principlesScan({
      principlesStore: memStore(ps),
      processRunner: runnerFor({}),
      repoRoot: "/repo",
    });
    expect(report.findings).toHaveLength(1);
    expect(report.findings[0]?.kind).toBe("scan-error");
    expect(report.findings[0]?.message).toMatch(/empty scan_command/);
  });

  it("respects --only filter via input.only", async () => {
    const ps = [principle("a"), principle("b")];
    const report = await principlesScan(
      {
        principlesStore: memStore(ps),
        processRunner: runnerFor({
          "scan-a": { stdout: "src/x.ts:1: bad", stderr: "", exitCode: 1 },
        }),
        repoRoot: "/repo",
      },
      { only: ["a"] },
    );
    expect(report.principlesScanned).toBe(1);
    expect(report.findings.every((f) => f.principle_slug === "a")).toBe(true);
  });

  it("falls back to raw line for unparseable stdout", async () => {
    const ps = [principle("a")];
    const report = await principlesScan({
      principlesStore: memStore(ps),
      processRunner: runnerFor({
        "scan-a": { stdout: "freeform finding\n", stderr: "", exitCode: 1 },
      }),
      repoRoot: "/repo",
    });
    expect(report.findings).toHaveLength(1);
    expect(report.findings[0]?.message).toBe("freeform finding");
    expect(report.findings[0]?.file).toBeUndefined();
  });

  it("scans zero principles when store is empty", async () => {
    const report = await principlesScan({
      principlesStore: memStore([]),
      processRunner: runnerFor({}),
      repoRoot: "/repo",
    });
    expect(report.principlesScanned).toBe(0);
    expect(report.findings).toEqual([]);
  });

  it("scan-error uses fallback message when stderr is also empty", async () => {
    const ps = [principle("a")];
    const report = await principlesScan({
      principlesStore: memStore(ps),
      processRunner: runnerFor({
        "scan-a": { stdout: "", stderr: "", exitCode: 2 },
      }),
      repoRoot: "/repo",
    });
    expect(report.findings).toHaveLength(1);
    expect(report.findings[0]?.kind).toBe("scan-error");
    expect(report.findings[0]?.message).toContain("empty stdout");
  });

  it("passes repoRoot as cwd to the runner", async () => {
    let receivedCwd: string | undefined;
    const runner: ProcessRunnerPort = {
      run: async (_cmd, opts) => {
        receivedCwd = opts?.cwd;
        return { stdout: "", stderr: "", exitCode: 0 };
      },
    };
    await principlesScan({
      principlesStore: memStore([principle("a")]),
      processRunner: runner,
      repoRoot: "/repo/here",
    });
    expect(receivedCwd).toBe("/repo/here");
  });
});
