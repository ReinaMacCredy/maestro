import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import {
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
  setFakeHerdrBehavior,
} from "./helpers-herdr.ts";
import { runCliAt, withFixture } from "./helpers.ts";
import { envelope, phaseFree, failureEnvelope } from "./helpers-slp.ts";

test("SLP v2 enforces the locked operation authority matrix without forbidden mutation", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Exercise every role boundary", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: {
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const peerWork = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer matrix work", "--to", "peer-matrix", "--json"],
      leadEnvironment,
    );
    const peerData = envelope<{ role: { paneId: string }; work: { id: string } }>(peerWork.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peerData.role.paneId };

    const allowed: Array<{
      args: string[];
      cwd: string;
      env: Record<string, string | undefined>;
    }> = [
      { args: ["status", "--json"], cwd: room, env: fake.env },
      { args: ["decide", "Owner matrix choice", "--why", "owner authority", "--json"], cwd: room, env: fake.env },
      { args: ["work", "note", data.work.id, "hub note", "--json"], cwd: room, env: fake.env },
      { args: ["status", "--json"], cwd: fixture.repo, env: leadEnvironment },
      { args: ["work", "note", data.work.id, "lead note", "--json"], cwd: fixture.repo, env: leadEnvironment },
      { args: ["decide", "Technical matrix choice", "--why", "lead authority", "--json"], cwd: fixture.repo, env: leadEnvironment },
      { args: ["status", peerData.work.id, "--json"], cwd: fixture.repo, env: peerEnvironment },
      { args: ["work", "note", peerData.work.id, "peer note", "--json"], cwd: fixture.repo, env: peerEnvironment },
    ];
    for (const command of allowed) {
      const result = await runCliAt(fixture, command.cwd, command.args, command.env);
      expect(phaseFree(result.stderr)).toBe("");
      expect(result.exitCode).toBe(0);
    }

    const projectBefore = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const slpBefore = projectBefore
      .query<{ id: string; state: string }, []>("SELECT id, state FROM slp_work ORDER BY id")
      .all();
    const activityBefore = projectBefore
      .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity")
      .get()?.count;
    projectBefore.close();

    const forbidden: Array<{
      args: string[];
      code: string;
      cwd: string;
      env: Record<string, string | undefined>;
      role: string;
    }> = [
      { args: ["work", "add", "Hub team work", "--to", lead.name, "--json"], code: "ROLE_FORBIDDEN", cwd: room, env: fake.env, role: "Hub Supervisor" },
      { args: ["work", "take", data.work.id, "--json"], code: "NO_ACTIVE_TEAM", cwd: room, env: fake.env, role: "Hub Supervisor" },
      { args: ["work", "return", data.work.id, "hub return", "--json"], code: "NO_ACTIVE_TEAM", cwd: room, env: fake.env, role: "Hub Supervisor" },
      // The Hub reviews the Lead and may stop the team, so these two are refused on the item's state, not on role.
      { args: ["work", "accept", data.work.id, "--json"], code: "INVALID_STATE", cwd: room, env: fake.env, role: "Hub Supervisor" },
      { args: ["team", "stop", data.team.teamId, "--json"], code: "TEAM_UNFINISHED", cwd: room, env: fake.env, role: "Hub Supervisor" },
      { args: ["team", "start", fixture.repo, "Lead cannot start", "--json"], code: "ROLE_FORBIDDEN", cwd: fixture.repo, env: leadEnvironment, role: "Lead" },
      { args: ["team", "stop", data.team.teamId, "--json"], code: "ROLE_FORBIDDEN", cwd: fixture.repo, env: leadEnvironment, role: "Lead" },
      { args: ["team", "start", fixture.repo, "Peer cannot start", "--json"], code: "ROLE_FORBIDDEN", cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
      { args: ["team", "stop", data.team.teamId, "--json"], code: "ROLE_FORBIDDEN", cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
      { args: ["work", "add", "Peer cannot add", "--to", "peer-other", "--json"], code: "ROLE_FORBIDDEN", cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
      { args: ["work", "accept", peerData.work.id, "--json"], code: "ROLE_FORBIDDEN", cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
      { args: ["decide", "Peer cannot decide", "--why", "no authority", "--json"], code: "ROLE_FORBIDDEN", cwd: fixture.repo, env: peerEnvironment, role: "Peer" },
    ];
    for (const attempt of forbidden) {
      const result = await runCliAt(fixture, attempt.cwd, attempt.args, attempt.env);
      expect(result.exitCode, `${attempt.role}: ${attempt.args.join(" ")}`).toBe(1);
      expect(failureEnvelope(result.stderr).error.code, `${attempt.role}: ${attempt.args.join(" ")}`).toBe(attempt.code);
    }

    const projectAfter = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      projectAfter.query<{ id: string; state: string }, []>("SELECT id, state FROM slp_work ORDER BY id").all(),
    ).toEqual(slpBefore);
    expect(
      projectAfter.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity").get()?.count,
    ).toBe(activityBefore);
    projectAfter.close();
  });
}, 25_000);

