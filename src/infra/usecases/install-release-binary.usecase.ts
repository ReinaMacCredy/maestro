import { chmod, mkdir, rename, rm } from "node:fs/promises";
import { homedir } from "node:os";
import { join, posix, win32 } from "node:path";
import { fileExists, renameForInPlaceReplace } from "@/shared/lib/fs.js";
import { MaestroError } from "@/shared/errors.js";
import { VERSION } from "@/shared/version.js";

const DEFAULT_RELEASE_REPO = "ReinaMacCredy/maestro";
const TARGET_BINARY_BASENAME = "maestro";

export function resolveInstalledBinaryName(
  platform: NodeJS.Platform = process.platform,
): string {
  return platform === "win32" ? `${TARGET_BINARY_BASENAME}.exe` : TARGET_BINARY_BASENAME;
}

export function resolveInstallDir(
  platform: NodeJS.Platform = process.platform,
  env: NodeJS.ProcessEnv = process.env,
): string {
  return env.MAESTRO_INSTALL_DIR ?? resolveDefaultInstallDir(platform, env);
}

export function resolveDefaultInstallDir(
  platform: NodeJS.Platform = process.platform,
  env: NodeJS.ProcessEnv = process.env,
): string {
  if (platform === "win32") {
    const base = env.LOCALAPPDATA ?? win32.join(homedir(), "AppData", "Local");
    return win32.join(base, "Programs", "maestro");
  }
  return posix.join(env.HOME ?? homedir(), ".local", "bin");
}

interface GitHubReleaseAssetPayload {
  readonly name?: string;
  readonly browser_download_url?: string;
}

interface GitHubReleasePayload {
  readonly tag_name?: string;
  readonly assets?: readonly GitHubReleaseAssetPayload[];
}

interface ReleaseAsset {
  readonly name: string;
  readonly downloadUrl: string;
}

export interface InstallReleaseBinaryOptions {
  readonly version?: string;
  readonly force?: boolean;
  readonly installDir?: string;
  readonly platform?: NodeJS.Platform;
  readonly arch?: string;
  readonly fetchImpl?: typeof fetch;
}

export interface InstallReleaseBinaryResult {
  readonly binaryUpdated: boolean;
  readonly alreadyCurrent: boolean;
  readonly installPath: string;
  readonly tagName: string;
  readonly version: string;
  readonly assetName: string;
}

interface ReplaceInstalledBinaryOptions {
  readonly removeImpl?: typeof rm;
  readonly renameImpl?: typeof rename;
}

export function normalizeReleaseTag(versionOrTag: string): string {
  const trimmed = versionOrTag.trim();
  if (!trimmed) {
    throw new MaestroError("Release version cannot be empty", [
      "Pass a version like 0.32.0 or a tag like v0.32.0",
    ]);
  }

  return trimmed.startsWith("v") ? trimmed : `v${trimmed}`;
}

export function versionFromTag(tagName: string): string {
  return tagName.startsWith("v") ? tagName.slice(1) : tagName;
}

export function resolveReleaseAssetName(
  platform: NodeJS.Platform = process.platform,
  arch: string = process.arch,
): string {
  const normalizedOs = resolveReleaseOs(platform);
  const normalizedArch = resolveReleaseArch(normalizedOs, arch);
  const suffix = normalizedOs === "windows" ? ".exe" : "";
  return `${TARGET_BINARY_BASENAME}-${normalizedOs}-${normalizedArch}${suffix}`;
}

export function buildReleaseDownloadUrl(
  assetName: string,
  tagName?: string,
): string {
  const releasesBaseUrl = getReleasesBaseUrl();
  if (!tagName) {
    return `${releasesBaseUrl}/latest/download/${assetName}`;
  }

  return `${releasesBaseUrl}/download/${normalizeReleaseTag(tagName)}/${assetName}`;
}

export async function installReleaseBinary(
  options: InstallReleaseBinaryOptions = {},
): Promise<InstallReleaseBinaryResult> {
  const fetchImpl = options.fetchImpl ?? fetch;
  const platform = options.platform ?? process.platform;
  const installDir = options.installDir ?? resolveInstallDir(platform);
  const installPath = join(installDir, resolveInstalledBinaryName(platform));
  const requestedTag = options.version ? normalizeReleaseTag(options.version) : undefined;
  const release = await resolveRelease(fetchImpl, {
    tagName: requestedTag,
    platform: options.platform,
    arch: options.arch,
  });
  const installedBinaryExists = await Bun.file(installPath).exists();

  if (!requestedTag && !options.force && installedBinaryExists && release.version === VERSION) {
    return {
      binaryUpdated: false,
      alreadyCurrent: true,
      installPath,
      tagName: release.tagName,
      version: release.version,
      assetName: release.asset.name,
    };
  }

  await mkdir(installDir, { recursive: true });
  const tempPath = join(installDir, `.maestro.tmp.${process.pid}.${Date.now()}`);

  try {
    const binaryBytes = await downloadAsset(fetchImpl, release.asset.downloadUrl);
    await Bun.write(tempPath, binaryBytes);
    if (platform !== "win32") {
      await chmod(tempPath, 0o755);
    }
    await replaceInstalledBinary(tempPath, installPath, platform);
  } catch (error) {
    // Best-effort temp cleanup; surfacing a cleanup failure here would mask
    // the original download/write/rename error the caller needs to see.
    await rm(tempPath, { force: true }).catch(() => undefined);
    throw error;
  }

  return {
    binaryUpdated: true,
    alreadyCurrent: false,
    installPath,
    tagName: release.tagName,
    version: release.version,
    assetName: release.asset.name,
  };
}

