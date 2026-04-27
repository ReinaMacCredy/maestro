import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { ensureDir, readJson, writeJson } from "@/shared/lib/fs.js";

export interface UpdateCheckCacheEntry {
  readonly checkedAt: string;
  readonly lastAttemptAt?: string;
  readonly currentVersion: string;
  readonly latestVersion: string;
  readonly latestTag: string;
}

const CACHE_FILE = "update-check.json";

export function resolveUpdateCheckCachePath(home: string = homedir()): string {
  return join(home, MAESTRO_DIR, CACHE_FILE);
}

export async function readUpdateCheckCache(
  path: string = resolveUpdateCheckCachePath(),
): Promise<UpdateCheckCacheEntry | undefined> {
  // A corrupted cache must never crash the CLI -- swallow any read/parse error
  // and let the caller treat the cache as missing so the next refresh rewrites it.
  try {
    const raw = await readJson<unknown>(path);
    return parseEntry(raw);
  } catch {
    return undefined;
  }
}

export async function writeUpdateCheckCache(
  entry: UpdateCheckCacheEntry,
  path: string = resolveUpdateCheckCachePath(),
): Promise<void> {
  await ensureDir(dirname(path));
  await writeJson(path, entry);
}

function parseEntry(raw: unknown): UpdateCheckCacheEntry | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const value = raw as Record<string, unknown>;
  const { checkedAt, lastAttemptAt, currentVersion, latestVersion, latestTag } = value;
  if (
    typeof checkedAt !== "string"
    || (lastAttemptAt !== undefined && typeof lastAttemptAt !== "string")
    || typeof currentVersion !== "string"
    || typeof latestVersion !== "string"
    || typeof latestTag !== "string"
  ) {
    return undefined;
  }
  return {
    checkedAt,
    ...(lastAttemptAt !== undefined ? { lastAttemptAt } : {}),
    currentVersion,
    latestVersion,
    latestTag,
  };
}
