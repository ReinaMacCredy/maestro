import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { mockRatchetStore } from "../../../../helpers/mocks.js";
import { checkRatchet } from "@/features/ratchet/usecases/ratchet-check.usecase.js";

describe("checkRatchet", () => {
  let dir: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-rcheck-"));
    await mkdir(join(dir, "src"), { recursive: true });
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it("passes when no violations found", async () => {
    await writeFile(join(dir, "src", "index.ts"), "import bun from 'bun';");
    const store = mockRatchetStore({
      assertions: [
        { id: "r1", correctionId: "c1", rule: "no npm", check: "npm install", createdAt: "2026-04-05" },
      ],
    });

    const result = await checkRatchet(store, dir);
    expect(result.passed).toBe(true);
    expect(result.passCount).toBe(1);
  });

  it("fails when violations found", async () => {
    await writeFile(join(dir, "src", "setup.sh"), "npm install express");
    const store = mockRatchetStore({
      assertions: [
        { id: "r1", correctionId: "c1", rule: "no npm", check: "npm install", createdAt: "2026-04-05" },
      ],
    });

    const result = await checkRatchet(store, dir);
    expect(result.passed).toBe(false);
    expect(result.results[0]!.passed).toBe(false);
  });

  it("returns empty results for empty suite", async () => {
    const store = mockRatchetStore();
    const result = await checkRatchet(store, dir);
    expect(result.totalCount).toBe(0);
    expect(result.passed).toBe(true);
  });

  it("writes baseline after check", async () => {
    const store = mockRatchetStore({
      assertions: [
        { id: "r1", correctionId: "c1", rule: "test", check: "NONEXISTENT_PATTERN_XYZ", createdAt: "2026-04-05" },
      ],
    });

    await checkRatchet(store, dir);
    const baseline = await store.getBaseline();
    expect(baseline?.passCount).toBe(1);
  });
});
