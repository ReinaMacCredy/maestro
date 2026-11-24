import { expect, test } from "bun:test";
import { scaffoldRoom } from "../src/plugins/room.ts";
import { fakeHerdrCommands, installFakeHerdr } from "./helpers-herdr.ts";
import { runCli, runCliAt, withFixture, type Fixture } from "./helpers.ts";

// room d122-d126 (store at ~/maestro): the collaboration and autonomy layer.
// d123: an owner walk-in is recorded as `work note --owner`, a provenance flag
// that stops nobody and pushes one line up. d124: the room store holds one
// declared presence, here or away, set only by the Hub on the owner's words.
// d125: while away, an owner-scope fork is resolved at once as a provisional
// decision the owner later confirms or supersedes. d126: no per-item mode.

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

test("d124: maestro owner here|away is the Hub's declared presence, default here, refused outside the room, printed on the Hub prompt hook line and by maestro status", async () => {
  await withFixture(async (fixture) => {
    const room = await markedRoom(fixture);
    const environment = { MAESTRO_SESSION_NONE: "1" };

    const initial = await runCliAt(fixture, room, ["owner", "--json"], environment);
    expect(initial.exitCode).toBe(0);
    const initialData = envelope<{ owner: { presence: string; setBy: string | null; updatedAt: string | null } }>(initial.stdout);
    expect(initialData.owner.presence).toBe("here");
    expect((await runCliAt(fixture, room, ["owner"], environment)).stdout.trim()).toBe("owner: here");

    const away = await runCliAt(fixture, room, ["owner", "away", "--json"], environment);
    expect(away.exitCode).toBe(0);
    const awayData = envelope<{ owner: { presence: string; setBy: string; updatedAt: string } }>(away.stdout);
    expect(awayData.owner.presence).toBe("away");
    expect(awayData.owner.setBy).toBe("hub-supervisor");
    expect(awayData.owner.updatedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    expect((await runCliAt(fixture, room, ["owner"], environment)).stdout.trim()).toBe(
      "owner: away; 0 provisional decisions waiting",
    );

    const invalid = await runCliAt(fixture, room, ["owner", "asleep", "--json"], environment);
    expect(invalid.exitCode).toBe(1);
    expect(failureCode(invalid.stderr)).toBe("INVALID_ARGUMENT");

    // Presence belongs to the room store alone.
    const outside = await runCli(fixture, ["owner", "here", "--json"]);
    expect(outside.exitCode).toBe(1);
    expect(failureCode(outside.stderr)).toBe("ROLE_FORBIDDEN");
    const outsideRead = await runCli(fixture, ["owner", "--json"]);
    expect(outsideRead.exitCode).toBe(1);
    expect(failureCode(outsideRead.stderr)).toBe("ROLE_FORBIDDEN");

    // The Hub prompt hook line carries it on both events; a repository does not.
    for (const event of ["SessionStart", "UserPromptSubmit"]) {
      const input = event === "UserPromptSubmit" ? JSON.stringify({ prompt: "back at it" }) : undefined;
      const roomHook = await runCliAt(
        fixture,
        room,
        ["hook", "record", "--event", event, "--harness", "codex"],
        {},
        input,
      );
      expect(roomHook.exitCode).toBe(0);
      expect(roomHook.stdout.split("\n")).toContain("owner: away; 0 provisional decisions waiting");
      const repoHook = await runCli(fixture, ["hook", "record", "--event", event, "--harness", "codex"], {}, input);
      expect(repoHook.exitCode).toBe(0);
      expect(repoHook.stdout).not.toContain("owner:");
    }

    // maestro status from the room prints the same line.
    const status = await runCliAt(fixture, room, ["status"], environment);
    expect(status.exitCode).toBe(0);
    expect(status.stdout.split("\n")).toContain("owner: away; 0 provisional decisions waiting");

    // A hook never flips it (d705): after the prompt hooks above it is still away.
    expect((await runCliAt(fixture, room, ["owner"], environment)).stdout.trim()).toStartWith("owner: away");

    const here = await runCliAt(fixture, room, ["owner", "here"], environment);
    expect(here.exitCode).toBe(0);
    expect((await runCliAt(fixture, room, ["owner"], environment)).stdout.trim()).toBe("owner: here");
    const hereHook = await runCliAt(
      fixture,
      room,
      ["hook", "record", "--event", "UserPromptSubmit", "--harness", "codex"],
      {},
      JSON.stringify({ prompt: "hi" }),
    );
    expect(hereHook.stdout.split("\n")).toContain("owner: here");
    expect((await runCliAt(fixture, room, ["status"], environment)).stdout.split("\n")).toContain("owner: here");
  });
}, 30_000);

test("d125: decide --provisional is a Hub ruling linked to work, listed by the room's status until decision confirm or a --replaces decision clears it", async () => {
  await withFixture(async (fixture) => {
    const { data, fake, hubEnvironment, room } = await startTeam(fixture);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const id = data.work.id;

    // Hub-only, and never without the work it rules on.
    const seatProvisional = await runCliAt(
      fixture,
      fixture.repo,
      ["decide", "Use vendor A", "--why", "lead guess", "--work", id, "--provisional", "--json"],
      leadEnvironment,
    );
    expect(seatProvisional.exitCode).toBe(1);
    expect(failureCode(seatProvisional.stderr)).toBe("ROLE_FORBIDDEN");
    // The Hub-only guard holds with or without an item to link.
    const seatUnlinked = await runCliAt(
      fixture,
      fixture.repo,
      ["decide", "Use vendor A", "--why", "lead guess", "--provisional", "--json"],
      leadEnvironment,
    );
    expect(seatUnlinked.exitCode).toBe(1);
    expect(failureCode(seatUnlinked.stderr)).toBe("ROLE_FORBIDDEN");

    // w736 F2: an owner-scope ruling with no item - a SPEC lock, a brief
    // amendment recorded before the item exists - is provisional like any
    // other away ruling. Refusing it left an away Hub two bad choices: link it
    // to an unrelated item, or record it as final, and final is the one thing
    // an unconfirmed away ruling is not.
    const unlinked = await runCliAt(
      fixture,
      room,
      ["decide", "Lock SPEC section 4 as written", "--why", "advisor; no item exists yet", "--provisional", "--json"],
      hubEnvironment,
    );
    expect(phaseFree(unlinked.stderr)).toBe("");
    expect(unlinked.exitCode).toBe(0);
    const unlinkedRuling = envelope<{
      decision: { id: string; provisional: boolean; workId: string | null };
    }>(unlinked.stdout);
    expect(unlinkedRuling.decision.provisional).toBe(true);
    expect(unlinkedRuling.decision.workId).toBe(null);
    expect(unlinked.stdout).toContain("provisional");
    const unlinkedId = unlinkedRuling.decision.id;
    expect((await runCliAt(fixture, room, ["status"], hubEnvironment)).stdout.split("\n"))
      .toContain(`provisional: ${unlinkedId} none Lock SPEC section 4 as written`);
    expect((await runCliAt(fixture, room, ["status", unlinkedId], hubEnvironment)).stdout)
      .toContain("provisional: yes");
    // It confirms like any other, so it does not sit in the list forever.
    const unlinkedConfirm = await runCliAt(
      fixture,
      room,
      ["decision", "confirm", unlinkedId, "--json"],
      hubEnvironment,
    );
    expect(unlinkedConfirm.exitCode).toBe(0);
    expect(envelope<{ decision: { provisional: boolean } }>(unlinkedConfirm.stdout).decision.provisional)
      .toBe(false);

    const ruled = await runCliAt(
      fixture,
      room,
      ["decide", "Use vendor A", "--why", "advisor: A is already licensed", "--work", id, "--provisional", "--json"],
      hubEnvironment,
    );
    expect(phaseFree(ruled.stderr)).toBe("");
    expect(ruled.exitCode).toBe(0);
    const ruling = envelope<{ decision: { id: string; provisional: boolean; workId: string } }>(ruled.stdout);
    expect(ruling.decision.provisional).toBe(true);
    expect(ruling.decision.workId).toBe(id);
    const decisionId = ruling.decision.id;

    // The flag is visible from the room and from the seat that reads the ruling.
    const hubShow = await runCliAt(fixture, room, ["status", decisionId], hubEnvironment);
    expect(hubShow.exitCode).toBe(0);
    expect(hubShow.stdout).toContain("provisional: yes");
    const seatShow = await runCliAt(fixture, fixture.repo, ["status", decisionId], leadEnvironment);
    expect(seatShow.exitCode).toBe(0);
    expect(seatShow.stdout).toContain("provisional: yes");

    // The room's status lists it whatever the presence; the away line counts it.
    const listed = await runCliAt(fixture, room, ["status"], hubEnvironment);
    expect(listed.exitCode).toBe(0);
    expect(listed.stdout.split("\n")).toContain(`provisional: ${decisionId} ${id} Use vendor A`);
    const listedJson = envelope<{ provisionalDecisions: Array<{ id: string; workId: string }> }>(
      (await runCliAt(fixture, room, ["status", "--json"], hubEnvironment)).stdout,
    );
    expect(listedJson.provisionalDecisions.map((decision) => decision.id)).toEqual([decisionId]);
    expect((await runCliAt(fixture, room, ["owner", "away"], hubEnvironment)).exitCode).toBe(0);
    expect((await runCliAt(fixture, room, ["owner"], hubEnvironment)).stdout.trim()).toBe(
      "owner: away; 1 provisional decision waiting",
    );
    const hook = await runCliAt(
      fixture,
      room,
      ["hook", "record", "--event", "UserPromptSubmit", "--harness", "codex"],
      hubEnvironment,
      JSON.stringify({ prompt: "status?" }),
    );
    expect(hook.stdout.split("\n")).toContain("owner: away; 1 provisional decision waiting");

    // Confirmation is Hub-only and clears the flag once.
    const seatConfirm = await runCliAt(fixture, fixture.repo, ["decision", "confirm", decisionId, "--json"], leadEnvironment);
    expect(seatConfirm.exitCode).toBe(1);
    const confirmed = await runCliAt(fixture, room, ["decision", "confirm", decisionId, "--json"], hubEnvironment);
    expect(phaseFree(confirmed.stderr)).toBe("");
    expect(confirmed.exitCode).toBe(0);
    expect(envelope<{ decision: { provisional: boolean } }>(confirmed.stdout).decision.provisional).toBe(false);
    expect((await runCliAt(fixture, room, ["status", decisionId], hubEnvironment)).stdout).not.toContain("provisional");
    expect((await runCliAt(fixture, room, ["status"], hubEnvironment)).stdout).not.toContain("provisional:");
    expect((await runCliAt(fixture, room, ["owner"], hubEnvironment)).stdout.trim()).toBe(
      "owner: away; 0 provisional decisions waiting",
    );
    const again = await runCliAt(fixture, room, ["decision", "confirm", decisionId, "--json"], hubEnvironment);
    expect(again.exitCode).toBe(1);
    expect(failureCode(again.stderr)).toBe("INVALID_STATE");
    const missing = await runCliAt(fixture, room, ["decision", "confirm", "d999", "--json"], hubEnvironment);
    expect(missing.exitCode).toBe(1);
    expect(failureCode(missing.stderr)).toBe("NOT_FOUND");

    // A superseding decision clears the ruling it replaces.
    const second = envelope<{ decision: { id: string } }>(
      (await runCliAt(
        fixture,
        room,
        ["decide", "Use vendor B", "--why", "advisor: B is cheaper", "--work", id, "--provisional", "--json"],
        hubEnvironment,
      )).stdout,
    ).decision.id;
    expect((await runCliAt(fixture, room, ["status"], hubEnvironment)).stdout).toContain(`provisional: ${second} ${id} Use vendor B`);
    const superseding = await runCliAt(
      fixture,
      room,
      ["decide", "Use vendor A after all", "--why", "owner's word", "--work", id, "--replaces", second, "--json"],
      hubEnvironment,
    );
    expect(phaseFree(superseding.stderr)).toBe("");
    expect(superseding.exitCode).toBe(0);
    expect(envelope<{ decision: { provisional: boolean } }>(superseding.stdout).decision.provisional).toBe(false);
    expect((await runCliAt(fixture, room, ["status", second], hubEnvironment)).stdout).not.toContain("provisional");
    expect((await runCliAt(fixture, room, ["status"], hubEnvironment)).stdout).not.toContain("provisional:");
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

    // The ledger in the same store runs its own dNNN sequence, so one id can
    // name a row in each; neither is allowed to hide the other.
    const drafted = await runCliAt(
      fixture,
      room,
      ["decision", "draft", "A ledger decision that happens to share an id", "--json"],
      hubEnvironment,
    );
    expect(drafted.exitCode).toBe(0);
    const ledgerId = envelope<{ decision: { id: string } }>(drafted.stdout).decision.id;
    expect(ledgerId).toBe(rulingId);

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
