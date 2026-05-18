import { afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, readFile, rm, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  BUILD_TIMEOUT_MS,
  buildCompiledCli,
  initGitRepo,
  runCompiled,
} from "../helpers/run-compiled-cli.js";

let tmpDir: string;

beforeAll(buildCompiledCli, BUILD_TIMEOUT_MS);

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-spec-e2e-"));
  await initGitRepo(tmpDir);
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("maestro spec new + spec validate", () => {
  it("creates a skeleton spec file with valid frontmatter", async () => {
    const result = await runCompiled(["spec", "new", "demo-feature", "--title", "Demo"], tmpDir);
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain(".maestro/specs/demo-feature.md");
    const text = await readFile(join(tmpDir, ".maestro/specs/demo-feature.md"), "utf8");
    expect(text).toContain("slug: demo-feature");
    expect(text).toContain("acceptance_criteria:");
    expect(text).toContain("risk_class: low");
    expect(text).toContain("mode: light");
  });

  it("validate exits 0 on a freshly-created spec", async () => {
    await runCompiled(["spec", "new", "good-spec"], tmpDir);
    const result = await runCompiled(
      ["spec", "validate", ".maestro/specs/good-spec.md"],
      tmpDir,
    );
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("is valid");
  });

  it("validate exits 1 with a field-named error on a broken spec", async () => {
    await mkdir(join(tmpDir, ".maestro/specs"), { recursive: true });
    const broken = `---\nslug: bad\nrisk_class: low\nmode: light\nwork_type: maintenance\n---\nbody\n`;
    await writeFile(join(tmpDir, ".maestro/specs/bad.md"), broken);
    const result = await runCompiled(
      ["spec", "validate", ".maestro/specs/bad.md"],
      tmpDir,
    );
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("acceptance_criteria");
  });

  it("spec new rejects duplicate slug with exit 1", async () => {
    await runCompiled(["spec", "new", "dup"], tmpDir);
    const second = await runCompiled(["spec", "new", "dup"], tmpDir);
    expect(second.exitCode).toBe(1);
    expect(second.stderr).toContain("already exists");
  });

  it("spec new rejects invalid slug with exit 1", async () => {
    const result = await runCompiled(["spec", "new", "Bad_Slug"], tmpDir);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("Invalid spec slug");
  });
});