test("SLP v2 team panes refuse the classic verbs SLP replaces and leave legacy records read-only", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const roomDatabasePath = join(room, ".maestro", "maestro.db");
    const legacy = new Database(roomDatabasePath);
    legacy.exec(`
      CREATE TABLE legacy_team_history (
        id TEXT PRIMARY KEY,
        payload TEXT NOT NULL
      );
      INSERT INTO legacy_team_history (id, payload)
      VALUES ('legacy-g1', 'opaque old lifecycle bytes');
    `);
    legacy.close();
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Use only the v2 surface", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
      work: { id: string };
    }>(started.stdout);
    const leadPane = data.team.roles.find((role) => role.role === "lead")?.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    for (const attempt of [
      { args: ["decision", "draft", "old decision", "--json"], cwd: fixture.repo, env: leadEnvironment, replacement: "decide" },
      { args: ["work", "start", data.work.id, "--json"], cwd: fixture.repo, env: leadEnvironment, replacement: "work take" },
    ]) {
      const result = await runCliAt(fixture, attempt.cwd, attempt.args, attempt.env);
      expect(result.exitCode).toBe(1);
      const error = JSON.parse(result.stderr) as { error: { code: string; message: string } };
      expect(error.error.code).toBe("SLP_V2_CUTOVER");
      expect(error.error.message).toContain(attempt.replacement);
    }
    const roomAfter = new Database(roomDatabasePath, { readonly: true });
    expect(
      roomAfter
        .query<{ payload: string }, []>("SELECT payload FROM legacy_team_history WHERE id = 'legacy-g1'")
        .get()?.payload,
    ).toBe("opaque old lifecycle bytes");
    roomAfter.close();
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(project.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM work").get()?.count)
      .toBe(0);
    expect(project.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work").get()?.count)
      .toBe(1);
    project.close();

    const emergencyStopped = await runCliAt(
      fixture,
      room,
      ["team", "stop", data.team.teamId, "--emergency", "--json"],
      fake.env,
    );
    expect(emergencyStopped.exitCode).toBe(0);
    for (const administrative of [
      ["work", "list", "--json"],
      ["decision", "list", "--json"],
    ]) {
      const result = await runCliAt(
        fixture,
        fixture.repo,
        administrative,
        leadEnvironment,
      );
      expect(result.exitCode, administrative.join(" ")).toBe(0);
    }
    for (const args of [
      ["work", "add", "must not reach legacy work", "--json"],
      ["work", "note", data.work.id, "must not reach legacy notes", "--json"],
    ]) {
      const retiredSharedVerb = await runCliAt(
        fixture,
        fixture.repo,
        args,
        leadEnvironment,
      );
      expect(retiredSharedVerb.exitCode).toBe(1);
      expect(JSON.parse(retiredSharedVerb.stderr).error.code).toBe("NO_ACTIVE_TEAM");
    }
    const afterSharedVerbs = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(afterSharedVerbs.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM work").get()?.count)
      .toBe(0);
    expect(
      afterSharedVerbs
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE kind = 'NOTE'",
        )
        .get()?.count,
    ).toBe(0);
    afterSharedVerbs.close();
  });
}, 20_000);

