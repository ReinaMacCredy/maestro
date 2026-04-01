import { join } from "node:path";
import { BUILD_UNIX, GIT_SHA, RELEASED_AT, VERSION } from "./version.js";

export interface VersionMetadata {
  readonly version: string;
  readonly buildUnix: number;
  readonly gitSha: string;
  readonly releasedAt: string;
}

interface VersionEnv {
  readonly MAESTRO_BUILD_GIT_SHA?: string;
  readonly [key: string]: string | undefined;
}

interface GitShaResolutionOptions {
  readonly buildGitSha?: string;
  readonly liveGitSha?: string;
  readonly trackedGitSha?: string;
}

const REPO_ROOT = join(import.meta.dir, "..");
const BUILD_GIT_SHA_OVERRIDE = normalizeGitSha(process.env.MAESTRO_BUILD_GIT_SHA);

function normalizeGitSha(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed && trimmed !== "unknown" ? trimmed : undefined;
}

function resolveBuildOverrideSha(env: VersionEnv): string | undefined {
  return normalizeGitSha(env.MAESTRO_BUILD_GIT_SHA) ?? BUILD_GIT_SHA_OVERRIDE;
}

export function resolveRuntimeGitSha(
  repoRoot: string = REPO_ROOT,
): string | undefined {
  try {
    const result = Bun.spawnSync(["git", "rev-parse", "--short=7", "HEAD"], {
      cwd: repoRoot,
      stdout: "pipe",
      stderr: "pipe",
    });
    if (result.exitCode !== 0) {
      return undefined;
    }

    return normalizeGitSha(new TextDecoder().decode(result.stdout).trim());
  } catch {
    return undefined;
  }
}

export function resolveDisplayedGitSha(
  options: GitShaResolutionOptions,
): string {
  return (
    normalizeGitSha(options.buildGitSha) ??
    normalizeGitSha(options.liveGitSha) ??
    normalizeGitSha(options.trackedGitSha) ??
    "unknown"
  );
}

export function getVersionMetadata(
  env: VersionEnv = {},
  liveGitSha?: string,
): VersionMetadata {
  const buildGitSha = resolveBuildOverrideSha(env);
  return {
    version: VERSION,
    buildUnix: BUILD_UNIX,
    gitSha: resolveDisplayedGitSha({
      buildGitSha,
      liveGitSha: buildGitSha ? undefined : liveGitSha,
      trackedGitSha: GIT_SHA,
    }),
    releasedAt: RELEASED_AT,
  };
}

export function formatRelativeAge(
  releasedAt: string,
  now: Date = new Date(),
): string {
  const releasedMs = Date.parse(releasedAt);
  if (!Number.isFinite(releasedMs)) {
    return "unknown age";
  }

  const elapsedSeconds = Math.max(
    0,
    Math.floor((now.getTime() - releasedMs) / 1_000),
  );

  if (elapsedSeconds < 60) {
    return `${elapsedSeconds}s ago`;
  }

  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) {
    return `${elapsedMinutes}m ago`;
  }

  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24) {
    return `${elapsedHours}h ago`;
  }

  const elapsedDays = Math.floor(elapsedHours / 24);
  if (elapsedDays < 7) {
    return `${elapsedDays}d ago`;
  }

  const elapsedWeeks = Math.floor(elapsedDays / 7);
  if (elapsedWeeks < 5) {
    return `${elapsedWeeks}w ago`;
  }

  const elapsedMonths = Math.floor(elapsedDays / 30);
  return `${elapsedMonths}mo ago`;
}

export function formatVersionOutput(
  metadata: VersionMetadata = getVersionMetadata(),
  now: Date = new Date(),
): string {
  return `${metadata.version}.${metadata.buildUnix}-g${metadata.gitSha} (released ${metadata.releasedAt}, ${formatRelativeAge(metadata.releasedAt, now)})`;
}

function isVersionFlag(arg: string): boolean {
  return arg === "--version" || arg === "-V";
}

export function formatVersionOutputForArgv(
  argv: readonly string[] = process.argv,
  env: VersionEnv = process.env,
  now: Date = new Date(),
): string {
  const wantsVersion = argv.slice(2).some(isVersionFlag);
  const buildOverrideSha = resolveBuildOverrideSha(env);
  const liveGitSha = wantsVersion && !buildOverrideSha
    ? resolveRuntimeGitSha()
    : undefined;
  return formatVersionOutput(getVersionMetadata(env, liveGitSha), now);
}
