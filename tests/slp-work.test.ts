import { createHash } from "node:crypto";
import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
} from "./helpers-herdr.ts";
import { runCliAt, withFixture } from "./helpers.ts";
import { envelope, phaseFree } from "./helpers-slp.ts";

test("SLP v2 keeps work objective and acceptance contract immutable across notes", async () => {
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
    const objective = "Ship parser; acceptance: malformed input is rejected";
    const started = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string; objective: string };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, objective, "--json"],
          fake.env,
        )
      ).stdout,
    );
    const leadPane = started.team.roles.find(
      (role) => role.role === "lead",
    )!.paneId;
    const environment = { ...fake.env, HERDR_PANE_ID: leadPane };

    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          [
            "work",
            "note",
            started.work.id,
            "requested change: accept malformed input instead",
            "--json",
          ],
          environment,
        )
      ).exitCode,
    ).toBe(0);
    const status = envelope<{
      notes: Array<{ body: string }>;
      work: { objective: string };
    }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["status", started.work.id, "--json"],
          environment,
        )
      ).stdout,
    );
    expect(status.work.objective).toBe(objective);
    expect(status.notes.map((note) => note.body)).toEqual([
      "requested change: accept malformed input instead",
    ]);

    const replacement = await runCliAt(
      fixture,
      fixture.repo,
      [
        "work",
        "add",
        "Ship parser; acceptance: malformed input is accepted",
        "--json",
      ],
      environment,
    );
    expect(replacement.exitCode).toBe(0);
    expect(envelope<{ work: { id: string; objective: string } }>(replacement.stdout).work)
      .toMatchObject({
        id: "w2",
        objective: "Ship parser; acceptance: malformed input is accepted",
      });
  });
});

test("SLP v2 drives one work item through OPEN ACTIVE RETURNED DONE with atomic activity", async () => {
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
      ["team", "start", fixture.repo, "Complete the normal work journey", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
      work: { id: string; state: string };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead");
    expect(lead).toBeDefined();
    expect(startData.work.state).toBe("OPEN");

    const taken = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(phaseFree(taken.stderr)).toBe("");
    expect(envelope<{ work: { owner: string; state: string } }>(taken.stdout).work).toMatchObject({
      owner: lead?.name,
      state: "ACTIVE",
    });

    const returned = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "return", startData.work.id, "result: complete; proof: focused test", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(phaseFree(returned.stderr)).toBe("");
    expect(envelope<{ work: { owner: null; state: string } }>(returned.stdout).work).toMatchObject({
      owner: null,
      state: "RETURNED",
    });

    const accepted = await runCliAt(
      fixture,
      room,
      ["work", "accept", startData.work.id, "--json"],
      fake.env,
    );
    expect(phaseFree(accepted.stderr)).toBe("");
    expect(envelope<{ work: { state: string } }>(accepted.stdout).work.state).toBe("DONE");

    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    const activity = database
      .query<{ operation: string }, []>(
        "SELECT operation FROM slp_activity ORDER BY id",
      )
      .all()
      .map((row) => row.operation);
    database.close();
    expect(activity).toEqual(["work.add", "work.take", "work.return", "work.accept"]);
  });
}, 15_000);

test("SLP v2 rejects a stale work transition without recording false activity", async () => {
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
      ["team", "start", fixture.repo, "Reject stale transition", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = data.team.roles.find((role) => role.role === "lead")?.paneId;
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const database = new Database(databasePath);
    database.exec(`
      CREATE TRIGGER ignore_work_take
      BEFORE UPDATE OF state ON slp_work
      WHEN OLD.id = '${data.work.id}' AND NEW.state = 'ACTIVE'
      BEGIN
        SELECT RAISE(IGNORE);
      END;
    `);
    const activityBefore = database
      .query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity")
      .get()?.count;
    database.close();

    const rejected = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", data.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: leadPane },
    );

    expect(rejected.exitCode).toBe(1);
    expect(rejected.stderr).toContain("INVALID_STATE");
    const after = new Database(databasePath, { readonly: true });
    expect(
      after.query<{ state: string }, [string]>("SELECT state FROM slp_work WHERE id = ?")
        .get(data.work.id)?.state,
    ).toBe("OPEN");
    expect(
      after.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_activity").get()?.count,
    ).toBe(activityBefore);
    after.close();
  });
}, 15_000);

