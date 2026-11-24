import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { fakeHerdrCommands, installFakeHerdr } from "./helpers-herdr.ts";
import { enableSlp, runCli, runCliAt, withFixture, type Fixture } from "./helpers.ts";

// room d122-d126 (store at ~/maestro): the collaboration and autonomy layer.
// d123: an owner walk-in is recorded as `work note --owner`, a provenance flag
// that stops nobody and pushes one line up. d126: no per-item mode.

interface StartedTeam {
  team: {
    generation: number;
    roles: Array<{ name: string; paneId: string; role: string }>;
    teamId: string;
  };
  work: { id: string };
}

function envelope<T>(stdout: string): T {
  return (JSON.parse(stdout) as { data: T }).data;
}

const runtimePhaseLine =
  /^\S+: (?:starting (?:claude|codex) pane in \S+|waiting for acknowledgement \(up to \d+s\)|ready in \d+s|running in \S+|resetting (?:claude|codex) context in \S+|already (?:acknowledged|running) in \S+; left alone)$/;

function phaseFree(stderr: string): string {
  return stderr
    .split("\n")
    .filter((line) => line !== "" && !runtimePhaseLine.test(line))
    .join("\n");
}

function failureCode(stderr: string): string {
  const line = phaseFree(stderr).split("\n").findLast((candidate) => candidate.startsWith("{"));
  return ((JSON.parse(line ?? "{}") as { error?: { code?: string } }).error?.code) ?? "";
}

async function markedRoom(fixture: Fixture): Promise<string> {
  const room = await scaffoldRoom(fixture.home);
  const marked = await runCliAt(fixture, room, ["room", "mark"], {
    MAESTRO_ROOM_SCAFFOLD: "1",
    MAESTRO_SESSION_NONE: "1",
  });
  expect(marked.exitCode).toBe(0);
  return room;
}

interface Started {
  data: StartedTeam;
  fake: Awaited<ReturnType<typeof installFakeHerdr>>;
  hubEnvironment: Record<string, string | undefined>;
  room: string;
}

async function startTeam(fixture: Fixture): Promise<Started> {
  const room = await markedRoom(fixture);
  const fake = await installFakeHerdr(fixture);
  const hubEnvironment = { ...fake.env, HERDR_PANE_ID: "hub:p0" };
  const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Owner layer", "--json"], hubEnvironment);
  expect(phaseFree(started.stderr)).toBe("");
  expect(started.exitCode).toBe(0);
  return { data: envelope<StartedTeam>(started.stdout), fake, hubEnvironment, room };
}

async function pushes(fake: Started["fake"]): Promise<string[][]> {
  return (await fakeHerdrCommands(fake)).filter(
    (command) =>
      command[0] === "agent" && command[1] === "prompt" && (command[3] ?? "").startsWith("[from "),
  );
}

