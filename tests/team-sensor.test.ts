import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import {
  detectSensorCandidates,
  runTeamSensorCycle,
  TeamSensorAuthorityError,
} from "../src/plugins/team-sensor.ts";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
} from "./fake-herdr.ts";
import {
  prepareInstallFixture,
  runCli,
  runCliAt,
  withFixture,
  type Fixture,
} from "./helpers.ts";

async function openTeam(
  fixture: Fixture,
  room: string,
  teamId: string,
  env: Record<string, string>,
): Promise<void> {
  const opened = await runCliAt(
    fixture,
    room,
    [
      "team",
      "open",
      teamId,
      "--repo",
      fixture.repo,
      "--operation",
      `open-${teamId}-sensor`,
      "--wait-ms",
      "0",
      "--json",
    ],
    env,
  );
  expect(opened.exitCode, opened.stderr).toBe(0);
}

test("the deterministic sensor maps bounded evidence to exactly the five semantic rules", () => {
  const longPrefix = "context ".repeat(2_000);
  const candidates = detectSensorCandidates({
    agents: [{
      name: "lead-repo",
      status: "working",
      text: [
        longPrefix,
        "Error: build failed in adapter",
        "Error: build failed in adapter",
        "Error: build failed in adapter",
        "w1 is done and the team is OPERABLE",
        "That was outside my role; not my role to authorize.",
        "I was wrong about the boundary.",
        "On second thought, I misread it.",
      ].join("\n"),
    }],
    dispatches: [{
      actor: "peer-x1",
      handbackFiled: false,
      id: "x1",
      lastProgressAt: "2026-08-30T00:00:00.000Z",
      stopCondition: "handback filed",
      workId: "w1",
    }],
    now: new Date("2026-08-30T00:10:00.000Z"),
    silenceMs: 300_000,
    teamId: "sensor",
    teamVerdict: "DRAINING",
    work: [{ id: "w1", state: "active", updatedAt: "2026-08-30T00:00:00.000Z" }],
  });

  expect(candidates.map((candidate) => candidate.ruleId).sort()).toEqual([
    "semantic.failure-third",
    "semantic.role-boundary",
    "semantic.self-correction",
    "semantic.status-contradiction",
    "semantic.stop-silence",
  ]);
  expect(candidates.every((candidate) => candidate.evidence.length <= 4_096)).toBe(true);
  expect(candidates.every((candidate) => candidate.excerpt.length <= 8_192)).toBe(true);
  expect(candidates.find((candidate) => candidate.ruleId === "semantic.failure-third")?.occurrences).toBe(3);
  expect(candidates.find((candidate) => candidate.ruleId === "semantic.self-correction")?.occurrences).toBe(2);
});

test("one live sensor cycle creates deduped capped packets and wakes Observer only on new evidence", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "psi", fake.env);
    await editFakeHerdrState(fake, (state) => {
      const outputs = state.outputs as Record<string, string>;
      outputs["lead-repo"] = [
        "Error: same adapter failure",
        "Error: same adapter failure",
        "Error: same adapter failure",
        "I was wrong about the adapter.",
        "On second thought, I misread the adapter.",
      ].join("\n");
    });
    const state = await readFakeHerdrState(fake);
    const workspaceId = state.workspaces.find(
      (workspace: Record<string, string>) => workspace.label === "team-psi-g1",
    )?.workspace_id;
    expect(workspaceId).toBeString();
    const config = {
      env: { ...fake.env, HOME: fixture.home },
      generation: 1,
      observerName: "observer-psi",
      repoPath: fixture.repo,
      teamId: "psi",
      workspaceId: workspaceId as string,
    };

    const first = await runTeamSensorCycle(config);
    expect(first).toMatchObject({ emitted: 2, stage: "ACTIVE" });
    expect(first.candidates.map((candidate) => candidate.ruleId).sort()).toEqual([
      "semantic.failure-third",
      "semantic.self-correction",
    ]);
    const observerPromptsAfterFirst = (await fakeHerdrCommands(fake)).filter(
      (command) => command[0] === "agent" && command[1] === "prompt" && command[2] === "observer-psi" &&
        String(command[3]).includes("[observer-packet"),
    );
    expect(observerPromptsAfterFirst).toHaveLength(2);

    const second = await runTeamSensorCycle(config);
    expect(second).toMatchObject({ deduped: 2, emitted: 0, stage: "ACTIVE" });
    const observerPromptsAfterSecond = (await fakeHerdrCommands(fake)).filter(
      (command) => command[0] === "agent" && command[1] === "prompt" && command[2] === "observer-psi" &&
        String(command[3]).includes("[observer-packet"),
    );
    expect(observerPromptsAfterSecond).toEqual(observerPromptsAfterFirst);

    const roomDatabase = new Database(join(room, ".maestro", "maestro.db"), { strict: true });
    const packets = roomDatabase
      .query<{ evidence: string; excerpt: string; rule_id: string; status: string }, []>(
        "SELECT rule_id, evidence, excerpt, status FROM team_review_packets ORDER BY created_at",
      )
      .all();
    roomDatabase.close();
    expect(packets.map((packet) => packet.rule_id).sort()).toEqual([
      "semantic.failure-third",
      "semantic.self-correction",
    ]);
    expect(packets.every((packet) => packet.status === "DELIVERED")).toBe(true);
    expect(packets.every((packet) => packet.evidence.length <= 4_096)).toBe(true);
    expect(packets.every((packet) => packet.excerpt.length <= 8_192)).toBe(true);
  });
});

