import { builtInPlugins } from "../plugins/index.ts";
import { Cli, type CliOptions } from "./cli.ts";
import { Events } from "./events.ts";
import { Loader } from "./loader.ts";
import type { BuiltInPlugin, LoaderOptions } from "./loader.ts";
import { EventLog } from "./log.ts";
import { Ready } from "./ready.ts";
import { Sessions } from "./sessions.ts";
import { resolveStoreLocation, Store } from "./store.ts";

export interface RunOptions {
  allowBuiltIn?: (plugin: BuiltInPlugin) => boolean;
  cli?: CliOptions;
  loadExternalPlugins?: boolean;
  readOnly?: boolean;
  trustExternalPlugin?: LoaderOptions["trustExternalPlugin"];
}

export async function run(args: string[], options: RunOptions = {}): Promise<number> {
  const repo = process.cwd();
  const home = process.env.HOME ?? repo;
  const storeLocation = resolveStoreLocation(repo);
  const store = new Store(storeLocation.path, { readonly: options.readOnly });
  const cli = new Cli(options.cli);
  const events = new Events();
  const log = new EventLog(store);
  const ready = new Ready();
  const sessions = new Sessions(store, storeLocation.root);
  const plugins = options.allowBuiltIn ? builtInPlugins.filter(options.allowBuiltIn) : builtInPlugins;
  const loader = new Loader(repo, home, plugins, {
    cli,
    events,
    log,
    ready,
    sessions,
    store,
  }, {
    loadExternalPlugins: options.loadExternalPlugins,
    trustExternalPlugin: options.trustExternalPlugin,
  });

  try {
    await loader.loadAll();
    const exitCode = await cli.dispatch(args);
    sessions.refresh();
    return exitCode;
  } finally {
    await loader.unloadAll();
    store.close();
  }
}
