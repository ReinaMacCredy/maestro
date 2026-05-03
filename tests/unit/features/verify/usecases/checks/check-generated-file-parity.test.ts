import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { checkGeneratedFileParity } from "@/features/verify/usecases/checks/check-generated-file-parity.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "generated-parity-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("checkGeneratedFileParity", () => {
  it("no package.json — empty findings", async () => {
    const findings = await checkGeneratedFileParity(tmpDir);
    expect(findings).toEqual([]);
  });

  it("package.json with no scripts — empty findings", async () => {
    await writeFile(join(tmpDir, "package.json"), JSON.stringify({ name: "test" }));
    const findings = await checkGeneratedFileParity(tmpDir);
    expect(findings).toEqual([]);
  });

  it("package.json with no sync: scripts — empty findings", async () => {
    await writeFile(
      join(tmpDir, "package.json"),
      JSON.stringify({ scripts: { build: "bun run build", test: "bun test" } }),
    );
    const findings = await checkGeneratedFileParity(tmpDir);
    expect(findings).toEqual([]);
  });

  it("repo with sync:bundled-skills script — emits info finding listing it", async () => {
    await writeFile(
      join(tmpDir, "package.json"),
      JSON.stringify({
        scripts: {
          build: "bun run build",
          "sync:bundled-skills": "bun scripts/sync-bundled-skills.ts",
        },
      }),
    );
    const findings = await checkGeneratedFileParity(tmpDir);
    expect(findings).toHaveLength(1);
    expect(findings[0].check).toBe("generated-file-parity");
    expect(findings[0].severity).toBe("info");
    expect(findings[0].details).toMatch(/sync:bundled-skills/);
    expect(findings[0].details).toMatch(/--regenerate/);
  });

  it("multiple sync: scripts — all listed in one finding", async () => {
    await writeFile(
      join(tmpDir, "package.json"),
      JSON.stringify({
        scripts: {
          "sync:bundled-skills": "bun scripts/sync-bundled-skills.ts",
          "sync:built-in-skills": "bun scripts/sync-built-in-skills.ts",
          build: "bun build",
        },
      }),
    );
    const findings = await checkGeneratedFileParity(tmpDir);
    expect(findings).toHaveLength(1);
    expect(findings[0].details).toMatch(/sync:bundled-skills/);
    expect(findings[0].details).toMatch(/sync:built-in-skills/);
  });
});
