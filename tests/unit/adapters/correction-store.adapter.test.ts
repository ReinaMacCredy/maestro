import { describe, it, expect, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { FsCorrectionStoreAdapter } from "../../../src/adapters/correction-store.adapter.js";
import type { CreateCorrectionInput } from "../../../src/domain/memory-types.js";

describe("FsCorrectionStoreAdapter", () => {
  let dir: string;
  let store: FsCorrectionStoreAdapter;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), "maestro-corr-"));
    store = new FsCorrectionStoreAdapter(dir);
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  const input: CreateCorrectionInput = {
    rule: "use bun not npm",
    source: "used npm install in setup script",
    trigger: { keywords: ["package", "install", "npm"], fileGlobs: ["*.sh"] },
    severity: "hard",
  };

  it("creates and retrieves a correction", async () => {
    const created = await store.create(input);
    expect(created.id).toBeTruthy();
    expect(created.rule).toBe("use bun not npm");
    expect(created.severity).toBe("hard");

    const retrieved = await store.get(created.id);
    expect(retrieved).toEqual(created);
  });

  it("lists all corrections sorted by date descending", async () => {
    const c1 = await store.create(input);
    const c2 = await store.create({ ...input, rule: "no fire-and-forget" });

    const all = await store.list();
    expect(all.length).toBe(2);
    expect(all[0].id).toBe(c2.id);
    expect(all[1].id).toBe(c1.id);
  });

  it("searches by keyword", async () => {
    await store.create(input);
    await store.create({
      rule: "prefer interface",
      source: "used type for object shape",
      trigger: { keywords: ["typescript", "type"], fileGlobs: ["*.ts"] },
      severity: "soft",
    });

    const results = await store.search({ keywords: ["npm"] });
    expect(results.length).toBe(1);
    expect(results[0].rule).toBe("use bun not npm");
  });

  it("searches by text in rule/source", async () => {
    await store.create(input);
    const results = await store.search({ text: "setup script" });
    expect(results.length).toBe(1);
    expect(results[0].source).toContain("setup script");
  });

  it("updates a correction", async () => {
    const created = await store.create(input);
    const updated = await store.update(created.id, { severity: "soft" });
    expect(updated?.severity).toBe("soft");
    expect(updated?.rule).toBe("use bun not npm");

    const retrieved = await store.get(created.id);
    expect(retrieved?.severity).toBe("soft");
  });

  it("returns undefined when updating non-existent id", async () => {
    const result = await store.update("nonexistent", { severity: "soft" });
    expect(result).toBeUndefined();
  });

  it("removes a correction", async () => {
    const created = await store.create(input);
    const removed = await store.remove(created.id);
    expect(removed).toBe(true);

    const retrieved = await store.get(created.id);
    expect(retrieved).toBeUndefined();

    const all = await store.list();
    expect(all.length).toBe(0);
  });

  it("returns false when removing non-existent id", async () => {
    const result = await store.remove("nonexistent");
    expect(result).toBe(false);
  });

  it("returns empty list from empty store", async () => {
    const all = await store.list();
    expect(all).toEqual([]);
  });

  it("returns undefined for non-existent get", async () => {
    const result = await store.get("nonexistent");
    expect(result).toBeUndefined();
  });

  it("builds keyword index after create", async () => {
    await store.create(input);
    const { readJson } = await import("../../../src/lib/fs.js");
    const index = await readJson<{ keywords: Record<string, string[]> }>(
      join(dir, ".maestro", "memory", "corrections", "_index.json"),
    );
    expect(index?.keywords["package"]).toBeTruthy();
    expect(index?.keywords["npm"]).toBeTruthy();
  });
});
