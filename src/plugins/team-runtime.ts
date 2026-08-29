import { createHash } from "node:crypto";
import { basename, resolve } from "node:path";

export type TeamRole = "supervisor" | "lead" | "observer";

export interface TeamRolePlan {
  agentName: string;
  kind: string;
  label: string;
  resourceKey: string;
  role: TeamRole;
}

export interface TeamPlan {
  expectedRevision: number;
  generation: number;
  repoPath: string;
  roles: TeamRolePlan[];
  sensorLabel: string;
  sensorResourceKey: string;
  teamId: string;
  workspaceLabel: string;
  workspaceResourceKey: string;
}

export interface RuntimeEffect {
  data: Record<string, unknown>;
  key: string;
  kind: string;
  ok: boolean;
  resourceKey: string;
}

export interface MissingPostcondition {
  actual: unknown;
  code: string;
  expected: unknown;
  resource: string;
}

export interface TeamInspection {
  actual: Record<string, unknown>;
  complete: boolean;
  inspectedAt: string;
  missing: MissingPostcondition[];
  runtimeRevision: string;
}

export interface TeamRuntime {
  ensure(
    plan: TeamPlan,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]>;
  inspect(plan: TeamPlan, effects: readonly RuntimeEffect[]): Promise<TeamInspection>;
  probeObserver(
    plan: TeamPlan,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]>;
}

export class TeamRuntimeError extends Error {
  constructor(
    message: string,
    readonly command: readonly string[],
    readonly stderr?: string,
  ) {
    super(message);
  }
}

interface WorkspaceRecord {
  cwd?: string;
  label?: string;
  workspace_id?: string;
}

interface PaneRecord {
  cwd?: string;
  label?: string;
  pane_id?: string;
  tab_id?: string;
  workspace_id?: string;
}

interface TabRecord {
  label?: string;
  root_pane_id?: string;
  tab_id?: string;
  workspace_id?: string;
}

interface AgentRecord {
  agent_status?: string;
  kind?: string;
  name?: string;
  pane_id?: string;
  workspace_id?: string;
}

function recordArray<T>(value: unknown, key: string): T[] {
  if (!value || typeof value !== "object") return [];
  const nested = (value as Record<string, unknown>)[key];
  return Array.isArray(nested) ? nested as T[] : [];
}

function resultOf(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object") return {};
  const result = (value as Record<string, unknown>).result;
  return result && typeof result === "object" ? result as Record<string, unknown> : {};
}

function objectAt(value: Record<string, unknown>, key: string): Record<string, unknown> {
  const candidate = value[key];
  return candidate && typeof candidate === "object" ? candidate as Record<string, unknown> : {};
}

function stringAt(value: Record<string, unknown>, key: string): string | undefined {
  const candidate = value[key];
  return typeof candidate === "string" ? candidate : undefined;
}

function samePath(left: string | undefined, right: string): boolean {
  return left !== undefined && resolve(left) === resolve(right);
}

function effectData(
  effects: readonly RuntimeEffect[],
  key: string,
): Record<string, unknown> | undefined {
  const effect = effects.find((candidate) => candidate.key === key && candidate.ok);
  return effect?.data;
}

function accepted(result: Record<string, unknown>): boolean {
  return result.accepted !== false && result.delivered !== false;
}

function canonicalHash(value: unknown): string {
  const normalize = (candidate: unknown): unknown => {
    if (Array.isArray(candidate)) return candidate.map(normalize);
    if (!candidate || typeof candidate !== "object") return candidate;
    return Object.fromEntries(
      Object.entries(candidate as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, normalize(nested)]),
    );
  };
  return createHash("sha256").update(JSON.stringify(normalize(value))).digest("hex");
}

function processIsForeground(info: Record<string, unknown>): boolean {
  if (typeof info.foreground_pgid === "number") return true;
  for (const key of ["foreground_processes", "processes"]) {
    if (Array.isArray(info[key]) && info[key].length > 0) return true;
  }
  return false;
}

function processText(info: Record<string, unknown>): string {
  return JSON.stringify(info).toLowerCase();
}