test("SLP v2 pushes one notice line to the counterpart after return, rework, and accept", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Push notices", "--json"],
      { ...fake.env, HERDR_PANE_ID: "hub:p0" },
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const notices = async () =>
      (await fakeHerdrCommands(fake)).filter(
        (command) =>
          command[0] === "agent" && command[1] === "prompt" && (command[3] ?? "").startsWith("[from "),
      );
    const id = data.work.id;
    // w696: team start already pushed this item's OPEN line to the Lead.
    const beforeNotices = (await notices()).length;

    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "take", id, "--json"], leadEnvironment)).exitCode,
    ).toBe(0);
    const returned = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "return", id, "result: first pass\nmore detail below", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(returned.stderr)).toBe("");
    expect(returned.exitCode).toBe(0);
    expect((await notices()).slice(beforeNotices)).toEqual([
      ["agent", "prompt", "hub:p0", `[from lead][${id} RETURNED] result: first pass; read: maestro status ${id}`],
    ]);

    const rework = await runCliAt(
      fixture,
      room,
      ["work", "note", id, "needs a test", "--rework", "--json"],
      fake.env,
    );
    expect(phaseFree(rework.stderr)).toBe("");
    expect(rework.exitCode).toBe(0);
    expect((await notices()).at(-1)).toEqual([
      "agent",
      "prompt",
      lead.name,
      `[from hub-supervisor][${id} RETURNED] rework granted: needs a test; read: maestro status ${id}`,
    ]);

    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "take", id, "--json"], leadEnvironment)).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", id, "result: with test", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const accepted = await runCliAt(
      fixture,
      room,
      ["work", "accept", id, "--json"],
      fake.env,
    );
    expect(phaseFree(accepted.stderr)).toBe("");
    expect(accepted.exitCode).toBe(0);
    expect((await notices()).slice(-2)).toEqual([
      ["agent", "prompt", lead.name, `[from hub-supervisor][${id} DONE] accepted; read: maestro status ${id}`],
      // The Hub notice goes to the pane that ran team start (row 17 g18), not
      // to a Herdr agent named supervisor.
      [
        "agent",
        "prompt",
        "hub:p0",
        `[from hub-supervisor][${id} DONE] accepted in ${data.team.teamId} g${data.team.generation}; read: maestro status`,
      ],
    ]);

    const peerWork = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer task", "--to", "peer-notice", "--json"],
      leadEnvironment,
    );
    expect(peerWork.exitCode).toBe(0);
    const peerData = envelope<{ role: { name: string; paneId: string }; work: { id: string } }>(
      peerWork.stdout,
    );
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: peerData.role.paneId };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", peerData.work.id, "--json"],
          peerEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    await setFakeHerdrBehavior(fake, { prompts: false });
    const refused = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "return", peerData.work.id, "result: peer done", "--json"],
      peerEnvironment,
    );
    expect(refused.exitCode).toBe(0);
    expect(refused.stderr).toContain(
      `warning: could not notify ${lead.name} about ${peerData.work.id} RETURNED`,
    );
    expect(envelope<{ work: { state: string } }>(refused.stdout).work.state).toBe("RETURNED");
  });
}, 20_000);

// Live row 17 (lab g18): a Peer pane that had already acknowledged never
// learned about its second item and a fresh Peer sat idle after READY until
// someone hand-prompted it; the Lead's herdr agent prompt was refused by the
// harness. work add --to now pushes the wake line itself.
test("SLP v2 work add --to pushes one [from lead][<id> OPEN] line to the assignee pane, fresh and reused alike, and a refused push leaves the item OPEN with a warning", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Wake on add", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const opens = async () =>
      (await fakeHerdrCommands(fake)).filter(
        (command) => command[0] === "agent" && command[1] === "prompt" && / OPEN\] /.test(command[3] ?? ""),
      );
    const before = (await opens()).length;

    const first = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "first task for alpha\nwith a second line", "--to", "alpha", "--json"],
      leadEnvironment,
    );
    expect(first.exitCode).toBe(0);
    const firstData = envelope<{ role: { name: string }; work: { id: string; state: string } }>(first.stdout);
    expect(firstData.work.state).toBe("OPEN");
    expect((await opens()).slice(before)).toEqual([
      ["agent", "prompt", firstData.role.name, `[from lead][${firstData.work.id} OPEN] first task for alpha; read: maestro status ${firstData.work.id}`],
    ]);

    const second = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "second task for alpha", "--to", "alpha", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(second.stderr)).toBe("");
    expect(second.exitCode).toBe(0);
    const secondData = envelope<{ role: { name: string }; work: { id: string } }>(second.stdout);
    expect(secondData.role.name).toBe(firstData.role.name);
    expect((await opens()).slice(before + 1)).toEqual([
      ["agent", "prompt", firstData.role.name, `[from lead][${secondData.work.id} OPEN] second task for alpha; read: maestro status ${secondData.work.id}`],
    ]);

    await setFakeHerdrBehavior(fake, { prompts: false });
    const refused = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "third task for alpha", "--to", "alpha", "--json"],
      leadEnvironment,
    );
    expect(refused.exitCode).toBe(0);
    const refusedData = envelope<{ work: { id: string; state: string } }>(refused.stdout);
    expect(refusedData.work.state).toBe("OPEN");
    expect(refused.stderr).toContain(
      `warning: could not notify ${firstData.role.name} about ${refusedData.work.id} OPEN`,
    );
    // The fake logs the attempt before refusing it: the push was tried.
    expect((await opens()).slice(before + 2).map((command) => command[3])).toEqual([
      `[from lead][${refusedData.work.id} OPEN] third task for alpha; read: maestro status ${refusedData.work.id}`,
    ]);
  });
}, 20_000);

