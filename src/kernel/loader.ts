import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import type { Cli } from "./cli.ts";
import type { Disposer, Events } from "./events.ts";
import type { EventLog } from "./log.ts";
import type { Ready } from "./ready.ts";
import type { Sessions } from "./sessions.ts";
import type { Store } from "./store.ts";

export type PluginSource = "built-in" | "global" | "repo";

export interface Plugin {
  name: string;
  inject?: string[];
  requires?: string;
  apply(context: PluginContext, config?: unknown): void | Promise<void>;
}

export interface BuiltInPlugin extends Plugin {
  defaultDisabled?: boolean;
}

export interface PluginRecord {
  artifact?: "directory" | "file";
  name: string;
  source: PluginSource;
  status: "active" | "disabled" | "error" | "unloaded";
  diagnostic?: string;
  path?: string;
  requires?: string;
}

export interface PluginContext {
  [service: string]: unknown;
  cli: Cli;
  effect(factory: () => Disposer | void): void;
  events: Events;
  loader: Loader;
  log: EventLog;
  provide(name: string, value: unknown): Disposer;
  ready: Ready;
  sessions: Sessions;
  store: Store;
}

interface PluginEntry {
  config?: unknown;
  disabled?: boolean;
  name: string;
}

interface ConfigFile {
  plugins?: PluginEntry[];
}

interface Candidate {
  artifact?: "directory" | "file";
  plugin: Plugin;
  source: PluginSource;
  path?: string;
  defaultDisabled?: boolean;
}

export interface LoaderOptions {
  loadExternalPlugins?: boolean;
}

export function resolvePluginEntrypoint(directory: string): string | null {
  const index = join(directory, "index.ts");
  if (existsSync(index)) return index;
  const rootTypeScriptFiles = readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".ts"))
    .map((entry) => join(directory, entry.name));
  return rootTypeScriptFiles.length === 1 ? (rootTypeScriptFiles[0] ?? null) : null;
}

export async function importPluginEntrypoint(path: string): Promise<Plugin> {
  const module = (await import(pathToFileURL(path).href)) as {
    default?: Plugin;
    plugin?: Plugin;
  };
  const plugin = module.default ?? module.plugin;
  if (!plugin?.name || typeof plugin.apply !== "function") {
    throw new Error("module must export { name, apply }");
  }
  return plugin;
}

export class Loader {
  readonly records: PluginRecord[] = [];
  readonly context: PluginContext;
  private readonly effects = new Map<string, Disposer[]>();
  private currentPlugin: string | null = null;
  private readonly services = new Set<string>();
  private readonly config = new Map<string, PluginEntry>();

  constructor(
    private readonly repo: string,
    private readonly home: string,
    private readonly builtIns: readonly BuiltInPlugin[],
    services: {
      cli: Cli;
      events: Events;
      log: EventLog;
      ready: Ready;
      sessions: Sessions;
      store: Store;
    },
    private readonly options: LoaderOptions = {},
  ) {
    const base = {
      ...services,
      loader: this,
      effect: (factory: () => Disposer | void) => this.effect(factory),
      provide: (name: string, value: unknown) => this.provide(name, value),
    } as PluginContext;
    this.context = base;
    for (const name of Object.keys(services)) this.services.add(name);
    this.services.add("loader");
    this.readConfig();
  }

  async loadAll(): Promise<void> {
    const candidates = await this.discover();
    const chosen = new Map<string, Candidate>();
    for (const candidate of candidates) chosen.set(candidate.plugin.name, candidate);

    const pending: Candidate[] = [];
    for (const candidate of chosen.values()) {
      const entry = this.config.get(candidate.plugin.name);
      if (entry?.disabled ?? candidate.defaultDisabled ?? false) {
        this.records.push({
          artifact: candidate.artifact,
          name: candidate.plugin.name,
          source: candidate.source,
          status: "disabled",
          path: candidate.path,
          requires: candidate.plugin.requires,
        });
      } else {
        pending.push(candidate);
      }
    }

    while (pending.length > 0) {
      let progressed = false;
      for (let index = 0; index < pending.length; ) {
        const candidate = pending[index] as Candidate;
        const missing = (candidate.plugin.inject ?? []).filter((name) => !this.services.has(name));
        if (missing.length > 0) {
          index += 1;
          continue;
        }
        pending.splice(index, 1);
        progressed = true;
        await this.load(candidate);
      }
      if (progressed) continue;
      for (const candidate of pending.splice(0)) {
        const missing = (candidate.plugin.inject ?? []).filter((name) => !this.services.has(name));
        this.records.push({
          artifact: candidate.artifact,
          name: candidate.plugin.name,
          source: candidate.source,
          status: "unloaded",
          path: candidate.path,
          diagnostic: `missing service: ${missing.join(", ")}`,
          requires: candidate.plugin.requires,
        });
      }
    }

    for (const entry of this.config.values()) {
      if (chosen.has(entry.name)) continue;
      this.records.push({
        name: entry.name,
        source: "repo",
        status: entry.disabled ? "disabled" : "unloaded",
        diagnostic: "plugin source not found",
      });
    }
  }

