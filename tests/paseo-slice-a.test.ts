import { expect, test } from "bun:test";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { idFrom, runCli, withFixture, type CliResult, type Fixture } from "./helpers.ts";

function session(id: string): Record<string, string> {
  return { MAESTRO_SESSION_ID: id, MAESTRO_SESSION_PID: String(process.pid) };
}

function dispatchId(result: CliResult): string {
  const id = result.stdout.match(/^(x\d+) \[open\]/)?.[1];
  if (!id) throw new Error(`missing dispatch id in stdout: ${result.stdout}`);
  return id;
}

function dispatchOpenArgs(work: string, lane: string): string[] {
  return [
    "dispatch",
    "open",
    work,
    "--objective",
    "produce comparison evidence",
    "--owned-scope",
    "read-only investigation",
    "--excluded-scope",
    "candidate selection and writes",
    "--mutation",
    "no-write",
    "--stop-condition",
    "evidence returned",
    "--lane",
    lane,
    "--evidence-required",
    "source: CLI readback",
    "--pane",
    "w1:p-shadow",
  ];
}

async function addWork(fixture: Fixture, title: string): Promise<string> {
  return idFrom(
    await runCli(fixture, ["work", "add", title, "--atomic-reason", "paseo slice fixture"]),
  );
}

test("380 dispatch accepts and renders the shadow lane in open, show, list, and the brief", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "shadow lane readback");
    const opened = await runCli(fixture, dispatchOpenArgs(work, "shadow"));
    expect(opened.exitCode).toBe(0);
    expect(opened.stdout).toContain("lane: shadow");
    const dispatch = dispatchId(opened);

    const shown = await runCli(fixture, ["dispatch", "show", dispatch]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("lane: shadow");
    const listed = await runCli(fixture, ["dispatch", "list", work]);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout).toContain(`| ${dispatch} | shadow |`);

    const brief = await runCli(
      fixture,
      ["hook", "record", "--event", "SessionStart"],
      session("shadow-brief"),
    );
    expect(brief.exitCode).toBe(0);
    expect(brief.stdout).toContain(
      "lane (scout no-write | decision x2-3 | delivery | challenge | shadow no-write)",
    );
  });
});

test("381 dispatch rejects an unknown lane with all five valid names", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "unknown lane rejection");
    const rejected = await runCli(fixture, dispatchOpenArgs(work, "observer"));
    expect(rejected.exitCode).not.toBe(0);
    expect(rejected.stderr).toContain("INVALID_LANE");
    expect(rejected.stderr).toContain(
      "expected one of: scout, decision, delivery, challenge, shadow",
    );
  });
});

test("382 accepting a shadow dispatch never takes the work write lease", async () => {
  await withFixture(async (fixture) => {
    const work = await addWork(fixture, "shadow lane lease boundary");
    const dispatch = dispatchId(await runCli(fixture, dispatchOpenArgs(work, "shadow")));
    const accepted = await runCli(
      fixture,
      ["dispatch", "accept", dispatch],
      session("shadow-holder"),
    );
    expect(accepted.exitCode).toBe(0);
    expect(accepted.stdout).toContain("held by: shadow-holder");

    const shown = await runCli(fixture, ["work", "show", work, "--json"]);
    expect(shown.exitCode).toBe(0);
    const envelope = JSON.parse(shown.stdout) as { data: { work: { heldBy: string | null } } };
    expect(envelope.data.work.heldBy).toBeNull();
  });
});

test("383 SLP maps all five maestro lanes to Paseo dispositions and shadow evidence semantics", async () => {
  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const peer = recipe.match(/### Peer\n([\s\S]*?)\n## Topology invariants/)?.[1] ?? "";
  for (const row of [
    "| scout | Scout | no |",
    "| decision | Architect | no |",
    "| delivery | Engineer/Owner | yes, one owner per scope |",
    "| challenge | Reviewer | no |",
    "| shadow | Shadow | no, evidence only |",
  ]) {
    expect(peer).toContain(row);
  }
  expect(peer).toMatch(/shadow.*comparison evidence/i);
  expect(peer).toMatch(/never.*candidate/i);
});

test("384 lane.md step 5 and the SLP role binding list all five lane types", async () => {
  const room = (
    await readFile(join(import.meta.dir, "..", "src", "plugins", "room.ts"), "utf8")
  ).replace(/\\`/g, "`");
  const stepFive = room.match(/^5\. .*$/m)?.[0] ?? "";
  for (const lane of ["scout", "decision", "delivery", "challenge", "shadow"]) {
    expect(stepFive).toContain(lane);
  }

  const recipe = await readFile(
    join(import.meta.dir, "..", "src", "plugins", "recipes", "slp.md"),
    "utf8",
  );
  const binding = recipe.match(/\| a pane the Lead opened with a dispatch \|.*$/m)?.[0] ?? "";
  for (const lane of ["scout", "decision", "delivery", "challenge", "shadow"]) {
    expect(binding).toContain(lane);
  }
});
