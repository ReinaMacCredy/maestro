/**
 * Caching wrappers for ports that spawn subprocesses.
 *
 * Bun 1.3.x leaks memory on repeated Bun.spawnSync calls.  The TUI polls
 * buildSnapshot every 1-2 s, which spawns ~4 processes per cycle (git +
 * git rev-parse).  Over hours this accumulates tens of GB.
 *
 * These wrappers add TTL caching so the vast majority of poll cycles spawn
 * zero processes.
 */
import type { GitPort } from "@/infra/ports/git.port.js";
import type { ConfigPort, ConfigLayers, ConfigScope } from "@/infra/ports/config.port.js";
import type { GitState } from "@/infra/domain/git-types.js";
import type { MaestroConfig } from "@/infra/domain/config-types.js";

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
