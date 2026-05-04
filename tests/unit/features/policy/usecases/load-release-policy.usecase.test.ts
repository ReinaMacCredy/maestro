import { describe, expect, it, beforeEach, afterEach } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { loadReleasePolicy } from "@/features/policy/usecases/load-release-policy.usecase.js";
import { MaestroError } from "@/shared/errors.js";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-release-policy-"));
  await mkdir(join(tmpDir, ".maestro", "policies"), { recursive: true });
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("loadReleasePolicy", () => {
  it("returns permissive defaults when file is missing", async () => {
    const policy = await loadReleasePolicy(tmpDir);

    expect(policy.kind).toBe("release");
    expect(policy.requireSignedCommits).toBe(false);
    expect(policy.requireProofMapComplete).toBe(false);
  });

  it("parses a valid release.yaml", async () => {
    const fixture = join(
      import.meta.dir,
      "../../../../fixtures/policies/release-valid.yaml",
    );
    const content = await readFile(fixture, "utf8");
    await writeFile(join(tmpDir, ".maestro", "policies", "release.yaml"), content);

    const policy = await loadReleasePolicy(tmpDir);

    expect(policy.kind).toBe("release");
    expect(policy.id).toBe("release-policy-custom");
    expect(policy.requireSignedCommits).toBe(true);
    expect(policy.requireProofMapComplete).toBe(true);
  });

  it("applies false defaults for absent boolean fields", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "release.yaml"),
      "kind: release\n",
    );

    const policy = await loadReleasePolicy(tmpDir);
    expect(policy.requireSignedCommits).toBe(false);
    expect(policy.requireProofMapComplete).toBe(false);
  });

  it("throws MaestroError when require_signed_commits is not a boolean", async () => {
    await writeFile(
      join(tmpDir, ".maestro", "policies", "release.yaml"),
      "require_signed_commits: yes-please\n",
    );

    const err = await loadReleasePolicy(tmpDir).catch((e) => e);
    expect(err).toBeInstanceOf(MaestroError);
    expect(err.message).toMatch(/require_signed_commits/i);
  });
});
