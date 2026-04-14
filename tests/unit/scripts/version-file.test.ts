import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  parseReleaseVersion,
  renderVersionFile,
  writeVersionArtifacts,
} from "../../../scripts/version-file";

let tmpDir: string;

beforeEach(async () => {
  tmpDir = await mkdtemp(join(tmpdir(), "maestro-version-file-"));
});

afterEach(async () => {
  await rm(tmpDir, { recursive: true, force: true });
});

describe("parseReleaseVersion", () => {
  it("accepts the 0.x.y release scheme", () => {
    expect(parseReleaseVersion("0.25.0")).toEqual({ feature: 25, patch: 0 });
    expect(parseReleaseVersion("0.25.1")).toEqual({ feature: 25, patch: 1 });
  });

  it("rejects non-zero major versions", () => {
    expect(() => parseReleaseVersion("2.4.2")).toThrow(
      "Invalid Maestro release version '2.4.2'. Expected 0.x.y with a zero major version.",
    );
  });

  it("rejects malformed versions", () => {
    expect(() => parseReleaseVersion("0.25")).toThrow(
      "Invalid Maestro release version '0.25'. Expected 0.x.y with a zero major version.",
    );
  });
});

describe("renderVersionFile", () => {
  it("renders the validated version literal", () => {
    const output = renderVersionFile({
      version: "0.25.1",
      buildUnix: 1776024000,
      gitSha: "abc1234",
      releasedAt: "2026-04-12T20:00:00.000Z",
    });

    expect(output).toContain('export const VERSION = "0.25.1";');
    expect(output).toContain("export const BUILD_UNIX = 1776024000;");
    expect(output).toContain('export const GIT_SHA = "abc1234";');
    expect(output).toContain('export const RELEASED_AT = "2026-04-12T20:00:00.000Z";');
  });
});

describe("writeVersionArtifacts", () => {
  it("writes both package.json and version.ts artifacts for a valid release version", async () => {
    const pkgPath = join(tmpDir, "package.json");
    const versionPath = join(tmpDir, "version.ts");
    const pkg = { version: "0.25.1", name: "maestro-handoff" };

    await writeVersionArtifacts({
      cwd: tmpDir,
      pkgPath,
      versionPath,
      pkg,
      version: "0.26.3",
    });

    expect(pkg.version).toBe("0.26.3");
    expect(JSON.parse(await readFile(pkgPath, "utf8")).version).toBe("0.26.3");

    const versionFile = await readFile(versionPath, "utf8");
    expect(versionFile).toContain('export const VERSION = "0.26.3";');
    expect(versionFile).toContain('export const GIT_SHA = "unknown";');
    expect(versionFile).toContain("export const BUILD_UNIX = ");
    expect(versionFile).toContain('export const RELEASED_AT = "');
  });

  it("rejects invalid release versions before writing any artifacts", async () => {
    const pkgPath = join(tmpDir, "package.json");
    const versionPath = join(tmpDir, "version.ts");

    await expect(writeVersionArtifacts({
      cwd: tmpDir,
      pkgPath,
      versionPath,
      pkg: { version: "0.25.1" },
      version: "1.0.0",
    })).rejects.toThrow(
      "Invalid Maestro release version '1.0.0'. Expected 0.x.y with a zero major version.",
    );

    expect(await Bun.file(pkgPath).exists()).toBe(false);
    expect(await Bun.file(versionPath).exists()).toBe(false);
  });
});
