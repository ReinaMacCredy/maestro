import { existsSync } from "node:fs";
import { rm } from "node:fs/promises";
import { resolve } from "node:path";
import { CliError } from "../kernel/cli.ts";
import { slpWatchRuntimeDirectory } from "./slp-watch.ts";

// d755: the acknowledgement is polled inside a fixed window with non-scrolling
// reads; d756: a blocked pane is classified from its own text.
const acknowledgementWindowMs = 30_000;
const acknowledgementPollMs = 1_000;
const acknowledgementQuietPolls = 4;
const paneTailLines = 15;
const trustDialogPattern =
  /Do you trust the contents of this directory|Quick safety check: Is this a project you created or one you trust|Yes, I trust this folder/;

export type SlpRole = "team-supervisor" | "lead" | "peer";

export interface SlpRolePlan {
  kind: "claude" | "codex";
  label: string;
  model: string;
  name: string;
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
  watch: boolean;
  workspace: boolean;
}

export const slpStopEnvironment = {
  closeWorkspace: "MAESTRO_SLP_STOP_CLOSE_WORKSPACE",
  helperTab: "MAESTRO_SLP_STOP_HELPER_TAB",
  helperWorkspace: "MAESTRO_SLP_STOP_HELPER_WORKSPACE",
  project: "MAESTRO_SLP_STOP_PROJECT",
  token: "MAESTRO_SLP_STOP_GRANT",
} as const;

interface WorkspaceRecord {
  cwd?: string;
  label?: string;
  workspace_id?: string;
}

interface PaneRecord {
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
  name?: string;
  pane_id?: string;
  workspace_id?: string;
}

interface SlpRuntimeEvidence {
  code?: string;
  directory?: string;
  harness?: string;
  paneTail?: readonly string[];
}

export class SlpRuntimeError extends CliError {
  constructor(
    message: string,
    readonly command: readonly string[],
    readonly stderr?: string,
    evidence: SlpRuntimeEvidence = {},
  ) {
    super(evidence.code ?? "SLP_RUNTIME", message, {
      command: [...command],
      ...(stderr ? { stderr } : {}),
      ...(evidence.harness ? { harness: evidence.harness } : {}),
      ...(evidence.directory ? { directory: evidence.directory } : {}),
      ...(evidence.paneTail ? { paneTail: [...evidence.paneTail] } : {}),
    });
  }
}

