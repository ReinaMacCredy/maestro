import { afterEach, describe, expect, it } from "bun:test";
import { createHash } from "node:crypto";
import type { PathLike } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { MaestroError } from "@/shared/errors.js";
import { VERSION } from "@/shared/version.js";
import {
  buildReleaseDownloadUrl,
  getReleasesApiBaseUrl,
  installReleaseBinary,
  normalizeReleaseTag,
  replaceInstalledBinary,
  resolveDefaultInstallDir,
  resolveInstallDir,
  resolveInstalledBinaryName,
  resolveReleaseAssetName,
} from "@/infra/usecases/install-release-binary.usecase.js";

const installDirs: string[] = [];

function asFetch(
  fn: (input: URL | RequestInfo, init?: RequestInit) => Promise<Response>,
): typeof fetch {
  return fn as unknown as typeof fetch;
}

function checksumFor(content: string): string {
  return createHash("sha256").update(content).digest("hex");
}

function checksumAsset(assetName: string): { readonly name: string; readonly browser_download_url: string } {
  return {
    name: `${assetName}.sha256`,
    browser_download_url: `https://github.com/ReinaMacCredy/maestro/releases/download/v9.9.9/${assetName}.sha256`,
  };
}

function binaryAsset(assetName: string): { readonly name: string; readonly browser_download_url: string } {
  return {
    name: assetName,
    browser_download_url: `https://github.com/ReinaMacCredy/maestro/releases/download/v9.9.9/${assetName}`,
  };
}

function checksumResponse(assetName: string, content: string): Response {
  return new Response(`${checksumFor(content)}  ${assetName}\n`, { status: 200 });
}

function restoreEnv(key: string, value: string | undefined): void {
  if (value === undefined) {
    delete process.env[key];
  } else {
    process.env[key] = value;
  }
}

afterEach(async () => {
  await Promise.all(
    installDirs.splice(0).map((dir) => rm(dir, { recursive: true, force: true })),
  );
});

