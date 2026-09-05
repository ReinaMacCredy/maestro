import { existsSync } from "node:fs";
import { rm } from "node:fs/promises";
import { resolve } from "node:path";
import {
  HerdrClient,
  SlpRuntimeError,
  type HerdrAgent,
  type HerdrPane,
  type HerdrProcessInfo,
  type HerdrReadSource,
  type HerdrTab,
  type HerdrWorkspace,
} from "./herdr-client.ts";
import { resolveHomeDirectory } from "./home.ts";
import { renderedProfilePath } from "./profiles.ts";
import { slpRuntimeDirectory } from "./slp-process.ts";

export { SlpRuntimeError };

// d755: the acknowledgement is polled inside a fixed window with non-scrolling
// reads; d756: a blocked pane is classified from its own text.
const acknowledgementWindowMs = 30_000;
const acknowledgementPollMs = 1_000;
const acknowledgementQuietPolls = 4;
const paneTailLines = 15;
const trustDialogPattern =
  /Do you trust the contents of this directory|Quick safety check: Is this a project you created or one you trust|Yes, I trust this folder/;

export type SlpRole = "team-supervisor" | "lead" | "peer";

// Hub d90/d98: a seat is launched as a rendered native profile; the harness
// and every launch flag come from the profile, never from the plan.
export interface SeatLaunch {
  autocompact?: number | null;
  harness: "claude" | "codex";
  profile: string;
}

export interface SlpRolePlan {
  autocompact?: number | null;
  kind: "claude" | "codex";
  label: string;
  name: string;
  profile: string;
  role: SlpRole;
}

export interface SlpRoleContract {
  acknowledgement: string;
  body: string;
  briefDigest: string;
  instanceId: string;
  packDigest: string;
  readyChallenge: string;
}

export interface SlpTeamPlan {
  generation: number;
  projectPath: string;
  roles: SlpRolePlan[];
  teamId: string;
  workspaceLabel: string;
}

export interface SlpRuntimeRole {
  briefDigest: string;
  instanceId: string;
  name: string;
  packDigest: string;
  paneId: string;
  profile: string;
  readyChallenge: string;
  role: SlpRole;
  workspaceId: string;
}

export type SlpAcknowledgedRole = Pick<
  SlpRuntimeRole,
  "briefDigest" | "instanceId" | "packDigest" | "paneId" | "readyChallenge"
>;

export interface SlpRuntimeStart {
  createdTabIds: string[];
  createdWorkspace: boolean;
  roles: SlpRuntimeRole[];
  startedPaneIds: string[];
  workspaceId: string;
}

export interface SlpRuntimePeer {
  createdTabId: string | null;
  role: SlpRuntimeRole;
  startedPaneId: string | null;
}

export interface SlpRuntimeInspection {
  missingPanes: string[];
  runtime: "available";
  runtimePane: boolean;
  workspace: boolean;
}

export const slpStopEnvironment = {
  closeWorkspace: "MAESTRO_SLP_STOP_CLOSE_WORKSPACE",
  helperTab: "MAESTRO_SLP_STOP_HELPER_TAB",
  helperWorkspace: "MAESTRO_SLP_STOP_HELPER_WORKSPACE",
  project: "MAESTRO_SLP_STOP_PROJECT",
  token: "MAESTRO_SLP_STOP_GRANT",
} as const;

