import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setupCheck } from "@/service/setup-check.usecase.js";

const BASE_DIRS = [
  ".maestro/tasks",
  ".maestro/missions",
  ".maestro/evidence",
  ".maestro/runs",
  "docs/principles",
] as const;

const PRINCIPLE_STUB = "# x\n## Rule\n\nx\n## Rationale\n\nx\n## Scan Command\n\n! rg x\n## Fix Recipe\n\nx\n";

async function seedBaseDirs(root: string, extras: readonly string[] = []): Promise<void> {
  for (const rel of [...BASE_DIRS, ...extras]) {
    await mkdir(join(root, rel), { recursive: true });
  }
}

describe("setupCheck", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "setup-check-"));
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
    await seedBaseDirs(root);
    await writeFile(join(root, "docs/principles/demo.md"), PRINCIPLE_STUB, "utf8");
    await writeFile(join(root, ".maestro/config.yaml"), "version: 1\n", "utf8");
    const report = await setupCheck({ repoRoot: root });
    expect(report.ok).toBe(true);
    expect(report.entries.find((e) => e.kind === "pack")?.status).toBe("ok");
  });

  it("warns (but stays ok) when principles directory is present but empty", async () => {
    await seedBaseDirs(root);
    const report = await setupCheck({ repoRoot: root });
    const pack = report.entries.find((e) => e.kind === "pack");
    expect(pack?.status).toBe("warn");
    expect(report.ok).toBe(true);
  });

  it("warns (not missing) when only .maestro/config.yaml is absent", async () => {
    await seedBaseDirs(root);
    await writeFile(join(root, "docs/principles/x.md"), PRINCIPLE_STUB, "utf8");
    const report = await setupCheck({ repoRoot: root });
    const config = report.entries.find((e) => e.path === ".maestro/config.yaml");
    expect(config?.status).toBe("warn");
  });

  it("warns when leftover .maestro/plans/ from a pre-0.102.0 layout is present", async () => {
    await seedBaseDirs(root, [".maestro/plans"]);
    const report = await setupCheck({ repoRoot: root });
    const plans = report.entries.find((e) => e.path === ".maestro/plans");
    expect(plans?.status).toBe("warn");
    expect(plans?.detail).toContain("0.100.x");
    expect(report.ok).toBe(true);
  });

  it("warns when leftover .maestro/missions.tmp/ from a pre-0.102.0 layout is present", async () => {
    await seedBaseDirs(root, [".maestro/missions.tmp"]);
    const report = await setupCheck({ repoRoot: root });
    const missionsTmp = report.entries.find((e) => e.path === ".maestro/missions.tmp");
    expect(missionsTmp?.status).toBe("warn");
    expect(report.ok).toBe(true);
  });
});