test("d123: work note --owner records the walk-in with an owner flag, pushes one OWNER line to the seat above, sets no blocked flag, and is exclusive with --blocked and --rework", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, room } = await startTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const id = data.work.id;
    const before = (await pushes(fake)).length;

    expect((await runCliAt(fixture, fixture.repo, ["work", "take", id], leadEnvironment)).exitCode).toBe(0);

    // Exclusive with the two flags that change what a note does.
    for (const other of ["--blocked", "--rework"]) {
      const both = await runCliAt(
        fixture,
        fixture.repo,
        ["work", "note", id, "x", "--owner", other, "--json"],
        leadEnvironment,
      );
      expect(both.exitCode).toBe(1);
      expect(failureCode(both.stderr)).toBe("INVALID_OPTION");
    }
    expect((await pushes(fake)).length).toBe(before);

    // The Lead records the walk-in; the Hub is the seat above, and its line
    // names the team because the Hub reads the room.
    const noted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", id, "owner asked: ship the CSV export too\nsecond line", "--owner", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(noted.stderr)).toBe("");
    expect(noted.exitCode).toBe(0);
    const note = envelope<{ note: { flag: string | null }; work: { state: string } }>(noted.stdout);
    expect(note.note.flag).toBe("owner");
    expect(note.work.state).toBe("ACTIVE");
    expect((await pushes(fake)).slice(before)).toEqual([
      [
        "agent",
        "prompt",
        "hub:p0",
        `[from lead][${id} OWNER] owner asked: ship the CSV export too in ${data.team.teamId} g${data.team.generation}; read: maestro status`,
      ],
    ]);

    // The store shows the provenance; nothing reads as blocked.
    const shown = await runCliAt(fixture, fixture.repo, ["status", id], leadEnvironment);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`note [owner] by ${lead.name}: owner asked: ship the CSV export too`);
    expect(shown.stdout).not.toContain("blocked");
    const shownJson = envelope<{ notes: Array<{ flag: string | null }> }>(
      (await runCliAt(fixture, fixture.repo, ["status", id, "--json"], leadEnvironment)).stdout,
    );
    expect(shownJson.notes.map((entry) => entry.flag)).toEqual(["owner"]);

    // A Peer's walk-in reaches its Lead.
    const peerWork = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer piece", "--to", "peer-walkin", "--json"],
      leadEnvironment,
    );
    expect(peerWork.exitCode).toBe(0);
    const peer = envelope<{ role: { paneId: string }; work: { id: string } }>(peerWork.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peer.role.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", peer.work.id], peerEnvironment)).exitCode).toBe(0);
    const peerNote = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", peer.work.id, "owner asked: use tabs", "--owner", "--json"],
      peerEnvironment,
    );
    expect(phaseFree(peerNote.stderr)).toBe("");
    expect(peerNote.exitCode).toBe(0);
    expect((await pushes(fake)).at(-1)).toEqual([
      "agent",
      "prompt",
      lead.name,
      `[from peer][${peer.work.id} OWNER] owner asked: use tabs; read: maestro status ${peer.work.id}`,
    ]);

    // w736 F1: the Hub is not a seat that receives a walk-in on a team's item.
    // The room now resolves the flag itself and names the reason (room d123)
    // instead of falling through to the room's own work table, whose
    // INVALID_OPTION said only that a plain repository has no owner flag.
    const hubNote = await runCliAt(fixture, room, ["work", "note", id, "x", "--owner", "--json"], fake.env);
    expect(hubNote.exitCode).toBe(1);
    expect(failureCode(hubNote.stderr)).toBe("ROLE_FORBIDDEN");
    expect(hubNote.stderr).toContain("maestro decide --scope owner");
  });
}, 40_000);

test("d126: maestro work add refuses --mode with INVALID_OPTION inside a team and on the Hub path", async () => {
  await withFixture(async (fixture) => {
    const { data, fake } = await startTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const before = (await pushes(fake)).length;
    const inTeam = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Collab piece", "--to", "peer-mode", "--mode", "collab", "--json"],
      leadEnvironment,
    );
    expect(inTeam.exitCode).toBe(1);
    expect(failureCode(inTeam.stderr)).toBe("INVALID_OPTION");
    expect((await pushes(fake)).length).toBe(before);
  });
  await withFixture(async (fixture) => {
    const plain = await runCli(fixture, ["work", "add", "Auto piece", "--mode", "auto", "--json"]);
    expect(plain.exitCode).toBe(1);
    expect(failureCode(plain.stderr)).toBe("INVALID_OPTION");
    const help = await runCli(fixture, ["help", "work", "add"]);
    expect(help.exitCode).toBe(0);
    // room d128 added --model, whose spelling contains --mode; the hidden
    // flag is the whole word.
    expect(help.stdout).not.toMatch(/--mode\b/);
  });
}, 40_000);

