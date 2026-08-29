import { createHash } from "node:crypto";
import { basename, resolve } from "node:path";

export type TeamRole = "supervisor" | "lead" | "observer";
export type TeamRepairResource = TeamRole | "sensor" | "workspace";

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

export interface AdvisorConsultationRequest {
  contextRefs: string[];
  decisionRef: string;
  operationId: string;
  question: string;
  requestedBy: string;
  stopCondition: string;
  timeoutMs: number;
}

export interface AdvisorRuntimeResult {
  effects: RuntimeEffect[];
  error: string | null;
  paneId: string | null;
  recommendation: string | null;
  stopped: boolean;
  tabId: string | null;
}

export interface TeamShutdownResult {
  effects: RuntimeEffect[];
  inspection: TeamInspection;
}

export interface TeamRuntime {
  consultAdvisor(
    plan: TeamPlan,
    request: AdvisorConsultationRequest,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<AdvisorRuntimeResult>;
  deliverObserver(
    plan: TeamPlan,
    body: string,
    effectKey: string,
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect>;
  ensure(
    plan: TeamPlan,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]>;
  inspect(plan: TeamPlan, effects: readonly RuntimeEffect[]): Promise<TeamInspection>;
  inspectAbsence(plan: TeamPlan, effects: readonly RuntimeEffect[]): Promise<TeamInspection>;
  reconcile(
    plan: TeamPlan,
    resources: readonly TeamRepairResource[],
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]>;
  probeObserver(
    plan: TeamPlan,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]>;
  requestDrain(
    plan: TeamPlan,
    operationId: string,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]>;
  shutdown(
    plan: TeamPlan,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<TeamShutdownResult>;
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

  private async textCommand(args: string[], cwd: string): Promise<string> {
    const child = Bun.spawn(["herdr", ...args], {
      cwd,
      env: process.env,
      stderr: "pipe",
      stdout: "pipe",
    });
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    if (exitCode !== 0) {
      throw new TeamRuntimeError(
        `Herdr command failed (${exitCode}): ${args.join(" ")}`,
        args,
        stderr.trim(),
      );
    }
    return stdout;
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

  async deliverObserver(
    plan: TeamPlan,
    body: string,
    effectKey: string,
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect> {
    const observer = plan.roles.find((role) => role.role === "observer");
    if (!observer) {
      throw new TeamRuntimeError("observer role is missing from the team plan", ["agent", "prompt"]);
    }
    const response = resultOf(await this.command(
      ["agent", "prompt", observer.agentName, body],
      plan.repoPath,
    ));
    const effect: RuntimeEffect = {
      data: {
        agentName: observer.agentName,
        delivered: accepted(response),
        generation: plan.generation,
      },
      key: effectKey,
      kind: "agent.prompt",
      ok: accepted(response),
      resourceKey: plan.sensorResourceKey,
    };
    await recordEffect(effect);
    return effect;
  }

  async consultAdvisor(
    plan: TeamPlan,
    request: AdvisorConsultationRequest,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<AdvisorRuntimeResult> {
    const effects = new Map(knownEffects.map((effect) => [effect.key, effect]));
    const remember = async (effect: RuntimeEffect): Promise<void> => {
      effects.set(effect.key, effect);
      await recordEffect(effect);
    };
    const advisorName = `advisor-${plan.teamId}`;
    const advisorKey = `team:${plan.teamId}:g${plan.generation}:advisor`;
    const advisorLabel = `${advisorKey}:${request.operationId}`;
    let paneId: string | null = null;
    let tabId: string | null = null;
    let recommendation: string | null = null;
    let failure: string | null = null;
    try {
      const matchingWorkspaces = (await this.workspaces(plan)).filter(
        (workspace) => workspace.label === plan.workspaceLabel,
      );
      if (matchingWorkspaces.length !== 1 || !matchingWorkspaces[0]?.workspace_id) {
        throw new TeamRuntimeError(
          `advisor requires one workspace ${plan.workspaceLabel}; found ${matchingWorkspaces.length}`,
          ["workspace", "list"],
        );
      }
      const workspaceId = matchingWorkspaces[0].workspace_id;
      const matchingAgents = (await this.agents(plan)).filter(
        (agent) => agent.name === advisorName && agent.workspace_id === workspaceId,
      );
      if (matchingAgents.length > 1) {
        throw new TeamRuntimeError(
          `advisor identity is duplicated: ${advisorName}`,
          ["agent", "list"],
        );
      }
      if (matchingAgents.length === 1) {
        paneId = matchingAgents[0]?.pane_id ?? null;
        const attached = paneId
          ? (await this.panes(plan, workspaceId)).find((pane) => pane.pane_id === paneId)
          : undefined;
        tabId = attached?.tab_id ?? null;
        await remember({
          data: { adopted: true, agentName: advisorName, paneId, tabId, workspaceId },
          key: "advisor.agent",
          kind: "agent.adopt",
          ok: Boolean(paneId),
          resourceKey: advisorKey,
        });
      } else {
        const matchingTabs = (await this.tabs(plan, workspaceId)).filter(
          (tab) => tab.label === advisorLabel,
        );
        if (matchingTabs.length > 1) {
          throw new TeamRuntimeError(
            `advisor tab identity is duplicated: ${advisorLabel}`,
            ["tab", "list"],
          );
        }
        if (matchingTabs.length === 1) {
          tabId = matchingTabs[0]?.tab_id ?? null;
          paneId = matchingTabs[0]?.root_pane_id ?? null;
        } else {
          const created = resultOf(await this.command([
            "tab",
            "create",
            "--workspace",
            workspaceId,
            "--cwd",
            plan.repoPath,
            "--label",
            advisorLabel,
            "--no-focus",
          ], plan.repoPath));
          const tab = objectAt(created, "tab");
          const rootPane = objectAt(created, "root_pane");
          tabId = stringAt(tab, "tab_id") ?? null;
          paneId = stringAt(rootPane, "pane_id") ?? null;
          await remember({
            data: { paneId, tabId, workspaceId },
            key: "advisor.pane",
            kind: "tab.create",
            ok: Boolean(paneId && tabId),
            resourceKey: advisorKey,
          });
        }
        if (!paneId) throw new TeamRuntimeError("advisor pane was not created", ["tab", "create"]);
        const started = resultOf(await this.command([
          "agent",
          "start",
          advisorName,
          "--kind",
          "codex",
          "--pane",
          paneId,
        ], plan.repoPath));
        await remember({
          data: { accepted: accepted(started), agentName: advisorName, paneId, tabId, workspaceId },
          key: "advisor.agent",
          kind: "agent.start",
          ok: accepted(started),
          resourceKey: advisorKey,
        });
        if (!accepted(started)) {
          throw new TeamRuntimeError("advisor agent did not start", ["agent", "start", advisorName]);
        }
      }

      const priorRecommendation = effects.get("advisor.recommendation");
      if (priorRecommendation?.ok && typeof priorRecommendation.data.recommendation === "string") {
        recommendation = priorRecommendation.data.recommendation;
      } else {
        const body = [
          `[advisor-consultation ${request.operationId}]`,
          `team=${plan.teamId}`,
          `generation=${plan.generation}`,
          `requestedBy=${request.requestedBy}`,
          `decision=${request.decisionRef}`,
          `question=${JSON.stringify(request.question)}`,
          `context=${JSON.stringify(request.contextRefs)}`,
          `stop=${JSON.stringify(request.stopCondition)}`,
          "You hold no work, lease, decision, mutation, or store authority.",
          "Finish with exactly one line: MAESTRO_ADVISOR_RETURN {\"recommendation\":\"non-empty text\"}",
        ].join("\n");
        const prompted = resultOf(await this.command([
          "agent",
          "prompt",
          advisorName,
          body,
          "--wait",
          "--until",
          "idle",
          "--until",
          "done",
          "--timeout",
          String(request.timeoutMs),
        ], plan.repoPath));
        await remember({
          data: { agentName: advisorName, delivered: accepted(prompted), paneId },
          key: "advisor.prompt",
          kind: "agent.prompt",
          ok: accepted(prompted),
          resourceKey: advisorKey,
        });
        if (!accepted(prompted)) {
          throw new TeamRuntimeError("advisor prompt was not accepted", ["agent", "prompt", advisorName]);
        }
        const output = await this.textCommand(
          ["agent", "read", advisorName, "--source", "recent-unwrapped", "--lines", "120"],
          plan.repoPath,
        );
        const marker = "MAESTRO_ADVISOR_RETURN ";
        const line = output.split("\n").reverse().find((candidate) => candidate.startsWith(marker));
        if (line) {
          try {
            const parsed = JSON.parse(line.slice(marker.length)) as { recommendation?: unknown };
            if (typeof parsed.recommendation === "string" && parsed.recommendation.trim()) {
              recommendation = parsed.recommendation.trim();
            }
          } catch {}
        }
        await remember({
          data: { outputTail: output.slice(-4_096), recommendation },
          key: "advisor.recommendation",
          kind: "agent.read",
          ok: recommendation !== null,
          resourceKey: advisorKey,
        });
        if (!recommendation) {
          failure = "advisor completed without a valid MAESTRO_ADVISOR_RETURN marker";
        }
      }
    } catch (error) {
      failure = error instanceof Error ? error.message : String(error);
    } finally {
      try {
        if (tabId) {
          const closed = resultOf(await this.command(["tab", "close", tabId], plan.repoPath));
          await remember({
            data: { closed: closed.closed !== false, tabId },
            key: "advisor.close",
            kind: "tab.close",
            ok: closed.closed !== false,
            resourceKey: advisorKey,
          });
        } else if (paneId) {
          const closed = resultOf(await this.command(["pane", "close", paneId], plan.repoPath));
          await remember({
            data: { closed: closed.closed !== false, paneId },
            key: "advisor.close",
            kind: "pane.close",
            ok: closed.closed !== false,
            resourceKey: advisorKey,
          });
        }
      } catch (error) {
        failure = failure ?? (error instanceof Error ? error.message : String(error));
      }
    }
    let stopped = false;
    try {
      stopped = !(await this.agents(plan)).some((agent) => agent.name === advisorName);
    } catch (error) {
      failure = failure ?? (error instanceof Error ? error.message : String(error));
    }
    if (!stopped) failure = failure ?? `advisor still live after bounded stop: ${advisorName}`;
    return {
      effects: [...effects.values()],
      error: failure,
      paneId,
      recommendation,
      stopped,
      tabId,
    };
  }

  async requestDrain(
    plan: TeamPlan,
    operationId: string,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]> {
    const effects = new Map(knownEffects.map((effect) => [effect.key, effect]));
    const workspaceIds = new Set(
      (await this.workspaces(plan))
        .filter((workspace) => workspace.label === plan.workspaceLabel)
        .map((workspace) => workspace.workspace_id)
        .filter((workspaceId): workspaceId is string => Boolean(workspaceId)),
    );
    if (workspaceIds.size === 0) return [...effects.values()];
    const leadName = plan.roles.find((role) => role.role === "lead")?.agentName;
    const advisorName = `advisor-${plan.teamId}`;
    const targets = (await this.agents(plan)).filter((agent) =>
      Boolean(agent.name && agent.workspace_id && workspaceIds.has(agent.workspace_id)) &&
      (agent.name === leadName || agent.name === advisorName || agent.name?.startsWith("peer-"))
    );
    for (const target of targets) {
      if (!target.name) continue;
      const effectKey = `shutdown.${operationId}.drain.${target.name}`;
      if (effects.get(effectKey)?.ok) continue;
      const response = resultOf(await this.command([
        "agent",
        "prompt",
        target.name,
        [
          `[from room][drain ${plan.teamId} g${plan.generation}]`,
          "Reach the current recorded stop without starting new work.",
          "File the bounded handback or note evidence, then release the work lease.",
          "Observer and sensor remain live until the drain is settled.",
        ].join(" "),
      ], plan.repoPath));
      const role = plan.roles.find((candidate) => candidate.agentName === target.name);
      const effect: RuntimeEffect = {
        data: {
          agentName: target.name,
          delivered: accepted(response),
          generation: plan.generation,
          paneId: target.pane_id ?? null,
          workspaceId: target.workspace_id ?? null,
        },
        key: effectKey,
        kind: "agent.prompt",
        ok: accepted(response),
        resourceKey: role?.resourceKey ?? `team:${plan.teamId}:g${plan.generation}:seat:${target.name}`,
      };
      effects.set(effect.key, effect);
      await recordEffect(effect);
    }
    return [...effects.values()];
  }

  async inspectAbsence(
    plan: TeamPlan,
    effects: readonly RuntimeEffect[],
  ): Promise<TeamInspection> {
    const missing: MissingPostcondition[] = [];
    const matchingWorkspaces = (await this.workspaces(plan)).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    const workspaceIds = new Set(
      matchingWorkspaces
        .map((workspace) => workspace.workspace_id)
        .filter((workspaceId): workspaceId is string => Boolean(workspaceId)),
    );
    const knownWorkspaceIds = new Set(workspaceIds);
    for (const effect of effects) {
      const workspaceId = effect.data.workspaceId;
      if (typeof workspaceId === "string") knownWorkspaceIds.add(workspaceId);
    }
    for (const workspace of matchingWorkspaces) {
      missing.push({
        actual: workspace,
        code: "shutdown.workspace",
        expected: "generation workspace absent",
        resource: plan.workspaceResourceKey,
      });
    }

    const plannedNames = new Set([
      ...plan.roles.map((role) => role.agentName),
      `advisor-${plan.teamId}`,
    ]);
    const remainingAgents = (await this.agents(plan)).filter((agent) =>
      Boolean(agent.workspace_id && knownWorkspaceIds.has(agent.workspace_id)) ||
      Boolean(agent.name && plannedNames.has(agent.name))
    );
    for (const agent of remainingAgents) {
      const role = plan.roles.find((candidate) => candidate.agentName === agent.name);
      missing.push({
        actual: agent,
        code: "shutdown.role",
        expected: "generation role absent",
        resource: role?.resourceKey ?? `team:${plan.teamId}:g${plan.generation}:seat:${agent.name ?? "unknown"}`,
      });
    }

    const remainingPanes: PaneRecord[] = [];
    const remainingProcesses: Array<{ info: Record<string, unknown>; paneId: string }> = [];
    for (const workspaceId of workspaceIds) {
      const panes = await this.panes(plan, workspaceId);
      remainingPanes.push(...panes);
      for (const pane of panes) {
        if (!pane.pane_id) continue;
        const info = await this.processInfo(plan, pane.pane_id);
        if (processIsForeground(info)) {
          remainingProcesses.push({ info, paneId: pane.pane_id });
        }
      }
    }
    for (const pane of remainingPanes) {
      missing.push({
        actual: pane,
        code: "shutdown.pane",
        expected: "generation pane absent",
        resource: `team:${plan.teamId}:g${plan.generation}:pane:${pane.pane_id ?? "unknown"}`,
      });
    }
    for (const process of remainingProcesses) {
      missing.push({
        actual: process.info,
        code: "shutdown.process",
        expected: "generation foreground process absent",
        resource: `team:${plan.teamId}:g${plan.generation}:process:${process.paneId}`,
      });
    }
    const actual = {
      agents: remainingAgents,
      panes: remainingPanes,
      processes: remainingProcesses,
      workspaces: matchingWorkspaces,
    };
    return {
      actual,
      complete: missing.length === 0,
      inspectedAt: new Date().toISOString(),
      missing,
      runtimeRevision: canonicalHash(actual),
    };
  }

  async shutdown(
    plan: TeamPlan,
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<TeamShutdownResult> {
    const effects = new Map(knownEffects.map((effect) => [effect.key, effect]));
    const remember = async (effect: RuntimeEffect): Promise<void> => {
      effects.set(effect.key, effect);
      await recordEffect(effect);
    };
    const closeTab = async (
      tab: TabRecord,
      resourceKey: string,
    ): Promise<void> => {
      if (!tab.tab_id) return;
      const response = resultOf(await this.command(["tab", "close", tab.tab_id], plan.repoPath));
      await remember({
        data: { closed: response.closed !== false, tabId: tab.tab_id, workspaceId: tab.workspace_id },
        key: `shutdown.tab.${tab.tab_id}`,
        kind: "tab.close",
        ok: response.closed !== false,
        resourceKey,
      });
    };
    const closePane = async (paneId: string, workspaceId: string): Promise<void> => {
      const response = resultOf(await this.command(["pane", "close", paneId], plan.repoPath));
      await remember({
        data: { closed: response.closed !== false, paneId, workspaceId },
        key: `shutdown.pane.${paneId}`,
        kind: "pane.close",
        ok: response.closed !== false,
        resourceKey: plan.sensorResourceKey,
      });
    };

    const matchingWorkspaces = (await this.workspaces(plan)).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    const supervisor = plan.roles.find((role) => role.role === "supervisor");
    const observer = plan.roles.find((role) => role.role === "observer");
    for (const workspace of matchingWorkspaces) {
      if (!workspace.workspace_id) continue;
      const workspaceId = workspace.workspace_id;
      const tabs = await this.tabs(plan, workspaceId);
      const panes = await this.panes(plan, workspaceId);
      const supervisorTabs = tabs.filter((tab) => tab.label === supervisor?.label);
      const observerTabs = tabs.filter((tab) => tab.label === observer?.label);
      const workTabs = tabs
        .filter((tab) => tab.label !== supervisor?.label && tab.label !== observer?.label)
        .sort((left, right) => (left.label ?? "").localeCompare(right.label ?? ""));

      for (const tab of workTabs) {
        const role = plan.roles.find((candidate) => candidate.label === tab.label);
        await closeTab(
          tab,
          role?.resourceKey ?? `team:${plan.teamId}:g${plan.generation}:seat:${tab.label ?? tab.tab_id}`,
        );
      }

      const sensorPaneIds = new Set<string>();
      const recordedSensorPane = stringAt(effectData([...effects.values()], "sensor.pane") ?? {}, "paneId");
      if (recordedSensorPane && panes.some((pane) => pane.pane_id === recordedSensorPane)) {
        sensorPaneIds.add(recordedSensorPane);
      }
      for (const pane of panes) {
        if (!pane.pane_id) continue;
        const info = await this.processInfo(plan, pane.pane_id);
        const text = processText(info);
        if (
          processIsForeground(info) &&
          text.includes("team-sensor") &&
          text.includes(plan.teamId.toLowerCase()) &&
          text.includes(String(plan.generation))
        ) {
          sensorPaneIds.add(pane.pane_id);
        }
      }
      for (const paneId of sensorPaneIds) await closePane(paneId, workspaceId);
      for (const tab of observerTabs) {
        await closeTab(tab, observer?.resourceKey ?? plan.sensorResourceKey);
      }
      for (const tab of supervisorTabs) {
        await closeTab(tab, supervisor?.resourceKey ?? plan.workspaceResourceKey);
      }
      const response = resultOf(await this.command([
        "workspace",
        "close",
        workspaceId,
      ], plan.repoPath));
      await remember({
        data: { closed: response.closed !== false, workspaceId },
        key: `shutdown.workspace.${workspaceId}`,
        kind: "workspace.close",
        ok: response.closed !== false,
        resourceKey: plan.workspaceResourceKey,
      });
    }
    const inspection = await this.inspectAbsence(plan, [...effects.values()]);
    return { effects: [...effects.values()], inspection };
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

    effects.delete("sensor.probe");
    return this.probeObserver(plan, [...effects.values()], recordEffect);
  }

  async reconcile(
    plan: TeamPlan,
    resources: readonly TeamRepairResource[],
    knownEffects: readonly RuntimeEffect[],
    recordEffect: (effect: RuntimeEffect) => Promise<void> | void,
  ): Promise<RuntimeEffect[]> {
    const selected = new Set(resources);
    const effects = new Map(knownEffects.map((effect) => [effect.key, effect]));
    const remember = async (effect: RuntimeEffect): Promise<void> => {
      effects.set(effect.key, effect);
      await recordEffect(effect);
    };
    let matchingWorkspaces = (await this.workspaces(plan)).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    if (matchingWorkspaces.length === 0 && selected.has("workspace")) {
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
        (workspace) => workspace.label === plan.workspaceLabel,
      );
    }
    if (matchingWorkspaces.length !== 1 || !matchingWorkspaces[0]?.workspace_id) {
      return [...effects.values()];
    }
    const workspaceId = matchingWorkspaces[0].workspace_id;
    for (const role of plan.roles) {
      if (!selected.has(role.role)) continue;
      const matches = (await this.agents(plan)).filter(
        (agent) => agent.name === role.agentName && agent.workspace_id === workspaceId,
      );
      if (matches.length > 1) {
        await remember({
          data: { duplicates: matches.map((agent) => agent.pane_id), workspaceId },
          key: `role.${role.role}.agent`,
          kind: "agent.adopt",
          ok: false,
          resourceKey: role.resourceKey,
        });
        continue;
      }
      let paneId = matches[0]?.pane_id;
      if (!paneId) {
        const matchingTabs = (await this.tabs(plan, workspaceId)).filter(
          (tab) => tab.label === role.label,
        );
        if (matchingTabs.length > 1) {
          await remember({
            data: { duplicates: matchingTabs.map((tab) => tab.tab_id), workspaceId },
            key: `role.${role.role}.pane`,
            kind: "tab.adopt",
            ok: false,
            resourceKey: role.resourceKey,
          });
          continue;
        }
        paneId = matchingTabs[0]?.root_pane_id;
        if (!paneId) {
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
            data: { accepted: accepted(response), agentName: role.agentName, paneId, workspaceId },
            key: `role.${role.role}.agent`,
            kind: "agent.start",
            ok: accepted(response),
            resourceKey: role.resourceKey,
          });
        }
      }
      if (!paneId) continue;
      const response = resultOf(await this.command([
        "agent",
        "prompt",
        role.agentName,
        `[from room][reconcile ${plan.teamId} g${plan.generation}] resume only role ${role.role}; preserve current assignment and report identity`,
      ], plan.repoPath));
      await remember({
        data: {
          agentName: role.agentName,
          delivered: accepted(response),
          generation: plan.generation,
          paneId,
        },
        key: `role.${role.role}.prompt`,
        kind: "agent.prompt",
        ok: accepted(response),
        resourceKey: role.resourceKey,
      });
    }

    if (selected.has("sensor")) {
      const observer = plan.roles.find((role) => role.role === "observer");
      const observerAgent = observer
        ? (await this.agents(plan)).find(
          (agent) => agent.name === observer.agentName && agent.workspace_id === workspaceId,
        )
        : undefined;
      if (observer && observerAgent?.pane_id) {
        const panes = await this.panes(plan, workspaceId);
        let sensorPaneId = stringAt(effectData([...effects.values()], "sensor.pane") ?? {}, "paneId");
        if (!sensorPaneId || !panes.some((pane) => pane.pane_id === sensorPaneId)) {
          const matching: string[] = [];
          for (const candidate of panes) {
            if (!candidate.pane_id) continue;
            const info = await this.processInfo(plan, candidate.pane_id);
            const text = processText(info);
            if (
              processIsForeground(info) &&
              text.includes("team-sensor") &&
              text.includes(plan.teamId.toLowerCase()) &&
              text.includes(String(plan.generation))
            ) matching.push(candidate.pane_id);
          }
          if (matching.length === 1) sensorPaneId = matching[0];
          if (matching.length === 0) {
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
            sensorPaneId = stringAt(objectAt(response, "pane"), "pane_id");
          }
          await remember({
            data: { adopted: matching.length === 1, paneId: sensorPaneId, workspaceId },
            key: "sensor.pane",
            kind: matching.length === 1 ? "pane.adopt" : "pane.split",
            ok: Boolean(sensorPaneId) && matching.length <= 1,
            resourceKey: plan.sensorResourceKey,
          });
        }
        if (sensorPaneId) {
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
            data: { accepted: accepted(response), generation: plan.generation, paneId: sensorPaneId, workspaceId },
            key: "sensor.run",
            kind: "pane.run",
            ok: accepted(response),
            resourceKey: plan.sensorResourceKey,
          });
        }
      }
    }
    effects.delete("sensor.probe");
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