test("SLP v2 work note --blocked flags the note and pushes one line to the seat above (d761)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Blocked notes", "--json"],
      { ...fake.env, HERDR_PANE_ID: "hub:p0" },
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: {
        generation: number;
        roles: Array<{ name: string; paneId: string; role: string }>;
        teamId: string;
      };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const notices = async () =>
      (await fakeHerdrCommands(fake)).filter(
        (command) =>
          command[0] === "agent" && command[1] === "prompt" && (command[3] ?? "").startsWith("[from "),
      );
    const id = data.work.id;
    // w696: team start already pushed this item's OPEN line to the Lead.
    const beforeNotices = (await notices()).length;
    const noteFlag = (stdout: string) => envelope<{ note: { flag: string | null } }>(stdout).note.flag;

    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "take", id, "--json"], leadEnvironment)).exitCode,
    ).toBe(0);
    const both = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", id, "x", "--blocked", "--rework", "--json"],
      leadEnvironment,
    );
    expect(both.exitCode).toBe(1);
    expect(failureEnvelope(both.stderr).error.code).toBe("INVALID_OPTION");

    const blocked = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", id, "need the API key name\nsecond line", "--blocked", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(blocked.stderr)).toBe("");
    expect(blocked.exitCode).toBe(0);
    expect(noteFlag(blocked.stdout)).toBe("blocked");
    expect((await notices()).slice(beforeNotices)).toEqual([
      [
        "agent",
        "prompt",
        "hub:p0",
        `[from lead][${id} BLOCKED] need the API key name in ${data.team.teamId} g${data.team.generation}; read: maestro status`,
      ],
    ]);

    const shown = await runCliAt(fixture, room, ["status", id], fake.env);
    expect(shown.exitCode).toBe(0);
    expect(shown.stdout).toContain(`note [blocked] by ${lead.name}: need the API key name`);
    const shownJson = envelope<{ notes: Array<{ flag: string | null }> }>(
      (await runCliAt(fixture, room, ["status", id, "--json"], fake.env)).stdout,
    );
    expect(shownJson.notes.map((note) => note.flag)).toEqual(["blocked"]);

    const plain = await runCliAt(fixture, fixture.repo, ["work", "note", id, "still going", "--json"], leadEnvironment);
    expect(plain.exitCode).toBe(0);
    expect(noteFlag(plain.stdout)).toBeNull();
    expect((await notices()).length - beforeNotices).toBe(1);
  });
}, 30_000);