export function buildTeamPlan(input: {
  expectedRevision: number;
  generation: number;
  repoPath: string;
  teamId: string;
}): TeamPlan {
  const prefix = `team:${input.teamId}:g${input.generation}`;
  return {
    expectedRevision: input.expectedRevision,
    generation: input.generation,
    repoPath: resolve(input.repoPath),
    roles: [
      {
        agentName: `supervisor-${input.teamId}`,
        kind: "claude",
        label: `${prefix}:supervisor`,
        resourceKey: `${prefix}:supervisor`,
        role: "supervisor",
      },
      {
        agentName: `lead-${basename(input.repoPath)}`,
        kind: "codex",
        label: `${prefix}:lead`,
        resourceKey: `${prefix}:lead`,
        role: "lead",
      },
      {
        agentName: `observer-${input.teamId}`,
        kind: "codex",
        label: `${prefix}:observer`,
        resourceKey: `${prefix}:observer`,
        role: "observer",
      },
    ],
    sensorLabel: `${prefix}:sensor`,
    sensorResourceKey: `${prefix}:sensor`,
    teamId: input.teamId,
    workspaceLabel: `team-${input.teamId}-g${input.generation}`,
    workspaceResourceKey: `${prefix}:workspace`,
  };
}

export class HerdrTeamRuntime implements TeamRuntime {
  private async command(args: string[], cwd: string): Promise<Record<string, unknown>> {
    let child: ReturnType<typeof Bun.spawn>;
    try {
      child = Bun.spawn(["herdr", ...args], {
        cwd,
        env: process.env,
        stderr: "pipe",
        stdout: "pipe",
      });
    } catch (error) {
      throw new TeamRuntimeError(
        `cannot start Herdr: ${error instanceof Error ? error.message : String(error)}`,
        args,
      );
    }
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(child.stdout as ReadableStream<Uint8Array>).text(),
      new Response(child.stderr as ReadableStream<Uint8Array>).text(),
      child.exited,
    ]);
    if (exitCode !== 0) {
      throw new TeamRuntimeError(
        `Herdr command failed (${exitCode}): ${args.join(" ")}`,
        args,
        stderr.trim(),
      );
    }
    try {
      return JSON.parse(stdout) as Record<string, unknown>;
    } catch {
      throw new TeamRuntimeError(
        `Herdr returned invalid JSON for: ${args.join(" ")}`,
        args,
        stdout.trim(),
      );
    }
  }

  private async workspaces(plan: TeamPlan): Promise<WorkspaceRecord[]> {
    return recordArray<WorkspaceRecord>(resultOf(await this.command(["workspace", "list"], plan.repoPath)), "workspaces");
  }

  private async panes(plan: TeamPlan, workspaceId: string): Promise<PaneRecord[]> {
    return recordArray<PaneRecord>(
      resultOf(await this.command(["pane", "list", "--workspace", workspaceId], plan.repoPath)),
      "panes",
    );
  }

  private async tabs(plan: TeamPlan, workspaceId: string): Promise<TabRecord[]> {
    return recordArray<TabRecord>(
      resultOf(await this.command(["tab", "list", "--workspace", workspaceId], plan.repoPath)),
      "tabs",
    );
  }

  private async agents(plan: TeamPlan): Promise<AgentRecord[]> {
    return recordArray<AgentRecord>(resultOf(await this.command(["agent", "list"], plan.repoPath)), "agents");
  }

  private async processInfo(plan: TeamPlan, paneId: string): Promise<Record<string, unknown>> {
    return resultOf(await this.command(["pane", "process-info", "--pane", paneId], plan.repoPath));
  }

  async ensure(
    plan: TeamPlan,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]> {
    const effects = new Map(knownEffects.map((effect) => [effect.key, effect]));
    const remember = async (effect: RuntimeEffect): Promise<void> => {
      const current = effects.get(effect.key);
      if (current?.ok) return;
      effects.set(effect.key, effect);
      await recordEffect(effect);
    };

    let matchingWorkspaces = (await this.workspaces(plan)).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    if (matchingWorkspaces.length === 0) {
      const response = resultOf(await this.command([
        "workspace",
        "create",
        "--cwd",
        plan.repoPath,
        "--label",
        plan.workspaceLabel,
        "--no-focus",
      ], plan.repoPath));
      const workspace = objectAt(response, "workspace");
      await remember({
        data: {
          cwd: stringAt(workspace, "cwd") ?? plan.repoPath,
          label: stringAt(workspace, "label") ?? plan.workspaceLabel,
          workspaceId: stringAt(workspace, "workspace_id"),
        },
        key: "workspace",
        kind: "workspace.create",
        ok: Boolean(stringAt(workspace, "workspace_id")),
        resourceKey: plan.workspaceResourceKey,
      });
      matchingWorkspaces = (await this.workspaces(plan)).filter(
        (candidate) => candidate.label === plan.workspaceLabel,
      );
    }
    if (matchingWorkspaces.length !== 1 || !matchingWorkspaces[0]?.workspace_id) {
      return [...effects.values()];
    }
    const workspaceId = matchingWorkspaces[0].workspace_id;

    for (const role of plan.roles) {
      const workspaceAgents = (await this.agents(plan)).filter(
        (agent) => agent.name === role.agentName && agent.workspace_id === workspaceId,
      );
      let paneId = workspaceAgents.length === 1 ? workspaceAgents[0]?.pane_id : undefined;
      if (!paneId && workspaceAgents.length === 0) {
        const matchingTabs = (await this.tabs(plan, workspaceId)).filter(
          (tab) => tab.label === role.label,
        );
        if (matchingTabs.length === 1) paneId = matchingTabs[0]?.root_pane_id;
        if (!paneId && matchingTabs.length === 0) {
          const response = resultOf(await this.command([
            "tab",
            "create",
            "--workspace",
            workspaceId,
            "--cwd",
            plan.repoPath,
            "--label",
            role.label,
            "--no-focus",
          ], plan.repoPath));
          const rootPane = objectAt(response, "root_pane");
          paneId = stringAt(rootPane, "pane_id");
          await remember({
            data: { paneId, workspaceId },
            key: `role.${role.role}.pane`,
            kind: "tab.create",
            ok: Boolean(paneId),
            resourceKey: role.resourceKey,
          });
        }
        if (paneId) {
          const response = resultOf(await this.command([
            "agent",
            "start",
            role.agentName,
            "--kind",
            role.kind,
            "--pane",
            paneId,
          ], plan.repoPath));
          await remember({
            data: {
              accepted: accepted(response),
              agentName: role.agentName,
              paneId,
              workspaceId,
            },
            key: `role.${role.role}.agent`,
            kind: "agent.start",
            ok: accepted(response),
            resourceKey: role.resourceKey,
          });
        }
      }
      if (!paneId) continue;
      const promptKey = `role.${role.role}.prompt`;
      const priorPrompt = effects.get(promptKey);
      if (!priorPrompt?.ok || priorPrompt.data.paneId !== paneId) {
        const body = [
          `[from room][bootstrap ${plan.teamId} g${plan.generation}]`,
          `You are ${role.agentName}.`,
          `Your role is ${role.role}; remain within team ${plan.teamId} generation ${plan.generation}.`,
          role.role === "observer"
            ? "Accept only bounded evidence packets and submit packet-bound review verdicts."
            : "Read the repository protocol before taking team work.",
        ].join(" ");
        const response = resultOf(await this.command(
          ["agent", "prompt", role.agentName, body],
          plan.repoPath,
        ));
        await remember({
          data: {
            agentName: role.agentName,
            delivered: accepted(response),
            generation: plan.generation,
            paneId,
          },
          key: promptKey,
          kind: "agent.prompt",
          ok: accepted(response),
          resourceKey: role.resourceKey,
        });
      }
    }

    const observer = plan.roles.find((role) => role.role === "observer");
    const observerAgent = observer
      ? (await this.agents(plan)).find(
        (agent) => agent.name === observer.agentName && agent.workspace_id === workspaceId,
      )
      : undefined;
    if (observer && observerAgent?.pane_id) {
      let sensorPaneId = stringAt(effectData([...effects.values()], "sensor.pane") ?? {}, "paneId");
      const currentPanes = await this.panes(plan, workspaceId);
      if (!sensorPaneId) {
        const matchingSensorPanes: string[] = [];
        for (const candidate of currentPanes) {
          if (!candidate.pane_id) continue;
          try {
            const info = await this.processInfo(plan, candidate.pane_id);
            const text = processText(info);
            if (
              processIsForeground(info) &&
              text.includes("team-sensor") &&
              text.includes(plan.teamId.toLowerCase()) &&
              text.includes(String(plan.generation))
            ) {
              matchingSensorPanes.push(candidate.pane_id);
            }
          } catch {}
        }
        if (matchingSensorPanes.length === 1) {
          sensorPaneId = matchingSensorPanes[0];
          await remember({
            data: { adopted: true, paneId: sensorPaneId, workspaceId },
            key: "sensor.pane",
            kind: "pane.adopt",
            ok: true,
            resourceKey: plan.sensorResourceKey,
          });
        } else if (matchingSensorPanes.length > 1) {
          await remember({
            data: { duplicates: matchingSensorPanes, workspaceId },
            key: "sensor.pane",
            kind: "pane.adopt",
            ok: false,
            resourceKey: plan.sensorResourceKey,
          });
          return this.probeObserver(plan, [...effects.values()], recordEffect);
        }
      }
      if (!sensorPaneId || !currentPanes.some((pane) => pane.pane_id === sensorPaneId)) {
        const response = resultOf(await this.command([
          "pane",
          "split",
          "--pane",
          observerAgent.pane_id,
          "--direction",
          "down",
          "--cwd",
          plan.repoPath,
          "--no-focus",
        ], plan.repoPath));
        const created = objectAt(response, "pane");
        sensorPaneId = stringAt(created, "pane_id");
        await remember({
          data: { paneId: sensorPaneId, workspaceId },
          key: "sensor.pane",
          kind: "pane.split",
          ok: Boolean(sensorPaneId),
          resourceKey: plan.sensorResourceKey,
        });
      }
      if (sensorPaneId) {
        let running = false;
        try {
          const info = await this.processInfo(plan, sensorPaneId);
          const text = processText(info);
          running = processIsForeground(info) &&
            text.includes("team-sensor") &&
            text.includes(plan.teamId.toLowerCase()) &&
            text.includes(String(plan.generation));
        } catch {}
        if (!running) {
          const response = resultOf(await this.command([
            "pane",
            "run",
            sensorPaneId,
            "maestro-team-sensor",
            "--team",
            plan.teamId,
            "--generation",
            String(plan.generation),
            "--observer",
            observer.agentName,
          ], plan.repoPath));
          await remember({
            data: {
              accepted: accepted(response),
              generation: plan.generation,
              paneId: sensorPaneId,
              workspaceId,
            },
            key: "sensor.run",
            kind: "pane.run",
            ok: accepted(response),
            resourceKey: plan.sensorResourceKey,
          });
        }
      }
    }

    return this.probeObserver(plan, [...effects.values()], recordEffect);
  }

  async probeObserver(
    plan: TeamPlan,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]> {
    const effects = new Map(knownEffects.map((effect) => [effect.key, effect]));
    const existing = effects.get("sensor.probe");
    if (existing?.ok) return [...effects.values()];
    const observer = plan.roles.find((role) => role.role === "observer");
    if (!observer) return [...effects.values()];
    const response = resultOf(await this.command([
      "agent",
      "prompt",
      observer.agentName,
      `[team-sensor-probe ${plan.teamId} g${plan.generation}] acknowledge delivery only; do not review`,
    ], plan.repoPath));
    const effect: RuntimeEffect = {
      data: {
        agentName: observer.agentName,
        delivered: accepted(response),
        generation: plan.generation,
      },
      key: "sensor.probe",
      kind: "agent.prompt",
      ok: accepted(response),
      resourceKey: plan.sensorResourceKey,
    };
    effects.set(effect.key, effect);
    await recordEffect(effect);
    return [...effects.values()];
  }

  async inspect(plan: TeamPlan, effects: readonly RuntimeEffect[]): Promise<TeamInspection> {
    const missing: MissingPostcondition[] = [];
    const workspaces = await this.workspaces(plan);
    const matchingWorkspaces = workspaces.filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    if (matchingWorkspaces.length !== 1) {
      missing.push({
        actual: matchingWorkspaces.map((workspace) => workspace.workspace_id),
        code: matchingWorkspaces.length === 0 ? "workspace.missing" : "workspace.duplicate",
        expected: { count: 1, cwd: plan.repoPath, label: plan.workspaceLabel },
        resource: plan.workspaceResourceKey,
      });
    }
    const workspace = matchingWorkspaces.length === 1 ? matchingWorkspaces[0] : undefined;
    if (workspace && !samePath(workspace.cwd, plan.repoPath)) {
      missing.push({
        actual: workspace.cwd ?? null,
        code: "workspace.cwd",
        expected: plan.repoPath,
        resource: plan.workspaceResourceKey,
      });
    }
    const workspaceId = workspace?.workspace_id;
    const panes = workspaceId ? await this.panes(plan, workspaceId) : [];
    const agents = workspaceId ? await this.agents(plan) : [];
    const processByPane: Record<string, Record<string, unknown>> = {};

    for (const role of plan.roles) {
      const matches = agents.filter(
        (agent) => agent.name === role.agentName && agent.workspace_id === workspaceId,
      );
      if (matches.length !== 1) {
        missing.push({
          actual: matches.map((agent) => ({ paneId: agent.pane_id, status: agent.agent_status })),
          code: matches.length === 0 ? "role.missing" : "role.duplicate",
          expected: { count: 1, name: role.agentName, workspaceId },
          resource: role.resourceKey,
        });
        continue;
      }
      const agent = matches[0] as AgentRecord;
      const attachedPane = panes.find((pane) => pane.pane_id === agent.pane_id);
      if (!attachedPane) {
        missing.push({
          actual: agent.pane_id ?? null,
          code: "role.pane",
          expected: { workspaceId },
          resource: role.resourceKey,
        });
        continue;
      }
      if (!samePath(attachedPane.cwd, plan.repoPath)) {
        missing.push({
          actual: attachedPane.cwd ?? null,
          code: "role.cwd",
          expected: plan.repoPath,
          resource: role.resourceKey,
        });
      }
      if (!agent.agent_status || ["stopped", "exited", "error"].includes(agent.agent_status)) {
        missing.push({
          actual: agent.agent_status ?? null,
          code: "role.unreachable",
          expected: "registered live agent",
          resource: role.resourceKey,
        });
      }
      if (agent.pane_id) {
        const info = await this.processInfo(plan, agent.pane_id);
        processByPane[agent.pane_id] = info;
        if (!processIsForeground(info) || !processText(info).includes(role.kind.toLowerCase())) {
          missing.push({
            actual: info,
            code: "role.process",
            expected: { foreground: true, harness: role.kind },
            resource: role.resourceKey,
          });
        }
      }
      const prompt = effectData(effects, `role.${role.role}.prompt`);
      if (
        prompt?.delivered !== true ||
        prompt.agentName !== role.agentName ||
        prompt.paneId !== agent.pane_id ||
        prompt.generation !== plan.generation
      ) {
        missing.push({
          actual: prompt ?? null,
          code: "role.prompt",
          expected: {
            agentName: role.agentName,
            delivered: true,
            generation: plan.generation,
            paneId: agent.pane_id,
          },
          resource: role.resourceKey,
        });
      }
    }

    const sensorProcessPanes: string[] = [];
    for (const pane of panes) {
      if (!pane.pane_id) continue;
      const info = processByPane[pane.pane_id] ?? await this.processInfo(plan, pane.pane_id);
      processByPane[pane.pane_id] = info;
      const text = processText(info);
      if (
        processIsForeground(info) &&
        text.includes("team-sensor") &&
        text.includes(plan.teamId.toLowerCase()) &&
        text.includes(String(plan.generation))
      ) {
        sensorProcessPanes.push(pane.pane_id);
      }
    }
    if (sensorProcessPanes.length > 1) {
      missing.push({
        actual: sensorProcessPanes,
        code: "sensor.duplicate",
        expected: { count: 1 },
        resource: plan.sensorResourceKey,
      });
    }
    const sensorPaneId = stringAt(effectData(effects, "sensor.pane") ?? {}, "paneId") ??
      (sensorProcessPanes.length === 1 ? sensorProcessPanes[0] : undefined);
    const sensorPane = panes.find((pane) => pane.pane_id === sensorPaneId);
    if (!sensorPaneId || !sensorPane) {
      missing.push({
        actual: sensorPaneId ?? null,
        code: "sensor.pane",
        expected: { workspaceId },
        resource: plan.sensorResourceKey,
      });
      missing.push({
        actual: null,
        code: "sensor.process",
        expected: {
          foreground: true,
          generation: plan.generation,
          marker: "team-sensor",
          teamId: plan.teamId,
        },
        resource: plan.sensorResourceKey,
      });
    } else {
      const info = await this.processInfo(plan, sensorPaneId);
      processByPane[sensorPaneId] = info;
      const text = processText(info);
      if (
        !processIsForeground(info) ||
        !text.includes("team-sensor") ||
        !text.includes(plan.teamId.toLowerCase()) ||
        !text.includes(String(plan.generation))
      ) {
        missing.push({
          actual: info,
          code: "sensor.process",
          expected: {
            foreground: true,
            generation: plan.generation,
            marker: "team-sensor",
            teamId: plan.teamId,
          },
          resource: plan.sensorResourceKey,
        });
      }
    }
    const probe = effectData(effects, "sensor.probe");
    if (
      probe?.delivered !== true ||
      probe.agentName !== `observer-${plan.teamId}` ||
      probe.generation !== plan.generation
    ) {
      missing.push({
        actual: probe ?? null,
        code: "sensor.delivery",
        expected: {
          agentName: `observer-${plan.teamId}`,
          delivered: true,
          generation: plan.generation,
        },
        resource: plan.sensorResourceKey,
      });
    }

    const actual = {
      agents: agents.filter((agent) => agent.workspace_id === workspaceId),
      effects,
      panes,
      processes: processByPane,
      workspace: workspace ?? null,
      workspaceMatches: matchingWorkspaces.length,
    };
    return {
      actual,
      complete: missing.length === 0,
      inspectedAt: new Date().toISOString(),
      missing,
      runtimeRevision: canonicalHash(actual),
    };
  }
}