// w736 F4: the Hub read its own rulings back with `maestro status dNNN` and
// tried `maestro decision show dNNN` for the same thing, which answered
// NOT_FOUND. The room holds two decision spaces in one store - the project
// ledger's `decisions` (draft/lock/withdraw) and the room's `slp_decisions` -
// and their ids are separate sequences that already overlap in the live room.
// So `decision show` in the room answers from both and labels each, the way
// w697 made status answer from both stores rather than letting one win.
test("w736 decision show in the room resolves a room ruling, and prints both spaces when one id names a row in each", async () => {
  await withFixture(async (fixture) => {
    await enableSlp(fixture);
    const room = await markedRoom(fixture);
    const hubEnvironment = { HERDR_PANE_ID: "hub:p0" };

    const ruled = await runCliAt(
      fixture,
      room,
      ["decide", "Lock the g13 brief as written", "--why", "owner's word in the Hub pane", "--json"],
      hubEnvironment,
    );
    expect(phaseFree(ruled.stderr)).toBe("");
    expect(ruled.exitCode).toBe(0);
    const rulingId = envelope<{ decision: { id: string } }>(ruled.stdout).decision.id;

    // What status already prints, decision show now prints too.
    const status = await runCliAt(fixture, room, ["status", rulingId], hubEnvironment);
    const shown = await runCliAt(fixture, room, ["decision", "show", rulingId], hubEnvironment);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("Lock the g13 brief as written");
    expect(shown.stdout).toContain("(hub store)");
    expect(shown.stdout.trim()).toBe(status.stdout.trim());

    // d894: new drafts take the next id, but a pre-d894 store can hold one id in each space.
    const drafted = await runCliAt(
      fixture,
      room,
      ["decision", "draft", "A ledger decision drafted after the ruling", "--json"],
      hubEnvironment,
    );
    expect(drafted.exitCode).toBe(0);
    const draftedId = envelope<{ decision: { id: string } }>(drafted.stdout).decision.id;
    expect(Number(draftedId.slice(1))).toBe(Number(rulingId.slice(1)) + 1);
    const ledgerId = rulingId;
    const roomStore = new Database(join(room, ".maestro", "maestro.db"));
    try {
      roomStore
        .query(
          `INSERT INTO decisions (id, text, state, created_at, updated_at)
           SELECT ?, 'A ledger decision that happens to share an id', state, created_at, updated_at
           FROM decisions WHERE id = ?`,
        )
        .run(ledgerId, draftedId);
    } finally {
      roomStore.close();
    }

    const both = await runCliAt(fixture, room, ["decision", "show", ledgerId], hubEnvironment);
    expect(both.exitCode).toBe(0);
    expect(both.stdout).toContain("A ledger decision that happens to share an id");
    expect(both.stdout).toContain("Lock the g13 brief as written");
    // w743 F5: both spaces live in this one store, so the store label cannot
    // separate them and the ledger's own render names no space at all. When the
    // id answers in both, each block says which space answered.
    expect(both.stdout).toContain(`${ledgerId} in the decision ledger:`);
    expect(both.stdout).toContain(`${ledgerId} in the SLP rulings:`);

    // An id in neither space still fails, and it names the room verb that reads
    // rulings so the reader is not sent to a third place.
    const missing = await runCliAt(fixture, room, ["decision", "show", "d4242", "--json"], hubEnvironment);
    expect(missing.exitCode).toBe(1);
    expect(failureCode(missing.stderr)).toBe("NOT_FOUND");
  });
});

// project lesson l4 (w719, w720, d121): F4 asked the same question of the room
// only, so inside a project checkout every ruling a team records - d879-d903 in
// the live store - answered NOT_FOUND to `decision show` while `status` printed
// it. A reader who gets NOT_FOUND from one store and a row from another learns
// the opposite of where the row lives, which is the half of l4 the convention
// alone cannot reach.
test("l4 decision show inside a project checkout reads that store's SLP rulings and labels them as the project store", async () => {
  await withFixture(async (fixture) => {
    const { data, fake } = await startTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", data.work.id], leadEnvironment)).exitCode).toBe(0);
    const ruled = await runCliAt(
      fixture,
      fixture.repo,
      ["decide", "Split the module by hand", "--why", "the generated map is wrong", "--work", data.work.id, "--json"],
      leadEnvironment,
    );
    expect(ruled.exitCode).toBe(0);
    const rulingId = envelope<{ decision: { id: string } }>(ruled.stdout).decision.id;

    // Unbound: a seat's own pane has `decision show` retired by the v2 cutover,
    // so this is the read every other session in the checkout performs.
    const shown = await runCli(fixture, ["decision", "show", rulingId]);
    expect({ code: shown.exitCode, stderr: shown.stderr }).toMatchObject({ code: 0 });
    expect(shown.stdout).toContain("Split the module by hand");
    // The label names the store the row was read from, not the seat that wrote
    // it: that inversion is what let a project row read as a hub row.
    expect(shown.stdout).toContain("(project store)");
    expect(shown.stdout).not.toContain("(hub store)");
    // w743 F5: one space answered, so there is no collision to head off.
    expect(shown.stdout).not.toContain("in the SLP rulings:");
    expect(shown.stdout).not.toContain("in the decision ledger:");

    const json = await runCli(fixture, ["decision", "show", rulingId, "--json"]);
    expect(json.exitCode).toBe(0);
    expect(envelope<{ slpDecisions: Array<{ id: string; store: string }> }>(json.stdout).slpDecisions)
      .toMatchObject([{ id: rulingId, store: "project" }]);

    const missing = await runCli(fixture, ["decision", "show", "d4242", "--json"]);
    expect(missing.exitCode).toBe(1);
    expect(failureCode(missing.stderr)).toBe("NOT_FOUND");
  });
}, 40_000);

