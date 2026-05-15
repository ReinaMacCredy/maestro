import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdir, mkdtemp, readdir, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setupBootstrap } from "@/v2/service/setup-bootstrap.usecase.js";

const V2_DIRS = [
  ".maestro/tasks",
  ".maestro/plans",
  ".maestro/evidence",
  ".maestro/runs",
  "docs/principles",
];

describe("setupBootstrap", () => {
  let root: string;

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), "v2-setup-bootstrap-"));
  });

  afterEach(async () => {
    await rm(root, { recursive: true, force: true });
  });

  it("creates every missing v2 directory with a .gitkeep stub", async () => {
    const result = await setupBootstrap({ repoRoot: root });
    expect(result.created.length).toBe(V2_DIRS.length);
    expect(result.skipped.length).toBe(0);
    for (const rel of V2_DIRS) {
      const abs = join(root, rel);
      expect((await stat(abs)).isDirectory()).toBe(true);
      const entries = await readdir(abs);
      expect(entries).toContain(".gitkeep");
    }
  });

  it("is idempotent: existing directories are skipped on a re-run", async () => {
    await setupBootstrap({ repoRoot: root });
    const result = await setupBootstrap({ repoRoot: root });
    expect(result.created.length).toBe(0);
    expect(result.skipped.length).toBe(V2_DIRS.length);
  });

  it("leaves existing files in a present directory untouched", async () => {
    await mkdir(join(root, ".maestro/tasks"), { recursive: true });
    const result = await setupBootstrap({ repoRoot: root });
    expect(result.skipped).toContain(".maestro/tasks");
    expect(result.created).not.toContain(".maestro/tasks");
  });
});
