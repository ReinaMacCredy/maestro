import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  scanSlopCleanup,
  formatSlopCleanupLines,
} from "@/features/gc/usecases/slop-cleanup.usecase.js";
import type { PrinciplesStorePort } from "@/v2/repo/principles-store.port.js";
import type { ProcessRunnerPort } from "@/v2/repo/process-runner.port.js";
import type { Principle } from "@/v2/types/principle.js";

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), "slop-"));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

const emptyPrinciples: PrinciplesStorePort = {
  list: async () => [],
  get: async () => undefined,
  exists: async () => false,
  write: async () => {},
};

const noopRunner: ProcessRunnerPort = {
  run: async () => ({ stdout: "", stderr: "", exitCode: 0 }),
};

describe("scanSlopCleanup", () => {
  it("returns empty groups when filtering for warn or above on a clean repo", async () => {
    const r = await scanSlopCleanup({
      projectRoot: dir,
      minSeverity: "warn",
      principlesStore: emptyPrinciples,
      processRunner: noopRunner,
    });
    expect(r.totalViolations).toBe(0);
    expect(r.filesAffected).toBe(0);
    expect(r.groups.length).toBe(0);
    expect(r.principleFindings).toEqual([]);
  });

  it("groups violations by file and counts severity buckets", async () => {
    await mkdir(join(dir, "src"), { recursive: true });
    await writeFile(
      join(dir, "src", "a.ts"),
      "console.log('one'); console.log('two');\n",
    );
    const r = await scanSlopCleanup({
      projectRoot: dir,
      principlesStore: emptyPrinciples,
      processRunner: noopRunner,
    });
    expect(r.totalViolations).toBeGreaterThan(0);
    expect(r.bySeverity.info).toBeGreaterThan(0);
    expect(r.groups[0]?.file).toContain("src/a.ts");
  });

  it("respects min-severity filter", async () => {
    await mkdir(join(dir, "src"), { recursive: true });
    await writeFile(join(dir, "src", "a.ts"), "console.log('x');\n");
    const r = await scanSlopCleanup({
      projectRoot: dir,
      minSeverity: "warn",
      principlesStore: emptyPrinciples,
      processRunner: noopRunner,
    });
    expect(r.totalViolations).toBe(0);
    expect(r.bySeverity.warn).toBe(0);
  });

  it("formats lines including by-rule and top-offenders", async () => {
    await mkdir(join(dir, "src"), { recursive: true });
    await writeFile(join(dir, "src", "a.ts"), "console.log('x');\n");
    const r = await scanSlopCleanup({
      projectRoot: dir,
      principlesStore: emptyPrinciples,
      processRunner: noopRunner,
    });
    const lines = formatSlopCleanupLines(r);
    expect(lines.some((l) => l.startsWith("Slop scan:"))).toBe(true);
    expect(lines.some((l) => l.startsWith("By rule:"))).toBe(true);
  });

  it("includes principle scan findings in result", async () => {
    const principle: Principle = {
      slug: "prefer-shared-utils",
      rule: "r",
      rationale: "y",
      scan_command: "scan-cmd",
      fix_recipe: "f",
    };
    const principlesStore: PrinciplesStorePort = {
      list: async () => [principle],
      get: async () => principle,
      exists: async () => true,
      write: async () => {},
    };
    const processRunner: ProcessRunnerPort = {
      run: async () => ({
        stdout: "src/x.ts:7: duplicate helper detected",
        stderr: "",
        exitCode: 1,
      }),
    };
    const r = await scanSlopCleanup({
      projectRoot: dir,
      principlesStore,
      processRunner,
    });
    expect(r.principleFindings).toHaveLength(1);
    expect(r.principleFindings[0]?.principle_slug).toBe("prefer-shared-utils");
    expect(r.byRule["prefer-shared-utils"]).toBe(1);
    expect(r.bySeverity.error).toBeGreaterThanOrEqual(1);
    const xGroup = r.groups.find((g) => g.file === "src/x.ts");
    expect(xGroup?.ruleIds).toContain("prefer-shared-utils");
  });

  it("merges principle findings with arch violations under one file group", async () => {
    await mkdir(join(dir, "src"), { recursive: true });
    await writeFile(join(dir, "src/x.ts"), "console.log('x');\n");
    const principle: Principle = {
      slug: "my-rule",
      rule: "r",
      rationale: "y",
      scan_command: "scan-cmd",
      fix_recipe: "f",
    };
    const principlesStore: PrinciplesStorePort = {
      list: async () => [principle],
      get: async () => principle,
      exists: async () => true,
      write: async () => {},
    };
    const processRunner: ProcessRunnerPort = {
      run: async () => ({
        stdout: "src/x.ts:1: same file as arch lint",
        stderr: "",
        exitCode: 1,
      }),
    };
    const r = await scanSlopCleanup({
      projectRoot: dir,
      principlesStore,
      processRunner,
    });
    const group = r.groups.find((g) => g.file === "src/x.ts");
    expect(group).toBeDefined();
    expect(group!.ruleIds).toContain("my-rule");
    expect(group!.ruleIds.length).toBeGreaterThan(1);
  });

  it("counts scan-error findings separately in formatted output", async () => {
    const principle: Principle = {
      slug: "broken-scan",
      rule: "r",
      rationale: "y",
      scan_command: "scan-cmd",
      fix_recipe: "f",
    };
    const principlesStore: PrinciplesStorePort = {
      list: async () => [principle],
      get: async () => principle,
      exists: async () => true,
      write: async () => {},
    };
    const processRunner: ProcessRunnerPort = {
      run: async () => ({ stdout: "", stderr: "boom", exitCode: 127 }),
    };
    const r = await scanSlopCleanup({
      projectRoot: dir,
      principlesStore,
      processRunner,
    });
    expect(r.principleFindings[0]?.kind).toBe("scan-error");
    const lines = formatSlopCleanupLines(r);
    expect(lines.some((l) => l.includes("principle findings: 1 (1 scan-error)"))).toBe(true);
  });
});