// Hub d101: --fresh reuses an acknowledged Peer pane with a fresh harness
// context. The reset is the harness's new-conversation command as its own
// input, the enter key as another (live g22 2026-09-05: one input made Claude
// Code parse "/clearslp ..."), a wait until Herdr reports a new harness
// session for the pane, then the same READY challenge a fresh pane passes;
// only then does the d840 OPEN line go out. Refused while the Peer holds
// ACTIVE work; a reset that never takes refuses with PEER_RESET_FAILED and
// writes nothing.
test("SLP v2 work add --to --fresh resets an acknowledged Claude Peer pane before the OPEN push, is a no-op on a fresh pane, is refused while the Peer holds ACTIVE work, and refuses with PEER_RESET_FAILED writing nothing when the harness session never changes", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { sessionResetDelayReads: 2 });
    await mkdir(join(fixture.home, "maestro", "profiles"), { recursive: true });
    await writeFile(join(fixture.home, "maestro", "profiles", "peer.md"), "---\nharness: claude\n---\n");
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Fresh context on reuse", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const traffic = async (from: number) =>
      (await fakeHerdrCommands(fake)).slice(from).filter(
        (command) =>
          (command[0] === "pane" && ["run", "send-keys", "send-text"].includes(command[1] ?? "")) ||
          (command[0] === "agent" && ["list", "prompt", "read"].includes(command[1] ?? "")),
      );
    const sessionOf = async (name: string) =>
      ((await readFakeHerdrState(fake)).agents as Array<{ agent_session: { value: string }; name: string }>)
        .find((agent) => agent.name === name)?.agent_session.value ?? "";
    const opens = (commands: string[][]) =>
      commands.filter((command) => command[1] === "prompt" && / OPEN\] /.test(command[3] ?? ""));
    const storedChallenge = (name: string) =>
      new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true })
        .query<{ ready_challenge: string }, [string]>(
          "SELECT ready_challenge FROM slp_local_roles WHERE name = ?",
        )
        .get(name)?.ready_challenge ?? "";
    const workRows = () =>
      new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true })
        .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work")
        .get()?.count ?? -1;

    // 1a. --fresh on a pane that is being opened is a no-op.
    let before = (await fakeHerdrCommands(fake)).length;
    const first = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "first task", "--to", "alpha", "--fresh", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(first.stderr)).toBe("");
    expect(first.exitCode).toBe(0);
    const firstData = envelope<{ role: { name: string; paneId: string }; work: { id: string; state: string } }>(first.stdout);
    const peer = firstData.role.name;
    const paneId = firstData.role.paneId;
    let seen = await traffic(before);
    expect(seen.filter((command) => command[0] === "pane")).toEqual([]);
    expect(opens(seen).map((command) => command[3])).toEqual([
      `[from lead][${firstData.work.id} OPEN] first task; read: maestro status ${firstData.work.id}`,
    ]);
    const openedChallenge = storedChallenge(peer);
    expect(openedChallenge).not.toBe("");

    // 1b. Without --fresh the reused pane gets only the OPEN line (d840).
    before = (await fakeHerdrCommands(fake)).length;
    const second = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "second task", "--to", "alpha", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(second.stderr)).toBe("");
    expect(second.exitCode).toBe(0);
    const secondData = envelope<{ work: { id: string } }>(second.stdout);
    // ensurePeer's own agent.list aside, the reused pane sees only the wake.
    seen = (await traffic(before)).filter((command) => command[1] !== "list");
    expect(seen).toEqual([
      ["agent", "prompt", peer, `[from lead][${secondData.work.id} OPEN] second task; read: maestro status ${secondData.work.id}`],
    ]);
    expect(storedChallenge(peer)).toBe(openedChallenge);

    // 1c. --fresh on the acknowledged pane: /clear, enter, agent.list until the
    // session changes, READY, one OPEN.
    const sessionBefore = await sessionOf(peer);
    expect(sessionBefore).not.toBe("");
    before = (await fakeHerdrCommands(fake)).length;
    const third = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "third task", "--to", "alpha", "--fresh", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(third.stderr)).toBe("");
    expect(third.exitCode).toBe(0);
    const thirdData = envelope<{ work: { id: string; state: string } }>(third.stdout);
    expect(thirdData.work.state).toBe("OPEN");
    seen = await traffic(before);
    const reset = seen.findIndex((command) => command[0] === "pane");
    expect(seen.filter((command) => command[0] === "pane")).toEqual([
      ["pane", "send-text", paneId, "/clear"],
      ["pane", "send-keys", paneId, "enter"],
    ]);
    const challenge = seen.findIndex(
      (command) => command[1] === "prompt" && /^slp team \S+ generation \d+ instance \S+; reply [0-9a-f]{32}$/.test(command[3] ?? ""),
    );
    const readyRead = seen.findIndex((command) => command[1] === "read" && command[2] === peer);
    const open = seen.findIndex((command) => command[1] === "prompt" && / OPEN\] /.test(command[3] ?? ""));
    expect([challenge, readyRead, open].every((index) => index > 0)).toBe(true);
    // The fake reports the new session on its second agent.list read after the
    // enter key, and refuses any prompt before that; two reads sit between the
    // key press and the challenge.
    const sessionReads = seen.filter((command, index) => index > reset + 1 && index < challenge && command[1] === "list");
    expect(sessionReads.length).toBeGreaterThanOrEqual(2);
    expect(challenge).toBeLessThan(readyRead);
    expect(readyRead).toBeLessThan(open);
    expect(await sessionOf(peer)).not.toBe(sessionBefore);
    expect(opens(seen).map((command) => command[3])).toEqual([
      `[from lead][${thirdData.work.id} OPEN] third task; read: maestro status ${thirdData.work.id}`,
    ]);
    // The reset conversation answered a new challenge, and the store holds it.
    expect(storedChallenge(peer)).not.toBe(openedChallenge);
    expect(seen[challenge]?.[3]).toContain(`reply ${storedChallenge(peer)}`);

    // 2. Refused while the Peer holds ACTIVE work; nothing is inserted.
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: paneId };
    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "take", thirdData.work.id, "--json"], peerEnvironment)).exitCode,
    ).toBe(0);
    const rows = workRows();
    before = (await fakeHerdrCommands(fake)).length;
    const refused = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "fourth task", "--to", "alpha", "--fresh", "--json"],
      leadEnvironment,
    );
    expect(refused.exitCode).toBe(1);
    const failure = failureEnvelope(refused.stderr).error;
    expect(failure.code).toBe("PEER_ACTIVE");
    expect(failure.message).toContain(thirdData.work.id);
    expect(workRows()).toBe(rows);
    expect(await traffic(before)).toEqual([]);

    // 3. A reset that never takes (the harness session id never changes):
    // PEER_RESET_FAILED naming the pane and the unchanged session, nothing
    // inserted, no challenge, no prompt.
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", thirdData.work.id, "result: done; proof: none", "--json"],
          peerEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    await setFakeHerdrBehavior(fake, { sessionResets: false });
    const challengeBefore = storedChallenge(peer);
    const sessionStuck = await sessionOf(peer);
    const rowsBefore = workRows();
    before = (await fakeHerdrCommands(fake)).length;
    const stuck = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "fifth task", "--to", "alpha", "--fresh", "--json"],
      leadEnvironment,
    );
    expect(stuck.exitCode).toBe(1);
    const stuckFailure = failureEnvelope(stuck.stderr).error;
    expect(stuckFailure.code).toBe("PEER_RESET_FAILED");
    expect(stuckFailure.message).toContain(paneId);
    expect(stuckFailure.message).toContain(sessionStuck);
    expect(workRows()).toBe(rowsBefore);
    seen = await traffic(before);
    expect(seen.filter((command) => command[0] === "pane")).toEqual([
      ["pane", "send-text", paneId, "/clear"],
      ["pane", "send-keys", paneId, "enter"],
    ]);
    expect(seen.filter((command) => command[1] === "prompt")).toEqual([]);
    expect(storedChallenge(peer)).toBe(challengeBefore);
    expect(await sessionOf(peer)).toBe(sessionStuck);
  });
}, 30_000);