async function resolveRelease(
  fetchImpl: typeof fetch,
  options: {
    readonly tagName?: string;
    readonly platform?: NodeJS.Platform;
    readonly arch?: string;
  },
): Promise<{
  readonly tagName: string;
  readonly version: string;
  readonly asset: ReleaseAsset;
}> {
  const assetName = resolveReleaseAssetName(options.platform, options.arch);
  const endpoint = options.tagName
    ? `${getReleasesApiBaseUrl()}/tags/${normalizeReleaseTag(options.tagName)}`
    : `${getReleasesApiBaseUrl()}/latest`;
  const payload = await fetchReleasePayload(fetchImpl, endpoint, options.tagName);
  const tagName = normalizeReleaseTag(payload.tag_name ?? "");
  const asset = resolveReleaseAsset(payload.assets, assetName, tagName);
  return {
    tagName,
    version: versionFromTag(tagName),
    asset,
  };
}

async function fetchReleasePayload(
  fetchImpl: typeof fetch,
  url: string,
  requestedTag?: string,
): Promise<GitHubReleasePayload> {
  const response = await fetchImpl(url, {
    headers: {
      Accept: "application/vnd.github+json",
      "User-Agent": "maestro-cli",
    },
  });

  if (!response.ok) {
    if (response.status === 404 && requestedTag) {
      const releaseRepo = getReleaseRepo();
      throw new MaestroError(`Release ${requestedTag} was not found`, [
        `Check https://github.com/${releaseRepo}/releases for published versions`,
      ]);
    }

    throw new MaestroError(`Could not fetch release metadata (${response.status})`, [
      `GitHub API URL: ${url}`,
    ]);
  }

  const payload = await response.json() as GitHubReleasePayload;
  if (!payload.tag_name || !Array.isArray(payload.assets)) {
    throw new MaestroError("GitHub release response was missing required fields", [
      `GitHub API URL: ${url}`,
    ]);
  }

  return payload;
}

function resolveReleaseAsset(
  assets: readonly GitHubReleaseAssetPayload[] | undefined,
  assetName: string,
  tagName: string,
): ReleaseAsset {
  const asset = assets?.find((candidate) => candidate.name === assetName);
  if (!asset?.browser_download_url) {
    const availableAssets = assets?.map((candidate) => candidate.name).filter(Boolean) ?? [];
    throw new MaestroError(`Release ${tagName} does not include ${assetName}`, [
      availableAssets.length > 0
        ? `Available assets: ${availableAssets.join(", ")}`
        : "The release is missing binary assets",
    ]);
  }

  return {
    name: assetName,
    downloadUrl: asset.browser_download_url,
  };
}

export async function replaceInstalledBinary(
  tempPath: string,
  installPath: string,
  platform: NodeJS.Platform,
  options: ReplaceInstalledBinaryOptions = {},
): Promise<void> {
  const renameImpl = options.renameImpl ?? rename;
  if (platform === "win32") {
    await renameForInPlaceReplace(installPath, {
      removeImpl: options.removeImpl,
      renameImpl,
    });
    try {
      await renameImpl(tempPath, installPath);
    } catch (error) {
      try {
        await rollbackWindowsBinary(installPath, renameImpl);
      } catch (rollbackError) {
        throw new MaestroError(
          "Could not replace installed Windows binary and restore the previous version",
          [
            `Replacement error: ${describeError(error)}`,
            `Rollback error: ${describeError(rollbackError)}`,
          ],
        );
      }
      throw error;
    }
    return;
  }
  await renameImpl(tempPath, installPath);
}

async function downloadAsset(
  fetchImpl: typeof fetch,
  url: string,
): Promise<Uint8Array<ArrayBuffer>> {
  const response = await fetchImpl(url, {
    headers: {
      Accept: "application/octet-stream",
      "User-Agent": "maestro-cli",
    },
  });

  if (!response.ok) {
    throw new MaestroError(`Binary download failed (${response.status})`, [
      `Download URL: ${url}`,
    ]);
  }

  return new Uint8Array(await response.arrayBuffer());
}

function resolveReleaseOs(platform: NodeJS.Platform): "darwin" | "linux" | "windows" {
  switch (platform) {
    case "darwin":
    case "linux":
      return platform;
    case "win32":
      return "windows";
    default:
      throw new MaestroError(`Unsupported platform for release installs: ${platform}`, [
        "Release installs currently support macOS, Linux, and Windows",
      ]);
  }
}

async function rollbackWindowsBinary(
  installPath: string,
  renameImpl: typeof rename,
): Promise<void> {
  const oldPath = `${installPath}.old`;
  if (!(await fileExists(oldPath))) return;
  await renameImpl(oldPath, installPath);
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function resolveReleaseArch(os: "darwin" | "linux" | "windows", arch: string): "arm64" | "x64" {
  switch (arch) {
    case "arm64":
    case "aarch64":
      if (os === "windows") {
        throw new MaestroError("Windows release installs currently support x64 only", [
          "Windows arm64 assets are not published yet",
        ]);
      }
      return "arm64";
    case "x64":
    case "amd64":
    case "x86_64":
      return "x64";
    default:
      throw new MaestroError(`Unsupported CPU architecture for release installs: ${arch}`, [
        "Release installs currently support x64 and arm64",
      ]);
  }
}

function getReleaseRepo(): string {
  return process.env.MAESTRO_RELEASE_REPO?.trim() || DEFAULT_RELEASE_REPO;
}

function getReleasesBaseUrl(): string {
  return process.env.MAESTRO_RELEASE_BASE_URL?.trim()
    || `https://github.com/${getReleaseRepo()}/releases`;
}

export function getReleasesApiBaseUrl(): string {
  return process.env.MAESTRO_RELEASE_API_BASE_URL?.trim()
    || `https://api.github.com/repos/${getReleaseRepo()}/releases`;
}
