import { expect, test } from "bun:test";
import { runCli, withFixture, writePlugin } from "./helpers.ts";

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

test("34 per-verb help is registry-driven for built-in and plugin verbs", async () => {
  await withFixture(async (fixture) => {
    await writePlugin(
      fixture,
      "repo",
      "greet",
      `
export default {
  name: "greet",
  apply(ctx) {
    ctx.effect(() => ctx.cli.register(
      "greet wave",
      async () => "hello",
      {
        description: "Wave hello to the current user.",
        flags: { "--enthusiasm": { value: true, description: "Set the greeting intensity." } },
      },
    ));
  },
};
`,
    );

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

    const pluginHelp = await runCli(fixture, ["help", "greet"]);
    expect(pluginHelp.exitCode).toBe(0);
    expect(pluginHelp.stdout).toContain("wave");
    expect(pluginHelp.stdout).toContain("Wave hello to the current user.");
    expect(pluginHelp.stdout).toContain("--enthusiasm");
    expect(pluginHelp.stdout).toContain("Set the greeting intensity.");
  });
});

test("35 top-level help lists every root verb with its registered description", async () => {
  await withFixture(async (fixture) => {
    const result = await runCli(fixture, ["help"]);

    expect(result.exitCode).toBe(0);
    for (const verb of [
      "decision",
      "help",
      "hook",
      "install",
      "plugin",
      "ready",
      "recipe",
      "search",
      "status",
      "trace",
      "version",
      "work",
    ]) {
      expect(result.stdout).toMatch(new RegExp(`^  ${verb} {2,}\\S`, "m"));
      expect(result.stdout).not.toMatch(new RegExp(`^  ${verb}$`, "m"));
    }
  });
});