test("SLP v2 work add --to --fresh sends /new to a reused Codex Peer pane (Hub d101)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Fresh Codex context", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const first = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "codex first", "--to", "beta", "--json"],
      leadEnvironment,
    );
    expect(first.exitCode).toBe(0);
    const firstData = envelope<{ role: { name: string; paneId: string } }>(first.stdout);
    const codexStart = (await fakeHerdrCommands(fake)).find(
      (command) => command[0] === "agent" && command[1] === "start" && command[2] === firstData.role.name,
    );
    expect(codexStart).toContain("codex");
    const before = (await fakeHerdrCommands(fake)).length;
    const second = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "codex second", "--to", "beta", "--fresh", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(second.stderr)).toBe("");
    expect(second.exitCode).toBe(0);
    const secondData = envelope<{ work: { id: string } }>(second.stdout);
    const seen = (await fakeHerdrCommands(fake)).slice(before);
    expect(seen.filter((command) => command[0] === "pane")).toEqual([
      ["pane", "send-text", firstData.role.paneId, "/new"],
      ["pane", "send-keys", firstData.role.paneId, "enter"],
    ]);
    expect(
      seen.filter((command) => command[1] === "prompt" && / OPEN\] /.test(command[3] ?? "")).map((command) => command[3]),
    ).toEqual([`[from lead][${secondData.work.id} OPEN] codex second; read: maestro status ${secondData.work.id}`]);
  });
}, 20_000);

