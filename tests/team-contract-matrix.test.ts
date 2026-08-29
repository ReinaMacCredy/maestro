import { expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import { buildEvidencePacket, coreTriggerRules } from "../src/plugins/team-observer.ts";
import { buildTeamPlan } from "../src/plugins/team-runtime.ts";
import { deriveTeamVerdict, type TeamHealth, type TeamReview, type TeamStage } from "../src/plugins/team.ts";
import {
  editFakeHerdrState,
  fakeHerdrCommands,
  installFakeHerdr,
  readFakeHerdrState,
} from "./fake-herdr.ts";
import { runCli, runCliAt, withFixture, type Fixture } from "./helpers.ts";

function envelope(value: string): Record<string, any> {
  return JSON.parse(value) as Record<string, any>;
}

async function openTeam(
  fixture: Fixture,
  room: string,
  teamId: string,
  env: Record<string, string>,
  operationId = `open-${teamId}-matrix`,
): Promise<Record<string, any>> {
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
      operationId,
      "--wait-ms",
      "0",
      "--json",
    ],
    env,
  );
  expect(opened.exitCode, opened.stderr).toBe(0);
  return envelope(opened.stdout).data;
}

test("all lifecycle axis combinations derive one deterministic verdict", () => {
  const stages: TeamStage[] = ["STARTING", "ACTIVE", "STOPPING", "STOPPED"];
  const healthValues: TeamHealth[] = [null, "READY", "DEGRADED"];
  const reviews: TeamReview[] = ["CLEAR", "REVIEW_REQUIRED"];
  const results: Array<[TeamStage, TeamHealth, TeamReview, string]> = [];

  for (const stage of stages) {
    for (const health of healthValues) {
      for (const review of reviews) {
        const expected = stage !== "ACTIVE"
          ? "CLOSED"
          : health === "DEGRADED"
            ? "DRAINING"
            : health === "READY" && review === "REVIEW_REQUIRED"
              ? "REVIEW_HOLD"
              : health === "READY"
                ? "OPERABLE"
                : "CLOSED";
        const actual = deriveTeamVerdict({ health, review, stage });
        expect(actual).toBe(expected);
        results.push([stage, health, review, actual]);
      }
    }
  }

  expect(results).toHaveLength(24);
});

test("one generation has deterministic identities for workspace, roles, and sensor", () => {
  const plan = buildTeamPlan({
    expectedRevision: 3,
    generation: 7,
    repoPath: "/tmp/team-contract-repo",
    teamId: "matrix",
  });

  expect(plan).toMatchObject({
    expectedRevision: 3,
    generation: 7,
    sensorLabel: "team:matrix:g7:sensor",
    sensorResourceKey: "team:matrix:g7:sensor",
    teamId: "matrix",
    workspaceLabel: "team-matrix-g7",
    workspaceResourceKey: "team:matrix:g7:workspace",
  });
  expect(plan.roles).toEqual([
    {
      agentName: "supervisor-matrix",
      kind: "claude",
      label: "team:matrix:g7:supervisor",
      resourceKey: "team:matrix:g7:supervisor",
      role: "supervisor",
    },
    {
      agentName: "lead-team-contract-repo",
      kind: "codex",
      label: "team:matrix:g7:lead",
      resourceKey: "team:matrix:g7:lead",
      role: "lead",
    },
    {
      agentName: "observer-matrix",
      kind: "codex",
      label: "team:matrix:g7:observer",
      resourceKey: "team:matrix:g7:observer",
      role: "observer",
    },
  ]);
});

test("mechanical trigger rules declare deterministic threshold, consequence, and dedupe", () => {
  const mechanical = coreTriggerRules.filter((rule) => rule.id.startsWith("mechanical."));
  expect(mechanical.map((rule) => [
    rule.id,
    rule.version,
    rule.minimumOccurrences,
    rule.consequence,
    rule.evidenceSource,
  ])).toEqual([
    ["mechanical.readiness-postcondition", 1, 1, "STARTING", "TeamRuntime readiness inspection"],
    ["mechanical.required-resource", 1, 1, "DEGRADED", "TeamRuntime required-resource inspection"],
    ["mechanical.stale-ledger", 1, 1, "REJECT", "Room-ledger generation and revision comparison"],
    ["mechanical.shutdown-leftover", 1, 1, "STOPPING", "TeamRuntime shutdown absence inspection"],
  ]);

  for (const rule of mechanical) {
    const input = {
      actor: "team-runtime",
      evidence: `${rule.id}:resource`,
      generation: 4,
      healthReceiptId: "health-4",
      rule,
      teamId: "matrix",
    };
    const first = buildEvidencePacket(input);
    const repeated = buildEvidencePacket(input);
    const changed = buildEvidencePacket({ ...input, evidence: `${input.evidence}:changed` });
    expect(repeated.dedupeKey).toBe(first.dedupeKey);
    expect(changed.dedupeKey).not.toBe(first.dedupeKey);
  }
});

