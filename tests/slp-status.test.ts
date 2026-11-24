import { Database } from "bun:sqlite";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { scaffoldRoom } from "../src/plugins/room.ts";
import {
  editFakeHerdrState,
  installFakeHerdr,
  setFakeHerdrBehavior,
  tripwireInvocations,
} from "./helpers-herdr.ts";
import { runCliAt, withFixture } from "./helpers.ts";
import { envelope, phaseFree } from "./helpers-slp.ts";

test("SLP v2 status gives Hub, team, and work-scoped truth and reports a missing pane", async () => {
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
      ["team", "start", fixture.repo, "Expose scoped status", "--json"],
      fake.env,
    );
    const startData = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const lead = startData.team.roles.find((role) => role.role === "lead");
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead?.paneId };
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
          ["work", "note", startData.work.id, "note: implementation detail", "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);
    const decision = await runCliAt(
      fixture,
      fixture.repo,
      [
        "decide",
        "Keep the public seam small",
        "--why",
        "The team needs one status reader",
        "--work",
        startData.work.id,
        "--json",
      ],
      leadEnvironment,
    );
    const decisionId = envelope<{ decision: { id: string } }>(decision.stdout).decision.id;
    const returned = "result: status ready; proof: scoped CLI test";
    expect(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["work", "return", startData.work.id, returned, "--json"],
          leadEnvironment,
        )
      ).exitCode,
    ).toBe(0);

    const hubStatus = await runCliAt(fixture, room, ["status", "--json"], fake.env);
    expect(phaseFree(hubStatus.stderr)).toBe("");
    expect(envelope<{ teams: Array<{ packDigest: string; state: string }> }>(hubStatus.stdout).teams)
      .toEqual([expect.objectContaining({ packDigest: expect.any(String), state: "RUNNING" })]);

    const teamStatus = await runCliAt(
      fixture,
      fixture.repo,
      ["status", "--json"],
      leadEnvironment,
    );
    expect(phaseFree(teamStatus.stderr)).toBe("");
    const teamData = envelope<{
      missingPanes: string[];
      runtime: string;
      roles: Array<{ role: string }>;
      runtimePane: string;
      work: Array<{ id: string; state: string }>;
    }>(teamStatus.stdout);
    expect(teamData.roles.map((role) => role.role)).toEqual(["lead"]);
    expect(teamData.work).toContainEqual(
      expect.objectContaining({ id: startData.work.id, state: "RETURNED" }),
    );
    expect(teamData).toMatchObject({ missingPanes: [], runtime: "available", runtimePane: "on" });
    await editFakeHerdrState(fake, (state) => {
      for (const agent of state.agents as Array<{ agent_status: string }>) {
        agent.agent_status = "working";
      }
    });
    const workingStatus = envelope<{ missingPanes: string[]; runtime: string }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["status", "--json"],
          leadEnvironment,
        )
      ).stdout,
    );
    expect(workingStatus).toMatchObject({ missingPanes: [], runtime: "available" });
    await setFakeHerdrBehavior(fake, { processInfo: false });
    const unavailableStatus = envelope<{ missingPanes: string[]; runtime: string }>(
      (
        await runCliAt(
          fixture,
          fixture.repo,
          ["status", "--json"],
          leadEnvironment,
        )
      ).stdout,
    );
    expect(unavailableStatus).toMatchObject({ missingPanes: [], runtime: "unavailable" });
    await setFakeHerdrBehavior(fake, { processInfo: true });

    const workStatus = await runCliAt(
      fixture,
      fixture.repo,
      ["status", startData.work.id, "--json"],
      leadEnvironment,
    );
    expect(phaseFree(workStatus.stderr)).toBe("");
    expect(envelope<{
      decisions: Array<{ id: string }>;
      notes: Array<{ body: string }>;
      work: { currentReturn: string; id: string; state: string };
    }>(workStatus.stdout)).toMatchObject({
      decisions: [{ id: decisionId }],
      notes: [{ body: "note: implementation detail" }],
      work: { currentReturn: returned, id: startData.work.id, state: "RETURNED" },
    });

    await editFakeHerdrState(fake, (state) => {
      const agents = state.agents as Array<{ name: string; pane_id: string }>;
      const panes = state.panes as Array<{ pane_id: string }>;
      const processes = state.processes as Record<string, unknown>;
      state.agents = agents.filter((agent) => agent.name !== lead?.name);
      state.panes = panes.filter((pane) => pane.pane_id !== lead?.paneId);
      if (lead?.paneId) delete processes[lead.paneId];
    });
    const degraded = await runCliAt(fixture, room, ["status", "--json"], fake.env);
    expect(
      envelope<{ teams: Array<{ missingPanes: string[] }> }>(degraded.stdout).teams[0]?.missingPanes,
    ).toEqual([lead!.name]);
    const degradedTeam = await runCliAt(
      fixture,
      fixture.repo,
      ["status", "--json"],
      leadEnvironment,
    );
    expect(
      envelope<{ missingPanes: string[] }>(degradedTeam.stdout).missingPanes,
    ).toEqual([lead!.name]);
  });
}, 20_000);