// No verb writes a provisional row any more; a room store from before the removal still holds them.
test("d125: a stored provisional ruling stays listed by the room's status and flagged in the decision view until a --replaces decision clears it", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, hubEnvironment, room } = await startTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const id = data.work.id;

    const database = new Database(join(room, ".maestro", "maestro.db"));
    try {
      const insert = database.query(
        `INSERT INTO slp_decisions
          (id, team_id, generation, choice, why, scope, work_id, replaces_id, actor, created_at, provisional)
         VALUES (?, ?, ?, ?, ?, 'owner', ?, NULL, 'hub-supervisor', ?, 1)`,
      );
      insert.run("d1", "hub", 0, "Lock SPEC section 4 as written", "advisor; no item exists yet", null, "2026-01-01T00:00:00.000Z");
      insert.run("d2", data.team.teamId, data.team.generation, "Use vendor A", "advisor: A is already licensed", id, "2026-01-01T00:00:01.000Z");
    } finally {
      database.close();
    }

    const listed = await runCliAt(fixture, room, ["status"], hubEnvironment);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout.split("\n")).toContain("provisional: d1 none Lock SPEC section 4 as written");
    expect(listed.stdout.split("\n")).toContain(`provisional: d2 ${id} Use vendor A`);
    const listedJson = envelope<{ provisionalDecisions: Array<{ id: string }> }>(
      (await runCliAt(fixture, room, ["status", "--json"], hubEnvironment)).stdout,
    );
    expect(listedJson.provisionalDecisions.map((decision) => decision.id)).toEqual(["d1", "d2"]);
    expect((await runCliAt(fixture, room, ["status", "d2"], hubEnvironment)).stdout).toContain("provisional: yes");
    expect((await runCliAt(fixture, fixture.repo, ["status", "d2"], leadEnvironment)).stdout).toContain("provisional: yes");

    const linked = await runCliAt(
      fixture,
      room,
      ["decide", "Use vendor A after all", "--why", "owner's word", "--work", id, "--replaces", "d2", "--json"],
      hubEnvironment,
    );
    expect(phaseFree(linked.stderr)).toBe("");
    expect(linked.exitCode).toBe(0);
    expect((await runCliAt(fixture, room, ["status", "d2"], hubEnvironment)).stdout).not.toContain("provisional");
    const afterLinked = await runCliAt(fixture, room, ["status"], hubEnvironment);
    expect(afterLinked.stdout).not.toContain("provisional: d2");
    expect(afterLinked.stdout.split("\n")).toContain("provisional: d1 none Lock SPEC section 4 as written");

    const unlinked = await runCliAt(
      fixture,
      room,
      ["decide", "Lock SPEC section 4 with the owner's edits", "--why", "owner's word", "--replaces", "d1", "--json"],
      hubEnvironment,
    );
    expect(phaseFree(unlinked.stderr)).toBe("");
    expect(unlinked.exitCode).toBe(0);
    expect((await runCliAt(fixture, room, ["status", "d1"], hubEnvironment)).stdout).not.toContain("provisional");
    expect((await runCliAt(fixture, room, ["status"], hubEnvironment)).stdout).not.toContain("provisional:");
    expect(
      envelope<{ provisionalDecisions: unknown[] }>((await runCliAt(fixture, room, ["status", "--json"], hubEnvironment)).stdout)
        .provisionalDecisions,
    ).toEqual([]);
  });
}, 40_000);

// Outside the room there is one decision space and `decision show` keeps
// exactly the behaviour it had.
test("w736 decision show outside the room is unchanged: the project ledger answers and an unknown id is NOT_FOUND", async () => {
  await withFixture(async (fixture) => {
    const drafted = await runCli(fixture, ["decision", "draft", "A project decision", "--json"]);
    expect(drafted.exitCode).toBe(0);
    const id = envelope<{ decision: { id: string } }>(drafted.stdout).decision.id;
    const shown = await runCli(fixture, ["decision", "show", id]);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain("A project decision");
    expect(shown.stdout).not.toContain("hub store");
    const missing = await runCli(fixture, ["decision", "show", "d4242", "--json"]);
    expect(missing.exitCode).toBe(1);
    expect(failureCode(missing.stderr)).toBe("NOT_FOUND");
  });
});