test("574 a duplicate required role identity in another workspace degrades and closes project gates", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await openTeam(fixture, room, "cross-workspace", fake.env);
    await editFakeHerdrState(fake, (state) => {
      const agents = state.agents as Array<Record<string, string>>;
      const workspaces = state.workspaces as Array<Record<string, string>>;
      const panes = state.panes as Array<Record<string, string>>;
      const observer = agents.find((agent) =>
        agent.name === "observer-cross-workspace"
      );
      if (!observer) throw new Error("fake Observer missing");
      const workspaceId = "foreign-workspace";
      const paneId = "foreign-workspace:p1";
      workspaces.push({ cwd: fixture.repo, label: "unmanaged-foreign", workspace_id: workspaceId });
      panes.push({ cwd: fixture.repo, pane_id: paneId, workspace_id: workspaceId });
      agents.push({ ...observer, pane_id: paneId, workspace_id: workspaceId });
    });

    // Perturbation: a globally duplicated authority alias must not be hidden by workspace filtering.
    const health = await runCliAt(
      fixture,
      room,
      ["team", "health", "cross-workspace", "--operation", "health-cross-workspace", "--json"],
      fake.env,
    );
    expect(health.exitCode, health.stderr).toBe(0);
    const result = envelope(health.stdout).data;
    expect(result.team).toMatchObject({ health: "DEGRADED", verdict: "DRAINING" });
    expect(result.receipt.missing).toContainEqual(expect.objectContaining({
      code: "role.duplicate",
      resource: "team:cross-workspace:g1:observer",
    }));

    const blocked = await runCli(
      fixture,
      ["work", "add", "must stay closed", "--atomic-reason", "duplicate authority"],
      fake.env,
    );
    expect(blocked.exitCode).not.toBe(0);
    expect(envelope(blocked.stderr).error.code).toBe("GATE_BLOCKED");
  });
});

type RuntimeFault = {
  code: string;
  kind: string;
  mutate: (state: Record<string, any>, teamId: string) => void;
  resource: "lead" | "observer" | "sensor" | "supervisor" | "workspace";
};

function sensorPaneId(state: Record<string, any>, teamId: string): string {
  const match = Object.entries(state.processes as Record<string, unknown>).find(([, process]) => {
    const text = JSON.stringify(process).toLowerCase();
    return text.includes("team-sensor") && text.includes(teamId);
  });
  if (!match) throw new Error(`fake sensor missing for ${teamId}`);
  return match[0];
}

