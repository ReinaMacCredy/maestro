import { expect, test } from "bun:test";
import {
  Cli,
  CliError,
  requiredPosition,
  stringOption,
  stringOptions,
  type CliCommandDescriptor,
  type CliInvocation,
} from "../src/kernel/cli.ts";
import { Events } from "../src/kernel/events.ts";
import { Loader } from "../src/kernel/loader.ts";
import { EventLog } from "../src/kernel/log.ts";
import { Sessions } from "../src/kernel/sessions.ts";
import { resolveStoreLocation, Store } from "../src/kernel/store.ts";
import { builtInPlugins } from "../src/plugins/index.ts";
import { runCli, runFixturePluginCliAt, type Fixture, withFixture } from "./helpers.ts";

async function describeFixtureCommands(fixture: Fixture): Promise<CliCommandDescriptor[]> {
  const location = resolveStoreLocation(fixture.repo);
  const store = new Store(location.path);
  const cli = new Cli();
  const events = new Events();
  const log = new EventLog(store);
  const sessions = new Sessions(store, location.root);
  const loader = new Loader(
    fixture.repo,
    fixture.home,
    builtInPlugins,
    { cli, events, log, sessions, store },
  );
  try {
    await loader.loadAll();
    return cli.describeCommands();
  } finally {
    await loader.unloadAll();
    store.close();
  }
}

test("299 CLI value decoders preserve positional, scalar, repeated, and absent contracts", () => {
  const invocation: CliInvocation = {
    command: "fixture",
    options: { absent: false, repeated: ["first", "second"], scalar: "value" },
    positionals: ["position"],
  };
  expect(requiredPosition(invocation, 0, "fixture id")).toBe("position");
  expect(stringOption(invocation, "scalar")).toBe("value");
  expect(stringOption(invocation, "absent")).toBeUndefined();
  expect(stringOptions(invocation, "repeated")).toEqual(["first", "second"]);
  expect(stringOptions(invocation, "scalar")).toEqual(["value"]);
  expect(stringOptions(invocation, "absent")).toEqual([]);
  try {
    requiredPosition(invocation, 1, "fixture id");
    throw new Error("missing positional was accepted");
  } catch (error) {
    expect(error).toBeInstanceOf(CliError);
    expect(error).toEqual(expect.objectContaining({
      code: "MISSING_ARGUMENT",
      message: "missing fixture id",
    }));
  }
});

test("25 bare and help invocations list verbs while unknown verbs suggest the nearest verb", async () => {
  await withFixture(async (fixture) => {
    const bare = await runCli(fixture, []);
    const help = await runCli(fixture, ["help"]);
    const flagHelp = await runCli(fixture, ["--help"]);
    const unknown = await runCli(fixture, ["wrok", "discarded-tail"]);
    const error = JSON.parse(unknown.stderr.trim());

    for (const result of [bare, help, flagHelp]) {
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("work");
      expect(result.stdout).toContain("plugin");
      expect(result.stdout).toContain("install");
    }
    expect(unknown.exitCode).not.toBe(0);
    expect(error.error.code).toBe("UNKNOWN_VERB");
    expect(error.error.message).toContain("work");
    expect(error.error.message).not.toContain("discarded-tail");
  });
});

test("34 [lint] per-verb help is registry-driven for built-in and plugin verbs", async () => {
  await withFixture(async (fixture) => {
    // Proves help metadata rendering, not handler parsing or command effects.
    const helpWork = await runCli(fixture, ["help", "work"]);
    const flagWork = await runCli(fixture, ["work", "--help"]);

    expect(helpWork.exitCode).toBe(0);
    expect(flagWork.exitCode).toBe(0);
    expect(flagWork.stdout).toBe(helpWork.stdout);
    for (const subverb of ["add", "start", "note", "done", "show", "list"]) {
      expect(helpWork.stdout).toMatch(new RegExp(`^  ${subverb} {2,}\\S`, "m"));
    }
    for (const flag of [
      "--claim",
      "--proof",
      "--evidence",
      "--parent",
      "--kind",
      "--atomic-reason",
    ]) {
      expect(helpWork.stdout).toContain(flag);
    }
    expect(helpWork.stdout).toContain("Record a completion claim.");
    expect(helpWork.stdout).toContain("Record proof paired with a claim.");
    expect(helpWork.stdout).toContain("Record opaque completion evidence.");

    const pluginHelp = await runFixturePluginCliAt(fixture, fixture.repo, ["greet"], ["help", "greet"]);
    expect(pluginHelp.exitCode).toBe(0);
    expect(pluginHelp.stdout).toContain("wave");
    expect(pluginHelp.stdout).toContain("Wave hello to the current user.");
    expect(pluginHelp.stdout).toContain("--enthusiasm");
    expect(pluginHelp.stdout).toContain("Set the greeting intensity.");
  });
});

test("35 top-level help roots match describeCommands and every registration is described", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["help"]);
    const descriptors = await describeFixtureCommands(fixture);
    const verbSection = result.stdout.split("\n\n", 1)[0] ?? "";
    const helpRows = Object.fromEntries(
      verbSection.split("\n").flatMap((line) => {
        const match = line.match(/^  (\S+) {2,}(.+)$/);
        return match?.[1] && match[2] ? [[match[1], match[2]]] : [];
      }),
    );
    const descriptorRoots = [...new Set([
      "help",
      ...descriptors.map((descriptor) => descriptor.name.split(" ")[0]),
    ])]
      .filter((root): root is string => root !== undefined)
      .sort();

    expect(result.exitCode).toBe(0);
    expect(Object.keys(helpRows).sort()).toEqual(descriptorRoots);
    for (const descriptor of descriptors) {
      expect(descriptor.description.trim().length).toBeGreaterThan(0);
    }
    for (const root of descriptorRoots) {
      expect(helpRows[root]?.trim().length ?? 0).toBeGreaterThan(0);
    }
  });
});

test("--help after a verb's arguments prints that verb's help", async () => {
  await withFixture(async (fixture) => {
    const plain = await runCli(fixture, ["work", "done", "--help"]);
    for (const args of [["work", "done", "w1", "--help"], ["work", "done", "w1", "--evidence", "ran it", "--help"]]) {
      const result = await runCli(fixture, args);
      expect(result.exitCode).toBe(0);
      expect(result.stdout).toBe(plain.stdout);
    }
  });
});
