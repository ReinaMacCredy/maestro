import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { initMaestro } from "../../../src/usecases/init.usecase.js";
import { mockConfig } from "../../helpers/mocks.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-init-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("initMaestro", () => {
  it("creates project .maestro directory", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });
    expect(result.scope).toBe("project");
    expect(result.created.length).toBeGreaterThan(0);

    const maestroDir = Bun.file(join(tmpDir, ".maestro"));
    // Directory created (ensureDir was called)
    expect(result.created.some((p) => p.includes(".maestro"))).toBe(true);
  });

  it("creates handoffs subdirectory for project scope", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });
    expect(result.created.some((p) => p.includes("handoffs"))).toBe(true);
  });

  it("does not overwrite existing config", async () => {
    let writeCount = 0;
    const config = mockConfig({
      exists: async () => true,
      write: async () => { writeCount++; },
    });
    await initMaestro(config, { global: false, dir: tmpDir });
    expect(writeCount).toBe(0);
  });

  it("writes config when none exists", async () => {
    let written = false;
    const config = mockConfig({
      exists: async () => false,
      write: async () => { written = true; },
    });
    await initMaestro(config, { global: false, dir: tmpDir });
    expect(written).toBe(true);
  });
});
