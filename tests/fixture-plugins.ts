import { Cli, failureEnvelope } from "../src/kernel/cli.ts";
import { Events } from "../src/kernel/events.ts";
import { Loader, type BuiltInPlugin } from "../src/kernel/loader.ts";
import { EventLog } from "../src/kernel/log.ts";
import { Sessions } from "../src/kernel/sessions.ts";
import { resolveStoreLocation, Store } from "../src/kernel/store.ts";
import { builtInPlugins } from "../src/plugins/index.ts";
import type { RecipeService } from "../src/plugins/recipe.ts";

const fixturePlugins: Record<string, BuiltInPlugin> = {
  "start-delay": {
    name: "start-delay",
    apply(context) {
      context.effect(() => context.events.on("work.start", async (input, next) => {
        await Bun.sleep(300);
        return next(input);
      }));
    },
  },
  "transition-delay": {
    name: "transition-delay",
    apply(context) {
      for (const event of ["work.start", "work.done", "work.cancel"]) {
        context.effect(() => context.events.on(event, async (input, next) => {
          if (process.env.MAESTRO_TEST_DELAY_EVENT === event) await Bun.sleep(300);
          return next(input);
        }));
      }
    },
  },
  "find-load": {
    name: "find-load",
    inject: ["recipe"],
    apply(context) {
      const recipe = context.recipe as RecipeService;
      for (let index = 0; index < 12; index += 1) {
        const suffix = String(index).padStart(2, "0");
        context.effect(() => context.cli.register(
          `task ${suffix}`,
          () => "ok",
          { description: "Track a task through a loaded verb." },
        ));
      }
      for (let index = 0; index < 7; index += 1) {
        const suffix = String(index).padStart(2, "0");
        context.effect(() => recipe.register({
          name: `task-recipe-${suffix}`,
          description: "Track a task through a loaded recipe.",
          body: "# Loaded",
        }));
      }
    },
  },
  greet: {
    name: "greet",
    apply(context) {
      context.effect(() => context.cli.register(
        "greet wave",
        async () => "hello",
        {
          description: "Wave hello to the current user.",
          flags: { "--enthusiasm": { value: true, description: "Set the greeting intensity." } },
        },
      ));
    },
  },
  "moon-tools": {
    name: "moon-tools",
    apply(context) {
      context.effect(() => context.cli.register(
        "moon launch",
        () => ({ data: { launched: true }, text: "launched" }),
        {
          description: "Launch moon work through a plugin verb.",
          flags: { "--target": { description: "Choose the launch target.", value: true } },
        },
      ));
    },
  },
  "moon-disabled": {
    name: "moon-disabled",
    apply(context) {
      context.effect(() => context.cli.register("moon secret", () => "hidden"));
    },
  },
};

async function main(args: string[]): Promise<number> {
  const extra = (process.env.MAESTRO_TEST_PLUGINS ?? "")
    .split(",")
    .filter(Boolean)
    .map((name) => {
      const plugin = fixturePlugins[name];
      if (!plugin) throw new Error(`unknown fixture plugin: ${name}`);
      return plugin;
    });
  const repo = process.cwd();
  const location = resolveStoreLocation(repo);
  const store = new Store(location.path);
  const cli = new Cli();
  const events = new Events();
  const log = new EventLog(store);
  const sessions = new Sessions(store, location.root);
  const loader = new Loader(repo, process.env.HOME ?? repo, [...builtInPlugins, ...extra], {
    cli,
    events,
    log,
    sessions,
    store,
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

if (import.meta.main) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${JSON.stringify(failureEnvelope(error))}\n`);
    process.exitCode = 1;
  }
}
