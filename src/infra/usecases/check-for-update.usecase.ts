import { VERSION } from "@/shared/version.js";
import {
  readUpdateCheckCache,
  writeUpdateCheckCache,
  type UpdateCheckCacheEntry,
} from "@/infra/adapters/update-check-cache.adapter.js";
import { fetchLatestVersion } from "./fetch-latest-version.usecase.js";

const STALE_AFTER_MS = 24 * 60 * 60 * 1000;
const ATTEMPT_COOLDOWN_MS = 15 * 60 * 1000;
const SEMVER_IDENTIFIER = "[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*";
const SEMVER_PATTERN = new RegExp(
  `^(0|[1-9]\\d*)\\.(0|[1-9]\\d*)\\.(0|[1-9]\\d*)(?:-(${SEMVER_IDENTIFIER}))?(?:\\+${SEMVER_IDENTIFIER})?$`,
);

export interface CheckForUpdateDeps {
  readonly now?: () => Date;
  readonly currentVersion?: string;
  readonly readCache?: () => Promise<UpdateCheckCacheEntry | undefined>;
  readonly writeCache?: (entry: UpdateCheckCacheEntry) => Promise<void>;
  readonly fetchImpl?: typeof fetch;
  // Cancels the background refresh if the user's command finishes before the
  // network call resolves. Without this, an in-flight fetch keeps the event
  // loop alive until its own 8s timeout fires (verified ~9s hang on cold
  // cache + slow network).
  readonly refreshSignal?: AbortSignal;
}

export interface CheckForUpdateResult {
  readonly cached: UpdateCheckCacheEntry | undefined;
  readonly hasNewerVersion: boolean;
  readonly refreshing: Promise<UpdateCheckCacheEntry | undefined> | undefined;
}

/**
 * Read the cached update-check result and, if stale or missing, kick off an
 * unawaited refresh. The current invocation never blocks on the refresh; the
 * fresh result lands for the *next* invocation.
 */
export async function checkForUpdate(
  deps: CheckForUpdateDeps = {},
): Promise<CheckForUpdateResult> {
  const now = deps.now ?? (() => new Date());
  const currentVersion = deps.currentVersion ?? VERSION;
  const readCache = deps.readCache ?? readUpdateCheckCache;
  const writeCache = deps.writeCache ?? writeUpdateCheckCache;

  const cached = await readCache().catch(() => undefined);
  const currentTime = now();
  const stale = isStale(cached, currentTime);
  const coolingDown = isAttemptCoolingDown(cached, currentTime);

  let refreshing: Promise<UpdateCheckCacheEntry | undefined> | undefined;
  if (stale && !coolingDown) {
    refreshing = refreshCache({
      now,
      currentVersion,
      writeCache,
      fetchImpl: deps.fetchImpl,
      cached,
      signal: deps.refreshSignal,
    }).catch(() => undefined);
  }

  return {
    cached,
    hasNewerVersion: !!cached && isNewerSemver(cached.latestVersion, currentVersion),
    refreshing,
  };
}

function isStale(cached: UpdateCheckCacheEntry | undefined, now: Date): boolean {
  if (!cached) return true;
  const checkedAt = Date.parse(cached.checkedAt);
  if (Number.isNaN(checkedAt)) return true;
  return now.getTime() - checkedAt >= STALE_AFTER_MS;
}

function isAttemptCoolingDown(cached: UpdateCheckCacheEntry | undefined, now: Date): boolean {
  if (!cached?.lastAttemptAt) return false;
  const attemptedAt = Date.parse(cached.lastAttemptAt);
  if (Number.isNaN(attemptedAt)) return false;
  return now.getTime() - attemptedAt < ATTEMPT_COOLDOWN_MS;
}

async function refreshCache(args: {
  readonly now: () => Date;
  readonly currentVersion: string;
  readonly writeCache: (entry: UpdateCheckCacheEntry) => Promise<void>;
  readonly fetchImpl?: typeof fetch;
  readonly cached?: UpdateCheckCacheEntry;
  readonly signal?: AbortSignal;
}): Promise<UpdateCheckCacheEntry> {
  const startedAt = args.now().toISOString();
  await args.writeCache(buildAttemptEntry(args.cached, args.currentVersion, startedAt)).catch(() => undefined);
  const latest = await fetchLatestVersion({ fetchImpl: args.fetchImpl, signal: args.signal });
  const entry: UpdateCheckCacheEntry = {
    checkedAt: args.now().toISOString(),
    currentVersion: args.currentVersion,
    latestVersion: latest.version,
    latestTag: latest.tag,
  };
  await args.writeCache(entry);
  return entry;
}

function buildAttemptEntry(
  cached: UpdateCheckCacheEntry | undefined,
  currentVersion: string,
  attemptedAt: string,
): UpdateCheckCacheEntry {
  return {
    checkedAt: cached?.checkedAt ?? "1970-01-01T00:00:00.000Z",
    lastAttemptAt: attemptedAt,
    currentVersion,
    latestVersion: cached?.latestVersion ?? currentVersion,
    latestTag: cached?.latestTag ?? `v${currentVersion}`,
  };
}

export function isNewerSemver(candidate: string, baseline: string): boolean {
  const a = parseSemver(candidate);
  const b = parseSemver(baseline);
  if (!a || !b) return false;
  if (a.major !== b.major) return a.major > b.major;
  if (a.minor !== b.minor) return a.minor > b.minor;
  if (a.patch !== b.patch) return a.patch > b.patch;
  return comparePrerelease(a.prerelease, b.prerelease) > 0;
}

interface ParsedSemver {
  readonly major: number;
  readonly minor: number;
  readonly patch: number;
  readonly prerelease: readonly string[];
}

function parseSemver(value: string): ParsedSemver | undefined {
  const match = value.match(SEMVER_PATTERN);
  if (!match) return undefined;
  const major = Number.parseInt(match[1]!, 10);
  const minor = Number.parseInt(match[2]!, 10);
  const patch = Number.parseInt(match[3]!, 10);
  if (!Number.isSafeInteger(major) || !Number.isSafeInteger(minor) || !Number.isSafeInteger(patch)) {
    return undefined;
  }
  const prerelease = match[4]?.split(".") ?? [];
  if (prerelease.some((identifier) => /^\d+$/.test(identifier) && !isNumericIdentifier(identifier))) {
    return undefined;
  }
  return {
    major,
    minor,
    patch,
    prerelease,
  };
}

function comparePrerelease(
  candidate: readonly string[],
  baseline: readonly string[],
): number {
  if (candidate.length === 0 && baseline.length === 0) return 0;
  if (candidate.length === 0) return 1;
  if (baseline.length === 0) return -1;

  const length = Math.max(candidate.length, baseline.length);
  for (let idx = 0; idx < length; idx++) {
    const left = candidate[idx];
    const right = baseline[idx];
    if (left === undefined) return -1;
    if (right === undefined) return 1;
    if (left === right) continue;

    const leftNumber = parseNumericIdentifier(left);
    const rightNumber = parseNumericIdentifier(right);
    if (leftNumber !== undefined && rightNumber !== undefined) {
      return leftNumber > rightNumber ? 1 : -1;
    }
    if (leftNumber !== undefined) return -1;
    if (rightNumber !== undefined) return 1;
    return left > right ? 1 : -1;
  }
  return 0;
}

function parseNumericIdentifier(value: string): number | undefined {
  if (!isNumericIdentifier(value)) return undefined;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : undefined;
}

function isNumericIdentifier(value: string): boolean {
  return /^(0|[1-9]\d*)$/.test(value);
}
