import { VERSION } from "@/shared/version.js";
import {
  readUpdateCheckCache,
  writeUpdateCheckCache,
  type UpdateCheckCacheEntry,
} from "@/infra/adapters/update-check-cache.adapter.js";
import { fetchLatestVersion } from "./fetch-latest-version.usecase.js";

const STALE_AFTER_MS = 24 * 60 * 60 * 1000;

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
  const stale = isStale(cached, now());

  let refreshing: Promise<UpdateCheckCacheEntry | undefined> | undefined;
  if (stale) {
    refreshing = refreshCache({
      now,
      currentVersion,
      writeCache,
      fetchImpl: deps.fetchImpl,
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

async function refreshCache(args: {
  readonly now: () => Date;
  readonly currentVersion: string;
  readonly writeCache: (entry: UpdateCheckCacheEntry) => Promise<void>;
  readonly fetchImpl?: typeof fetch;
  readonly signal?: AbortSignal;
}): Promise<UpdateCheckCacheEntry> {
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

export function isNewerSemver(candidate: string, baseline: string): boolean {
  const a = parseSemver(candidate);
  const b = parseSemver(baseline);
  if (!a || !b) return false;
  if (a[0] !== b[0]) return a[0] > b[0];
  if (a[1] !== b[1]) return a[1] > b[1];
  return a[2] > b[2];
}

function parseSemver(value: string): [number, number, number] | undefined {
  const parts = value.split(".");
  if (parts.length < 3) return undefined;
  const major = Number.parseInt(parts[0] ?? "", 10);
  const minor = Number.parseInt(parts[1] ?? "", 10);
  const patch = Number.parseInt(parts[2] ?? "", 10);
  if (!Number.isFinite(major) || !Number.isFinite(minor) || !Number.isFinite(patch)) {
    return undefined;
  }
  return [major, minor, patch];
}
