// w740 (room d151-d155): the question-back seat's store mechanisms.
// d898 the design signal - `maestro bundle open --work <slp id>` wakes the
// item's reviewer once; d899 `maestro decide --work <id>` wakes the item's
// assignee, which is what makes the Hub's order reach the Lead.
import { expect, test } from "bun:test";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { fakeHerdrCommands, installFakeHerdr } from "./helpers-herdr.ts";
import { runCli, runCliAt, withFixture, type Fixture } from "./helpers.ts";

interface StartedTeam {
  team: {
    generation: number;
    roles: Array<{ name: string; paneId: string; role: string }>;
    teamId: string;
    workspaceId: string;
  };
  work: { id: string };
}

function envelope<T>(stdout: string): T {
  return (JSON.parse(stdout) as { data: T }).data;
}

async function startTeam(fixture: Fixture) {
  const room = await scaffoldRoom(fixture.home);
  expect(
    (await runCliAt(fixture, room, ["room", "mark"], {
      MAESTRO_ROOM_SCAFFOLD: "1",
      MAESTRO_SESSION_NONE: "1",
    })).exitCode,
  ).toBe(0);
  const fake = await installFakeHerdr(fixture, { runtimePane: "spawn" });
  const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Design the thing", "--json"], fake.env);
  expect(started.exitCode).toBe(0);
  return { data: envelope<StartedTeam>(started.stdout), fake, room };
}

async function pushedLines(fake: Awaited<ReturnType<typeof installFakeHerdr>>) {
  return (await fakeHerdrCommands(fake))
    .filter((command) => command[0] === "agent" && command[1] === "prompt")
    .map((command) => ({ target: command[2] ?? "", line: command[3] ?? "" }));
}

test("questioner-signal: bundle open --work on an SLP item pushes one DESIGN line to that item's reviewer (d898, red 1)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake } = await startTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode).toBe(0);

    const opened = await runCliAt(
      fixture,
      fixture.repo,
      ["bundle", "open", "shape-of-it", "--work", workId],
      leadEnvironment,
    );
    expect(opened.exitCode).toBe(0);

    // The Lead's reviewer is the Hub, so the line lands on the Hub pane.
    const design = (await pushedLines(fake)).filter((push) => push.line.includes(`[${workId} DESIGN]`));
    expect(design).toHaveLength(1);
    expect(design[0]!.line).toStartWith(`[from lead][${workId} DESIGN]`);
    expect(design[0]!.line).toContain("shape-of-it");
    // The Hub form of the line: it names the team, and it points at the room's
    // own status because the Hub cannot read a team item by bare id.
    expect(design[0]!.line).toContain(`in ${data.team.teamId} g${data.team.generation}`);
    expect(design[0]!.line).toEndWith("read: maestro status");
    expect(design[0]!.target).not.toBe(lead.name);
    expect(design[0]!.target).not.toBe(lead.paneId);

    // A6/A2: a second open of the same bundle is a resume or a DUPLICATE, and
    // either way the reviewer is not woken twice for one design.
    await runCliAt(fixture, fixture.repo, ["bundle", "open", "shape-of-it", "--work", workId], leadEnvironment);
    const again = (await pushedLines(fake)).filter((push) => push.line.includes(`[${workId} DESIGN]`));
    expect(again).toHaveLength(1);
  });
}, 60_000);

test("questioner-signal-plain: bundle open outside a running team is unchanged and pushes nothing (d898, anti-goal A3)", async () => {
  await withFixture(async (fixture) => {
    const added = await runCli(fixture, ["work", "add", "plain item", "--json"]);
    expect(added.exitCode).toBe(0);
    const id = (JSON.parse(added.stdout) as { data: { work: { id: string } } }).data.work.id;
    const opened = await runCli(fixture, ["bundle", "open", "plain-bundle", "--work", id]);
    expect(opened.stderr).toBe("");
    expect(opened.exitCode).toBe(0);
    expect(opened.stdout).toContain("plain-bundle");
  });
}, 30_000);

test("questioner-order: maestro decide --work wakes the item's assignee, which is how the Hub's order reaches the Lead (d899, red 3)", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = data.work.id;
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode).toBe(0);

    const ruled = await runCliAt(
      fixture,
      room,
      ["decide", "open the question-back pass on this design", "--why", "room d153", "--work", workId],
      fake.env,
    );
    expect(ruled.exitCode).toBe(0);

    const decided = (await pushedLines(fake)).filter((push) => push.line.includes(`[${workId} DECIDED]`));
    expect(decided).toHaveLength(1);
    expect(decided[0]!.target).toBe(lead.name);
    expect(decided[0]!.line).toContain("open the question-back pass on this design");
    expect(decided[0]!.line).toContain(`read: maestro status ${workId}`);
  });
}, 60_000);