test("SLP v2 work add refuses --acceptance and --blocked-by instead of discarding them, naming both flags and the objective text, before any Peer pane opens; the Hub path keeps both flags and its blocker validation (w701, d859)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Discarded flags", "--json"],
      { ...fake.env, HERDR_PANE_ID: "hub:p0" },
    );
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    // The Lead's own view: every item this team assigned to it.
    const workRows = async () =>
      envelope<{ work: Array<{ id: string }> }>(
        (await runCliAt(fixture, fixture.repo, ["status", "--json"], leadEnvironment)).stdout,
      ).work;
    const paneCreates = async () =>
      (await fakeHerdrCommands(fake)).filter(
        (command) => command[0] === "tab" && command[1] === "create",
      ).length;
    const beforeWork = (await workRows()).map((row) => row.id);
    const beforePaneCreates = await paneCreates();

    // The specimen: a Lead brief carrying both flags. Both parse, and before
    // w701 both vanished with a success envelope and no word.
    const both = await runCliAt(
      fixture,
      fixture.repo,
      [
        "work",
        "add",
        "Independent result",
        "--to",
        "peer-one",
        "--acceptance",
        "the suite is green",
        "--blocked-by",
        data.work.id,
        "--json",
      ],
      leadEnvironment,
    );
    expect(both.exitCode).toBe(1);
    const refusal = failureEnvelope(both.stderr).error;
    expect(refusal.code).toBe("INVALID_OPTION");
    expect(refusal.message).toContain("--acceptance");
    expect(refusal.message).toContain("--blocked-by");
    expect(refusal.message).toContain("objective text");
    // Refused before anything happened: no item, and no Peer pane opened for it.
    expect((await workRows()).map((row) => row.id)).toEqual(beforeWork);
    expect(await paneCreates()).toBe(beforePaneCreates);

    // Each flag alone is refused too, by the flag the caller actually passed.
    const acceptanceOnly = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Lead task", "--acceptance", "green", "--json"],
      leadEnvironment,
    );
    expect(acceptanceOnly.exitCode).toBe(1);
    expect(failureEnvelope(acceptanceOnly.stderr).error.message).toContain("--acceptance");
    const blockedByOnly = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Lead task", "--blocked-by", data.work.id, "--json"],
      leadEnvironment,
    );
    expect(blockedByOnly.exitCode).toBe(1);
    expect(failureEnvelope(blockedByOnly.stderr).error.message).toContain("--blocked-by");

    // Neither flag: SLP work add is unchanged.
    const plain = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Lead task", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(plain.stderr)).toBe("");
    expect(plain.exitCode).toBe(0);
    expect((await workRows()).length).toBe(beforeWork.length + 1);

    // The Hub path below this branch keeps both flags, blocker validation included.
    const hubEnvironment = { MAESTRO_ROOM_SCAFFOLD: "1" };
    const first = envelope<{ work: { id: string } }>(
      (await runCliAt(fixture, room, ["work", "add", "first", "--json"], hubEnvironment)).stdout,
    ).work;
    const second = await runCliAt(
      fixture,
      room,
      ["work", "add", "second", "--acceptance", "the report reads clean", "--blocked-by", first.id, "--json"],
      hubEnvironment,
    );
    expect(second.exitCode).toBe(0);
    const stored = envelope<{ work: { acceptance: string | null; id: string } }>(second.stdout).work;
    expect(stored.acceptance).toBe("the report reads clean");
    expect(
      (await runCliAt(fixture, room, ["work", "show", stored.id], hubEnvironment)).stdout,
    ).toContain("the report reads clean");
    const unknownBlocker = await runCliAt(
      fixture,
      room,
      ["work", "add", "third", "--blocked-by", "w999", "--json"],
      hubEnvironment,
    );
    expect(unknownBlocker.exitCode).toBe(1);
  });
}, 30_000);

