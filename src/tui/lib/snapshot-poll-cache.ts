// TTL-caching wrappers for ports that spawn subprocesses or stat files.
// The TUI polls buildSnapshot every 1-2s; without these wrappers the git
// port alone spawns ~4 processes per cycle, which leaks memory on Bun
// 1.3.x and adds latency.
import type { GitPort } from "@/infra/ports/git.port.js";
import type { ConfigPort, ConfigLayers, ConfigScope } from "@/infra/ports/config.port.js";
import type { GitState } from "@/infra/domain/git-types.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";

export interface CacheEntry<T> {
  readonly value: T;
  readonly expiresAt: number;
}

/** Returns the cached value if still fresh, otherwise undefined. */
export function cached<T>(entry: CacheEntry<T> | undefined): T | undefined {
  if (entry && entry.expiresAt > Date.now()) return entry.value;
  return undefined;
}

/** Build a new cache entry with an expiry `ttlMs` from now. */
export function makeEntry<T>(value: T, ttlMs: number): CacheEntry<T> {
  return { value, expiresAt: Date.now() + ttlMs };
}

// ---------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------

const GIT_STATE_TTL_MS = 10_000;
const GIT_IS_REPO_TTL_MS = 60_000;

export class CachingGitPort implements GitPort {
  private readonly stateByCwd = new Map<string, CacheEntry<GitState>>();
  private readonly isRepoByCwd = new Map<string, CacheEntry<boolean>>();

  constructor(
    private readonly inner: GitPort,
    private readonly stateTtlMs = GIT_STATE_TTL_MS,
    private readonly isRepoTtlMs = GIT_IS_REPO_TTL_MS,
  ) {}

  async getState(cwd: string): Promise<GitState> {
    const hit = cached(this.stateByCwd.get(cwd));
    if (hit !== undefined) return hit;

    const value = await this.inner.getState(cwd);
    this.stateByCwd.set(cwd, makeEntry(value, this.stateTtlMs));
    return value;
  }

  async isRepo(cwd: string): Promise<boolean> {
    const hit = cached(this.isRepoByCwd.get(cwd));
    if (hit !== undefined) return hit;

    const value = await this.inner.isRepo(cwd);
    this.isRepoByCwd.set(cwd, makeEntry(value, this.isRepoTtlMs));
    return value;
  }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

const CONFIG_LAYERS_TTL_MS = 10_000;

export class CachingConfigPort implements ConfigPort {
  private layersCache: CacheEntry<ConfigLayers> | undefined;
  private readonly existsByKey = new Map<string, CacheEntry<boolean>>();

  constructor(
    private readonly inner: ConfigPort,
    private readonly layersTtlMs = CONFIG_LAYERS_TTL_MS,
  ) {}

  async load(projectDir: string): Promise<MaestroConfig> {
    const layers = await this.loadLayers(projectDir);
    return layers.effective;
  }

  async loadLayers(projectDir: string): Promise<ConfigLayers> {
    const hit = cached(this.layersCache);
    if (hit !== undefined) return hit;

    const value = await this.inner.loadLayers(projectDir);
    this.layersCache = makeEntry(value, this.layersTtlMs);
    return value;
  }

  async write(scope: ConfigScope, projectDir: string, config: MaestroConfig): Promise<void> {
    this.layersCache = undefined;
    this.existsByKey.clear();
    return this.inner.write(scope, projectDir, config);
  }

  async exists(scope: ConfigScope, projectDir: string): Promise<boolean> {
    const key = `${scope}:${projectDir}`;
    const hit = cached(this.existsByKey.get(key));
    if (hit !== undefined) return hit;

    const value = await this.inner.exists(scope, projectDir);
    this.existsByKey.set(key, makeEntry(value, this.layersTtlMs));
    return value;
  }
}

// ---------------------------------------------------------------------------
// Bun.which() cache
// ---------------------------------------------------------------------------

const WHICH_TTL_MS = 120_000;
const whichCache = new Map<string, CacheEntry<string | null>>();

export function cachedWhich(command: string): string | null {
  const hit = cached(whichCache.get(command));
  if (hit !== undefined) return hit;

  const value = Bun.which(command) ?? null;
  whichCache.set(command, makeEntry(value, WHICH_TTL_MS));
  return value;
}