test("SLP v2 rolls back every project mutation when its activity record cannot be inserted", async () => {
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
    const started = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Prove atomic project mutations", "--json"],
          fake.env,
        )
      ).stdout,
    );
    const leadPane = started.team.roles.find((role) => role.role === "lead")!.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    const rejectActivity = async (
      args: string[],
      environment: Record<string, string | undefined>,
      cwd = fixture.repo,
    ) => {
      const before = new Database(databasePath);
      before.exec(`
        CREATE TRIGGER reject_activity_insert
        BEFORE INSERT ON slp_activity
        BEGIN
          SELECT RAISE(ABORT, 'injected activity insertion failure');
        END;
      `);
      before.close();
      const result = await runCliAt(fixture, cwd, args, environment);
      const after = new Database(databasePath);
      after.exec("DROP TRIGGER reject_activity_insert");
      after.close();
      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("injected activity insertion failure");
    };
    const readDatabase = () => new Database(databasePath, { readonly: true });

    await rejectActivity(
      ["work", "take", started.work.id, "--json"],
      leadEnvironment,
    );
    let database = readDatabase();
    expect(
      database.query<{ state: string }, [string]>("SELECT state FROM slp_work WHERE id = ?")
        .get(started.work.id)?.state,
    ).toBe("OPEN");
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", started.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "note", started.work.id, "must roll back", "--json"],
      leadEnvironment,
    );
    database = readDatabase();
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE work_id = ? AND kind = 'NOTE'",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    database.close();

    await rejectActivity(
      ["work", "return", started.work.id, "result: rejected audit", "--json"],
      leadEnvironment,
    );
    database = readDatabase();
    expect(
      database
        .query<{ current_return: string | null; return_revision: number; state: string }, [string]>(
          "SELECT state, current_return, return_revision FROM slp_work WHERE id = ?",
        )
        .get(started.work.id),
    ).toEqual({ current_return: null, return_revision: 0, state: "ACTIVE" });
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE work_id = ? AND kind = 'RETURN'",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", started.work.id, "result: first return", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "note", started.work.id, "grant must roll back", "--rework", "--json"],
      fake.env,
      room,
    );
    database = readDatabase();
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_rework_grants WHERE work_id = ?",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE work_id = ? AND kind = 'NOTE'",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          room,
          ["work", "note", started.work.id, "reviewer grants rework", "--rework", "--json"],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "take", started.work.id, "--json"],
      leadEnvironment,
    );
    database = readDatabase();
    expect(
      database
        .query<{ state: string }, [string]>("SELECT state FROM slp_work WHERE id = ?")
        .get(started.work.id)?.state,
    ).toBe("RETURNED");
    expect(
      database
        .query<{ consumed_at: string | null }, [string]>(
          "SELECT consumed_at FROM slp_rework_grants WHERE work_id = ?",
        )
        .get(started.work.id)?.consumed_at,
    ).toBeNull();
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", started.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", started.work.id, "result: second return", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "accept", started.work.id, "--json"],
      fake.env,
      room,
    );
    database = readDatabase();
    expect(
      database
        .query<{ acceptance_outcome: string | null; state: string }, [string]>(
          "SELECT state, acceptance_outcome FROM slp_work WHERE id = ?",
        )
        .get(started.work.id),
    ).toEqual({ acceptance_outcome: null, state: "RETURNED" });
    expect(
      database
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_work_entries WHERE work_id = ? AND kind = 'ACCEPTANCE'",
        )
        .get(started.work.id)?.count,
    ).toBe(0);
    database.close();
    expect(
      (
        await runCliAt(
          fixture,
          room,
          ["work", "accept", started.work.id, "--json"],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);

    await rejectActivity(
      ["work", "add", "audit rejected work", "--json"],
      leadEnvironment,
    );
    database = readDatabase();
    expect(database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work").get()?.count)
      .toBe(1);
    database.close();
    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "audit accepted work", "--json"],
      leadEnvironment,
    );
    expect(added.exitCode).toBe(0);
    expect(envelope<{ work: { id: string } }>(added.stdout).work.id).toBe("w2");

    await rejectActivity(
      ["decide", "Rejected decision", "--why", "audit unavailable", "--json"],
      leadEnvironment,
    );
    database = readDatabase();
    expect(
      database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_decisions").get()?.count,
    ).toBe(0);
    database.close();
    const decided = await runCliAt(
      fixture,
      fixture.repo,
      ["decide", "Accepted decision", "--why", "audit available", "--json"],
      leadEnvironment,
    );
    expect(decided.exitCode).toBe(0);
    expect(envelope<{ decision: { id: string } }>(decided.stdout).decision.id).toBe("d1");
  });
}, 30_000);