test("SLP v2 in-team status text is structured while JSON keeps its shape (d758)", async () => {
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
    const objective = "Structured status objective";
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, objective, "--json"],
      fake.env,
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
    const id = data.work.id;
    const header = (role: "lead", who: { name: string; paneId: string }) =>
      `${data.team.teamId} g${data.team.generation} ${role} ${who.name} in ${who.paneId}`;
    const lines = async (args: string[], environment: Record<string, string>, cwd = fixture.repo) => {
      const result = await runCliAt(fixture, cwd, args, environment);
      expect(result.exitCode).toBe(0);
      return result.stdout.trim().split("\n");
    };
    const run = async (args: string[], environment: Record<string, string>, cwd = fixture.repo) => {
      expect((await runCliAt(fixture, cwd, args, environment)).exitCode).toBe(0);
    };

    expect(await lines(["status"], leadEnvironment)).toEqual([
      header("lead", lead),
      `* ${id} OPEN hub-supervisor -> ${lead.name}: ${objective}`,
      "decisions: none",
    ]);
    expect(await lines(["status", id], leadEnvironment)).toEqual([
      `${id} OPEN hub-supervisor -> ${lead.name}`,
      "revision: 0",
      `objective: ${objective}`,
      "entries: none",
      "decisions: none",
      `next: work take ${id}`,
    ]);

    await run(["work", "take", id, "--json"], leadEnvironment);
    expect((await lines(["status", id], leadEnvironment)).at(-1)).toBe(
      `next: work return ${id} "<result>"`,
    );
    expect((await lines(["status", id], fake.env, room)).at(-1)).toBe(
      `next: waiting on ${lead.name}`,
    );
    await run(["work", "return", id, "result: first pass\nsecond line", "--json"], leadEnvironment);
    await run(
      ["decide", "Ship as is", "--why", "the first pass reads clean", "--work", id, "--json"],
      leadEnvironment,
    );
    expect(await lines(["status"], leadEnvironment)).toEqual([
      header("lead", lead),
      `  ${id} RETURNED hub-supervisor -> ${lead.name}: ${objective}`,
      `decisions: d1 (${id})`,
    ]);
    expect(await lines(["status", id], fake.env, room)).toEqual([
      `${id} RETURNED hub-supervisor -> ${lead.name}`,
      "revision: 1",
      `objective: ${objective}`,
      `return by ${lead.name}: result: first pass`,
      "decisions: d1",
      `next: work accept ${id} | work note ${id} "<gap>" --rework | work accept ${id} --outcome cancelled`,
    ]);
    expect((await lines(["status", id], leadEnvironment)).at(-1)).toBe(
      "next: waiting on hub-supervisor",
    );

    await run(["work", "note", id, "needs a test", "--rework", "--json"], fake.env, room);
    const leadRework = await lines(["status", id], leadEnvironment);
    expect(leadRework[3]).toBe("note by hub-supervisor: needs a test");
    // room d129 point 4: the rework grant marks the item, so the read carries
    // the mark and the next: line reminds the holder what its return owes.
    expect(leadRework.at(-2)).toBe("lesson candidate: yes, from rework note on revision 1");
    expect(leadRework.at(-1)).toBe(
      `next: work take ${id}; this item carries a lesson candidate: ` +
        "state in the return the lesson you filed (maestro lesson file) or why none applies",
    );
    expect((await lines(["status"], leadEnvironment))[1]).toBe(
      `* ${id} RETURNED hub-supervisor -> ${lead.name}: ${objective}`,
    );

    await run(["work", "take", id, "--json"], leadEnvironment);
    await run(["work", "return", id, "result: with test", "--json"], leadEnvironment);
    await run(["work", "accept", id, "--json"], fake.env, room);
    expect(await lines(["status"], leadEnvironment)).toEqual([
      header("lead", lead),
      "1 DONE (accepted); --all to list",
      `decisions: d1 (${id})`,
    ]);
    expect(await lines(["status", "--all"], leadEnvironment)).toEqual([
      header("lead", lead),
      `  ${id} DONE (accepted) hub-supervisor -> ${lead.name}: ${objective}`,
      `decisions: d1 (${id})`,
    ]);
    const leadDone = await lines(["status", id], leadEnvironment);
    expect(leadDone[3]).toBe("acceptance by hub-supervisor: accepted");
    expect(leadDone.at(-1)).toBe("next: none (accepted)");

    const teamJson = envelope<Record<string, unknown>>(
      (await runCliAt(fixture, fixture.repo, ["status", "--json"], leadEnvironment)).stdout,
    );
    expect(Object.keys(teamJson).sort()).toEqual([
      "generation",
      "missingPanes",
      "role",
      "roles",
      "runtime",
      "runtimePane",
      "teamId",
      "work",
    ]);
    expect(teamJson.work).toEqual([]);
    const workJson = envelope<Record<string, unknown>>(
      (await runCliAt(fixture, fixture.repo, ["status", id, "--json"], leadEnvironment)).stdout,
    );
    expect(Object.keys(workJson).sort()).toEqual([
      "acceptance",
      "decisions",
      "lessonCandidate",
      "notes",
      "returns",
      "work",
    ]);
  });
}, 30_000);

