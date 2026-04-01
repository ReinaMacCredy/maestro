import { getGitShortSha } from "./git-short-sha";

interface PackageJson {
  version: string;
  [key: string]: unknown;
}

interface VersionFileData {
  readonly version: string;
  readonly buildUnix: number;
  readonly gitSha: string;
  readonly releasedAt: string;
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
  pkg.version = version;
  await Bun.write(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
  await Bun.write(versionPath, renderVersionFile(await buildVersionFileData(cwd, version)));
}