const runtimeFaults: RuntimeFault[] = [
  {
    code: "role.missing",
    kind: "missing Supervisor",
    mutate: (state, teamId) => {
      state.agents = state.agents.filter((agent: Record<string, string>) =>
        agent.name !== `supervisor-${teamId}`
      );
    },
    resource: "supervisor",
  },
  {
    code: "role.process",
    kind: "dead Lead process",
    mutate: (state, teamId) => {
      const lead = state.agents.find((agent: Record<string, string>) =>
        agent.name === "lead-repo" && agent.workspace_id === state.workspaces.find(
          (workspace: Record<string, string>) => workspace.label === `team-${teamId}-g1`,
        )?.workspace_id
      );
      if (!lead) throw new Error("fake Lead missing");
      delete state.processes[lead.pane_id];
    },
    resource: "lead",
  },
  {
    code: "role.missing",
    kind: "missing Observer",
    mutate: (state, teamId) => {
      state.agents = state.agents.filter((agent: Record<string, string>) =>
        agent.name !== `observer-${teamId}`
      );
    },
    resource: "observer",
  },
  {
    code: "role.unreachable",
    kind: "dead Observer agent",
    mutate: (state, teamId) => {
      const observer = state.agents.find((agent: Record<string, string>) =>
        agent.name === `observer-${teamId}`
      );
      if (!observer) throw new Error("fake Observer missing");
      observer.agent_status = "stopped";
    },
    resource: "observer",
  },
  {
    code: "sensor.pane",
    kind: "missing sensor pane",
    mutate: (state, teamId) => {
      const paneId = sensorPaneId(state, teamId);
      state.panes = state.panes.filter((pane: Record<string, string>) => pane.pane_id !== paneId);
      delete state.processes[paneId];
    },
    resource: "sensor",
  },
  {
    code: "sensor.process",
    kind: "dead sensor process",
    mutate: (state, teamId) => {
      delete state.processes[sensorPaneId(state, teamId)];
    },
    resource: "sensor",
  },
  {
    code: "sensor.duplicate",
    kind: "duplicate sensor process",
    mutate: (state, teamId) => {
      const paneId = sensorPaneId(state, teamId);
      const duplicatePaneId = `${paneId}:duplicate`;
      const pane = state.panes.find((candidate: Record<string, string>) =>
        candidate.pane_id === paneId
      );
      state.panes.push({ ...pane, pane_id: duplicatePaneId });
      state.processes[duplicatePaneId] = {
        ...state.processes[paneId],
        pane_id: duplicatePaneId,
      };
    },
    resource: "sensor",
  },
  {
    code: "workspace.duplicate",
    kind: "duplicate generation workspace",
    mutate: (state, teamId) => {
      const workspace = state.workspaces.find((candidate: Record<string, string>) =>
        candidate.label === `team-${teamId}-g1`
      );
      state.workspaces.push({ ...workspace, workspace_id: `${workspace.workspace_id}:duplicate` });
    },
    resource: "workspace",
  },
];

for (const [index, fault] of runtimeFaults.entries()) {
  test(`fresh health records ${fault.kind} without automatic repair`, async () => {
    await withFixture(async (fixture) => {
      const room = join(fixture.root, "room");
      await mkdir(room);
      const teamId = `fault-${index + 1}`;
      const fake = await installFakeHerdr(fixture);
      await openTeam(fixture, room, teamId, fake.env);
      const baseline = await readFakeHerdrState(fake);
      await editFakeHerdrState(fake, (state) => fault.mutate(state, teamId));
      const before = await fakeHerdrCommands(fake);

      const health = await runCliAt(
        fixture,
        room,
        ["team", "health", teamId, "--operation", `health-${teamId}`, "--json"],
        fake.env,
      );

      expect(health.exitCode, health.stderr).toBe(0);
      const result = envelope(health.stdout).data;
      expect(result.team).toMatchObject({ health: "DEGRADED", stage: "ACTIVE", verdict: "DRAINING" });
      const missing = result.receipt.missing.find((entry: Record<string, unknown>) =>
        entry.code === fault.code && entry.resource === `team:${teamId}:g1:${fault.resource}`
      );
      expect(missing).toBeDefined();
      expect(missing).toHaveProperty("actual");
      expect(missing).toHaveProperty("expected");
      if (fault.code === "role.missing") {
        expect(missing.actual).toEqual([]);
        expect(missing.expected).toMatchObject({
          count: 1,
          name: fault.resource === "supervisor" ? `supervisor-${teamId}` : `observer-${teamId}`,
        });
      } else if (fault.code === "role.unreachable") {
        expect(missing.actual).toBe("stopped");
        expect(missing.expected).toBe("registered live agent");
      } else if (fault.code === "role.process") {
        expect(missing.actual).toMatchObject({ foreground_pgid: null, foreground_processes: [] });
        expect(missing.expected).toEqual({ foreground: true, harness: "codex" });
      } else if (fault.code === "sensor.pane") {
        expect(missing.actual).toBe(sensorPaneId(baseline, teamId));
        expect(missing.expected).toHaveProperty("workspaceId");
      } else if (fault.code === "sensor.process") {
        expect(missing.actual).toMatchObject({ foreground_pgid: null, foreground_processes: [] });
        expect(missing.expected).toMatchObject({
          foreground: true,
          generation: 1,
          marker: "team-sensor",
          teamId,
        });
      } else {
        expect(missing.actual).toHaveLength(2);
        if (fault.code === "workspace.duplicate") {
          expect(missing.expected).toMatchObject({
            count: 1,
            label: `team-${teamId}-g1`,
          });
        } else {
          expect(missing.expected).toEqual({ count: 1 });
        }
      }
      const commands = (await fakeHerdrCommands(fake)).slice(before.length);
      for (const forbidden of [
        "workspace create",
        "tab create",
        "agent start",
        "pane split",
        "pane run",
        "pane close",
        "tab close",
      ]) {
        expect(commands.some((command) => command.slice(0, 2).join(" ") === forbidden)).toBe(false);
      }
    });
  });
}