test("SLP v2 serializes ten concurrent work additions without duplicate ids or lost activity", async () => {
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
    const started = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Serialize concurrent work writes", "--json"],
          fake.env,
        )
      ).stdout,
    );
    const leadPane = started.team.roles.find(
      (role) => role.role === "lead",
    )!.paneId;
    const environment = { ...fake.env, HERDR_PANE_ID: leadPane };

    const results = await Promise.all(
      Array.from({ length: 10 }, (_, index) =>
        runCliAt(
          fixture,
          fixture.repo,
          ["work", "add", `Concurrent work ${index}`, "--json"],
          environment,
        )
      ),
    );

    expect(results.map((result) => ({ exitCode: result.exitCode, stderr: result.stderr })))
      .toEqual(Array.from({ length: 10 }, () => ({ exitCode: 0, stderr: "" })));
    const ids = results.map(
      (result) => envelope<{ work: { id: string } }>(result.stdout).work.id,
    );
    expect(new Set(ids).size).toBe(10);
    expect(ids.toSorted()).toEqual(
      ["w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10", "w11"].toSorted(),
    );
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work").get()?.count)
      .toBe(11);
    expect(
      database
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'work.add'",
        )
        .get()?.count,
    ).toBe(11);
    database.close();
  });
}, 20_000);

test("SLP v2 reports a structured store-busy failure without partial mutation", async () => {
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
    const started = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
    }>(
      (
        await runCliAt(
          fixture,
          room,
          ["team", "start", fixture.repo, "Report busy stores", "--json"],
          fake.env,
        )
      ).stdout,
    );
    const leadPane = started.team.roles.find(
      (role) => role.role === "lead",
    )!.paneId;
    const databasePath = join(fixture.repo, ".maestro", "maestro.db");
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["status", "--json"],
          { ...fake.env, HERDR_PANE_ID: leadPane },
        )
      ).exitCode,
    ).toBe(0);
    const blocker = new Database(databasePath);
    blocker.exec("BEGIN IMMEDIATE");
    const rejected = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "must not partially commit", "--json"],
      { ...fake.env, HERDR_PANE_ID: leadPane },
    );
    blocker.exec("ROLLBACK");
    blocker.close();

    expect(rejected.exitCode).toBe(1);
    expect(JSON.parse(rejected.stderr)).toMatchObject({
      error: { code: "STORE_BUSY", sqliteCode: expect.stringMatching(/^SQLITE_BUSY/) },
      ok: false,
    });
    const database = new Database(databasePath, { readonly: true });
    expect(database.query<{ count: number }, []>("SELECT COUNT(*) AS count FROM slp_work").get()?.count)
      .toBe(1);
    expect(
      database
        .query<{ count: number }, []>(
          "SELECT COUNT(*) AS count FROM slp_activity WHERE operation = 'work.add'",
        )
        .get()?.count,
    ).toBe(1);
    database.close();
  });
}, 45_000);

