/**
 * Caching wrappers for ports that spawn subprocesses.
 *
 * Bun 1.3.x leaks memory on repeated Bun.spawnSync calls.  The TUI polls
 * buildSnapshot every 1-2 s, which spawns ~6 processes per cycle (4x git,
 * 1x git rev-parse, 1x which cass).  Over hours this accumulates tens of GB.
 *
 * These wrappers add TTL caching so the vast majority of poll cycles spawn
 * zero processes.
 */
import type { GitPort } from "../ports/git.port.js";
import type { CassPort } from "../ports/cass.port.js";
import type { ConfigPort, ConfigLayers, ConfigScope } from "../ports/config.port.js";
import type { GitState, CassSearchResponse, MaestroConfig } from "../domain/types.js";

interface CacheEntry<T> {
  value: T;
  expiresAt: number;
}

function cached<T>(entry: CacheEntry<T> | undefined): T | undefined {
  if (entry && entry.expiresAt > Date.now()) return entry.value;
  return undefined;
}

// ---------------------------------------------------------------------------
// Git
// ---------------------------------------------------------------------------

const GIT_STATE_TTL_MS = 10_000;
const GIT_IS_REPO_TTL_MS = 60_000;

export class CachingGitPort implements GitPort {
  private stateCache: CacheEntry<GitState> | undefined;
  private isRepoCache: CacheEntry<boolean> | undefined;

  constructor(
    private readonly inner: GitPort,
    private readonly stateTtlMs = GIT_STATE_TTL_MS,
    private readonly isRepoTtlMs = GIT_IS_REPO_TTL_MS,
  ) {}

  async getState(cwd: string): Promise<GitState> {
    const hit = cached(this.stateCache);
    if (hit !== undefined) return hit;

    const value = await this.inner.getState(cwd);
    this.stateCache = { value, expiresAt: Date.now() + this.stateTtlMs };
    return value;
  }

  async isRepo(cwd: string): Promise<boolean> {
    const hit = cached(this.isRepoCache);
    if (hit !== undefined) return hit;

    const value = await this.inner.isRepo(cwd);
    this.isRepoCache = { value, expiresAt: Date.now() + this.isRepoTtlMs };
    return value;
  }
}

// ---------------------------------------------------------------------------
// Cass
// ---------------------------------------------------------------------------

const CASS_HAS_BINARY_TTL_MS = 60_000;

export class CachingCassPort implements CassPort {
  private hasBinaryCache: CacheEntry<boolean> | undefined;

  constructor(
    private readonly inner: CassPort,
    private readonly hasBinaryTtlMs = CASS_HAS_BINARY_TTL_MS,
  ) {}

  async hasBinary(): Promise<boolean> {
    const hit = cached(this.hasBinaryCache);
    if (hit !== undefined) return hit;

    const value = await this.inner.hasBinary();
    this.hasBinaryCache = { value, expiresAt: Date.now() + this.hasBinaryTtlMs };
    return value;
  }

  // Pass-through methods (not called during polling)

  async isAvailable(): Promise<boolean> {
    return this.inner.isAvailable();
  }

  async indexOnce(sessionPaths: readonly string[]): Promise<void> {
    return this.inner.indexOnce(sessionPaths);
  }

  async search(
    query: string,
    options: { agent?: string; workspace?: string; limit?: number },
  ): Promise<CassSearchResponse> {
    return this.inner.search(query, options);
  }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

const CONFIG_LAYERS_TTL_MS = 10_000;

export class CachingConfigPort implements ConfigPort {
  private layersCache: CacheEntry<ConfigLayers> | undefined;

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
    this.layersCache = { value, expiresAt: Date.now() + this.layersTtlMs };
    return value;
  }

  async write(scope: ConfigScope, projectDir: string, config: MaestroConfig): Promise<void> {
    this.layersCache = undefined; // invalidate on write
    return this.inner.write(scope, projectDir, config);
  }

  async exists(scope: ConfigScope, projectDir: string): Promise<boolean> {
    return this.inner.exists(scope, projectDir);
  }
}

// ---------------------------------------------------------------------------
// Bun.which() cache
// ---------------------------------------------------------------------------

const WHICH_TTL_MS = 120_000;
const whichCache = new Map<string, CacheEntry<string | null>>();

/**
 * Cached wrapper around Bun.which().
 * Bun.which() resolves PATH on each call; caching avoids repeated lookups
 * inside the TUI polling loop.
 */
export function cachedWhich(command: string): string | null {
  const entry = whichCache.get(command);
  if (entry && entry.expiresAt > Date.now()) return entry.value;

  const value = Bun.which(command) ?? null;
  whichCache.set(command, { value, expiresAt: Date.now() + WHICH_TTL_MS });
  return value;
}
