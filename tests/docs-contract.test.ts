import { expect, test } from "bun:test";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type Fixture } from "./helpers.ts";

const handbackStatuses = [
  "DONE",
  "BLOCKED",
  "UNTESTABLE",
  "UNKNOWN",
  "FAILED",
  "CHALLENGE",
  "REOPEN_REQUEST",
  "DEPENDENCY_REQUEST",
  "COUNCIL_REQUEST",
] as const;

const requestStatuses = new Set<string>([
  "BLOCKED",
  "REOPEN_REQUEST",
  "DEPENDENCY_REQUEST",
  "COUNCIL_REQUEST",
]);

function session(id: string): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(process.pid) };
}

function dispatchId(stdout: string): string {
  const id = stdout.match(/^(x\d+) \[open\]/)?.[1];
  if (!id) throw new Error(`missing dispatch id in stdout: ${stdout}`);
  return id;
}

async function openDispatch(fixture: Fixture, index: number): Promise<string> {
  const work = idFrom(
    await runCli(fixture, [
      "work",
      "add",
      `documented handback ${index}`,
      "--atomic-reason",
      "docs contract",
    ]),
  );
  const opened = await runCli(fixture, [
    "dispatch",
    "open",
    work,
    "--objective",
    "validate one documented return status",
    "--owned-scope",
    "docs contract fixture",
    "--excluded-scope",
    "product source",
    "--mutation",
    "no-write",
    "--stop-condition",
    "handback accepted",
    "--lane",
    "delivery",
    "--evidence-required",
    "source: CLI response",
    "--pane",
    `docs:${index}`,
  ]);
  expect(opened.exitCode).toBe(0);
  return dispatchId(opened.stdout);
}

test("308 SLP Peer return statuses are exactly the runtime vocabulary", async () => {
  const recipe = await Bun.file(join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"))
    .text();
  const peer = recipe.match(/### Peer\n([\s\S]*?)\n## Topology invariants/)?.[1] ?? "";
  const documented = [
    ...new Set([...peer.matchAll(/\b[A-Z][A-Z_]+\b/g)].map((match) => match[0])),
  ];
  expect(documented).toEqual([...handbackStatuses]);

  await withFixture(async (fixture) => {
    for (const [index, status] of handbackStatuses.entries()) {
      const dispatch = await openDispatch(fixture, index);
      const holder = session(`docs-holder-${index}`);
      expect((await runCli(fixture, ["dispatch", "accept", dispatch], holder)).exitCode).toBe(0);
      expect(
        (
          await runCli(fixture, [
            "dispatch",
            "confirm",
            dispatch,
            "--session",
            `docs-holder-${index}`,
          ])
        ).exitCode,
      ).toBe(0);
      const filed = await runCli(
        fixture,
        [
          "handback",
          "file",
          dispatch,
          "--status",
          status,
          ...(requestStatuses.has(status)
            ? ["--request", `documented ${status} request`]
            : []),
          "--claim",
          status === "DEPENDENCY_REQUEST"
            ? "topology dependency requested through the Lead"
            : `documented ${status} return accepted`,
          "--proof",
          "source: CLI response",
          "--assumptions",
          "None",
          "--residual-risks",
          "None",
          "--incidental-findings",
          "None",
        ],
        holder,
      );
      expect(filed.exitCode).toBe(0);
      if (status === "DEPENDENCY_REQUEST") expect(filed.stdout).toContain(status);
    }
  });
  // Nine statuses, each several CLI spawns: 5.4s on an idle developer machine,
  // so the 5s default is a flake waiting for a loaded CI runner.
}, 120_000);

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
});
