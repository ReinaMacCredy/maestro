import { afterEach, describe, expect, it } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { MaestroError } from "@/shared/errors.js";
import { VERSION } from "@/shared/version.js";
import {
  buildReleaseDownloadUrl,
  installReleaseBinary,
  normalizeReleaseTag,
  resolveReleaseAssetName,
} from "@/infra/usecases/install-release-binary.usecase.js";

const installDirs: string[] = [];

function asFetch(
  fn: (input: URL | RequestInfo, init?: RequestInit) => Promise<Response>,
): typeof fetch {
  return fn as unknown as typeof fetch;
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
  });

  it("normalizes release tags and direct download URLs", () => {
    expect(normalizeReleaseTag("0.32.0")).toBe("v0.32.0");
    expect(buildReleaseDownloadUrl("maestro-darwin-arm64")).toContain("/latest/download/maestro-darwin-arm64");
    expect(buildReleaseDownloadUrl("maestro-darwin-arm64", "0.32.0")).toContain("/download/v0.32.0/maestro-darwin-arm64");
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
            {
              name: "maestro-darwin-arm64",
              browser_download_url: "https://downloads.example.test/maestro-darwin-arm64",
            },
          ],
        });
      }

        if (url === "https://downloads.example.test/maestro-darwin-arm64") {
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
            {
              name: "maestro-darwin-arm64",
              browser_download_url: "https://downloads.example.test/maestro-darwin-arm64",
            },
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
            {
              name: "maestro-darwin-arm64",
              browser_download_url: "https://downloads.example.test/maestro-darwin-arm64",
            },
          ],
        });
      }

      if (url === "https://downloads.example.test/maestro-darwin-arm64") {
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

  it("fails clearly when the platform is unsupported", () => {
    expect(() => resolveReleaseAssetName("win32", "x64")).toThrow(MaestroError);
  });
});
