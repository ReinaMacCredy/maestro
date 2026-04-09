import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { initMaestro } from "@/usecases/init.usecase.js";
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
    expect(result.bootstrapGenerated).toBe(true);

    const maestroDir = Bun.file(join(tmpDir, ".maestro"));
    // Directory created (ensureDir was called)
    expect(result.created.some((p) => p.includes(".maestro"))).toBe(true);
  });

  it("creates handoffs subdirectory for project scope", async () => {
    const config = mockConfig();
    const result = await initMaestro(config, { global: false, dir: tmpDir });
    expect(result.created.some((p) => p.includes("handoffs"))).toBe(true);
    expect(result.created).toContain(join(tmpDir, ".maestro", "bootstrap", "services.yaml"));
    expect(result.created).toContain(join(tmpDir, ".maestro", "AGENTS.md"));
  });

  it("does not overwrite existing config", async () => {
    let writeCount = 0;
    const config = mockConfig({
      exists: async () => true,
      write: async () => { writeCount++; },
    });
    const result = await initMaestro(config, { global: false, dir: tmpDir });
    expect(writeCount).toBe(0);
    expect(result.skipped).toContain(join(tmpDir, ".maestro", "config.yaml"));
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

  it("skips existing bootstrap files by default", async () => {
    const config = mockConfig();
    const agentsPath = join(tmpDir, ".maestro", "AGENTS.md");
    await Bun.write(agentsPath, "keep me\n");

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.skipped).toContain(agentsPath);
    expect(await readFile(agentsPath, "utf8")).toBe("keep me\n");
  });

  it("replaces existing bootstrap files when confirmed", async () => {
    const config = mockConfig();
    const agentsPath = join(tmpDir, ".maestro", "AGENTS.md");
    await Bun.write(agentsPath, "old content\n");

    const result = await initMaestro(config, {
      global: false,
      dir: tmpDir,
      confirmReplace: async (path) => path === agentsPath,
    });

    expect(result.created).toContain(agentsPath);
    expect(await readFile(agentsPath, "utf8")).toContain("Maestro Project Bootstrap");
  });

  it("keeps global init minimal", async () => {
    let writeCount = 0;
    const config = mockConfig({
      exists: async () => false,
      write: async () => { writeCount++; },
    });

    const result = await initMaestro(config, { global: true, dir: tmpDir });

    expect(result.scope).toBe("global");
    expect(result.bootstrapGenerated).toBe(false);
    expect(writeCount).toBe(1);
    expect(result.created.some((path) => path.includes("bootstrap"))).toBe(false);
  });

  it("does not re-report existing directories on rerun", async () => {
    const config = mockConfig();

    await initMaestro(config, { global: false, dir: tmpDir });
    const second = await initMaestro(config, { global: false, dir: tmpDir });

    expect(second.created).toEqual([]);
    expect(second.skipped).toContain(join(tmpDir, ".maestro", "config.yaml"));
  });

  it("migrates legacy .factory bootstrap files into .maestro", async () => {
    const config = mockConfig();
    await mkdir(join(tmpDir, ".factory", "library"), { recursive: true });
    await writeFile(
      join(tmpDir, ".factory", "services.yaml"),
      "commands:\n  test: echo legacy-test\nservices: {}\n",
    );
    await writeFile(
      join(tmpDir, ".factory", "library", "architecture.md"),
      "# Legacy Architecture\n",
    );

    const result = await initMaestro(config, { global: false, dir: tmpDir });

    expect(result.created).toContain(join(tmpDir, ".maestro", "bootstrap", "services.yaml"));
    expect(await readFile(join(tmpDir, ".maestro", "bootstrap", "services.yaml"), "utf8")).toContain(
      "legacy-test",
    );
    expect(await readFile(join(tmpDir, ".maestro", "bootstrap", "library", "architecture.md"), "utf8")).toContain(
      "# Legacy Architecture",
    );
  });

  it("scaffolds gitignore entries for runtime state", async () => {
    const config = mockConfig();

    await initMaestro(config, { global: false, dir: tmpDir });

    const gitignore = await readFile(join(tmpDir, ".gitignore"), "utf8");
    expect(gitignore).toContain(".maestro/handoffs/");
    expect(gitignore).toContain(".maestro/missions/");
    expect(gitignore).toContain(".maestro/sessions/");
  });

  it("rejects symlinked .maestro paths that escape the project root", async () => {
    const config = mockConfig();
    const outsideDir = join(tmpDir, "outside");

    await mkdir(outsideDir, { recursive: true });
    await symlink(outsideDir, join(tmpDir, ".maestro"));

    await expect(initMaestro(config, { global: false, dir: tmpDir })).rejects.toThrow(
      "Refusing to initialize through symlinked path",
    );
  });

  it("rejects symlinked project roots", async () => {
    const config = mockConfig();
    const realRoot = join(tmpDir, "real-project");
    const linkRoot = join(tmpDir, "project-link");

    await mkdir(realRoot, { recursive: true });
    await symlink(realRoot, linkRoot);

    await expect(initMaestro(config, { global: false, dir: linkRoot })).rejects.toThrow(
      "Refusing to initialize through symlinked project root",
    );
  });
});