test("SLP v2 lets a Lead open its own item with work add and no --to, which the Hub reviews, while the Hub's work add --to stays refused (room d118, w725)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Lead opens its own follow-up", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const promptsBefore = (await fakeHerdrCommands(fake))
      .filter((command) => command[0] === "agent" && command[1] === "prompt").length;

    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "A follow-up the Lead will do itself", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(added.stderr)).toBe("");
    expect(added.exitCode).toBe(0);
    const own = envelope<{
      role: { name: string; role: string };
      work: { assignedTo: string; createdBy: string; id: string; owner: string | null; state: string };
    }>(added.stdout);
    expect(own.work).toMatchObject({ assignedTo: lead.name, createdBy: lead.name, owner: null, state: "OPEN" });
    expect(own.role).toMatchObject({ name: lead.name, role: "lead" });
    // Nothing to wake: no Peer pane was opened and no OPEN line went out.
    const promptsAfter = (await fakeHerdrCommands(fake))
      .filter((command) => command[0] === "agent" && command[1] === "prompt").length;
    expect(promptsAfter).toBe(promptsBefore);

    const listed = await runCliAt(fixture, fixture.repo, ["status"], leadEnvironment);
    expect(listed.stdout).toContain(`* ${own.work.id} OPEN`);
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", own.work.id], leadEnvironment)).exitCode)
      .toBe(0);
    expect(
      (await runCliAt(fixture, fixture.repo, ["work", "return", own.work.id, "done"], leadEnvironment))
        .exitCode,
    ).toBe(0);
    const waiting = await runCliAt(fixture, fixture.repo, ["status", own.work.id], leadEnvironment);
    expect(waiting.stdout).toContain("waiting on hub-supervisor");
    const accepted = await runCliAt(
      fixture,
      room,
      ["work", "accept", own.work.id, "--json"],
      fake.env,
    );
    expect(accepted.exitCode).toBe(0);
    expect(envelope<{ work: { acceptedBy: string; state: string } }>(accepted.stdout).work)
      .toMatchObject({ acceptedBy: "hub-supervisor", state: "DONE" });

    // The Hub still cannot assign team work directly.
    const hubAdd = await runCliAt(fixture, room, ["work", "add", "Hub team work", "--to", lead.name], fake.env);
    expect(hubAdd.exitCode).not.toBe(0);
    expect(`${hubAdd.stdout}${hubAdd.stderr}`).toContain("ROLE_FORBIDDEN");
  });
});

// room d137, as amended by d140: per-pane session telemetry starts with the
// compact count. A completed compaction reaches Claude Code as SessionStart
// with source "compact" - there is no PostCompact event - and the hook records
// one row per compaction for the session and pane it fires in, which the
// reviewer reads back beside the assignee seat. It gates nothing: a seat with
// two compactions is refused nothing (d96/d97 hold).
test("SLP v2 reads the compact count back beside the assignee seat and prints nothing for a seat that never compacted (room d137)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (
        await runCliAt(fixture, room, ["room", "mark"], {
          MAESTRO_ROOM_SCAFFOLD: "1",
          MAESTRO_SESSION_NONE: "1",
        })
      ).exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture);
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, "Count the compactions", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const workId = startData.work.id;

    // Nobody has compacted: the line is absent, not "compacts: 0".
    const quiet = await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment);
    expect(quiet.exitCode).toBe(0);
    expect(quiet.stdout).toContain(`${workId} OPEN hub-supervisor -> ${lead.name}`);
    expect(quiet.stdout).not.toContain("compacts:");

    for (const session of ["compact-session-a", "compact-session-a"]) {
      const recorded = await runCliAt(
        fixture,
        fixture.repo,
        ["hook", "record", "--event", "SessionStart", "--source", "compact", "--harness", "claude"],
        { ...leadEnvironment, MAESTRO_SESSION_ID: session, MAESTRO_SESSION_PID: String(process.pid) },
      );
      expect(recorded.exitCode).toBe(0);
    }

    // A SessionStart that is not a compaction is not a compaction.
    expect(
      (await runCliAt(
        fixture,
        fixture.repo,
        ["hook", "record", "--event", "SessionStart", "--source", "startup", "--harness", "claude"],
        { ...leadEnvironment, MAESTRO_SESSION_ID: "compact-session-b", MAESTRO_SESSION_PID: String(process.pid) },
      )).exitCode,
    ).toBe(0);

    const counted = await runCliAt(fixture, fixture.repo, ["status", workId], leadEnvironment);
    expect(counted.exitCode).toBe(0);
    expect(counted.stdout).toMatch(
      new RegExp(`^${workId} OPEN hub-supervisor -> ${lead.name} compacts: 2 \\(last \\d+[smhd]\\)$`, "m"),
    );
    // The reviewer sees the assignee's figure, not its own.
    const reviewer = await runCliAt(fixture, room, ["status", workId], fake.env);
    expect(reviewer.stdout).toContain(`-> ${lead.name} compacts: 2 (last `);
    // Telemetry only: the seat may still run its operations.
    expect((await runCliAt(fixture, fixture.repo, ["work", "take", workId], leadEnvironment)).exitCode)
      .toBe(0);
  });
}, 30_000);
