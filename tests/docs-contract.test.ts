import { expect, test } from "bun:test";
import { join } from "node:path";
import { builtInPlugins } from "../src/plugins/index.ts";
import { runCli, withFixture } from "./helpers.ts";

const slpOperations = [
  "maestro team start",
  "maestro team stop",
  "maestro status [work-id]",
  "maestro work add",
  "maestro work take",
  "maestro work note",
  "maestro work return",
  "maestro work accept",
  "maestro decide",
] as const;

test("308 [lint] the canonical Workspace Pack exposes only the locked SLP v2 contract", async () => {
  const pack = await Bun.file(join(import.meta.dir, "..", "src", "plugins", "resources", "SLP.md"))
    .text();
  const publicSurface = (
    pack.match(/The public SLP surface is exactly:\n\n```text\n([\s\S]*?)\n```/)?.[1] ?? ""
  ).split("\n");
  expect(publicSurface).toEqual([...slpOperations]);
  expect(pack).toContain("OPEN -> ACTIVE -> RETURNED -> DONE");

  const peer = pack.match(
    /<!-- slp:role:peer:begin -->([\s\S]*?)<!-- slp:role:peer:end -->/,
  )?.[1] ?? "";
  for (const allowed of ["status", "take assigned work", "notes", "return results"]) {
    expect(peer).toContain(allowed);
  }
  for (const forbidden of ["team start", "team stop", "accept Peer", "decide technical"]) {
    expect(peer).not.toContain(forbidden);
  }

  const recipe = await Bun.file(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"))
    .text();
  const readme = await Bun.file(join(import.meta.dir, "..", "README.md")).text();
  for (const surface of [recipe, readme]) {
    for (const operation of slpOperations) expect(surface).toContain(operation);
    expect(surface).toContain("~/maestro/SLP.md");
    expect(surface).not.toContain("advisor-<team>");
    expect(surface).not.toContain("observer-<team>");
    expect(surface).not.toContain("maestro-team-sensor");
  }
});

function documentedCommands(readme: string): string[][] {
  const verbTour = readme.match(/## Verb tour\n([\s\S]*?)(?=\n## )/)?.[1] ?? "";
  return [...verbTour.matchAll(/`([^`]+)`/g)]
    .map((match) => (match[1] ?? "").replaceAll(/\s+/g, " ").trim())
    .filter((value) => value.startsWith("maestro "))
    .flatMap((value) => {
      const args = value.split(" ").slice(1).filter((arg) => !/^<[^>]+>$/.test(arg));
      const alternatives = args.findIndex((arg) => arg.includes("|"));
      if (alternatives < 0) return [args];
      return (args[alternatives] as string).split("|").map((alternative) =>
        args.map((arg, index) => index === alternatives ? alternative : arg)
      );
    });
}

test("309 [lint] every README verb-tour command resolves through CLI help", async () => {
  // Proves documentation-to-registry resolution, not valid-fixture execution of each command family.
  const readme = await Bun.file(join(import.meta.dir, "..", "README.md")).text();
  const commands = documentedCommands(readme);
  expect(commands).toContainEqual(["dispatch", "list"]);
  expect(commands).toContainEqual(["handback", "show"]);
  expect(commands).not.toContainEqual(["handback", "list"]);

  await withFixture(async (fixture) => {
    for (const command of commands) {
      const args = command[0]?.startsWith("-") || command[0] === "help"
        ? command
        : ["help", ...command];
      const result = await runCli(fixture, args);
      expect({ command: command.join(" "), exitCode: result.exitCode }).toEqual({
        command: command.join(" "),
        exitCode: 0,
      });
    }
  });
  // 17 documented commands, one CLI process each, serially: 1.38s on an idle
  // machine (junit reporter, three runs, 1.32 to 1.39). The default 5000ms
  // tolerates a 3.6x slowdown and nothing more, and it ran out once during a
  // release gate while several agents shared the machine. 30s keeps the test
  // honest about a genuine hang while surviving the contention this repository
  // actually works under.
}, 30_000);

test("310 [lint] supervised-team site guidance matches the registered lifecycle surface", async () => {
  const docsRoot = join(import.meta.dir, "..", "site", "src", "content", "docs");
  const setup = await Bun.file(join(docsRoot, "getting-started", "slp-setup.md")).text();
  const guide = await Bun.file(join(docsRoot, "guides", "supervised-teams.md")).text();
  const roles = await Bun.file(join(docsRoot, "concepts", "roles.md")).text();
  const lanes = await Bun.file(join(docsRoot, "concepts", "lanes.md")).text();
  const observerMode = await Bun.file(join(docsRoot, "guides", "observer-mode.md")).text();
  const scenarios = await Bun.file(join(docsRoot, "guides", "slp-scenarios.md")).text();
  const reference = await Bun.file(join(docsRoot, "reference", "cli.md")).text();
  const combined = [setup, guide, roles, lanes, observerMode, scenarios, reference].join("\n");

  for (const command of slpOperations) expect(combined).toContain(command);
  expect(guide).toContain("The Observer is the only seat outside the work lifecycle");
  expect(guide).toContain("there is no Advisor, scheduler, health or reconcile");
  expect(guide).toContain("foreground Watch Pane");
  expect(guide).toContain("not an agent");
  expect(guide).toContain("transcript is runtime-only and is deleted at stop.");
  expect(setup).toContain("~/maestro/SLP.md");
  expect(setup).toContain("<project>/.maestro/SLP.md");
  expect(scenarios).not.toContain("herdr workspace create --cwd ~/Code/rewrite");

  await withFixture(async (fixture) => {
    for (const command of [
      ["team"],
      ["team", "start"],
      ["team", "stop"],
      ["status"],
      ["work", "add"],
      ["work", "take"],
      ["work", "note"],
      ["work", "return"],
      ["work", "accept"],
      ["decide"],
    ]) {
      const result = await runCli(fixture, ["help", ...command]);
      expect({ command: command.join(" "), exitCode: result.exitCode }).toEqual({
        command: command.join(" "),
        exitCode: 0,
      });
    }
  });
}, 30_000);

test("311 [lint] SLP v2 is the only registered team architecture and states its cooperative boundary", async () => {
  const root = join(import.meta.dir, "..");
  const pluginNames = builtInPlugins.map((plugin) => plugin.name);
  expect(pluginNames.filter((name) => name === "slp-v2")).toEqual(["slp-v2"]);
  for (const retired of ["team", "team-runtime", "team-observer", "team-sensor", "team-advisor"]) {
    expect(pluginNames).not.toContain(retired);
    expect(await Bun.file(join(root, "src", "plugins", `${retired}.ts`)).exists()).toBe(false);
  }

  const surfaces = await Promise.all([
    Bun.file(join(root, "src", "plugins", "resources", "SLP.md")).text(),
    Bun.file(join(root, "src", "plugins", "recipes", "slp.md")).text(),
    Bun.file(join(root, "README.md")).text(),
    Bun.file(join(root, "site", "src", "content", "docs", "guides", "supervised-teams.md")).text(),
    Bun.file(join(root, "site", "src", "content", "docs", "concepts", "roles.md")).text(),
    Bun.file(join(root, "site", "src", "content", "docs", "getting-started", "slp-setup.md")).text(),
  ]);
  for (const surface of surfaces) {
    expect(surface).toContain("cooperative-agent protocol");
    expect(surface).toContain("not a shell security sandbox");
    expect(surface).toContain("direct Herdr");
  }
});
