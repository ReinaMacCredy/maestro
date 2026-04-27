import { describe, expect, it } from "bun:test";
import {
  checkForUpdate,
  isNewerSemver,
} from "@/infra/usecases/check-for-update.usecase.js";
import type { UpdateCheckCacheEntry } from "@/infra/adapters/update-check-cache.adapter.js";

function asFetch(
  fn: (input: URL | RequestInfo, init?: RequestInit) => Promise<Response>,
): typeof fetch {
  return fn as unknown as typeof fetch;
}

const FIXED_NOW = new Date("2026-04-26T12:00:00.000Z");

function freshCache(): UpdateCheckCacheEntry {
  return {
    checkedAt: new Date(FIXED_NOW.getTime() - 60 * 60 * 1000).toISOString(),
    currentVersion: "0.59.0",
    latestVersion: "0.60.0",
    latestTag: "v0.60.0",
  };
}

function staleCache(): UpdateCheckCacheEntry {
  return {
    checkedAt: new Date(FIXED_NOW.getTime() - 25 * 60 * 60 * 1000).toISOString(),
    currentVersion: "0.59.0",
    latestVersion: "0.59.0",
    latestTag: "v0.59.0",
  };
}

describe("isNewerSemver", () => {
  it("returns true when candidate is strictly higher", () => {
    expect(isNewerSemver("0.60.0", "0.59.0")).toBe(true);
    expect(isNewerSemver("1.0.0", "0.99.99")).toBe(true);
    expect(isNewerSemver("0.59.1", "0.59.0")).toBe(true);
  });

  it("returns false when candidate is equal or lower", () => {
    expect(isNewerSemver("0.59.0", "0.59.0")).toBe(false);
    expect(isNewerSemver("0.58.9", "0.59.0")).toBe(false);
    expect(isNewerSemver("0.0.0", "0.0.1")).toBe(false);
  });

  it("returns false on unparseable versions", () => {
    expect(isNewerSemver("not-a-version", "0.59.0")).toBe(false);
    expect(isNewerSemver("0.59", "0.59.0")).toBe(false);
  });
});

describe("checkForUpdate", () => {
  it("flags newer version when fresh cache says so", async () => {
    const result = await checkForUpdate({
      now: () => FIXED_NOW,
      currentVersion: "0.59.0",
      readCache: async () => freshCache(),
      writeCache: async () => {
        throw new Error("should not write when cache is fresh");
      },
      fetchImpl: asFetch(async () => {
        throw new Error("should not fetch when cache is fresh");
      }),
    });
    expect(result.hasNewerVersion).toBe(true);
    expect(result.cached?.latestVersion).toBe("0.60.0");
    expect(result.refreshing).toBeUndefined();
  });

  it("does not flag when cached latestVersion equals current", async () => {
    const result = await checkForUpdate({
      now: () => FIXED_NOW,
      currentVersion: "0.60.0",
      readCache: async () => freshCache(),
      writeCache: async () => undefined,
      fetchImpl: asFetch(async () => new Response("{}")),
    });
    expect(result.hasNewerVersion).toBe(false);
  });

  it("does not flag when cached latestVersion is older than current (post-upgrade)", async () => {
    const result = await checkForUpdate({
      now: () => FIXED_NOW,
      currentVersion: "0.61.0",
      readCache: async () => freshCache(),
      writeCache: async () => undefined,
      fetchImpl: asFetch(async () => new Response("{}")),
    });
    expect(result.hasNewerVersion).toBe(false);
  });

  it("triggers a background refresh when cache is stale", async () => {
    const writes: UpdateCheckCacheEntry[] = [];
    const result = await checkForUpdate({
      now: () => FIXED_NOW,
      currentVersion: "0.59.0",
      readCache: async () => staleCache(),
      writeCache: async (entry) => {
        writes.push(entry);
      },
      fetchImpl: asFetch(async () =>
        new Response(JSON.stringify({ tag_name: "v0.60.0" }), { status: 200 })
      ),
    });
    expect(result.refreshing).toBeDefined();
    const written = await result.refreshing;
    expect(written?.latestVersion).toBe("0.60.0");
    expect(written?.latestTag).toBe("v0.60.0");
    expect(written?.currentVersion).toBe("0.59.0");
    expect(writes).toHaveLength(1);
  });

  it("triggers refresh when no cache exists and swallows errors", async () => {
    const result = await checkForUpdate({
      now: () => FIXED_NOW,
      currentVersion: "0.59.0",
      readCache: async () => undefined,
      writeCache: async () => undefined,
      fetchImpl: asFetch(async () => new Response("nope", { status: 500 })),
    });
    expect(result.cached).toBeUndefined();
    expect(result.hasNewerVersion).toBe(false);
    expect(await result.refreshing).toBeUndefined();
  });

  it("forwards refreshSignal so callers can cancel the in-flight fetch", async () => {
    const controller = new AbortController();
    let observedSignal: AbortSignal | undefined;
    const result = await checkForUpdate({
      now: () => FIXED_NOW,
      currentVersion: "0.59.0",
      readCache: async () => undefined,
      writeCache: async () => undefined,
      refreshSignal: controller.signal,
      fetchImpl: asFetch((_url, init) => {
        observedSignal = init?.signal ?? undefined;
        return new Promise((_, reject) => {
          init?.signal?.addEventListener("abort", () => {
            reject(new DOMException("aborted", "AbortError"));
          });
        });
      }),
    });
    controller.abort();
    expect(await result.refreshing).toBeUndefined();
    expect(observedSignal).toBeDefined();
    expect(observedSignal!.aborted).toBe(true);
  });

  it("treats malformed checkedAt as stale", async () => {
    const result = await checkForUpdate({
      now: () => FIXED_NOW,
      currentVersion: "0.59.0",
      readCache: async () => ({
        ...freshCache(),
        checkedAt: "not-a-date",
      }),
      writeCache: async () => undefined,
      fetchImpl: asFetch(async () =>
        new Response(JSON.stringify({ tag_name: "v0.60.0" }))
      ),
    });
    expect(result.refreshing).toBeDefined();
  });
});