// project lesson l9 (w739, d896): a Peer read the one-line objective the text
// mode prints and took it for the whole request, which was a 4,000-word brief.
// Nothing in the output suggested anything was missing.
test("l9 objective-truncation: the text read of an item names the objective's size and the command that prints it whole, and a one-line objective is unchanged", async () => {
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
    const brief = [
      "Port the module map into the worktree and prove the split holds.",
      "",
      "Scope: .maestro/worktree/g1-slp-split only; main is not yours to touch.",
      "Gates: tsc clean, the replay gate green, no allowlist line added.",
    ].join("\n");
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, brief, "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const data = envelope<{
      team: { roles: Array<{ name: string; paneId: string; role: string }> };
      work: { id: string };
    }>(started.stdout);
    const lead = data.team.roles.find((role) => role.role === "lead")!;
    const leadEnvironment = { ...fake.env, HERDR_PANE_ID: lead.paneId };
    const id = data.work.id;
    const lines = async (args: string[]) => {
      const result = await runCliAt(fixture, fixture.repo, args, leadEnvironment);
      expect(result.exitCode).toBe(0);
      return result.stdout.trim().split("\n");
    };

    const read = await lines(["status", id]);
    expect(read[2]).toBe("objective: Port the module map into the worktree and prove the split holds.");
    expect(read[3]).toBe(
      `objective truncated here: ${brief.length} characters over 4 lines; ` +
        `read it in full with: maestro status ${id} --json`,
    );
    // The pointer is honest: --json carries the objective whole.
    const json = envelope<{ work: { objective: string } }>(
      (await runCliAt(fixture, fixture.repo, ["status", id, "--json"], leadEnvironment)).stdout,
    );
    expect(json.work.objective).toBe(brief);

    // An objective that already fits on one line gains no second line.
    const added = await runCliAt(
      fixture,
      fixture.repo,
      ["work", "add", "Short objective", "--to", "peer-short", "--json"],
      leadEnvironment,
    );
    expect(added.exitCode).toBe(0);
    const peer = envelope<{ role: { name: string }; work: { id: string } }>(added.stdout);
    expect((await lines(["status", peer.work.id])).slice(0, 4)).toEqual([
      `${peer.work.id} OPEN ${lead.name} -> ${peer.role.name}`,
      "revision: 0",
      "objective: Short objective",
      "entries: none",
    ]);
  });
}, 30_000);

