import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  isRootBunBuildArtifact,
  removeRootBunBuildArtifacts,
} from "../../../scripts/build-lib";

let tempRoot: string;

beforeEach(async () => {
  tempRoot = await mkdtemp(join(tmpdir(), "maestro-build-lib-"));
});

afterEach(async () => {
  await rm(tempRoot, { recursive: true, force: true });
});

describe("isRootBunBuildArtifact", () => {
  it("matches Bun build temp artifacts by suffix", () => {
    expect(isRootBunBuildArtifact(".18a5b3bdcb3ea974-00000000.bun-build")).toBe(true);
    expect(isRootBunBuildArtifact("plain.bun-build")).toBe(true);
    expect(isRootBunBuildArtifact("maestro")).toBe(false);
    expect(isRootBunBuildArtifact(".bun-build.keep")).toBe(false);
  });
});

describe("removeRootBunBuildArtifacts", () => {
  it("removes only root bun-build temp files", async () => {
    const artifactA = join(tempRoot, ".one.bun-build");
    const artifactB = join(tempRoot, "two.bun-build");
    const keepFile = join(tempRoot, "keep.txt");
    const nestedDir = join(tempRoot, "nested");
    const nestedArtifact = join(nestedDir, ".three.bun-build");

    await writeFile(artifactA, "a");
    await writeFile(artifactB, "b");
    await writeFile(keepFile, "keep");
    await mkdir(nestedDir, { recursive: true });
    await writeFile(nestedArtifact, "nested");

    const removed = await removeRootBunBuildArtifacts(tempRoot);

    expect(removed).toEqual([artifactA, artifactB]);
    await expect(readFile(artifactA, "utf8")).rejects.toThrow();
    await expect(readFile(artifactB, "utf8")).rejects.toThrow();
    expect(await readFile(keepFile, "utf8")).toBe("keep");
    expect(await readFile(nestedArtifact, "utf8")).toBe("nested");
  });
});