test("ATTEMPTED receipt exists before every runtime command and snapshot appears only after FINALIZED", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    const operationId = "open-temporal-1";
    const databasePath = join(room, ".maestro", "maestro.db");
    const env = {
      ...fake.env,
      FAKE_HERDR_RECEIPT_DB: databasePath,
      FAKE_HERDR_RECEIPT_OPERATION: operationId,
      FAKE_HERDR_RECEIPT_TEAM: "temporal",
    };

    await openTeam(fixture, room, "temporal", env, operationId);

    const state = await readFakeHerdrState(fake);
    expect(state.receipt_audit.length).toBeGreaterThan(0);
    expect(state.receipt_audit.every((entry: Record<string, unknown>) =>
      entry.receiptStatus === "ATTEMPTED" && entry.snapshotCount === 0
    )).toBe(true);
    const database = new Database(databasePath, { strict: true });
    expect(
      database.query<{ status: string }, [string]>(
        "SELECT status FROM team_receipts WHERE operation_id = ?",
      ).get(operationId)?.status,
    ).toBe("FINALIZED");
    expect(
      database.query<{ count: number }, [string]>(
        "SELECT COUNT(*) AS count FROM team_lifecycle WHERE team_id = ?",
      ).get("temporal")?.count,
    ).toBe(1);
    database.close();
  });
});

test("legacy named panes remain unmanaged until team open creates a proved generation", async () => {
  await withFixture(async (fixture) => {
    const room = join(fixture.root, "room");
    await mkdir(room);
    const fake = await installFakeHerdr(fixture);
    await editFakeHerdrState(fake, (state) => {
      state.workspaces = [{ workspace_id: "legacy-w1", cwd: fixture.repo, label: "team-legacy" }];
      const tabs = ["supervisor-legacy", "lead-repo", "observer-legacy"].map((label, index) => ({
        label,
        root_pane_id: `legacy-p${index + 1}`,
        tab_id: `legacy-t${index + 1}`,
        workspace_id: "legacy-w1",
      }));
      state.tabs = tabs;
      state.panes = tabs.map((tab) => ({
        cwd: fixture.repo,
        label: tab.label,
        pane_id: tab.root_pane_id,
        tab_id: tab.tab_id,
        workspace_id: tab.workspace_id,
      }));
    });
    const before = await fakeHerdrCommands(fake);

    const status = await runCliAt(fixture, room, ["team", "status", "legacy", "--json"], fake.env);
    const health = await runCliAt(
      fixture,
      room,
      ["team", "health", "legacy", "--operation", "health-legacy-unmanaged", "--json"],
      fake.env,
    );

    expect(status.exitCode).toBe(1);
    expect(envelope(status.stderr).error.code).toBe("NOT_FOUND");
    expect(health.exitCode).toBe(1);
    expect(envelope(health.stderr).error.code).toBe("NOT_FOUND");
    expect(await fakeHerdrCommands(fake)).toEqual(before);
    const unmanaged = new Database(join(room, ".maestro", "maestro.db"), { strict: true });
    expect(
      unmanaged.query<{ count: number }, []>(
        "SELECT COUNT(*) AS count FROM team_lifecycle",
      ).get()?.count,
    ).toBe(0);
    unmanaged.close();

    const opened = await openTeam(fixture, room, "legacy", fake.env, "open-legacy-managed");
    expect(opened.team).toMatchObject({ generation: 1, stage: "ACTIVE", verdict: "OPERABLE" });
    const state = await readFakeHerdrState(fake);
    expect(state.workspaces.map((workspace: Record<string, string>) => workspace.label)).toEqual(
      expect.arrayContaining(["team-legacy", "team-legacy-g1"]),
    );
  });
});