test("sensor authority rejects a stale generation before reading any pane", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "omega", fake.env);
    const state = await readFakeHerdrState(fake);
    const workspaceId = state.workspaces[0]?.workspace_id as string;
    const before = await fakeHerdrCommands(fake);

    let failure: unknown;
    try {
      await runTeamSensorCycle({
        env: { ...fake.env, HOME: fixture.home },
        generation: 2,
        observerName: "observer-omega",
        repoPath: fixture.repo,
        teamId: "omega",
        workspaceId,
      });
    } catch (error) {
      failure = error;
    }
    expect(failure).toBeInstanceOf(TeamSensorAuthorityError);
    expect((failure as TeamSensorAuthorityError).code).toBe("STALE_GENERATION");
    expect(await fakeHerdrCommands(fake)).toEqual(before);
  });
});

test("the temporary installed sensor stays foreground and stale generation exits before Herdr", async () => {
  await withFixture(async (fixture) => {
    const install = await prepareInstallFixture(fixture);
    const installed = await runCli(fixture, ["install"], { PATH: install.path });
    expect(installed.exitCode, installed.stderr).toBe(0);
    const sensorShim = join(install.localBin, "maestro-team-sensor");

    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "kappa", fake.env);
    const state = await readFakeHerdrState(fake);
    const workspaceId = state.workspaces.find(
      (workspace: Record<string, string>) => workspace.label === "team-kappa-g1",
    )?.workspace_id as string;
    const env = {
      ...process.env,
      ...fake.env,
      HERDR_WORKSPACE_ID: workspaceId,
      HOME: fixture.home,
      PATH: `${install.localBin}:${fake.env.PATH}`,
    };

    const sensor = Bun.spawn([
      sensorShim,
      "--team",
      "kappa",
      "--generation",
      "1",
      "--observer",
      "observer-kappa",
    ], {
      cwd: fixture.repo,
      env,
      stderr: "pipe",
      stdout: "pipe",
    });
    const foreground = await Promise.race([
      sensor.exited.then((exitCode) => ({ exitCode, exited: true })),
      Bun.sleep(1_000).then(() => ({ exitCode: null, exited: false })),
    ]);
    expect(foreground, foreground.exited
      ? await new Response(sensor.stderr).text()
      : "sensor should remain attached to its foreground pane").toMatchObject({ exited: false });
    sensor.kill();
    await sensor.exited;
    const liveCommands = await fakeHerdrCommands(fake);
    expect(liveCommands.some((command) => command.slice(0, 2).join(" ") === "workspace list")).toBe(true);
    expect(liveCommands.some((command) => command.slice(0, 2).join(" ") === "agent read")).toBe(true);

    const beforeStale = await fakeHerdrCommands(fake);
    const stale = Bun.spawn([
      sensorShim,
      "--team",
      "kappa",
      "--generation",
      "2",
      "--observer",
      "observer-kappa",
    ], {
      cwd: fixture.repo,
      env,
      stderr: "pipe",
      stdout: "pipe",
    });
    const [staleError, staleExit] = await Promise.all([
      new Response(stale.stderr).text(),
      stale.exited,
    ]);
    expect(staleExit).toBe(78);
    expect(staleError).toContain("STALE_GENERATION");
    expect(await fakeHerdrCommands(fake)).toEqual(beforeStale);
  });
});