function cleanPaneLines(output: string): string[] {
  return output
    .replaceAll(/\u001b\[[0-?]*[ -/]*[@-~]/g, "")
    .split(/\r?\n/)
    .map(normalizeAcknowledgementLine);
}

function paneTailOf(lines: readonly string[]): string[] {
  return lines.filter((line) => line !== "").slice(-paneTailLines);
}

function resultOf(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object") return {};
  const result = (value as Record<string, unknown>).result;
  return result && typeof result === "object" ? result as Record<string, unknown> : {};
}

function records<T>(value: unknown, key: string): T[] {
  const result = resultOf(value);
  const nested = result[key];
  return Array.isArray(nested) ? nested as T[] : [];
}

function objectAt(value: Record<string, unknown>, key: string): Record<string, unknown> {
  const nested = value[key];
  return nested && typeof nested === "object" ? nested as Record<string, unknown> : {};
}

function stringAt(value: Record<string, unknown>, key: string): string | null {
  const nested = value[key];
  return typeof nested === "string" ? nested : null;
}

function accepted(value: Record<string, unknown>): boolean {
  return value.accepted !== false && value.delivered !== false;
}

function foreground(value: Record<string, unknown>): boolean {
  if (
    typeof value.foreground_process_group_id === "number" ||
    typeof value.foreground_pgid === "number"
  ) return true;
  return ["foreground_processes", "processes"].some(
    (key) => Array.isArray(value[key]) && value[key].length > 0,
  );
}

function settled(agent: AgentRecord): boolean {
  return agent.agent_status === "idle" || agent.agent_status === "done";
}

function normalizeAcknowledgementLine(line: string): string {
  return line
    .trim()
    .replace(/^[^\p{L}\p{N}]+(?=SLP_ROLE_READY(?:\s|$))/u, "");
}

function includesExactAcknowledgement(
  lines: readonly string[],
  acknowledgement: string,
): boolean {
  for (let start = 0; start < lines.length; start += 1) {
    if (!/^SLP_ROLE_READY(?:\s|$)/.test(lines[start] ?? "")) continue;
    let candidate = lines[start] ?? "";
    for (let index = start; candidate.length <= acknowledgement.length; index += 1) {
      if (candidate === acknowledgement) return true;
      const continuation = lines[index + 1];
      if (!continuation) break;
      candidate = `${candidate} ${continuation}`;
    }
  }
  return false;
}

function herdrErrorCode(error: unknown): string | null {
  if (!(error instanceof SlpRuntimeError) || !error.stderr) return null;
  try {
    const envelope = JSON.parse(error.stderr) as { error?: { code?: unknown } };
    return typeof envelope.error?.code === "string" ? envelope.error.code : null;
  } catch {
    return null;
  }
}

export function buildSlpTeamPlan(input: {
  generation: number;
  leadModel: string;
  projectPath: string;
  supervisorModel: string;
  teamId: string;
}): SlpTeamPlan {
  const projectPath = resolve(input.projectPath);
  const prefix = `slp:${input.teamId}:g${input.generation}`;
  return {
    generation: input.generation,
    projectPath,
    roles: [
      {
        kind: "claude",
        label: `${prefix}:team-supervisor`,
        model: input.supervisorModel,
        name: `supervisor-${input.teamId}`,
        role: "team-supervisor",
      },
      {
        kind: "codex",
        label: `${prefix}:lead`,
        model: input.leadModel,
        name: `lead-${input.teamId}`,
        role: "lead",
      },
    ],
    teamId: input.teamId,
    workspaceLabel: `slp-${input.teamId}-g${input.generation}`,
  };
}

export class HerdrSlpRuntime {
  constructor(
    private readonly commandTimeoutMs = 15_000,
    private readonly environment: Record<string, string | undefined> = process.env,
    private readonly agentReadyTimeoutMs = 5_000,
    private readonly promptReadyTimeoutMs = 30_000,
  ) {}

  private async command(
    args: string[],
    cwd: string,
    timeoutMs = this.commandTimeoutMs,
    allowEmpty = false,
  ): Promise<Record<string, unknown>> {
    let child: ReturnType<typeof Bun.spawn>;
    try {
      child = Bun.spawn(["herdr", ...args], {
        cwd,
        env: this.environment,
        stderr: "pipe",
        stdout: "pipe",
      });
    } catch (error) {
      throw new SlpRuntimeError(
        `cannot start Herdr: ${error instanceof Error ? error.message : String(error)}`,
        args,
      );
    }
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill(9);
    }, timeoutMs);
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(child.stdout as ReadableStream<Uint8Array>).text(),
      new Response(child.stderr as ReadableStream<Uint8Array>).text(),
      child.exited,
    ]).finally(() => clearTimeout(timer));
    const commandName = args.slice(0, 3).join(" ");
    if (timedOut) {
      throw new SlpRuntimeError(
        `Herdr command timed out after ${timeoutMs}ms: ${commandName}`,
        args,
        stderr.trim(),
      );
    }
    if (exitCode !== 0) {
      const diagnostic = stderr.trim();
      throw new SlpRuntimeError(
        `Herdr command failed (${exitCode}): ${commandName}${diagnostic ? `; ${diagnostic}` : ""}`,
        args,
        diagnostic,
      );
    }
    if (allowEmpty && stdout.trim() === "") return {};
    try {
      return JSON.parse(stdout) as Record<string, unknown>;
    } catch {
      throw new SlpRuntimeError(
        `Herdr returned invalid JSON for: ${commandName}`,
        args,
        stdout.trim(),
      );
    }
  }

  private async textCommand(
    args: string[],
    cwd: string,
    timeoutMs = this.commandTimeoutMs,
  ): Promise<string> {
    let child: ReturnType<typeof Bun.spawn>;
    try {
      child = Bun.spawn(["herdr", ...args], {
        cwd,
        env: this.environment,
        stderr: "pipe",
        stdout: "pipe",
      });
    } catch (error) {
      throw new SlpRuntimeError(
        `cannot start Herdr: ${error instanceof Error ? error.message : String(error)}`,
        args,
      );
    }
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill(9);
    }, timeoutMs);
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(child.stdout as ReadableStream<Uint8Array>).text(),
      new Response(child.stderr as ReadableStream<Uint8Array>).text(),
      child.exited,
    ]).finally(() => clearTimeout(timer));
    const commandName = args.slice(0, 3).join(" ");
    if (timedOut) {
      throw new SlpRuntimeError(
        `Herdr command timed out after ${timeoutMs}ms: ${commandName}`,
        args,
        stderr.trim(),
      );
    }
    if (exitCode !== 0) {
      const diagnostic = stderr.trim();
      throw new SlpRuntimeError(
        `Herdr command failed (${exitCode}): ${commandName}${diagnostic ? `; ${diagnostic}` : ""}`,
        args,
        diagnostic,
      );
    }
    return stdout;
  }

  private note(line: string): void {
    process.stderr.write(`${line}\n`);
  }

  private async readPane(
    plan: SlpTeamPlan,
    target: string,
    source: "visible" | "recent-unwrapped",
    lines: number,
  ): Promise<string[]> {
    return cleanPaneLines(
      await this.textCommand(
        ["agent", "read", target, "--source", source, "--lines", String(lines), "--format", "text"],
        plan.projectPath,
      ),
    );
  }

  private async paneTail(plan: SlpTeamPlan, target: string): Promise<string[]> {
    try {
      return paneTailOf(await this.readPane(plan, target, "visible", 40));
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
      lines = await this.readPane(plan, name, "visible", 40);
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

  private async settledAgent(plan: SlpTeamPlan, name: string): Promise<boolean> {
    const matches = (await this.agents(plan)).filter((agent) => agent.name === name);
    return matches.length === 1 && settled(matches[0] as AgentRecord);
  }

  private async requireAcknowledgement(
    plan: SlpTeamPlan,
    roleName: string,
    contract: SlpRoleContract,
  ): Promise<void> {
    const deadline = Date.now() + acknowledgementWindowMs;
    let previous: string | null = null;
    let quietPolls = 0;
    while (true) {
      const visible = await this.readPane(plan, roleName, "visible", 60);
      if (includesExactAcknowledgement(visible, contract.acknowledgement)) return;
      const snapshot = visible.join("\n");
      if (snapshot === previous) quietPolls += 1;
      else {
        previous = snapshot;
        quietPolls = 0;
      }
      const remaining = deadline - Date.now();
      const quiet =
        quietPolls >= acknowledgementQuietPolls && (await this.settledAgent(plan, roleName));
      if (remaining <= 0 || quiet) {
        const recent = await this.readPane(plan, roleName, "recent-unwrapped", 120);
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

  private async workspaces(plan: SlpTeamPlan): Promise<WorkspaceRecord[]> {
    return records<WorkspaceRecord>(
      await this.command(["workspace", "list"], plan.projectPath),
      "workspaces",
    );
  }

  private async tabs(plan: SlpTeamPlan, workspaceId: string): Promise<TabRecord[]> {
    return records<TabRecord>(
      await this.command(["tab", "list", "--workspace", workspaceId], plan.projectPath),
      "tabs",
    );
  }

  private async panes(plan: SlpTeamPlan, workspaceId: string): Promise<PaneRecord[]> {
    return records<PaneRecord>(
      await this.command(["pane", "list", "--workspace", workspaceId], plan.projectPath),
      "panes",
    );
  }

  private async agents(plan: SlpTeamPlan): Promise<AgentRecord[]> {
    return records<AgentRecord>(
      await this.command(["agent", "list"], plan.projectPath),
      "agents",
    );
  }

  private async startAgent(plan: SlpTeamPlan, args: string[]): Promise<Record<string, unknown>> {
    const deadline = Date.now() + this.agentReadyTimeoutMs;
    while (true) {
      try {
        return await this.command(args, plan.projectPath, Math.max(this.commandTimeoutMs, 75_000));
      } catch (error) {
        const errorCode = herdrErrorCode(error);
        if (errorCode === "agent_not_ready") {
          const name = args[2];
          const paneOption = args.indexOf("--pane");
          const paneId = paneOption >= 0 ? args[paneOption + 1] : undefined;
          const kindOption = args.indexOf("--kind");
          const harness = kindOption >= 0 ? args[kindOption + 1] ?? "agent" : "agent";
          const stderr = error instanceof SlpRuntimeError ? error.stderr : undefined;
          if (!name || !paneId) throw error;
          while (true) {
            const matches = (await this.agents(plan)).filter((agent) => agent.name === name);
            if (
              matches.length === 1 &&
              matches[0]?.pane_id === paneId &&
              settled(matches[0])
            ) {
              return { result: { accepted: true, name } };
            }
            if (
              matches.length === 1 &&
              matches[0]?.pane_id === paneId &&
              matches[0].agent_status === "blocked"
            ) {
              throw await this.blockedFailure(plan, name, harness, args, stderr);
            }
            const remaining = deadline - Date.now();
            if (remaining <= 0) {
              throw new SlpRuntimeError(
                `agent ${name} did not become ready within ${this.agentReadyTimeoutMs}ms`,
                args,
                stderr,
                { paneTail: await this.paneTail(plan, name) },
              );
            }
            await Bun.sleep(Math.min(100, remaining));
          }
        }
        if (errorCode !== "agent_pane_busy") throw error;
        const remaining = deadline - Date.now();
        if (remaining <= 0) {
          throw new SlpRuntimeError(
            `pane did not become an available shell within ${this.agentReadyTimeoutMs}ms`,
            args,
            error instanceof SlpRuntimeError ? error.stderr : undefined,
          );
        }
        await Bun.sleep(Math.min(100, remaining));
      }
    }
  }

  private async promptAgent(plan: SlpTeamPlan, args: string[]): Promise<Record<string, unknown>> {
    const deadline = Date.now() + this.promptReadyTimeoutMs;
    while (true) {
      try {
        return await this.command(args, plan.projectPath, 130_000);
      } catch (error) {
        const code = herdrErrorCode(error);
        const name = args[2] ?? "";
        const stderr = error instanceof SlpRuntimeError ? error.stderr : undefined;
        if (code === "agent_blocked") {
          const harness = plan.roles.find((role) => role.name === name)?.kind ?? "codex";
          throw await this.blockedFailure(plan, name, harness, args, stderr);
        }
        if (code !== "agent_prompt_stalled") throw error;
        const remaining = deadline - Date.now();
        if (remaining <= 0) {
          throw new SlpRuntimeError(
            `role contract prompt remained stalled for ${this.promptReadyTimeoutMs}ms`,
            args,
            stderr,
            { paneTail: await this.paneTail(plan, name) },
          );
        }
        await Bun.sleep(Math.min(100, remaining));
      }
    }
  }

  private async waitForWorkspaceAbsence(
    plan: SlpTeamPlan,
    workspaceId: string,
    label: string,
    action: "rollback" | "shutdown",
  ): Promise<void> {
    const deadline = Date.now() + 10_000;
    while (true) {
      const remaining = (await this.workspaces(plan)).filter(
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
    plan: SlpTeamPlan,
    workspaceId: string,
    label: string,
    action: "rollback" | "shutdown",
  ): Promise<void> {
    const present = (await this.workspaces(plan)).some(
      (workspace) => workspace.workspace_id === workspaceId,
    );
    if (!present) return;
    let closeError: unknown = null;
    try {
      await this.command(["workspace", "close", workspaceId], plan.projectPath);
    } catch (error) {
      closeError = error;
    }
    try {
      await this.waitForWorkspaceAbsence(plan, workspaceId, label, action);
    } catch (error) {
      throw closeError ?? error;
    }
  }

  private async processInfo(plan: SlpTeamPlan, paneId: string): Promise<Record<string, unknown>> {
    const result = resultOf(
      await this.command(["pane", "process-info", "--pane", paneId], plan.projectPath),
    );
    const nested = result.process_info;
    return nested && typeof nested === "object"
      ? nested as Record<string, unknown>
      : result;
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
    try {
      let matching = (await this.workspaces(plan)).filter(
        (workspace) => workspace.label === plan.workspaceLabel,
      );
      if (matching.length === 0) {
        const created = resultOf(
          await this.command([
            "workspace",
            "create",
            "--cwd",
            plan.projectPath,
            "--label",
            plan.workspaceLabel,
            "--no-focus",
          ], plan.projectPath),
        );
        workspaceId = stringAt(objectAt(created, "workspace"), "workspace_id");
        createdWorkspace = workspaceId !== null;
        matching = (await this.workspaces(plan)).filter(
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
        const globalMatches = (await this.agents(plan)).filter(
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
          const matchingTabs = (await this.tabs(plan, workspaceId)).filter(
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
            const created = resultOf(
              await this.command([
                "tab",
                "create",
                "--workspace",
                workspaceId,
                "--cwd",
                plan.projectPath,
                "--label",
                role.label,
                "--no-focus",
              ], plan.projectPath),
            );
            const tab = objectAt(created, "tab");
            const rootPane = objectAt(created, "root_pane");
            const tabId = stringAt(tab, "tab_id");
            paneId = stringAt(rootPane, "pane_id");
            if (tabId) createdTabIds.push(tabId);
          }
          if (!paneId) {
            throw new SlpRuntimeError(`role pane was not created: ${role.name}`, ["tab", "create"]);
          }
          this.note(`${role.name}: starting ${role.kind} pane in ${plan.workspaceLabel}`);
          const args = [
            "agent",
            "start",
            role.name,
            "--kind",
            role.kind,
            "--pane",
            paneId,
            "--timeout",
            "60000",
          ];
          if (role.model !== "default") args.push("--", "--model", role.model);
          const started = resultOf(await this.startAgent(plan, args));
          if (!accepted(started)) {
            throw new SlpRuntimeError(`role did not start: ${role.name}`, args);
          }
        }
        this.note(
          `${role.name}: waiting for acknowledgement (up to ${Math.round(acknowledgementWindowMs / 1000)}s)`,
        );
        const prompted = resultOf(
          await this.promptAgent(plan, [
            "agent",
            "prompt",
            role.name,
            contract.body,
            "--wait",
            "--timeout",
            "120000",
          ]),
        );
        if (!accepted(prompted)) {
          throw new SlpRuntimeError(`role contract was not delivered: ${role.name}`, [
            "agent",
            "prompt",
            role.name,
          ]);
        }
        await this.requireAcknowledgement(plan, role.name, contract);
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
    const panes = await this.panes(plan, workspaceId);
    const paneIds = new Set(panes.flatMap((pane) => pane.pane_id ? [pane.pane_id] : []));
    const agents = await this.agents(plan);
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
      if (!foreground(await this.processInfo(plan, paneId))) {
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
    const matchingWorkspaces = (await this.workspaces(plan)).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    if (matchingWorkspaces.length !== 1 || !matchingWorkspaces[0]?.workspace_id) {
      throw new SlpRuntimeError(
        `expected exactly one workspace ${plan.workspaceLabel}; found ${matchingWorkspaces.length}`,
        ["workspace", "list"],
      );
    }
    const workspaceId = matchingWorkspaces[0].workspace_id;
    const globalMatches = (await this.agents(plan)).filter((agent) => agent.name === peer.name);
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
          readyChallenge: acknowledged.readyChallenge,
          role: "peer",
          workspaceId,
        },
        startedPaneId: null,
      };
    }
    try {
      if (!paneId) {
        const matchingTabs = (await this.tabs(plan, workspaceId)).filter(
          (tab) => tab.label === peer.label,
        );
        if (matchingTabs.length > 1) {
          throw new SlpRuntimeError(`peer tab is duplicated: ${peer.label}`, ["tab", "list"]);
        }
        const matchingTab = matchingTabs[0];
        paneId = matchingTab?.root_pane_id ?? null;
        if (paneId) startedPaneId = paneId;
        if (!paneId) {
          const created = resultOf(
            await this.command([
              "tab",
              "create",
              "--workspace",
              workspaceId,
              "--cwd",
              plan.projectPath,
              "--label",
              peer.label,
              "--no-focus",
            ], plan.projectPath),
          );
          createdTabId = stringAt(objectAt(created, "tab"), "tab_id");
          paneId = stringAt(objectAt(created, "root_pane"), "pane_id");
        }
        if (!paneId) {
          throw new SlpRuntimeError(`peer pane was not created: ${peer.name}`, ["tab", "create"]);
        }
        this.note(`${peer.name}: starting ${peer.kind} pane in ${plan.workspaceLabel}`);
        const args = [
          "agent",
          "start",
          peer.name,
          "--kind",
          peer.kind,
          "--pane",
          paneId,
          "--timeout",
          "60000",
        ];
        if (peer.model !== "default") args.push("--", "--model", peer.model);
        const started = resultOf(await this.startAgent(plan, args));
        if (!accepted(started)) {
          throw new SlpRuntimeError(`peer did not start: ${peer.name}`, args);
        }
      }
      this.note(
        `${peer.name}: waiting for acknowledgement (up to ${Math.round(acknowledgementWindowMs / 1000)}s)`,
      );
      const prompted = resultOf(
        await this.promptAgent(plan, [
          "agent",
          "prompt",
          peer.name,
          contract.body,
          "--wait",
          "--timeout",
          "120000",
        ]),
      );
      if (!accepted(prompted)) {
        throw new SlpRuntimeError(`peer contract was not delivered: ${peer.name}`, [
          "agent",
          "prompt",
          peer.name,
        ]);
      }
      await this.requireAcknowledgement(plan, peer.name, contract);
      this.note(`${peer.name}: ready in ${Math.round((Date.now() - startedAt) / 1000)}s`);
      const matches = (await this.agents(plan)).filter(
        (agent) => agent.name === peer.name && agent.workspace_id === workspaceId,
      );
      if (matches.length !== 1 || !matches[0]?.pane_id) {
        throw new SlpRuntimeError(`peer is not uniquely attached: ${peer.name}`, ["agent", "list"]);
      }
      if (!settled(matches[0])) {
        throw new SlpRuntimeError(`peer is not ready: ${peer.name}`, ["agent", "list"]);
      }
      paneId = matches[0].pane_id;
      if (!foreground(await this.processInfo(plan, paneId))) {
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

  async closeCreatedTab(plan: SlpTeamPlan, tabId: string): Promise<void> {
    await this.command(["tab", "close", tabId], plan.projectPath);
  }

  async closeStartedPane(plan: SlpTeamPlan, paneId: string): Promise<void> {
    await this.command(["pane", "close", paneId], plan.projectPath);
  }

  async delegateStop(
    plan: SlpTeamPlan,
    roomPath: string,
    token: string,
    cliEntry: string,
  ): Promise<void> {
    const helperPlan = { ...plan, projectPath: roomPath };
    const sharedHub = (await this.workspaces(helperPlan)).filter(
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
      const existing = (await this.workspaces(helperPlan)).filter(
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
        const created = resultOf(
          await this.command([
            "workspace",
            "create",
            "--cwd",
            roomPath,
            "--label",
            ephemeralLabel,
            "--no-focus",
          ], roomPath),
        );
        workspaceId = stringAt(objectAt(created, "workspace"), "workspace_id");
      }
    }
    if (!workspaceId) {
      throw new SlpRuntimeError("stop helper workspace was not created", ["workspace", "create"]);
    }

    if (!closeWorkspace) {
      for (const stale of (await this.tabs(helperPlan, workspaceId)).filter(
        (tab) => tab.label === helperLabel,
      )) {
        if (stale.tab_id) await this.command(["tab", "close", stale.tab_id], roomPath);
      }
    }

    let helperTabId: string | null = null;
    try {
      const created = resultOf(
        await this.command([
          "tab",
          "create",
          "--workspace",
          workspaceId,
          "--cwd",
          roomPath,
          "--label",
          helperLabel,
          "--no-focus",
        ], roomPath),
      );
      helperTabId = stringAt(objectAt(created, "tab"), "tab_id");
      const helperPaneId = stringAt(objectAt(created, "root_pane"), "pane_id");
      if (!helperTabId || !helperPaneId) {
        throw new SlpRuntimeError("stop helper pane was not created", ["tab", "create"]);
      }
      const launched = resultOf(
        await this.command(
          [
            "pane",
            "run",
            helperPaneId,
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
          ],
          roomPath,
          this.commandTimeoutMs,
          true,
        ),
      );
      if (!accepted(launched)) {
        throw new SlpRuntimeError("stop helper command was not accepted", ["pane", "run"]);
      }
    } catch (error) {
      try {
        if (closeWorkspace) await this.command(["workspace", "close", workspaceId], roomPath);
        else if (helperTabId) await this.command(["tab", "close", helperTabId], roomPath);
      } catch {}
      throw error;
    }
  }

  async closeStopHelper(
    roomPath: string,
    helperTabId: string,
    helperWorkspaceId: string,
    closeWorkspace: boolean,
  ): Promise<void> {
    await this.command(
      closeWorkspace
        ? ["workspace", "close", helperWorkspaceId]
        : ["tab", "close", helperTabId],
      roomPath,
    );
  }

  async inspect(
    plan: SlpTeamPlan,
    expectedRoles: readonly SlpRuntimeRole[],
  ): Promise<SlpRuntimeInspection> {
    const workspaces = (await this.workspaces(plan)).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    if (workspaces.length === 0) {
      return {
        missingPanes: expectedRoles.map((role) => role.name).sort(),
        runtime: "available",
        watch: false,
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
    const agents = await this.agents(plan);
    const panes = await this.panes(plan, workspaceId);
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
      if (!foreground(await this.processInfo(plan, paneId))) missingPanes.push(expected.name);
    }
    let watch = false;
    for (const pane of panes) {
      if (!pane.pane_id || expectedRoles.some((role) => role.paneId === pane.pane_id)) continue;
      const info = await this.processInfo(plan, pane.pane_id);
      const text = JSON.stringify(info).toLowerCase();
      if (
        foreground(info) &&
        text.includes("maestro-slp-watch") &&
        text.includes(plan.teamId.toLowerCase()) &&
        text.includes(String(plan.generation))
      ) {
        watch = true;
        break;
      }
    }
    return { missingPanes: missingPanes.sort(), runtime: "available", watch, workspace: true };
  }

  async stop(
    plan: SlpTeamPlan,
    expectedRoles: readonly SlpRuntimeRole[],
  ): Promise<void> {
    const matchingWorkspaces = (await this.workspaces(plan)).filter(
      (workspace) => workspace.label === plan.workspaceLabel,
    );
    const runtimeDirectory = slpWatchRuntimeDirectory(
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
    const tabs = await this.tabs(plan, workspaceId);
    const panes = await this.panes(plan, workspaceId);
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
          await this.command(["pane", "close", paneId], plan.projectPath);
          closedPanes.add(paneId);
          return;
        }
        await this.command(["tab", "close", tab.tab_id], plan.projectPath);
        closedTabs.add(tab.tab_id);
        for (const candidate of panes) {
          if (candidate.tab_id === tab.tab_id && candidate.pane_id) {
            closedPanes.add(candidate.pane_id);
          }
        }
        return;
      }
      if (pane) {
        await this.command(["pane", "close", paneId], plan.projectPath);
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
    const preFinalRemainder = (await this.panes(plan, workspaceId)).filter(
      (pane) => pane.pane_id && !supervisorPanes.has(pane.pane_id),
    );
    if (preFinalRemainder.length > 0) {
      throw new SlpRuntimeError(
        `non-Supervisor panes remain before final shutdown: ${preFinalRemainder.map((pane) => pane.pane_id).join(", ")}`,
        ["pane", "list", "--workspace", workspaceId],
      );
    }

    await closeRole("team-supervisor");
    await this.closeWorkspace(plan, workspaceId, plan.workspaceLabel, "shutdown");
  }

  async rollback(
    plan: SlpTeamPlan,
    created: Pick<SlpRuntimeStart, "createdTabIds" | "createdWorkspace" | "startedPaneIds"> & {
      workspaceId: string | null;
    },
  ): Promise<void> {
    if (created.createdWorkspace && created.workspaceId) {
      await this.closeWorkspace(plan, created.workspaceId, created.workspaceId, "rollback");
      return;
    }
    for (const paneId of [...created.startedPaneIds].reverse()) {
      await this.command(["pane", "close", paneId], plan.projectPath);
    }
    for (const tabId of [...created.createdTabIds].reverse()) {
      await this.command(["tab", "close", tabId], plan.projectPath);
    }
    if (created.workspaceId) {
      const remainingStartedPanes = (await this.panes(plan, created.workspaceId)).filter(
        (pane) => pane.pane_id && created.startedPaneIds.includes(pane.pane_id),
      );
      if (remainingStartedPanes.length > 0) {
        throw new SlpRuntimeError(
          `started panes remain after rollback: ${remainingStartedPanes.map((pane) => pane.pane_id).join(", ")}`,
          ["pane", "close"],
        );
      }
      const remaining = (await this.tabs(plan, created.workspaceId)).filter(
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
