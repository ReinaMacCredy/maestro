import { resolve } from "node:path";
import { SlpRuntimeError, type HerdrAgent, type HerdrProcessInfo } from "./herdr-client.ts";

// d755: the acknowledgement is polled inside a fixed window with non-scrolling
// reads; d756: a blocked pane is classified from its own text.
export const acknowledgementWindowMs = 30_000;

export const acknowledgementPollMs = 1_000;

export const acknowledgementQuietPolls = 4;

const paneTailLines = 15;

export const trustDialogPattern =
  /Do you trust the contents of this directory|Quick safety check: Is this a project you created or one you trust|Yes, I trust this folder/;

export type SlpRole = "lead" | "peer";

export interface SeatLaunch {
  autocompact?: number | null;
  harness: "claude" | "codex";
  // room d128: null leaves the seat file's own model to the harness.
  model?: string | null;
}

export interface SlpRolePlan {
  autocompact?: number | null;
  kind: "claude" | "codex";
  label: string;
  model?: string | null;
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
  // true when the pane was already acknowledged and left alone (Hub d101:
  // the only pane --fresh resets).
  reused: boolean;
  role: SlpRuntimeRole;
  startedPaneId: string | null;
}

// Hub d101: the harness's new-conversation slash command. Claude Code /clear
// is what the owner sends by hand; Codex /new is read off the installed codex
// 0.153.3 binary ("start a new chat during a conversation"), not yet run live.
export const newConversationCommands: Record<SeatLaunch["harness"], string> = {
  claude: "/clear",
  codex: "/new",
};

// A Claude Code /clear fires SessionStart, whose Herdr hook re-reports the
// pane's session, so a changed agent_session id is the proof the reset took
// (live 2026-09-05). Codex's Herdr hook also reports only on SessionStart and
// skips a thread id that differs from CODEX_THREAD_ID; whether /new passes
// that gate is unverified, so Codex falls back to the settled wait.
export const sessionProvenResets: Record<SeatLaunch["harness"], boolean> = {
  claude: true,
  codex: false,
};

export interface SlpRuntimeInspection {
  missingPanes: string[];
  runtime: "available";
  runtimePane: boolean;
  workspace: boolean;
}

export function cleanPaneLines(output: string): string[] {
  return output
    .replaceAll(/\u001b\[[0-?]*[ -/]*[@-~]/g, "")
    .split(/\r?\n/)
    .map(normalizeAcknowledgementLine);
}

export function paneTailOf(lines: readonly string[]): string[] {
  return lines.filter((line) => line !== "").slice(-paneTailLines);
}

export function foreground(value: HerdrProcessInfo): boolean {
  if (typeof value.foreground_process_group_id === "number") return true;
  const record = value as unknown as Record<string, unknown>;
  const processes = record.foreground_processes ?? record.processes;
  return Array.isArray(processes) && processes.length > 0;
}

export function settled(agent: HerdrAgent): boolean {
  return agent.agent_status === "idle" || agent.agent_status === "done";
}

function normalizeAcknowledgementLine(line: string): string {
  return line
    .trim()
    .replace(/^[^\p{L}\p{N}]+(?=SLP_ROLE_READY(?:\s|$))/u, "");
}

// d768: the two challenge halves arrive on separate contract lines, so a
// reply that joins them with one space still proves both were read. A harness
// also hard-wraps its own output at the pane width, inside a token when the
// pane is narrow ("chal" / "lenge=..."), so whitespace is ignored and every
// other byte stays exact.
export function includesExactAcknowledgement(
  lines: readonly string[],
  acknowledgement: string,
): boolean {
  const expected = acknowledgement.replaceAll(/\s+/g, "");
  for (let start = 0; start < lines.length; start += 1) {
    if (!/^SLP_ROLE_READY(?:\s|$)/.test(lines[start] ?? "")) continue;
    let candidate = "";
    for (let index = start; index < lines.length && candidate.length < expected.length; index += 1) {
      const line = lines[index];
      if (!line) break;
      candidate += line.replaceAll(/\s+/g, "");
    }
    if (candidate === expected) return true;
  }
  return false;
}

export function herdrErrorCode(error: unknown): string | null {
  return error instanceof SlpRuntimeError ? error.herdrCode : null;
}

// The runtime pane is created outside this plan (d97).
export function buildSlpTeamPlan(input: {
  generation: number;
  lead: SeatLaunch;
  projectPath: string;
  teamId: string;
}): SlpTeamPlan {
  const projectPath = resolve(input.projectPath);
  const prefix = `slp:${input.teamId}:g${input.generation}`;
  return {
    generation: input.generation,
    projectPath,
    roles: [
      {
        autocompact: input.lead.autocompact ?? null,
        kind: input.lead.harness,
        label: `${prefix}:lead`,
        model: input.lead.model ?? null,
        name: `lead-${input.teamId}`,
        profile: "lead",
        role: "lead",
      },
    ],
    teamId: input.teamId,
    workspaceLabel: `slp-${input.teamId}-g${input.generation}`,
  };
}

// F10, room d128: autocompact and the model ride the launch line, so the seat file and its pinned digest never change.
export function launchArguments(
  role: Pick<SlpRolePlan, "autocompact" | "kind" | "model" | "role">,
): string[] {
  const args = role.kind === "codex"
    ? ["--profile", `maestro-${role.role}`]
    : ["--agent", `maestro-${role.role}`];
  if (role.kind === "claude" && role.autocompact) args.push("--autocompact", String(role.autocompact));
  if (role.model) args.push("--model", role.model);
  return args;
}

export function startCommand(role: Pick<SlpRolePlan, "kind" | "name">, paneId: string, launch: string[]): string[] {
  return ["agent", "start", role.name, "--kind", role.kind, "--pane", paneId, "--timeout", "60000", "--", ...launch];
}
