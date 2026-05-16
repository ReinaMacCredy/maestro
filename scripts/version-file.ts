import { getGitShortSha } from "./git-short-sha";

interface PackageJson {
  version: string;
  [key: string]: unknown;
}

interface ReleaseVersionParts {
  readonly major: number;
  readonly feature: number;
  readonly patch: number;
  readonly preRelease?: string;
}

interface VersionFileData {
  readonly version: string;
  readonly buildUnix: number;
  readonly gitSha: string;
  readonly releasedAt: string;
}

const RELEASE_VERSION_PATTERN = /^(\d+)\.(\d+)\.(\d+)(?:-(rc\.\d+))?$/;

export function parseReleaseVersion(version: string): ReleaseVersionParts {
  const match = RELEASE_VERSION_PATTERN.exec(version);
  if (!match) {
    throw new Error(
      `Invalid Maestro release version '${version}'. Expected MAJOR.MINOR.PATCH or MAJOR.MINOR.PATCH-rc.N.`,
    );
  }

  return {
    major: Number(match[1]),
    feature: Number(match[2]),
    patch: Number(match[3]),
    ...(match[4] !== undefined ? { preRelease: match[4] } : {}),
  };
}

async function buildVersionFileData(
  cwd: string,
  version: string,
): Promise<VersionFileData> {
  const releasedAt = new Date().toISOString();
    return {
      version,
      buildUnix: Math.floor(Date.now() / 1_000),
      gitSha: (await getGitShortSha(cwd)) ?? "unknown",
      releasedAt,
    };
  }

export function renderVersionFile(data: VersionFileData): string {
  return [
    `export const VERSION = "${data.version}";`,
    `export const BUILD_UNIX = ${data.buildUnix};`,
    `export const GIT_SHA = "${data.gitSha}";`,
    `export const RELEASED_AT = "${data.releasedAt}";`,
    "",
  ].join("\n");
}

export async function writeVersionArtifacts(options: {
  readonly cwd: string;
  readonly pkgPath: string;
  readonly versionPath: string;
  readonly pkg: PackageJson;
  readonly version: string;
}): Promise<void> {
  const { cwd, pkgPath, versionPath, pkg, version } = options;
  parseReleaseVersion(version);
  pkg.version = version;
  await Bun.write(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
  await Bun.write(versionPath, renderVersionFile(await buildVersionFileData(cwd, version)));
}
