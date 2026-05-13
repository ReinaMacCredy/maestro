import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  scanSlopCleanup,
  formatSlopCleanupLines,
} from "@/features/gc/usecases/slop-cleanup.usecase.js";

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), "slop-"));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

describe("scanSlopCleanup", () => {
  it("returns empty groups when filtering for warn or above on a clean repo", async () => {
    const r = await scanSlopCleanup({ projectRoot: dir, minSeverity: "warn" });
    expect(r.totalViolations).toBe(0);
    expect(r.filesAffected).toBe(0);
    expect(r.groups.length).toBe(0);
  });

  it("groups violations by file and counts severity buckets", async () => {
    await mkdir(join(dir, "src"), { recursive: true });
    await writeFile(
      join(dir, "src", "a.ts"),
      "console.log('one'); console.log('two');\n",
    );
    const r = await scanSlopCleanup({ projectRoot: dir });
    expect(r.totalViolations).toBeGreaterThan(0);
    expect(r.bySeverity.info).toBeGreaterThan(0);
    expect(r.groups[0]?.file).toContain("src/a.ts");
  });

  it("respects min-severity filter", async () => {
    await mkdir(join(dir, "src"), { recursive: true });
    await writeFile(join(dir, "src", "a.ts"), "console.log('x');\n");
    const r = await scanSlopCleanup({ projectRoot: dir, minSeverity: "warn" });
    expect(r.totalViolations).toBe(0);
    expect(r.bySeverity.warn).toBe(0);
  });

  it("formats lines including by-rule and top-offenders", async () => {
    await mkdir(join(dir, "src"), { recursive: true });
    await writeFile(join(dir, "src", "a.ts"), "console.log('x');\n");
    const r = await scanSlopCleanup({ projectRoot: dir });
    const lines = formatSlopCleanupLines(r);
    expect(lines.some((l) => l.startsWith("Slop scan:"))).toBe(true);
    expect(lines.some((l) => l.startsWith("By rule:"))).toBe(true);
  });
});
