import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { Cli, CliError } from "../src/kernel/cli.ts";
import { Events } from "../src/kernel/events.ts";
import { Loader, type BuiltInPlugin } from "../src/kernel/loader.ts";
import { EventLog } from "../src/kernel/log.ts";
import { Ready } from "../src/kernel/ready.ts";
import { Sessions } from "../src/kernel/sessions.ts";
import { resolveStoreLocation, Store } from "../src/kernel/store.ts";
import { pluginManagerPlugin } from "../src/plugins/plugin-manager.ts";
import { runCli, setPlugin, withFixture, writeConfig, type Fixture } from "./helpers.ts";

const helloPlugin: BuiltInPlugin = {
  name: "hello",
  apply(context) {
    context.effect(() => context.cli.register("hello", async () => "hello from effect"));
  },
};

async function withLoader<T>(
  fixture: Fixture,
  plugins: BuiltInPlugin[],
  run: (kernel: { cli: Cli; events: Events; loader: Loader }) => Promise<T>,
): Promise<T> {
  const location = resolveStoreLocation(fixture.repo);
  const store = new Store(location.path);
  const cli = new Cli();
  const events = new Events();
  const loader = new Loader(fixture.repo, fixture.home, plugins, {
    cli,
    events,
    log: new EventLog(store),
    ready: new Ready(),
    sessions: new Sessions(store, location.root),
    store,
  });
  try {
    await loader.loadAll();
    return await run({ cli, events, loader });
  } finally {
    await loader.unloadAll();
    store.close();
  }
}

async function unknownVerb(cli: Cli, args: string[]): Promise<string | undefined> {
  try {
    await cli.execute(args);
    return undefined;
  } catch (error) {
    return error instanceof CliError ? error.code : String(error);
  }
}

test("1 plugin-registered effect dispatches its verb on the next invocation", async () => {
  await withFixture(async (fixture) => {
    await withLoader(fixture, [helloPlugin], async ({ cli }) => {
      expect((await cli.execute(["hello"])).text).toContain("hello from effect");
    });
  });
});

test("2 a disabled plugin contributes neither verbs nor gates", async () => {
  await withFixture(async (fixture) => {
    const stopper: BuiltInPlugin = {
      name: "stopper",
      apply(context) {
        context.effect(() => context.cli.register("hello", async () => "should not run"));
        context.effect(() => context.events.on("work.start", async () => ({
          blocked: true,
          origin: "stopper",
          reason: "should not run",
        })));
      },
    };
    await writeConfig(fixture, [{ name: "stopper", disabled: true }]);

    await withLoader(fixture, [stopper], async ({ cli, events, loader }) => {
      expect(await unknownVerb(cli, ["hello"])).toBe("UNKNOWN_VERB");
      expect(await events.serial("work.start", {})).toEqual([]);
      expect(loader.records).toEqual([
        { name: "stopper", source: "built-in", status: "disabled", requires: undefined },
      ]);
    });
  });
});

test("4 a missing injected service isolates that plugin and reports the dependency", async () => {
  await withFixture(async (fixture) => {
    const broken: BuiltInPlugin = {
      name: "broken",
      inject: ["nosuch"],
      apply() {
        throw new Error("must not apply");
      },
    };

    await withLoader(fixture, [pluginManagerPlugin, broken, helloPlugin], async ({ cli }) => {
      const listed = await cli.execute(["plugin", "list"]);
      const healthy = await cli.execute(["hello"]);

      expect(listed.text).toContain("broken");
      expect(listed.text).toContain("nosuch");
      expect(healthy.text).toContain("hello from effect");
    });
  });
});

test("5 disabling an enabled plugin unwinds all of its effects", async () => {
  await withFixture(async (fixture) => {
    await withLoader(fixture, [helloPlugin], async ({ cli, loader }) => {
      expect((await cli.execute(["hello"])).text).toContain("hello from effect");
      await loader.unload("hello");
      expect(await unknownVerb(cli, ["hello"])).toBe("UNKNOWN_VERB");
    });

    await setPlugin(fixture, "topic", true);
    const enabled = await runCli(fixture, ["plugin", "enable", "topic"]);
    const present = await runCli(fixture, ["topic", "list"]);
    const disabled = await runCli(fixture, ["plugin", "disable", "topic"]);
    const absent = await runCli(fixture, ["topic", "list"]);

    expect(enabled.exitCode).toBe(0);
    expect(present.exitCode).toBe(0);
    expect(disabled.exitCode).toBe(0);
    expect(absent.exitCode).not.toBe(0);
    expect(absent.stderr).toContain("unknown verb");
  });
});

test("26 plugin enable refuses an unresolvable source without changing config", async () => {
  await withFixture(async (fixture) => {
    const configPath = join(fixture.repo, ".maestro", "config");
    const before = await readFile(configPath, "utf8");

    const enabled = await runCli(fixture, ["plugin", "enable", "policy-nosuch"]);

    expect(enabled.exitCode).not.toBe(0);
    expect(enabled.stderr).toContain("policy-nosuch");
    expect(enabled.stderr).toContain("source");
    expect(await readFile(configPath, "utf8")).toBe(before);
  });
});

test("d888 a stored config naming plugins that no longer exist loads quietly and loads nothing from disk", async () => {
  await withFixture(async (fixture) => {
    const marker = join(fixture.repo, "leftover-plugin-loaded");
    await mkdir(join(fixture.repo, ".maestro", "plugins"), { recursive: true });
    await writeFile(
      join(fixture.repo, ".maestro", "plugins", "leftover.ts"),
      `import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(marker)}, "loaded");
export default {
  name: "leftover",
  apply(ctx) {
    ctx.effect(() => ctx.cli.register("leftover", async () => "leftover ran"));
  },
};
`,
    );
    await writeConfig(fixture, [
      { name: "import-rust" },
      { name: "plugin-trust", disabled: true },
      { name: "leftover" },
    ]);

    const listed = await runCli(fixture, ["plugin", "list"]);
    const work = await runCli(fixture, ["work", "list"]);
    const leftover = await runCli(fixture, ["leftover"]);

    expect(listed.exitCode).toBe(0);
    expect(listed.stderr).toBe("");
    for (const name of ["import-rust", "plugin-trust", "leftover"]) {
      expect(listed.stdout).not.toContain(name);
    }
    expect(work.exitCode).toBe(0);
    expect(work.stderr).toBe("");
    expect(leftover.exitCode).toBe(2);
    expect(leftover.stderr).toContain("unknown verb");
    expect(existsSync(marker)).toBe(false);
  });
});
