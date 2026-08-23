import { builtInPlugins } from "../plugins/index.ts";
import { Cli } from "./cli.ts";
import { Events } from "./events.ts";
import { Loader } from "./loader.ts";
import { EventLog } from "./log.ts";
import { Ready } from "./ready.ts";
import { Sessions } from "./sessions.ts";
import { resolveStoreLocation, Store } from "./store.ts";

export async function run(args: string[]): Promise<number> {
  const repo = process.cwd();
  const home = process.env.HOME ?? repo;
  const storeLocation = resolveStoreLocation(repo);
  if (storeLocation.orphanPath) {
    process.stderr.write(
      `[orphan] private maestro store left untouched: ${storeLocation.orphanPath}\n`,
    );
  }
  const store = new Store(storeLocation.path);
  const cli = new Cli();
  const events = new Events();
  const log = new EventLog(store);
  const ready = new Ready();
  const sessions = new Sessions(store);
  const loader = new Loader(repo, home, builtInPlugins, {
    cli,
    events,
    log,
    ready,
    sessions,
    store,
  });

  try {
    await loader.loadAll();
    return await cli.dispatch(args);
  } finally {
    await loader.unloadAll();
    store.close();
  }
}