test("done-outcome: a cancelled item and an accepted one are different lines in the DONE listing and different numbers in the collapsed count, with the four states unchanged (w707)", async () => {
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
    const delivered = "Delivered objective";
    const started = await runCliAt(
      fixture,
      room,
      ["team", "start", fixture.repo, delivered, "--json"],
      fake.env,
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
    const lines = async (args: string[], environment: Record<string, string>) => {
      const result = await runCliAt(fixture, fixture.repo, args, environment);
      expect(result.exitCode).toBe(0);
      return result.stdout.trim().split("\n");
    };
    const run = async (args: string[], environment: Record<string, string>, cwd = fixture.repo) => {
      const result = await runCliAt(fixture, cwd, args, environment);
      expect(result.exitCode).toBe(0);
      return result;
    };

    // One item travels the whole path and ships.
    const shipped = data.work.id;
    await run(["work", "take", shipped, "--json"], leadEnvironment);
    await run(["work", "return", shipped, "result: built", "--json"], leadEnvironment);
    await run(["work", "accept", shipped, "--json"], fake.env, room);

    // The collapsed count says what the one DONE item was, not just that it is done.
    expect((await lines(["status"], leadEnvironment))[1]).toBe("1 DONE (accepted); --all to list");

    // A second item is cancelled untaken: the live specimen w698 in this team.
    const cancelled = "Cancelled objective";
    const added = await run(["work", "add", cancelled, "--json"], leadEnvironment);
    const parked = envelope<{ work: { id: string } }>(added.stdout).work.id;
    await run(["work", "accept", parked, "--outcome", "cancelled", "--json"], fake.env, room);

    expect(await lines(["status"], leadEnvironment)).toEqual([
      `${data.team.teamId} g${data.team.generation} lead ${lead.name} in ${lead.paneId}`,
      "2 DONE (1 accepted, 1 cancelled); --all to list",
      "decisions: none",
    ]);
    expect(await lines(["status", "--all"], leadEnvironment)).toEqual([
      `${data.team.teamId} g${data.team.generation} lead ${lead.name} in ${lead.paneId}`,
      `  ${shipped} DONE (accepted) hub-supervisor -> ${lead.name}: ${delivered}`,
      `  ${parked} DONE (cancelled) ${lead.name} -> ${lead.name}: ${cancelled}`,
      "decisions: none",
    ]);

    // No state was added: both rows are DONE, and the outcome stays a field the
    // JSON already carried, which is why this is a read-path change.
    const workJson = async (id: string) =>
      envelope<{ work: { acceptanceOutcome: string; state: string } }>(
        (await runCliAt(fixture, fixture.repo, ["status", id, "--json"], leadEnvironment)).stdout,
      ).work;
    expect(await workJson(shipped)).toMatchObject({ acceptanceOutcome: "accepted", state: "DONE" });
    expect(await workJson(parked)).toMatchObject({ acceptanceOutcome: "cancelled", state: "DONE" });
  });
}, 30_000);