test("SLP v2 Lead adds OPEN work to one lazily created and reusable Peer", async () => {
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
      ["team", "start", fixture.repo, "Delegate one Peer task", "--json"],
      fake.env,
    );
    expect(started.exitCode).toBe(0);
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead");
    expect(lead).toBeDefined();
    const environment = { ...fake.env, HERDR_PANE_ID: lead?.paneId };

    const first = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Design independently", "--to", "peer-design", "--json"],
      environment,
    );
    expect(phaseFree(first.stderr)).toBe("");
    expect(first.exitCode).toBe(0);
    const firstData = envelope<{
      role: { name: string; role: string };
      work: { assignedTo: string; id: string; state: string };
    }>(first.stdout);
    const peerName = firstData.role.name;
    expect(firstData.role).toMatchObject({ name: expect.stringMatching(/^peer-design-/), role: "peer" });
    expect(firstData.work).toMatchObject({ assignedTo: firstData.role.name, state: "OPEN" });

    const second = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Implement independently", "--to", "peer-design", "--json"],
      environment,
    );
    expect(phaseFree(second.stderr)).toBe("");
    expect(second.exitCode).toBe(0);
    const secondData = envelope<{
      role: { name: string; role: string };
      work: { assignedTo: string; id: string; state: string };
    }>(second.stdout);
    expect(secondData.role).toEqual(firstData.role);
    expect(secondData.work).toMatchObject({ assignedTo: firstData.role.name, state: "OPEN" });
    expect(secondData.work.id).not.toBe(firstData.work.id);
    // One contract prompt for the pane, then one OPEN wake per item (live row
    // 17 g18: a reused pane never learned about its second item before).
    const peerPrompts = (await fakeHerdrCommands(fake)).filter(
      (command) =>
        command[0] === "agent" && command[1] === "prompt" && command[2] === peerName,
    );
    expect(peerPrompts).toHaveLength(3);
    expect(peerPrompts.slice(1).map((command) => command[3])).toEqual([
      `[from lead][${firstData.work.id} OPEN] Design independently; read: maestro status ${firstData.work.id}`,
      `[from lead][${secondData.work.id} OPEN] Implement independently; read: maestro status ${secondData.work.id}`,
    ]);

    const runtime = await readFakeHerdrState(fake);
    expect(runtime.workspaces).toHaveLength(1);
    expect(
      runtime.agents
        .filter((agent: { name: string }) => agent.name.startsWith("peer-"))
        .map((agent: { name: string }) => agent.name),
    ).toEqual([firstData.role.name]);
  });
}, 15_000);

test("SLP v2 rolls back a newly provisioned Peer when its first work insert fails", async () => {
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
      ["team", "start", fixture.repo, "Rollback failed Peer work", "--json"],
      fake.env,
    );
    const data = envelope<{
      team: { roles: Array<{ paneId: string; role: string }>; teamId: string };
    }>(started.stdout);
    const leadPane = data.team.roles.find((role) => role.role === "lead")?.paneId;
    const peerName = `peer-rollback-${createHash("sha256")
      .update(`${data.team.teamId}\0rollback`)
      .digest("hex")
      .slice(0, 6)}`;
    const existingPeerPane = "existing-peer-root";
    const peerSiblingPane = "existing-peer-sibling";
    await editFakeHerdrState(fake, (state) => {
      const workspace = (state.workspaces as Array<{ workspace_id: string }>)[0]!;
      const tabId = "existing-peer-tab";
      (state.tabs as Array<Record<string, unknown>>).push({
        label: `slp:${data.team.teamId}:g1:peer:${peerName}`,
        root_pane_id: existingPeerPane,
        tab_id: tabId,
        workspace_id: workspace.workspace_id,
      });
      (state.panes as Array<Record<string, unknown>>).push(
        {
          cwd: fixture.repo,
          pane_id: existingPeerPane,
          tab_id: tabId,
          workspace_id: workspace.workspace_id,
        },
        {
          cwd: fixture.repo,
          pane_id: peerSiblingPane,
          tab_id: tabId,
          workspace_id: workspace.workspace_id,
        },
      );
      (state.processes as Record<string, unknown>)[peerSiblingPane] = {
        foreground_pgid: 9002,
        foreground_processes: [{ command: "sibling" }],
        pane_id: peerSiblingPane,
      };
    });
    const projectPath = join(fixture.repo, ".maestro", "maestro.db");
    const project = new Database(projectPath);
    project.exec(`
      CREATE TRIGGER reject_peer_work
      BEFORE INSERT ON slp_work
      WHEN NEW.assigned_to = '${peerName}'
      BEGIN
        SELECT RAISE(ABORT, 'injected peer work failure');
      END;
    `);
    project.close();

    const failed = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Must roll back", "--to", "peer-rollback", "--json"],
      { ...fake.env, HERDR_PANE_ID: leadPane },
    );

    expect(failed.exitCode).toBe(1);
    const projectAfter = new Database(projectPath, { readonly: true });
    expect(
      projectAfter
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_local_roles WHERE name = ?",
        )
        .get(peerName)?.count,
    ).toBe(0);
    projectAfter.close();
    const roomAfter = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    expect(
      roomAfter
        .query<{ count: number }, [string]>(
          "SELECT COUNT(*) AS count FROM slp_team_roles WHERE name = ?",
        )
        .get(peerName)?.count,
    ).toBe(0);
    roomAfter.close();
    const runtime = await readFakeHerdrState(fake);
    expect(
      runtime.agents.some((agent: { name: string }) => agent.name === peerName),
    ).toBe(false);
    expect(runtime.panes.some((pane: { pane_id: string }) => pane.pane_id === existingPeerPane))
      .toBe(false);
    expect(runtime.panes.some((pane: { pane_id: string }) => pane.pane_id === peerSiblingPane))
      .toBe(true);
    expect(runtime.tabs.some((tab: { root_pane_id: string }) => tab.root_pane_id === peerSiblingPane))
      .toBe(true);
  });
}, 15_000);

