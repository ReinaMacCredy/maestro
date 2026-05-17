import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setupCheck } from "@/service/setup-check.usecase.js";

describe("setupCheck", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-setup-check-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("flags every required directory as missing on an empty repo and ok=false", async () => {
    const report = await setupCheck({ repoRoot: root });
    expect(report.ok).toBe(false);
    const dirEntries = report.entries.filter((e) => e.kind === "directory");
    expect(dirEntries.length).toBe(5);
    for (const entry of dirEntries) expect(entry.status).toBe("missing");
  });

  it("reports ok for present directories", async () => {
    for (const rel of [
      ".maestro/tasks",
      ".maestro/missions",
      ".maestro/evidence",
      ".maestro/runs",
      "docs/principles",
    ]) {
      await mkdir(join(root, rel), { recursive: true });
    }
    await writeFile(join(root, "docs/principles/demo.md"), "# demo\n## Rule\n\nx\n## Rationale\n\nx\n## Scan Command\n\n! rg x\n## Fix Recipe\n\nx\n", "utf8");
    await writeFile(join(root, ".maestro/config.yaml"), "version: 1\n", "utf8");
    const report = await setupCheck({ repoRoot: root });
    expect(report.ok).toBe(true);
    expect(report.entries.find((e) => e.kind === "pack")?.status).toBe("ok");
  });

  it("warns (but stays ok) when principles directory is present but empty", async () => {
    for (const rel of [
      ".maestro/tasks",
      ".maestro/missions",
      ".maestro/evidence",
      ".maestro/runs",
      "docs/principles",
    ]) {
      await mkdir(join(root, rel), { recursive: true });
    }
    const report = await setupCheck({ repoRoot: root });
    const pack = report.entries.find((e) => e.kind === "pack");
    expect(pack?.status).toBe("warn");
    expect(report.ok).toBe(true);
  });

  it("warns (not missing) when only .maestro/config.yaml is absent", async () => {
    for (const rel of [
      ".maestro/tasks",
      ".maestro/missions",
      ".maestro/evidence",
      ".maestro/runs",
      "docs/principles",
    ]) {
      await mkdir(join(root, rel), { recursive: true });
    }
    await writeFile(join(root, "docs/principles/x.md"), "# x\n## Rule\n\nx\n## Rationale\n\nx\n## Scan Command\n\n! rg x\n## Fix Recipe\n\nx\n", "utf8");
    const report = await setupCheck({ repoRoot: root });
    const config = report.entries.find((e) => e.path === ".maestro/config.yaml");
    expect(config?.status).toBe("warn");
  });
});