describe("install release binary usecase", () => {
  it("maps release assets from platform and arch", () => {
    expect(resolveReleaseAssetName("darwin", "arm64")).toBe("maestro-darwin-arm64");
    expect(resolveReleaseAssetName("darwin", "x64")).toBe("maestro-darwin-x64");
    expect(resolveReleaseAssetName("linux", "x86_64")).toBe("maestro-linux-x64");
    expect(resolveReleaseAssetName("linux", "aarch64")).toBe("maestro-linux-arm64");
    expect(resolveReleaseAssetName("win32", "x64")).toBe("maestro-windows-x64.exe");
  });

  it("rejects Windows arm64 until release assets exist for that platform", () => {
    expect(() => resolveReleaseAssetName("win32", "arm64")).toThrow(MaestroError);
  });

  it("adds .exe suffix to installed binary name on Windows", () => {
    expect(resolveInstalledBinaryName("darwin")).toBe("maestro");
    expect(resolveInstalledBinaryName("linux")).toBe("maestro");
    expect(resolveInstalledBinaryName("win32")).toBe("maestro.exe");
  });

  it("resolves platform-specific default install directories", () => {
    expect(resolveDefaultInstallDir("linux", { HOME: "/home/u" } as NodeJS.ProcessEnv)).toMatch(/\.local[\\/]bin$/);
    expect(resolveDefaultInstallDir("darwin", { HOME: "/home/u" } as NodeJS.ProcessEnv)).toMatch(/\.local[\\/]bin$/);
    const win = resolveDefaultInstallDir(
      "win32",
      { LOCALAPPDATA: "C:\\Users\\u\\AppData\\Local" } as NodeJS.ProcessEnv,
    );
    expect(win).toBe("C:\\Users\\u\\AppData\\Local\\Programs\\maestro");
  });

  it("resolveInstallDir prefers MAESTRO_INSTALL_DIR over the platform default", () => {
    const override = resolveInstallDir(
      "linux",
      { HOME: "/home/u", MAESTRO_INSTALL_DIR: "/opt/maestro" } as NodeJS.ProcessEnv,
    );
    expect(override).toBe("/opt/maestro");

    const fallback = resolveInstallDir("linux", { HOME: "/home/u" } as NodeJS.ProcessEnv);
    expect(fallback).toMatch(/\.local[\\/]bin$/);
  });

  it("installs into MAESTRO_INSTALL_DIR when no explicit installDir is provided", async () => {
    const homeDir = await mkdtemp(join(tmpdir(), "maestro-release-home-"));
    const installDir = join(homeDir, "custom-bin");
    installDirs.push(homeDir);

    const previousHome = process.env.HOME;
    const previousInstallDir = process.env.MAESTRO_INSTALL_DIR;
    process.env.HOME = homeDir;
    process.env.MAESTRO_INSTALL_DIR = installDir;

    const fetchImpl = asFetch(async (input) => {
      const url = String(input);
      if (url.endsWith("/releases/latest")) {
        return Response.json({
          tag_name: "v9.9.9",
          assets: [
            binaryAsset("maestro-linux-x64"),
            checksumAsset("maestro-linux-x64"),
          ],
        });
      }
      if (url.endsWith("/maestro-linux-x64.sha256")) {
        return checksumResponse("maestro-linux-x64", "new-binary");
      }
      if (url.endsWith("/maestro-linux-x64")) {
        return new Response("new-binary", { status: 200 });
      }
      throw new Error(`Unexpected fetch: ${url}`);
    });

    try {
      const result = await installReleaseBinary({
        fetchImpl,
        platform: "linux",
        arch: "x64",
      });

      expect(result.installPath).toBe(join(installDir, "maestro"));
      expect(await Bun.file(join(installDir, "maestro")).text()).toBe("new-binary");
    } finally {
      if (previousHome === undefined) {
        delete process.env.HOME;
      } else {
        process.env.HOME = previousHome;
      }
      if (previousInstallDir === undefined) {
        delete process.env.MAESTRO_INSTALL_DIR;
      } else {
        process.env.MAESTRO_INSTALL_DIR = previousInstallDir;
      }
    }
  });

  it("renames an existing Windows binary to .old before replacing (sidesteps locked exe)", async () => {
    const installDir = await mkdtemp(join(tmpdir(), "maestro-release-install-"));
    installDirs.push(installDir);
    const existingPath = join(installDir, "maestro.exe");
    await Bun.write(existingPath, "old-binary");

    const fetchImpl = asFetch(async (input) => {
      const url = String(input);
      if (url.endsWith("/releases/latest")) {
        return Response.json({
          tag_name: "v9.9.9",
          assets: [
            binaryAsset("maestro-windows-x64.exe"),
            checksumAsset("maestro-windows-x64.exe"),
          ],
        });
      }
      if (url.endsWith("/maestro-windows-x64.exe.sha256")) {
        return checksumResponse("maestro-windows-x64.exe", "new-binary");
      }
      if (url.endsWith("/maestro-windows-x64.exe")) {
        return new Response("new-binary", { status: 200 });
      }
      throw new Error(`Unexpected fetch: ${url}`);
    });

    const result = await installReleaseBinary({
      fetchImpl,
      installDir,
      platform: "win32",
      arch: "x64",
    });

    expect(result.binaryUpdated).toBe(true);
    expect(await Bun.file(existingPath).text()).toBe("new-binary");
    expect(await Bun.file(`${existingPath}.old`).text()).toBe("old-binary");
  });

    it("restores the previous Windows binary when the final rename fails", async () => {
      const installDir = await mkdtemp(join(tmpdir(), "maestro-release-install-"));
      installDirs.push(installDir);
      const installPath = join(installDir, "maestro.exe");
    const tempPath = join(installDir, "maestro.exe.tmp");
    await Bun.write(installPath, "old-binary");
    await Bun.write(tempPath, "new-binary");

    let renameCalls = 0;
    const renameImpl = async (from: PathLike, to: PathLike): Promise<void> => {
      renameCalls += 1;
      if (renameCalls === 2) {
        throw new Error("rename failed");
      }
      await Bun.write(String(to), await Bun.file(String(from)).text());
      await rm(from, { force: true });
    };

    await expect(
      replaceInstalledBinary(tempPath, installPath, "win32", { renameImpl }),
    ).rejects.toThrow("rename failed");

      expect(await Bun.file(installPath).text()).toBe("old-binary");
      expect(await Bun.file(`${installPath}.old`).exists()).toBe(false);
      expect(await Bun.file(tempPath).text()).toBe("new-binary");
    });

    it("surfaces a MaestroError when replacement and rollback both fail on Windows", async () => {
      const installDir = await mkdtemp(join(tmpdir(), "maestro-release-install-"));
      installDirs.push(installDir);
      const installPath = join(installDir, "maestro.exe");
      const tempPath = join(installDir, "maestro.exe.tmp");
      await Bun.write(installPath, "old-binary");
      await Bun.write(tempPath, "new-binary");

      let renameCalls = 0;
      const renameImpl = async (from: PathLike, to: PathLike): Promise<void> => {
        renameCalls += 1;
        if (renameCalls === 2) {
          throw new Error("rename failed");
        }
        if (renameCalls === 3) {
          throw new Error("rollback failed");
        }
        await Bun.write(String(to), await Bun.file(String(from)).text());
        await rm(from, { force: true });
      };

      let caught: unknown;
      try {
        await replaceInstalledBinary(tempPath, installPath, "win32", { renameImpl });
      } catch (err) {
        caught = err;
      }

      expect(caught).toBeInstanceOf(MaestroError);
      expect((caught as MaestroError).message).toBe(
        "Could not replace installed Windows binary and restore the previous version",
      );
      expect((caught as MaestroError).hints).toEqual([
        "Replacement error: rename failed",
        "Rollback error: rollback failed",
      ]);
      expect(await Bun.file(installPath).exists()).toBe(false);
      expect(await Bun.file(`${installPath}.old`).text()).toBe("old-binary");
      expect(await Bun.file(tempPath).text()).toBe("new-binary");
    });

    it("normalizes release tags and direct download URLs", () => {
      expect(normalizeReleaseTag("0.32.0")).toBe("v0.32.0");
      expect(buildReleaseDownloadUrl("maestro-darwin-arm64")).toContain("/latest/download/maestro-darwin-arm64");
      expect(buildReleaseDownloadUrl("maestro-darwin-arm64", "0.32.0")).toContain("/download/v0.32.0/maestro-darwin-arm64");
    });

  it("ignores release source environment overrides", () => {
    const previousRepo = process.env.MAESTRO_RELEASE_REPO;
    const previousBaseUrl = process.env.MAESTRO_RELEASE_BASE_URL;
    const previousApiBaseUrl = process.env.MAESTRO_RELEASE_API_BASE_URL;
    process.env.MAESTRO_RELEASE_REPO = "attacker/repo";
    process.env.MAESTRO_RELEASE_BASE_URL = "https://evil.example/releases";
    process.env.MAESTRO_RELEASE_API_BASE_URL = "https://evil.example/api";

    try {
      expect(buildReleaseDownloadUrl("maestro-darwin-arm64")).toBe(
        "https://github.com/ReinaMacCredy/maestro/releases/latest/download/maestro-darwin-arm64",
      );
      expect(getReleasesApiBaseUrl()).toBe("https://api.github.com/repos/ReinaMacCredy/maestro/releases");
    } finally {
      restoreEnv("MAESTRO_RELEASE_REPO", previousRepo);
      restoreEnv("MAESTRO_RELEASE_BASE_URL", previousBaseUrl);
      restoreEnv("MAESTRO_RELEASE_API_BASE_URL", previousApiBaseUrl);
    }
  });

  it("installs the matching asset from the latest release", async () => {
    const installDir = await mkdtemp(join(tmpdir(), "maestro-release-install-"));
    installDirs.push(installDir);

      const fetchImpl = asFetch(async (input) => {
        const url = String(input);
        if (url.endsWith("/releases/latest")) {
          return Response.json({
          tag_name: "v9.9.9",
          assets: [
            binaryAsset("maestro-darwin-arm64"),
            checksumAsset("maestro-darwin-arm64"),
          ],
        });
      }

        if (url.endsWith("/maestro-darwin-arm64.sha256")) {
          return checksumResponse("maestro-darwin-arm64", "binary-data");
        }

        if (url.endsWith("/maestro-darwin-arm64")) {
          return new Response("binary-data", { status: 200 });
        }

        throw new Error(`Unexpected fetch: ${url}`);
      });

    const result = await installReleaseBinary({
      fetchImpl,
      installDir,
      platform: "darwin",
      arch: "arm64",
    });

    expect(result.binaryUpdated).toBe(true);
    expect(result.alreadyCurrent).toBe(false);
    expect(result.version).toBe("9.9.9");
    expect(result.assetName).toBe("maestro-darwin-arm64");
    expect(await Bun.file(join(installDir, "maestro")).text()).toBe("binary-data");
  });

  it("skips the download when already on the latest released version", async () => {
    const installDir = await mkdtemp(join(tmpdir(), "maestro-release-install-"));
    installDirs.push(installDir);
    await Bun.write(join(installDir, "maestro"), "existing-binary");

    let downloadRequested = false;
      const fetchImpl = asFetch(async (input) => {
        const url = String(input);
        if (url.endsWith("/releases/latest")) {
          return Response.json({
          tag_name: `v${VERSION}`,
          assets: [
            binaryAsset("maestro-darwin-arm64"),
            checksumAsset("maestro-darwin-arm64"),
          ],
        });
      }

        downloadRequested = true;
        return new Response("binary-data", { status: 200 });
      });

    const result = await installReleaseBinary({
      fetchImpl,
      installDir,
      platform: "darwin",
      arch: "arm64",
    });

    expect(result.binaryUpdated).toBe(false);
    expect(result.alreadyCurrent).toBe(true);
    expect(result.version).toBe(VERSION);
    expect(downloadRequested).toBe(false);
    expect(await Bun.file(join(installDir, "maestro")).text()).toBe("existing-binary");
  });

  it("downloads the latest binary when versions match but no installed binary exists", async () => {
    const installDir = await mkdtemp(join(tmpdir(), "maestro-release-install-"));
    installDirs.push(installDir);

    let downloadRequested = false;
      const fetchImpl = asFetch(async (input) => {
        const url = String(input);
        if (url.endsWith("/releases/latest")) {
          return Response.json({
          tag_name: `v${VERSION}`,
          assets: [
            binaryAsset("maestro-darwin-arm64"),
            checksumAsset("maestro-darwin-arm64"),
          ],
        });
      }

      if (url.endsWith("/maestro-darwin-arm64.sha256")) {
        return checksumResponse("maestro-darwin-arm64", "binary-data");
      }

      if (url.endsWith("/maestro-darwin-arm64")) {
        downloadRequested = true;
        return new Response("binary-data", { status: 200 });
      }

        throw new Error(`Unexpected fetch: ${url}`);
      });

    const result = await installReleaseBinary({
      fetchImpl,
      installDir,
      platform: "darwin",
      arch: "arm64",
    });

    expect(result.binaryUpdated).toBe(true);
    expect(result.alreadyCurrent).toBe(false);
    expect(downloadRequested).toBe(true);
    expect(await Bun.file(join(installDir, "maestro")).text()).toBe("binary-data");
  });

  it("rejects releases that omit the checksum asset", async () => {
    const installDir = await mkdtemp(join(tmpdir(), "maestro-release-install-"));
    installDirs.push(installDir);

    const fetchImpl = asFetch(async (input) => {
      const url = String(input);
      if (url.endsWith("/releases/latest")) {
        return Response.json({
          tag_name: "v9.9.9",
          assets: [binaryAsset("maestro-darwin-arm64")],
        });
      }
      throw new Error(`Unexpected fetch: ${url}`);
    });

    await expect(
      installReleaseBinary({
        fetchImpl,
        installDir,
        platform: "darwin",
        arch: "arm64",
      }),
    ).rejects.toThrow(/does not include maestro-darwin-arm64\.sha256/);
    expect(await Bun.file(join(installDir, "maestro")).exists()).toBe(false);
  });

  it("rejects binaries when the checksum does not match", async () => {
    const installDir = await mkdtemp(join(tmpdir(), "maestro-release-install-"));
    installDirs.push(installDir);

    const fetchImpl = asFetch(async (input) => {
      const url = String(input);
      if (url.endsWith("/releases/latest")) {
        return Response.json({
          tag_name: "v9.9.9",
          assets: [
            binaryAsset("maestro-darwin-arm64"),
            checksumAsset("maestro-darwin-arm64"),
          ],
        });
      }
      if (url.endsWith("/maestro-darwin-arm64.sha256")) {
        return checksumResponse("maestro-darwin-arm64", "different-binary");
      }
      if (url.endsWith("/maestro-darwin-arm64")) {
        return new Response("binary-data", { status: 200 });
      }
      throw new Error(`Unexpected fetch: ${url}`);
    });

    await expect(
      installReleaseBinary({
        fetchImpl,
        installDir,
        platform: "darwin",
        arch: "arm64",
      }),
    ).rejects.toThrow(/checksum mismatch/);
    expect(await Bun.file(join(installDir, "maestro")).exists()).toBe(false);
  });

  it("fails clearly when the platform is unsupported", () => {
    expect(() => resolveReleaseAssetName("freebsd" as NodeJS.Platform, "x64")).toThrow(MaestroError);
  });
});