test("SLP v2 rejects Peer self-acceptance and lets the Lead accept the return", async () => {
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
      ["team", "start", fixture.repo, "Review Peer work", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead");
    expect(lead).toBeDefined();
    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Peer-owned result", "--to", "peer-review", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    const addedData = envelope<{
      role: { name: string; paneId: string };
      work: { id: string };
    }>(added.stdout);
    const peerEnvironment = { ...fake.env, HERDR_PANE_ID: addedData.role.paneId };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", addedData.work.id, "--json"],
          peerEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", addedData.work.id, "result: peer result; proof: peer proof", "--json"],
          peerEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const selfAccepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", addedData.work.id, "--json"],
      peerEnvironment,
    );
    expect(selfAccepted.exitCode).toBe(1);
    expect(selfAccepted.stderr).toContain("ROLE_FORBIDDEN");

    const wrongReviewer = await runCliAt(
      fixture,
      room,
      ["work", "note", addedData.work.id, "Hub cannot grant Peer rework", "--rework", "--json"],
      fake.env,
    );
    expect(wrongReviewer.exitCode).toBe(1);
    expect(JSON.parse(wrongReviewer.stderr).error.code).toBe("ROLE_FORBIDDEN");
    const leadGrant = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", addedData.work.id, "Lead grants Peer rework", "--rework", "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(phaseFree(leadGrant.stderr)).toBe("");
    expect(leadGrant.exitCode).toBe(0);

    const leadAccepted = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "accept", addedData.work.id, "--json"],
      { ...fake.env, HERDR_PANE_ID: lead?.paneId },
    );
    expect(phaseFree(leadAccepted.stderr)).toBe("");
    expect(leadAccepted.exitCode).toBe(0);
    expect(envelope<{ work: { acceptedBy: string; state: string } }>(leadAccepted.stdout).work)
      .toMatchObject({ acceptedBy: lead?.name, state: "DONE" });
  });
}, 15_000);