test("orphan-status-verdict: Hub status reports a RUNNING generation with no lead pane as ORPHANED? and names maestro team stop --emergency, reports a healthy one as not orphaned, and commits nothing on either read (w715, d870, d873)", async () => {
  await withFixture(async (fixture) => {
    const room = await scaffoldRoom(fixture.home);
    expect(
      (await runCliAt(fixture, room, ["room", "mark"], { MAESTRO_ROOM_SCAFFOLD: "1", MAESTRO_SESSION_NONE: "1" }))
        .exitCode,
    ).toBe(0);
    const fake = await installFakeHerdr(fixture, { runtimePane: "record" });
    const started = await runCliAt(fixture, room, ["team", "start", fixture.repo, "Status verdict", "--json"], fake.env);
    expect(started.exitCode).toBe(0);
    const team = envelope<{
      team: { generation: number; roles: Array<{ name: string; paneId: string; role: string }>; teamId: string };
    }>(started.stdout).team;
    const lead = team.roles.find((role) => role.role === "lead")!;
    const dropPanes = (paneIds: string[]) =>
      editFakeHerdrState(fake, (state) => {
        state.agents = state.agents.filter((agent: { pane_id: string }) => !paneIds.includes(agent.pane_id));
        state.panes = state.panes.filter((pane: { pane_id: string }) => !paneIds.includes(pane.pane_id));
        for (const paneId of paneIds) delete state.processes[paneId];
      });
    // The verdict has to land in both renderings: --json carries the data the
    // brief asks for, the plain read is what a person actually sees.
    const hubStatus = async () => {
      const shown = await runCliAt(fixture, room, ["status", "--json"], fake.env);
      expect(shown.exitCode).toBe(0);
      const plain = await runCliAt(fixture, room, ["status"], fake.env);
      expect(plain.exitCode).toBe(0);
      return {
        team: envelope<{ teams: Array<{ missingPanes: string[]; orphaned: boolean; teamId: string }> }>(shown.stdout).teams[0]!,
        text: plain.stdout,
      };
    };

    const healthy = await hubStatus();
    expect(healthy.team.orphaned).toBe(false);
    expect(healthy.text).not.toContain("ORPHANED?");

    await dropPanes([lead.paneId]);
    const orphaned = await hubStatus();
    expect(orphaned.team.orphaned).toBe(true);
    expect(orphaned.team.missingPanes).toEqual([lead.name]);
    expect(orphaned.text).toContain("ORPHANED?");
    expect(orphaned.text).toContain("no lead pane");
    expect(orphaned.text).toContain(`maestro team stop ${team.teamId} --emergency`);
    // Not presented as certainty: the read cannot tell a downed Herdr apart.
    expect(orphaned.text).toContain("a Herdr that is down reads the same from here");

    // d870: a status read never stops anything.
    const project = new Database(join(fixture.repo, ".maestro", "maestro.db"), { readonly: true });
    const hub = new Database(join(room, ".maestro", "maestro.db"), { readonly: true });
    try {
      const state = (database: Database, table: string) =>
        database
          .query<{ state: string }, [string, number]>(`SELECT state FROM ${table} WHERE team_id = ? AND generation = ?`)
          .get(team.teamId, team.generation)?.state;
      expect(state(project, "slp_local_teams")).toBe("RUNNING");
      expect(state(hub, "slp_teams")).toBe("RUNNING");
      const count = (database: Database, sql: string) =>
        database.query<{ count: number }, [string, number]>(sql).get(team.teamId, team.generation)?.count;
      expect(count(project, "SELECT COUNT(*) AS count FROM slp_lifecycle_operations WHERE team_id = ? AND generation = ? AND operation = 'STOP'")).toBe(0);
      expect(count(project, "SELECT COUNT(*) AS count FROM slp_work WHERE team_id = ? AND generation = ? AND abandoned_at IS NOT NULL")).toBe(0);
      for (const database of [project, hub]) {
        expect(count(database, "SELECT COUNT(*) AS count FROM slp_activity WHERE team_id = ? AND generation = ? AND operation = 'team.stop.emergency'")).toBe(0);
      }
    } finally {
      project.close();
      hub.close();
    }
    expect(await tripwireInvocations(fake)).toEqual([]);
  });
}, 60_000);
