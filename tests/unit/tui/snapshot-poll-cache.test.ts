import { describe, expect, it } from "bun:test";
import type { ConfigLayers, ConfigPort } from "@/infra/ports/config.port.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";
import {
  cached,
  CachingConfigPort,
  makeEntry,
  setCachedEntry,
} from "@/tui/lib/snapshot-poll-cache.js";

function makeLayers(projectDir: string): ConfigLayers {
  const effective: MaestroConfig = {
    execution: {
      defaultWorker: projectDir,
    },
  };

  return {
    defaults: {},
    effective,
    project: effective,
    global: undefined,
    errors: [],
    paths: {
      project: `${projectDir}/.maestro/config.yaml`,
      global: "~/.maestro/config.yaml",
    },
  };
}

describe("CachingConfigPort", () => {
  it("caches config layers per project directory", async () => {
    let loadLayersCalls = 0;
    const inner: ConfigPort = {
      load: async (projectDir) => makeLayers(projectDir).effective,
      loadLayers: async (projectDir) => {
        loadLayersCalls += 1;
        return makeLayers(projectDir);
      },
      write: async () => undefined,
      exists: async () => true,
    };

    const port = new CachingConfigPort(inner);

    const first = await port.loadLayers("/tmp/project-a");
    const second = await port.loadLayers("/tmp/project-b");
    const third = await port.loadLayers("/tmp/project-a");

    expect(first.effective.execution?.defaultWorker).toBe("/tmp/project-a");
    expect(second.effective.execution?.defaultWorker).toBe("/tmp/project-b");
    expect(third.effective.execution?.defaultWorker).toBe("/tmp/project-a");
    expect(loadLayersCalls).toBe(2);
  });

  it("does not cache exists() results across calls", async () => {
    let existsCalls = 0;
    let currentResult = false;
    const inner: ConfigPort = {
      load: async () => ({}),
      loadLayers: async () => makeLayers("/tmp/project"),
      write: async () => undefined,
      exists: async () => {
        existsCalls += 1;
        return currentResult;
      },
    };

    const port = new CachingConfigPort(inner);

    expect(await port.exists("project", "/tmp/project")).toBe(false);
    currentResult = true;
    expect(await port.exists("project", "/tmp/project")).toBe(true);
    expect(existsCalls).toBe(2);
  });
});

describe("setCachedEntry", () => {
  it("evicts the oldest entry when a bounded cache reaches capacity", () => {
    const cache = new Map<string, ReturnType<typeof makeEntry<number>>>();

    setCachedEntry(cache, "first", 1, 1_000, 2);
    setCachedEntry(cache, "second", 2, 1_000, 2);
    setCachedEntry(cache, "third", 3, 1_000, 2);

    expect(cached(cache.get("first"))).toBeUndefined();
    expect(cached(cache.get("second"))).toBe(2);
    expect(cached(cache.get("third"))).toBe(3);
  });
});