test("SLP v2 rework requires one correct reviewer grant for the current return revision", async () => {
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
      ["team", "start", fixture.repo, "Exercise rework", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = startData.team.roles.find((role) => role.role === "lead")?.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", startData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", startData.work.id, "result: first attempt", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const directRetake = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(directRetake.exitCode).toBe(1);
    expect(JSON.parse(directRetake.stderr).error.code).toBe("REWORK_REQUIRED");

    const wrongReviewer = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "note", startData.work.id, "self-granted rework", "--rework", "--json"],
      leadEnvironment,
    );
    expect(wrongReviewer.exitCode).toBe(1);
    expect(JSON.parse(wrongReviewer.stderr).error.code).toBe("ROLE_FORBIDDEN");

    const ordinaryNote = await runCliAt(
      fixture,
      room,
      ["work", "note", startData.work.id, "context: the missing case is Windows", "--json"],
      fake.env,
    );
    expect(ordinaryNote.exitCode).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", startData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(1);

    const granted = await runCliAt(
      fixture,
      room,
      [
        "work",
        "note",
        startData.work.id,
        "add the missing Windows negative case",
        "--rework",
        "--json",
      ],
      fake.env,
    );
    expect(phaseFree(granted.stderr)).toBe("");
    expect(granted.exitCode).toBe(0);
    expect(envelope<{ work: { state: string } }>(granted.stdout).work.state).toBe("RETURNED");
    const duplicateGrant = await runCliAt(
      fixture,
      room,
      ["work", "note", startData.work.id, "duplicate grant", "--rework", "--json"],
      fake.env,
    );
    expect(duplicateGrant.exitCode).toBe(1);
    expect(JSON.parse(duplicateGrant.stderr).error.code).toBe("REWORK_ALREADY_GRANTED");

    const retaken = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(phaseFree(retaken.stderr)).toBe("");
    expect(envelope<{ work: { currentReturn: string | null; state: string } }>(retaken.stdout).work)
      .toMatchObject({ currentReturn: null, state: "ACTIVE" });

    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", startData.work.id, "result: second attempt", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const staleGrant = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(staleGrant.exitCode).toBe(1);
    expect(JSON.parse(staleGrant.stderr).error.code).toBe("REWORK_REQUIRED");
    expect(
      (
        await runCliAt(
          fixture,
          room,
          ["work", "note", startData.work.id, "review the second return", "--rework", "--json"],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", startData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const database = new Database(join(fixture.repo, ".maestro", "maestro.db"), {
      readonly: true,
    });
    expect(
      database
        .query<{ count: number }, []>(
          `SELECT COUNT(*) AS count FROM slp_rework_grants
           WHERE consumed_at IS NOT NULL`,
        )
        .get()?.count,
    ).toBe(2);
    database.close();
  });
}, 15_000);

test("SLP v2 keeps a blocker in the return body and resumes without a BLOCKED state", async () => {
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
      ["team", "start", fixture.repo, "Resolve one blocker", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = startData.team.roles.find((role) => role.role === "lead")?.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", startData.work.id, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const blocker = "blocker: owner must provide the missing API contract";
    const returned = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "return", startData.work.id, blocker, "--json"],
      leadEnvironment,
    );
    expect(envelope<{ work: { currentReturn: string; state: string } }>(returned.stdout).work)
      .toMatchObject({ currentReturn: blocker, state: "RETURNED" });

    expect(
      (
        await runCliAt(
          fixture,
          room,
          [
            "work",
            "note",
            startData.work.id,
            "resolved: contract supplied",
            "--rework",
            "--json",
          ],
          fake.env,
        )
      ).exitCode,
    ).toBe(0);
    const resumed = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "take", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(envelope<{ work: { state: string } }>(resumed.stdout).work.state).toBe("ACTIVE");
  });
}, 15_000);

test("SLP v2 cancels OPEN work through acceptance and requires ACTIVE work to return first", async () => {
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
      ["team", "start", fixture.repo, "Exercise cancellation", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const leadPane = startData.team.roles.find((role) => role.role === "lead")?.paneId;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: leadPane };

    const openCancelled = await runCliAt(
      fixture,
      room,
      ["work", "accept", startData.work.id, "--outcome", "cancelled", "--json"],
      fake.env,
    );
    expect(phaseFree(openCancelled.stderr)).toBe("");
    expect(
      envelope<{ work: { acceptanceOutcome: string; state: string } }>(openCancelled.stdout).work,
    ).toMatchObject({ acceptanceOutcome: "cancelled", state: "DONE" });

    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Cancel after start", "--json"],
      leadEnvironment,
    );
    const activeId = envelope<{ work: { id: string } }>(added.stdout).work.id;
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "take", activeId, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const rejected = await runCliAt(
      fixture,
      room,
      ["work", "accept", activeId, "--outcome", "cancelled", "--json"],
      fake.env,
    );
    expect(rejected.exitCode).toBe(1);
    expect(rejected.stderr).toContain("work return");
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", activeId, "result: cancellation requested", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const returnedCancelled = await runCliAt(
      fixture,
      room,
      ["work", "accept", activeId, "--outcome", "cancelled", "--json"],
      fake.env,
    );
    expect(phaseFree(returnedCancelled.stderr)).toBe("");
    expect(
      envelope<{ work: { acceptanceOutcome: string; state: string } }>(returnedCancelled.stdout).work,
    ).toMatchObject({ acceptanceOutcome: "cancelled", state: "DONE" });
  });
}, 15_000);
