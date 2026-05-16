import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  migrateCorrections,
  renderLegacyPrinciple,
} from "@/service/migrate-corrections.usecase.js";

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), "migrate-corrections-"));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

async function seed(name: string, body: object): Promise<void> {
  const correctionsDir = join(dir, ".maestro/memory/corrections");
  await mkdir(correctionsDir, { recursive: true });
  await writeFile(join(correctionsDir, name), JSON.stringify(body), "utf8");
}

const SAMPLE = {
  id: "correction-001",
  rule: "Never `cat .maestro/...` from inside maestro source.",
  source: "manual-edit",
  trigger: {
    keywords: ["cat", "shell"],
    fileGlobs: ["src/**"],
  },
  severity: "hard" as const,
  createdAt: "2026-04-01T00:00:00Z",
  updatedAt: "2026-04-01T00:00:00Z",
};

describe("renderLegacyPrinciple", () => {
  it("includes all 4 section headers", () => {
    const md = renderLegacyPrinciple(SAMPLE);
    expect(md).toContain("# correction-001");
    expect(md).toContain("## Rule");
    expect(md).toContain("## Rationale");
    expect(md).toContain("## Scan Command");
    expect(md).toContain("## Fix Recipe");
  });

  it("notes hard-severity rationale", () => {
    const md = renderLegacyPrinciple(SAMPLE);
    expect(md).toContain("hard correction");
  });

  it("notes soft-severity rationale", () => {
    const md = renderLegacyPrinciple({ ...SAMPLE, severity: "soft" });
    expect(md).toContain("soft correction");
  });

  it("renders trigger keywords + fileGlobs as scan-command comments", () => {
    const md = renderLegacyPrinciple(SAMPLE);
    expect(md).toContain("keywords: cat, shell");
    expect(md).toContain("fileGlobs: src/**");
  });
});

describe("migrateCorrections", () => {
  it("no-op when source dir is missing (not an error)", async () => {
    const result = await migrateCorrections({ repoRoot: dir });
    expect(result.missing_source).toBe(true);
    expect(result.scanned).toBe(0);
    expect(result.migrated).toEqual([]);
  });

  it("migrates a single correction to docs/principles/legacy/<id>.md", async () => {
    await seed("correction-001.json", SAMPLE);
    const result = await migrateCorrections({ repoRoot: dir });
    expect(result.scanned).toBe(1);
    expect(result.migrated).toEqual(["correction-001"]);
    expect(result.skipped).toEqual([]);
    const md = await readFile(
      join(dir, "docs/principles/legacy/correction-001.md"),
      "utf8",
    );
    expect(md).toContain("# correction-001");
    expect(md).toContain("Never `cat");
  });

  it("skips correction whose destination already exists when overwrite=false", async () => {
    await seed("correction-001.json", SAMPLE);
    await mkdir(join(dir, "docs/principles/legacy"), { recursive: true });
    await writeFile(
      join(dir, "docs/principles/legacy/correction-001.md"),
      "preexisting\n",
      "utf8",
    );
    const result = await migrateCorrections({ repoRoot: dir });
    expect(result.migrated).toEqual([]);
    expect(result.skipped).toEqual(["correction-001"]);
    const md = await readFile(
      join(dir, "docs/principles/legacy/correction-001.md"),
      "utf8",
    );
    expect(md).toBe("preexisting\n");
  });

  it("overwrites when overwrite=true", async () => {
    await seed("correction-001.json", SAMPLE);
    await mkdir(join(dir, "docs/principles/legacy"), { recursive: true });
    await writeFile(
      join(dir, "docs/principles/legacy/correction-001.md"),
      "preexisting\n",
      "utf8",
    );
    const result = await migrateCorrections({ repoRoot: dir }, { overwrite: true });
    expect(result.migrated).toEqual(["correction-001"]);
    const md = await readFile(
      join(dir, "docs/principles/legacy/correction-001.md"),
      "utf8",
    );
    expect(md).toContain("# correction-001");
  });

  it("skips malformed JSON files instead of throwing", async () => {
    const correctionsDir = join(dir, ".maestro/memory/corrections");
    await mkdir(correctionsDir, { recursive: true });
    await writeFile(join(correctionsDir, "bad.json"), "{this is not json", "utf8");
    const result = await migrateCorrections({ repoRoot: dir });
    expect(result.scanned).toBe(1);
    expect(result.migrated).toEqual([]);
    expect(result.skipped).toEqual(["bad.json"]);
  });

  it("skips records missing required fields", async () => {
    await seed("almost.json", { id: "almost" });
    const result = await migrateCorrections({ repoRoot: dir });
    expect(result.scanned).toBe(1);
    expect(result.migrated).toEqual([]);
    expect(result.skipped).toEqual(["almost.json"]);
  });

  it("handles multiple corrections", async () => {
    await seed("a.json", { ...SAMPLE, id: "a" });
    await seed("b.json", { ...SAMPLE, id: "b" });
    const result = await migrateCorrections({ repoRoot: dir });
    expect(result.scanned).toBe(2);
    expect(result.migrated.sort()).toEqual(["a", "b"]);
  });

  it("idempotent: second run skips all", async () => {
    await seed("a.json", { ...SAMPLE, id: "a" });
    await migrateCorrections({ repoRoot: dir });
    const second = await migrateCorrections({ repoRoot: dir });
    expect(second.migrated).toEqual([]);
    expect(second.skipped).toEqual(["a"]);
  });
});
