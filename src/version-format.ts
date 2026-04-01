import { BUILD_UNIX, GIT_SHA, RELEASED_AT, VERSION } from "./version.js";

export interface VersionMetadata {
  readonly version: string;
  readonly buildUnix: number;
  readonly gitSha: string;
  readonly releasedAt: string;
}

export function getVersionMetadata(): VersionMetadata {
  return {
    version: VERSION,
    buildUnix: BUILD_UNIX,
    gitSha: GIT_SHA,
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
