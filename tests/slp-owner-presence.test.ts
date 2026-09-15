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

async function startTeam(fixture: Fixture, leadOnly = false): Promise<Started> {
  const room = await markedRoom(fixture);
  const fake = await installFakeHerdr(fixture);
  const hubEnvironment = { ...fake.env, HERDR_PANE_ID: "hub:p0" };
  const argv = ["team", "start", fixture.repo, "Owner layer", "--json"];
  if (leadOnly) argv.splice(4, 0, "--lead-only");
  const started = await runCliAt(fixture, room, argv, hubEnvironment);
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
    const supervisor = data.team.roles.find((role) => role.role === "team-supervisor")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const supervisorEnvironment = { ...fake.env, HERDR_PANE_ID: supervisor.paneId };
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

    // The Lead records the walk-in; the Team Supervisor is the seat above.
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
        supervisor.name,
        `[from lead][${id} OWNER] owner asked: ship the CSV export too; read: maestro status ${id}`,
      ],
    ]);

    // The store shows the provenance; nothing reads as blocked.
    const shown = await runCliAt(fixture, fixture.repo, ["status", id], supervisorEnvironment);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`note [owner] by ${lead.name}: owner asked: ship the CSV export too`);
    expect(shown.stdout).not.toContain("blocked");
    const shownJson = envelope<{ notes: Array<{ flag: string | null }> }>(
      (await runCliAt(fixture, fixture.repo, ["status", id, "--json"], supervisorEnvironment)).stdout,
    );
    expect(shownJson.notes.map((entry) => entry.flag)).toEqual(["owner"]);

    // The Team Supervisor's walk-in climbs to the Hub pane the same way its
    // --blocked note does, with the team named because the Hub reads the room.
    const supervisorNote = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", id, "owner asked: stop after this item", "--owner", "--json"],
      supervisorEnvironment,
    );
    expect(phaseFree(supervisorNote.stderr)).toBe("");
    expect(supervisorNote.exitCode).toBe(0);
    expect((await pushes(fake)).at(-1)).toEqual([
      "agent",
      "prompt",
      "hub:p0",
      `[from team-supervisor][${id} OWNER] owner asked: stop after this item in ${data.team.teamId} g${data.team.generation}; read: maestro status`,
    ]);

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

    // The Hub is not a seat that notes; a plain repository has no owner flag.
    const hubNote = await runCliAt(fixture, room, ["work", "note", id, "x", "--owner", "--json"], fake.env);
    expect(hubNote.exitCode).toBe(1);
    expect(failureCode(hubNote.stderr)).toBe("INVALID_OPTION");
  });
}, 40_000);

test("d123: in a lead-only generation the Lead's --owner note climbs to the Hub pane, as its --blocked note does", async () => {
  await withFixture(async (fixture) => {
    const { data, fake } = await startTeam(fixture, true);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const id = data.work.id;
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", id], leadEnvironment)).exitCode).toBe(0);
    const noted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", id, "owner asked: rename the flag", "--owner", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(noted.stderr)).toBe("");
    expect(noted.exitCode).toBe(0);
    expect((await pushes(fake)).at(-1)).toEqual([
      "agent",
      "prompt",
      "hub:p0",
      `[from lead][${id} OWNER] owner asked: rename the flag in ${data.team.teamId} g${data.team.generation}; read: maestro status`,
    ]);
  });
}, 30_000);

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
    const unlinked = await runCliAt(
      fixture,
      room,
      ["decide", "Use vendor A", "--why", "advisor", "--provisional", "--json"],
      hubEnvironment,
    );
    expect(unlinked.exitCode).toBe(1);
    expect(failureCode(unlinked.stderr)).toBe("MISSING_ARGUMENT");

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
    expect(help.stdout).not.toContain("--mode");
  });
}, 40_000);
