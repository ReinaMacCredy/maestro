import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { Cli } from "./cli.ts";
import type { Disposer, Events } from "./events.ts";
import type { EventLog } from "./log.ts";
import type { Sessions } from "./sessions.ts";
import type { Store } from "./store.ts";

export interface Plugin {
  name: string;
  inject?: string[];
  requires?: string;
  apply(context: PluginContext, config?: unknown): void | Promise<void>;
}

export interface BuiltInPlugin extends Plugin {
  applyDisabled?(context: PluginContext): void | Promise<void>;
  defaultDisabled?: boolean;
}

export interface PluginRecord {
  name: string;
  source: "built-in";
  status: "active" | "disabled" | "error" | "unloaded";
  diagnostic?: string;
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
      sessions: Sessions;
      store: Store;
    },
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
    const pending: BuiltInPlugin[] = [];
    const disabled: BuiltInPlugin[] = [];
    for (const plugin of this.builtIns) {
      const entry = this.config.get(plugin.name);
      if (entry?.disabled ?? plugin.defaultDisabled ?? false) {
        this.records.push({
          name: plugin.name,
          source: "built-in",
          status: "disabled",
          requires: plugin.requires,
        });
        disabled.push(plugin);
      } else {
        pending.push(plugin);
      }
    }

    while (pending.length > 0) {
      let progressed = false;
      for (let index = 0; index < pending.length; ) {
        const plugin = pending[index] as BuiltInPlugin;
        const missing = (plugin.inject ?? []).filter((name) => !this.services.has(name));
        if (missing.length > 0) {
          index += 1;
          continue;
        }
        pending.splice(index, 1);
        progressed = true;
        await this.load(plugin);
      }
      if (progressed) continue;
      for (const plugin of pending.splice(0)) {
        const missing = (plugin.inject ?? []).filter((name) => !this.services.has(name));
        this.records.push({
          name: plugin.name,
          source: "built-in",
          status: "unloaded",
          diagnostic: `missing service: ${missing.join(", ")}`,
          requires: plugin.requires,
        });
      }
    }
    for (const plugin of disabled) await this.applyDisabled(plugin);
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

  private async load(plugin: BuiltInPlugin): Promise<void> {
    this.currentPlugin = plugin.name;
    this.context.cli.owner = plugin.name;
    try {
      await plugin.apply(this.context, this.config.get(plugin.name)?.config);
      this.records.push({
        name: plugin.name,
        source: "built-in",
        status: "active",
        requires: plugin.requires,
      });
    } catch (error) {
      await this.unload(plugin.name);
      this.records.push({
        name: plugin.name,
        source: "built-in",
        status: "error",
        diagnostic: error instanceof Error ? error.message : String(error),
        requires: plugin.requires,
      });
    } finally {
      this.currentPlugin = null;
      this.context.cli.owner = null;
    }
  }

  private async applyDisabled(plugin: BuiltInPlugin): Promise<void> {
    if (!plugin.applyDisabled) return;
    this.currentPlugin = plugin.name;
    this.context.cli.owner = plugin.name;
    try {
      await plugin.applyDisabled(this.context);
    } catch (error) {
      await this.unload(plugin.name);
      const record = this.records.find((candidate) => candidate.name === plugin.name);
      if (record) record.diagnostic = error instanceof Error ? error.message : String(error);
    } finally {
      this.currentPlugin = null;
      this.context.cli.owner = null;
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
}