  async unload(name: string): Promise<void> {
    const disposers = this.effects.get(name) ?? [];
    for (const disposer of [...disposers].reverse()) await disposer();
    this.effects.delete(name);
    const record = this.records.find((candidate) => candidate.name === name);
    if (record?.status === "active") record.status = "unloaded";
  }

  async unloadAll(): Promise<void> {
    for (const name of [...this.effects.keys()].reverse()) await this.unload(name);
  }

  private effect(factory: () => Disposer | void): void {
    if (!this.currentPlugin) throw new Error("effects may only be registered while applying a plugin");
    const disposer = factory();
    if (!disposer) return;
    const effects = this.effects.get(this.currentPlugin) ?? [];
    effects.push(disposer);
    this.effects.set(this.currentPlugin, effects);
  }

  private provide(name: string, value: unknown): Disposer {
    if (this.services.has(name)) throw new Error(`service already registered: ${name}`);
    this.services.add(name);
    this.context[name] = value;
    return () => {
      this.services.delete(name);
      delete this.context[name];
    };
  }

  private async load(candidate: Candidate): Promise<void> {
    this.currentPlugin = candidate.plugin.name;
    try {
      await candidate.plugin.apply(this.context, this.config.get(candidate.plugin.name)?.config);
      this.records.push({
        artifact: candidate.artifact,
        name: candidate.plugin.name,
        source: candidate.source,
        status: "active",
        path: candidate.path,
        requires: candidate.plugin.requires,
      });
    } catch (error) {
      await this.unload(candidate.plugin.name);
      this.records.push({
        artifact: candidate.artifact,
        name: candidate.plugin.name,
        source: candidate.source,
        status: "error",
        path: candidate.path,
        diagnostic: error instanceof Error ? error.message : String(error),
        requires: candidate.plugin.requires,
      });
    } finally {
      this.currentPlugin = null;
    }
  }

  private readConfig(): void {
    const global = this.parseConfig(join(this.home, ".maestro", "config"));
    const repo = this.parseConfig(join(this.repo, ".maestro", "config"));
    for (const entry of global.plugins ?? []) this.config.set(entry.name, entry);
    for (const entry of repo.plugins ?? []) this.config.set(entry.name, entry);
  }

  private parseConfig(path: string): ConfigFile {
    if (!existsSync(path)) return {};
    try {
      return JSON.parse(readFileSync(path, "utf8")) as ConfigFile;
    } catch (error) {
      throw new Error(
        `invalid plugin config ${path}: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  }

  private async discover(): Promise<Candidate[]> {
    const candidates: Candidate[] = this.builtIns.map((plugin) => ({
      plugin,
      source: "built-in" as const,
      defaultDisabled: plugin.defaultDisabled,
    }));
    if (this.options.loadExternalPlugins ?? true) {
      candidates.push(...(await this.discoverDirectory(join(this.home, ".maestro", "plugins"), "global")));
      candidates.push(...(await this.discoverDirectory(join(this.repo, ".maestro", "plugins"), "repo")));
    }
    return candidates;
  }

  private async discoverDirectory(directory: string, source: PluginSource): Promise<Candidate[]> {
    if (!existsSync(directory)) return [];
    const candidates: Candidate[] = [];
    for (const entry of readdirSync(directory, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const path = entry.isDirectory()
        ? resolvePluginEntrypoint(join(directory, entry.name))
        : entry.isFile() && entry.name.endsWith(".ts")
          ? join(directory, entry.name)
          : null;
      if (!path || !existsSync(path)) continue;
      try {
        const plugin = await importPluginEntrypoint(path);
        candidates.push({
          artifact: entry.isDirectory() ? "directory" : "file",
          plugin,
          source,
          path,
        });
      } catch (error) {
        const name = entry.name.replace(/\.ts$/, "");
        this.records.push({
          artifact: entry.isDirectory() ? "directory" : "file",
          name,
          source,
          status: "error",
          path,
          diagnostic: error instanceof Error ? error.message : String(error),
        });
      }
    }
    return candidates;
  }
}