function cleanPaneLines(output: string): string[] {
  return output
    .replaceAll(/\u001b\[[0-?]*[ -/]*[@-~]/g, "")
    .split(/\r?\n/)
    .map(normalizeAcknowledgementLine);
}

function paneTailOf(lines: readonly string[]): string[] {
  return lines.filter((line) => line !== "").slice(-paneTailLines);
}

function foreground(value: HerdrProcessInfo): boolean {
  if (typeof value.foreground_process_group_id === "number") return true;
  const record = value as unknown as Record<string, unknown>;
  const processes = record.foreground_processes ?? record.processes;
  return Array.isArray(processes) && processes.length > 0;
}

function settled(agent: HerdrAgent): boolean {
  return agent.agent_status === "idle" || agent.agent_status === "done";
}

function normalizeAcknowledgementLine(line: string): string {
  return line
    .trim()
    .replace(/^[^\p{L}\p{N}]+(?=SLP_ROLE_READY(?:\s|$))/u, "");
}

// d768: the two challenge halves arrive on separate contract lines, so a
// reply that joins them with one space (a wrap inside the value, or a small
// model's habit) still proves both were read; every other byte stays exact.
function includesExactAcknowledgement(
  lines: readonly string[],
  acknowledgement: string,
): boolean {
  const spaced = acknowledgement.replace(
    /challenge=([0-9a-f]{16})([0-9a-f]{16})$/,
    "challenge=$1 $2",
  );
  for (let start = 0; start < lines.length; start += 1) {
    if (!/^SLP_ROLE_READY(?:\s|$)/.test(lines[start] ?? "")) continue;
    let candidate = lines[start] ?? "";
    for (let index = start; candidate.length <= spaced.length; index += 1) {
      if (candidate === acknowledgement || candidate === spaced) return true;
      const continuation = lines[index + 1];
      if (!continuation) break;
      candidate = `${candidate} ${continuation}`;
    }
  }
  return false;
}

function herdrErrorCode(error: unknown): string | null {
  return error instanceof SlpRuntimeError ? error.herdrCode : null;
}

export function buildSlpTeamPlan(input: {
  generation: number;
  lead: SeatLaunch;
  projectPath: string;
  teamId: string;
  teamSupervisor: SeatLaunch;
}): SlpTeamPlan {
  const projectPath = resolve(input.projectPath);
  const prefix = `slp:${input.teamId}:g${input.generation}`;
  return {
    generation: input.generation,
    projectPath,
    roles: [
      {
        autocompact: input.teamSupervisor.autocompact ?? null,
        kind: input.teamSupervisor.harness,
        label: `${prefix}:team-supervisor`,
        name: `supervisor-${input.teamId}`,
        profile: input.teamSupervisor.profile,
        role: "team-supervisor",
      },
      {
        autocompact: input.lead.autocompact ?? null,
        kind: input.lead.harness,
        label: `${prefix}:lead`,
        name: `lead-${input.teamId}`,
        profile: input.lead.profile,
        role: "lead",
      },
    ],
    teamId: input.teamId,
    workspaceLabel: `slp-${input.teamId}-g${input.generation}`,
  };
}

// d90: the harness flag names the rendered profile and nothing else; the
// agent file has no carrier for autocompact, so it rides the launch line (F10).
export function launchArguments(role: Pick<SlpRolePlan, "autocompact" | "kind" | "profile">): string[] {
  if (role.kind === "codex") return ["--profile", `maestro-${role.profile}`];
  const args = ["--agent", `maestro-${role.profile}`];
  if (role.autocompact) args.push("--autocompact", String(role.autocompact));
  return args;
}

function startCommand(role: Pick<SlpRolePlan, "kind" | "name">, paneId: string, launch: string[]): string[] {
  return ["agent", "start", role.name, "--kind", role.kind, "--pane", paneId, "--timeout", "60000", "--", ...launch];
}

export class HerdrSlpRuntime {
  readonly client: HerdrClient;

  constructor(
    private readonly commandTimeoutMs = 15_000,
    private readonly environment: Record<string, string | undefined> = process.env,
    private readonly agentReadyTimeoutMs = 5_000,
    private readonly promptReadyTimeoutMs = 30_000,
  ) {
    this.client = new HerdrClient(environment, commandTimeoutMs);
  }

  private note(line: string): void {
    process.stderr.write(`${line}\n`);
  }

  private async readPane(
    target: string,
    source: HerdrReadSource,
    lines: number,
  ): Promise<string[]> {
    return cleanPaneLines(await this.client.agentRead(target, source, lines));
  }

  private async paneTail(target: string): Promise<string[]> {
    try {
      return paneTailOf(await this.readPane(target, "visible", 40));
    } catch {
      return [];
    }
  }

  private async blockedFailure(
    plan: SlpTeamPlan,
    name: string,
    harness: string,
    command: readonly string[],
    stderr: string | undefined,
  ): Promise<SlpRuntimeError> {
    let lines: string[] = [];
    try {
      lines = await this.readPane(name, "visible", 40);
    } catch {}
    const paneTail = paneTailOf(lines);
    if (lines.some((line) => trustDialogPattern.test(line))) {
      return new SlpRuntimeError(
        `${harness} is waiting on its directory trust dialog in ${plan.projectPath}; open that directory once, run ${harness}, accept the dialog, then rerun this command`,
        command,
        stderr,
        { code: "TRUST_DIALOG", directory: plan.projectPath, harness, paneTail },
      );
    }
    return new SlpRuntimeError(
      `agent ${name} is blocked on interactive input in ${plan.projectPath}`,
      command,
      stderr,
      { code: "AGENT_BLOCKED", directory: plan.projectPath, harness, paneTail },
    );
  }

  private async settledAgent(name: string): Promise<boolean> {
    const matches = (await this.client.agentList()).filter((agent) => agent.name === name);
    return matches.length === 1 && settled(matches[0] as HerdrAgent);
  }

  private async requireAcknowledgement(
    roleName: string,
    contract: SlpRoleContract,
  ): Promise<void> {
    const deadline = Date.now() + acknowledgementWindowMs;
    let previous: string | null = null;
    let quietPolls = 0;
    while (true) {
      const visible = await this.readPane(roleName, "visible", 60);
      if (includesExactAcknowledgement(visible, contract.acknowledgement)) return;
      const snapshot = visible.join("\n");
      if (snapshot === previous) quietPolls += 1;
      else {
        previous = snapshot;
        quietPolls = 0;
      }
      const remaining = deadline - Date.now();
      const quiet =
        quietPolls >= acknowledgementQuietPolls && (await this.settledAgent(roleName));
      if (remaining <= 0 || quiet) {
        const recent = await this.readPane(roleName, "recent_unwrapped", 120);
        if (includesExactAcknowledgement(recent, contract.acknowledgement)) return;
        throw new SlpRuntimeError(
          `ROLE_ACKNOWLEDGEMENT_MISMATCH: ${roleName} did not return its exact generation contract acknowledgement within ${Math.round(acknowledgementWindowMs / 1000)}s`,
          ["agent", "read", roleName],
          undefined,
          { code: "ROLE_ACKNOWLEDGEMENT_MISMATCH", paneTail: paneTailOf(visible) },
        );
      }
      await Bun.sleep(Math.min(acknowledgementPollMs, remaining));
    }
  }

  // A2: a missing render fails before any Herdr call, never as a pane with a
  // harness error dialog (d98: work add renders nothing on demand).
  private requireRenderedProfile(role: Pick<SlpRolePlan, "kind" | "name" | "profile">): void {
    const home = resolveHomeDirectory({ environmentHome: this.environment.HOME });
    const path = renderedProfilePath(home, role.kind, role.profile);
    if (existsSync(path)) return;
    throw new SlpRuntimeError(
      `${role.name} launches with profile maestro-${role.profile} but ${path} is not rendered; run maestro install`,
      ["agent", "start", role.name],
      undefined,
      { code: "PROFILE_NOT_INSTALLED", harness: role.kind },
    );
  }

  private workspaces(): Promise<HerdrWorkspace[]> {
    return this.client.workspaceList();
  }

  private tabs(workspaceId: string): Promise<HerdrTab[]> {
    return this.client.tabList(workspaceId);
  }

  private panes(workspaceId: string): Promise<HerdrPane[]> {
    return this.client.paneList(workspaceId);
  }

  private agents(): Promise<HerdrAgent[]> {
    return this.client.agentList();
  }

  // The named agent is active for prompts only once agent.list carries it
  // settled in its pane; agent.start over the socket can answer before that
  // (live 2026-09-05: the contract prompt hit agent_not_ready right after a
  // successful start), so the wait is ours either way.
  private async awaitActiveAgent(
    plan: SlpTeamPlan,
    role: Pick<SlpRolePlan, "kind" | "name">,
    paneId: string,
    command: readonly string[],
    deadline: number,
    stderr: string | undefined,
    requireSettled: boolean,
  ): Promise<void> {
    while (true) {
      const matches = (await this.agents()).filter((agent) => agent.name === role.name);
      if (
        matches.length === 1 && matches[0]?.pane_id === paneId &&
        (requireSettled ? settled(matches[0]) : matches[0].agent_status !== "blocked")
      ) return;
      if (
        matches.length === 1 &&
        matches[0]?.pane_id === paneId &&
        matches[0].agent_status === "blocked"
      ) {
        throw await this.blockedFailure(plan, role.name, role.kind, command, stderr);
      }
      const remaining = deadline - Date.now();
      if (remaining <= 0) {
        throw new SlpRuntimeError(
          `agent ${role.name} did not become ready within ${this.agentReadyTimeoutMs}ms`,
          command,
          stderr,
          { paneTail: await this.paneTail(role.name) },
        );
      }
      await Bun.sleep(Math.min(100, remaining));
    }
  }

  private async startAgent(
    plan: SlpTeamPlan,
    role: Pick<SlpRolePlan, "autocompact" | "kind" | "name" | "profile">,
    paneId: string,
  ): Promise<void> {
    const launch = launchArguments(role);
    const command = startCommand(role, paneId, launch);
    const deadline = Date.now() + this.agentReadyTimeoutMs;
    while (true) {
      try {
        const started = await this.client.agentStart(
          { args: launch, kind: role.kind, name: role.name, pane_id: paneId, timeout_ms: 60_000 },
          Math.max(this.commandTimeoutMs, 75_000),
        );
        const ready = started !== null && started.pane_id === paneId && settled(started) &&
          started.launch_pending !== true && started.interactive_ready !== false;
        if (!ready) {
          await this.awaitActiveAgent(plan, role, paneId, command, Date.now() + this.agentReadyTimeoutMs, undefined, false);
        }
        return;
      } catch (error) {
        const errorCode = herdrErrorCode(error);
        const stderr = error instanceof SlpRuntimeError ? error.stderr : undefined;
        if (errorCode === "agent_not_ready") {
          await this.awaitActiveAgent(plan, role, paneId, command, deadline, stderr, true);
          return;
        }
        if (errorCode !== "agent_pane_busy") {
          if (error instanceof SlpRuntimeError) {
            throw new SlpRuntimeError(`role did not start: ${role.name}; ${error.message}`, command, stderr, {
              code: error.code === "SLP_RUNTIME" ? undefined : error.code,
              herdrCode: error.herdrCode ?? undefined,
            });
          }
          throw error;
        }
        const remaining = deadline - Date.now();
        if (remaining <= 0) {
          throw new SlpRuntimeError(
            `pane did not become an available shell within ${this.agentReadyTimeoutMs}ms`,
            command,
            stderr,
          );
        }
        await Bun.sleep(Math.min(100, remaining));
      }
    }
  }

  private async promptAgent(plan: SlpTeamPlan, name: string, body: string): Promise<void> {
    const command = ["agent", "prompt", name, body, "--wait", "--timeout", "120000"];
    const deadline = Date.now() + this.promptReadyTimeoutMs;
    while (true) {
      try {
        await this.client.agentPrompt(name, body, { timeout_ms: 120_000 }, 130_000);
        return;
      } catch (error) {
        const code = herdrErrorCode(error);
        const stderr = error instanceof SlpRuntimeError ? error.stderr : undefined;
        if (code === "agent_blocked") {
          const harness = plan.roles.find((role) => role.name === name)?.kind ?? "codex";
          throw await this.blockedFailure(plan, name, harness, command, stderr);
        }
        // agent_not_ready here is the name not yet active after a successful
        // start (live 2026-09-05); it clears within the same window as a stall.
        if (code !== "agent_prompt_stalled" && code !== "agent_not_ready") {
          if (error instanceof SlpRuntimeError) {
            throw new SlpRuntimeError(
              `role contract was not delivered: ${name}; ${error.message}`,
              ["agent", "prompt", name],
              stderr,
              { code: error.code === "SLP_RUNTIME" ? undefined : error.code, herdrCode: error.herdrCode ?? undefined },
            );
          }
          throw error;
        }
        const remaining = deadline - Date.now();
        if (remaining <= 0) {
          throw new SlpRuntimeError(
            `role contract prompt remained ${code === "agent_not_ready" ? "not ready" : "stalled"} for ${this.promptReadyTimeoutMs}ms`,
            command,
            stderr,
            { paneTail: await this.paneTail(name) },
          );
        }
        await Bun.sleep(Math.min(100, remaining));
      }
    }
  }

  private async waitForWorkspaceAbsence(
    workspaceId: string,
    label: string,
    action: "rollback" | "shutdown",
  ): Promise<void> {
    const deadline = Date.now() + 10_000;
    while (true) {
      const remaining = (await this.workspaces()).filter(
        (workspace) => workspace.workspace_id === workspaceId,
      );
      if (remaining.length === 0) return;
      const delay = Math.min(100, deadline - Date.now());
      if (delay <= 0) {
        throw new SlpRuntimeError(
          `workspace ${label} remains after ${action}`,
          ["workspace", "close", workspaceId],
        );
      }
      await Bun.sleep(delay);
    }
  }

  private async closeWorkspace(
    workspaceId: string,
    label: string,
    action: "rollback" | "shutdown",
  ): Promise<void> {
    const present = (await this.workspaces()).some(
      (workspace) => workspace.workspace_id === workspaceId,
    );
    if (!present) return;
    let closeError: unknown = null;
    try {
      await this.client.workspaceClose(workspaceId);
    } catch (error) {
      closeError = error;
    }
    try {
      await this.waitForWorkspaceAbsence(workspaceId, label, action);
    } catch (error) {
      throw closeError ?? error;
    }
  }

  private processInfo(paneId: string): Promise<HerdrProcessInfo> {
    return this.client.paneProcessInfo(paneId);
  }

  private async findWorkspace(plan: SlpTeamPlan): Promise<string> {
    const matching = (await this.workspaces()).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    if (matching.length !== 1 || !matching[0]?.workspace_id) {
      throw new SlpRuntimeError(
        `expected exactly one workspace ${plan.workspaceLabel}; found ${matching.length}`,
        ["workspace", "list"],
      );
    }
    return matching[0].workspace_id;
  }

  async start(
    plan: SlpTeamPlan,
    contracts: ReadonlyMap<SlpRole, SlpRoleContract>,
    acknowledged: ReadonlyMap<SlpRole, SlpAcknowledgedRole> = new Map(),
  ): Promise<SlpRuntimeStart> {
    const createdTabIds: string[] = [];
    const startedPaneIds: string[] = [];
    const reused = new Map<SlpRole, SlpAcknowledgedRole>();
    let createdWorkspace = false;
    let workspaceId: string | null = null;
    for (const role of plan.roles) this.requireRenderedProfile(role);
    try {
      let matching = (await this.workspaces()).filter(
        (workspace) => workspace.label === plan.workspaceLabel,
      );
      if (matching.length === 0) {
        const created = await this.client.workspaceCreate({
          cwd: plan.projectPath,
          label: plan.workspaceLabel,
        });
        workspaceId = created.workspace?.workspace_id ?? null;
        createdWorkspace = workspaceId !== null;
        matching = (await this.workspaces()).filter(
          (workspace) => workspace.label === plan.workspaceLabel,
        );
      }
      if (matching.length !== 1 || !matching[0]?.workspace_id) {
        throw new SlpRuntimeError(
          `expected exactly one workspace ${plan.workspaceLabel}; found ${matching.length}`,
          ["workspace", "list"],
        );
      }
      workspaceId = matching[0].workspace_id;

      for (const role of plan.roles) {
        const startedAt = Date.now();
        const globalMatches = (await this.agents()).filter(
          (agent) => agent.name === role.name,
        );
        const workspaceMatches = globalMatches.filter(
          (agent) => agent.workspace_id === workspaceId,
        );
        if (globalMatches.length > workspaceMatches.length || workspaceMatches.length > 1) {
          throw new SlpRuntimeError(
            `role identity is duplicated outside the generation: ${role.name}`,
            ["agent", "list"],
          );
        }
        let paneId = workspaceMatches[0]?.pane_id ?? null;
        const contract = contracts.get(role.role);
        if (!contract) {
          throw new SlpRuntimeError(`missing role contract: ${role.role}`, ["agent", "prompt"]);
        }
        const prior = acknowledged.get(role.role);
        if (paneId && prior && prior.paneId === paneId && prior.instanceId === contract.instanceId) {
          reused.set(role.role, prior);
          this.note(`${role.name}: already acknowledged in ${paneId}; left alone`);
          continue;
        }
        if (!paneId) {
          const matchingTabs = (await this.tabs(workspaceId)).filter(
            (tab) => tab.label === role.label,
          );
          if (matchingTabs.length > 1) {
            throw new SlpRuntimeError(
              `role tab is duplicated: ${role.label}`,
              ["tab", "list"],
            );
          }
          const matchingTab = matchingTabs[0];
          paneId = matchingTab?.root_pane_id ?? null;
          if (paneId) startedPaneIds.push(paneId);
          if (!paneId) {
            const created = await this.client.tabCreate({
              cwd: plan.projectPath,
              label: role.label,
              workspace_id: workspaceId,
            });
            const tabId = created.tab?.tab_id ?? null;
            paneId = created.root_pane?.pane_id ?? null;
            if (tabId) createdTabIds.push(tabId);
          }
          if (!paneId) {
            throw new SlpRuntimeError(`role pane was not created: ${role.name}`, ["tab", "create"]);
          }
          this.note(`${role.name}: starting ${role.kind} pane in ${plan.workspaceLabel}`);
          await this.startAgent(plan, role, paneId);
        }
        this.note(
          `${role.name}: waiting for acknowledgement (up to ${Math.round(acknowledgementWindowMs / 1000)}s)`,
        );
        await this.promptAgent(plan, role.name, contract.body);
        await this.requireAcknowledgement(role.name, contract);
        this.note(`${role.name}: ready in ${Math.round((Date.now() - startedAt) / 1000)}s`);
      }

      const roles = await this.inspectRequired(plan, workspaceId, contracts, reused);
      return { createdTabIds, createdWorkspace, roles, startedPaneIds, workspaceId };
    } catch (error) {
      await this.rollback(plan, { createdTabIds, createdWorkspace, startedPaneIds, workspaceId });
      throw error;
    }
  }

  private async inspectRequired(
    plan: SlpTeamPlan,
    workspaceId: string,
    contracts: ReadonlyMap<SlpRole, SlpRoleContract>,
    reused: ReadonlyMap<SlpRole, SlpAcknowledgedRole>,
  ): Promise<SlpRuntimeRole[]> {
    const panes = await this.panes(workspaceId);
    const paneIds = new Set(panes.flatMap((pane) => pane.pane_id ? [pane.pane_id] : []));
    const agents = await this.agents();
    const roles: SlpRuntimeRole[] = [];
    for (const role of plan.roles) {
      const matches = agents.filter(
        (agent) => agent.name === role.name && agent.workspace_id === workspaceId,
      );
      if (matches.length !== 1 || !matches[0]?.pane_id || !paneIds.has(matches[0].pane_id)) {
        throw new SlpRuntimeError(`role is not uniquely attached: ${role.name}`, ["agent", "list"]);
      }
      const identity = reused.get(role.role);
      if (!identity && !settled(matches[0])) {
        throw new SlpRuntimeError(`role is not ready: ${role.name}`, ["agent", "list"]);
      }
      const paneId = matches[0].pane_id;
      if (!foreground(await this.processInfo(paneId))) {
        throw new SlpRuntimeError(`role process is not ready: ${role.name}`, [
          "pane",
          "process-info",
          "--pane",
          paneId,
        ]);
      }
      const contract = contracts.get(role.role);
      if (!contract) {
        throw new SlpRuntimeError(`missing role contract: ${role.role}`, ["agent", "read", role.name]);
      }
      const source = identity ?? contract;
      roles.push({
        briefDigest: source.briefDigest,
        instanceId: source.instanceId,
        name: role.name,
        packDigest: source.packDigest,
        paneId,
        profile: role.profile,
        readyChallenge: source.readyChallenge,
        role: role.role,
        workspaceId,
      });
    }
    return roles;
  }

  async ensurePeer(
    plan: SlpTeamPlan,
    peer: SlpRolePlan,
    contract: SlpRoleContract,
    acknowledged: SlpAcknowledgedRole | null = null,
  ): Promise<SlpRuntimePeer> {
    const startedAt = Date.now();
    this.requireRenderedProfile(peer);
    const workspaceId = await this.findWorkspace(plan);
    const globalMatches = (await this.agents()).filter((agent) => agent.name === peer.name);
    const workspaceMatches = globalMatches.filter((agent) => agent.workspace_id === workspaceId);
    if (globalMatches.length > workspaceMatches.length || workspaceMatches.length > 1) {
      throw new SlpRuntimeError(
        `peer identity is duplicated outside the generation: ${peer.name}`,
        ["agent", "list"],
      );
    }
    let paneId = workspaceMatches[0]?.pane_id ?? null;
    let createdTabId: string | null = null;
    let startedPaneId: string | null = null;
    if (paneId && acknowledged && acknowledged.paneId === paneId) {
      this.note(`${peer.name}: already acknowledged in ${paneId}; left alone`);
      return {
        createdTabId: null,
        role: {
          briefDigest: acknowledged.briefDigest,
          instanceId: acknowledged.instanceId,
          name: peer.name,
          packDigest: acknowledged.packDigest,
          paneId,
          profile: peer.profile,
          readyChallenge: acknowledged.readyChallenge,
          role: "peer",
          workspaceId,
        },
        startedPaneId: null,
      };
    }
    try {
      if (!paneId) {
        const matchingTabs = (await this.tabs(workspaceId)).filter(
          (tab) => tab.label === peer.label,
        );
        if (matchingTabs.length > 1) {
          throw new SlpRuntimeError(`peer tab is duplicated: ${peer.label}`, ["tab", "list"]);
        }
        const matchingTab = matchingTabs[0];
        paneId = matchingTab?.root_pane_id ?? null;
        if (paneId) startedPaneId = paneId;
        if (!paneId) {
          const created = await this.client.tabCreate({
            cwd: plan.projectPath,
            label: peer.label,
            workspace_id: workspaceId,
          });
          createdTabId = created.tab?.tab_id ?? null;
          paneId = created.root_pane?.pane_id ?? null;
        }
        if (!paneId) {
          throw new SlpRuntimeError(`peer pane was not created: ${peer.name}`, ["tab", "create"]);
        }
        this.note(`${peer.name}: starting ${peer.kind} pane in ${plan.workspaceLabel}`);
        await this.startAgent(plan, peer, paneId);
      }
      this.note(
        `${peer.name}: waiting for acknowledgement (up to ${Math.round(acknowledgementWindowMs / 1000)}s)`,
      );
      await this.promptAgent(plan, peer.name, contract.body);
      await this.requireAcknowledgement(peer.name, contract);
      this.note(`${peer.name}: ready in ${Math.round((Date.now() - startedAt) / 1000)}s`);
      const matches = (await this.agents()).filter(
        (agent) => agent.name === peer.name && agent.workspace_id === workspaceId,
      );
      if (matches.length !== 1 || !matches[0]?.pane_id) {
        throw new SlpRuntimeError(`peer is not uniquely attached: ${peer.name}`, ["agent", "list"]);
      }
      if (!settled(matches[0])) {
        throw new SlpRuntimeError(`peer is not ready: ${peer.name}`, ["agent", "list"]);
      }
      paneId = matches[0].pane_id;
      if (!foreground(await this.processInfo(paneId))) {
        throw new SlpRuntimeError(`peer process is not ready: ${peer.name}`, [
          "pane",
          "process-info",
          "--pane",
          paneId,
        ]);
      }
      return {
        createdTabId,
        role: {
          briefDigest: contract.briefDigest,
          instanceId: contract.instanceId,
          name: peer.name,
          packDigest: contract.packDigest,
          paneId,
          profile: peer.profile,
          readyChallenge: contract.readyChallenge,
          role: "peer",
          workspaceId,
        },
        startedPaneId,
      };
    } catch (error) {
      if (startedPaneId) await this.closeStartedPane(plan, startedPaneId);
      else if (createdTabId) await this.closeCreatedTab(plan, createdTabId);
      throw error;
    }
  }

  async closeCreatedTab(_plan: SlpTeamPlan, tabId: string): Promise<void> {
    await this.client.tabClose(tabId);
  }

  async closeStartedPane(_plan: SlpTeamPlan, paneId: string): Promise<void> {
    await this.client.paneClose(paneId);
  }

  async notify(_projectPath: string, target: string, line: string): Promise<void> {
    try {
      await this.client.agentPrompt(target, line);
    } catch (error) {
      if (!(error instanceof SlpRuntimeError)) throw error;
      throw new SlpRuntimeError(
        `agent did not accept the notice: ${target}; ${error.message}`,
        ["agent", "prompt", target],
        error.stderr,
        { code: error.code === "SLP_RUNTIME" ? undefined : error.code, herdrCode: error.herdrCode ?? undefined },
      );
    }
  }

  async delegateStop(
    plan: SlpTeamPlan,
    roomPath: string,
    token: string,
    cliEntry: string,
  ): Promise<void> {
    const sharedHub = (await this.workspaces()).filter(
      (workspace) =>
        workspace.label === "maestro" &&
        typeof workspace.cwd === "string" &&
        resolve(workspace.cwd) === resolve(roomPath),
    );
    if (sharedHub.length > 1) {
      throw new SlpRuntimeError(
        `expected at most one Hub workspace; found ${sharedHub.length}`,
        ["workspace", "list"],
      );
    }

    const helperLabel = `slp-stop:${plan.teamId}:g${plan.generation}`;
    const ephemeralLabel = `maestro-${helperLabel}`;
    let closeWorkspace = false;
    let workspaceId = sharedHub[0]?.workspace_id ?? null;
    if (!workspaceId) {
      const existing = (await this.workspaces()).filter(
        (workspace) => workspace.label === ephemeralLabel,
      );
      if (existing.length > 1) {
        throw new SlpRuntimeError(
          `expected at most one stop helper workspace ${ephemeralLabel}; found ${existing.length}`,
          ["workspace", "list"],
        );
      }
      workspaceId = existing[0]?.workspace_id ?? null;
      closeWorkspace = true;
      if (!workspaceId) {
        const created = await this.client.workspaceCreate({ cwd: roomPath, label: ephemeralLabel });
        workspaceId = created.workspace?.workspace_id ?? null;
      }
    }
    if (!workspaceId) {
      throw new SlpRuntimeError("stop helper workspace was not created", ["workspace", "create"]);
    }

    if (!closeWorkspace) {
      for (const stale of (await this.tabs(workspaceId)).filter(
        (tab) => tab.label === helperLabel,
      )) {
        if (stale.tab_id) await this.client.tabClose(stale.tab_id);
      }
    }

    let helperTabId: string | null = null;
    try {
      const created = await this.client.tabCreate({
        cwd: roomPath,
        label: helperLabel,
        workspace_id: workspaceId,
      });
      helperTabId = created.tab?.tab_id ?? null;
      const helperPaneId = created.root_pane?.pane_id ?? null;
      if (!helperTabId || !helperPaneId) {
        throw new SlpRuntimeError("stop helper pane was not created", ["tab", "create"]);
      }
      await this.client.paneSendInput(
        helperPaneId,
        [
          "/usr/bin/env",
          `${slpStopEnvironment.token}=${token}`,
          `${slpStopEnvironment.project}=${plan.projectPath}`,
          `${slpStopEnvironment.helperTab}=${helperTabId}`,
          `${slpStopEnvironment.helperWorkspace}=${workspaceId}`,
          `${slpStopEnvironment.closeWorkspace}=${closeWorkspace ? "1" : "0"}`,
          process.execPath,
          cliEntry,
          "team",
          "stop",
          plan.teamId,
          "--json",
        ].join(" "),
      );
    } catch (error) {
      try {
        if (closeWorkspace) await this.client.workspaceClose(workspaceId);
        else if (helperTabId) await this.client.tabClose(helperTabId);
      } catch {}
      throw error;
    }
  }

  async closeStopHelper(
    _roomPath: string,
    helperTabId: string,
    helperWorkspaceId: string,
    closeWorkspace: boolean,
  ): Promise<void> {
    if (closeWorkspace) await this.client.workspaceClose(helperWorkspaceId);
    else await this.client.tabClose(helperTabId);
  }

  async paneAlive(paneId: string): Promise<boolean> {
    try {
      return foreground(await this.processInfo(paneId));
    } catch {
      return false;
    }
  }

  async inspect(
    plan: SlpTeamPlan,
    expectedRoles: readonly SlpRuntimeRole[],
    runtimePaneId = "",
  ): Promise<SlpRuntimeInspection> {
    const workspaces = (await this.workspaces()).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    if (workspaces.length === 0) {
      return {
        missingPanes: expectedRoles.map((role) => role.name).sort(),
        runtime: "available",
        runtimePane: false,
        workspace: false,
      };
    }
    if (workspaces.length !== 1 || !workspaces[0]?.workspace_id) {
      throw new SlpRuntimeError(
        `expected exactly one workspace ${plan.workspaceLabel}; found ${workspaces.length}`,
        ["workspace", "list"],
      );
    }
    const workspaceId = workspaces[0].workspace_id;
    const agents = await this.agents();
    const panes = await this.panes(workspaceId);
    const paneIds = new Set(panes.flatMap((pane) => pane.pane_id ? [pane.pane_id] : []));
    const missingPanes: string[] = [];
    for (const expected of expectedRoles) {
      const agent = agents.find(
        (candidate) =>
          candidate.name === expected.name && candidate.workspace_id === workspaceId,
      );
      const paneId = agent?.pane_id;
      if (!paneId || !paneIds.has(paneId)) {
        missingPanes.push(expected.name);
        continue;
      }
      if (!foreground(await this.processInfo(paneId))) missingPanes.push(expected.name);
    }
    const runtimePane = runtimePaneId !== "" && paneIds.has(runtimePaneId) &&
      foreground(await this.processInfo(runtimePaneId));
    return {
      missingPanes: missingPanes.sort(),
      runtime: "available",
      runtimePane,
      workspace: true,
    };
  }

  async stop(
    plan: SlpTeamPlan,
    expectedRoles: readonly SlpRuntimeRole[],
  ): Promise<void> {
    const matchingWorkspaces = (await this.workspaces()).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    const runtimeDirectory = slpRuntimeDirectory(
      plan.projectPath,
      plan.teamId,
      plan.generation,
    );
    if (matchingWorkspaces.length === 0) {
      await rm(runtimeDirectory, { force: true, recursive: true });
      return;
    }
    if (matchingWorkspaces.length !== 1 || !matchingWorkspaces[0]?.workspace_id) {
      throw new SlpRuntimeError(
        `expected exactly one workspace ${plan.workspaceLabel}; found ${matchingWorkspaces.length}`,
        ["workspace", "list"],
      );
    }
    const workspaceId = matchingWorkspaces[0].workspace_id;
    const tabs = await this.tabs(workspaceId);
    const panes = await this.panes(workspaceId);
    const closedTabs = new Set<string>();
    const closedPanes = new Set<string>();
    const closePane = async (paneId: string): Promise<void> => {
      if (closedPanes.has(paneId)) return;
      const pane = panes.find((candidate) => candidate.pane_id === paneId);
      const tab = tabs.find(
        (candidate) => candidate.root_pane_id === paneId || candidate.tab_id === pane?.tab_id,
      );
      if (tab?.tab_id) {
        if (closedTabs.has(tab.tab_id)) return;
        const hasOpenSibling = panes.some(
          (candidate) =>
            candidate.tab_id === tab.tab_id &&
            candidate.pane_id !== paneId &&
            candidate.pane_id !== undefined &&
            !closedPanes.has(candidate.pane_id),
        );
        if (hasOpenSibling) {
          await this.client.paneClose(paneId);
          closedPanes.add(paneId);
          return;
        }
        await this.client.tabClose(tab.tab_id);
        closedTabs.add(tab.tab_id);
        for (const candidate of panes) {
          if (candidate.tab_id === tab.tab_id && candidate.pane_id) {
            closedPanes.add(candidate.pane_id);
          }
        }
        return;
      }
      if (pane) {
        await this.client.paneClose(paneId);
        closedPanes.add(paneId);
      }
    };
    const closeRole = async (role: SlpRole): Promise<void> => {
      for (const current of expectedRoles
        .filter((candidate) => candidate.role === role)
        .sort((left, right) => left.name.localeCompare(right.name))) {
        await closePane(current.paneId);
      }
    };

    await closeRole("peer");
    await closeRole("lead");

    for (const pane of panes) {
      if (!pane.pane_id || expectedRoles.some((role) => role.paneId === pane.pane_id)) continue;
      await closePane(pane.pane_id);
    }

    await rm(runtimeDirectory, { force: true, recursive: true });
    if (existsSync(runtimeDirectory)) {
      throw new SlpRuntimeError(
        `SLP runtime transcript remains: ${runtimeDirectory}`,
        ["runtime", "cleanup"],
      );
    }

    const supervisorPanes = new Set(
      expectedRoles
        .filter((role) => role.role === "team-supervisor")
        .map((role) => role.paneId),
    );
    const preFinalRemainder = (await this.panes(workspaceId)).filter(
      (pane) => pane.pane_id && !supervisorPanes.has(pane.pane_id),
    );
    if (preFinalRemainder.length > 0) {
      throw new SlpRuntimeError(
        `non-Supervisor panes remain before final shutdown: ${preFinalRemainder.map((pane) => pane.pane_id).join(", ")}`,
        ["pane", "list", "--workspace", workspaceId],
      );
    }

    await closeRole("team-supervisor");
    await this.closeWorkspace(workspaceId, plan.workspaceLabel, "shutdown");
  }

  async rollback(
    plan: SlpTeamPlan,
    created: Pick<SlpRuntimeStart, "createdTabIds" | "createdWorkspace" | "startedPaneIds"> & {
      workspaceId: string | null;
    },
  ): Promise<void> {
    if (created.createdWorkspace && created.workspaceId) {
      await this.closeWorkspace(created.workspaceId, created.workspaceId, "rollback");
      return;
    }
    for (const paneId of [...created.startedPaneIds].reverse()) {
      await this.client.paneClose(paneId);
    }
    for (const tabId of [...created.createdTabIds].reverse()) {
      await this.client.tabClose(tabId);
    }
    if (created.workspaceId) {
      const remainingStartedPanes = (await this.panes(created.workspaceId)).filter(
        (pane) => pane.pane_id && created.startedPaneIds.includes(pane.pane_id),
      );
      if (remainingStartedPanes.length > 0) {
        throw new SlpRuntimeError(
          `started panes remain after rollback: ${remainingStartedPanes.map((pane) => pane.pane_id).join(", ")}`,
          ["pane", "close"],
        );
      }
      const remaining = (await this.tabs(created.workspaceId)).filter(
        (tab) => tab.tab_id && created.createdTabIds.includes(tab.tab_id),
      );
      if (remaining.length > 0) {
        throw new SlpRuntimeError(
          `created tabs remain after rollback: ${remaining.map((tab) => tab.tab_id).join(", ")}`,
          ["tab", "close"],
        );
      }
    }
  }
}
