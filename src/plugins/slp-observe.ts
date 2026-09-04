import { existsSync } from "node:fs";
import { mkdir, readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { Store, resolveStoreLocation } from "../kernel/store.ts";
import { acquireProcessLock, runSlpProcess } from "./slp-process.ts";
import { slpWatchRuntimeDirectory } from "./slp-watch.ts";

// d762/d765/d767: the sentinel is a foreground reader beside the Watch Pane.
// Every poll it lists agents; a role pane that just turned blocked gets the
// Observer a packet at once, otherwise one packet per tick. It judges
// nothing and touches no store: the Observer reads, the store stays the truth.
export const sentinelTickMs = 300_000;
export const sentinelPollMs = 5_000;
export const sentinelSilenceMs = 15 * 60_000;
export const sentinelRepeatThreshold = 3;
// team start launches the shim before finalization writes the local team row.
export const sentinelStartGraceMs = 10 * 60_000;
const tailLines = 30;

interface ObserveRole {
  name: string;
  pane_id: string;
  role: string;
}

interface ObserveWork {
  assigned_to: string;
  created_by: string;
  id: string;
  owner: string | null;
  return_revision: number;
  state: string;
  updated_at: string;
}

interface ObserveEntry {
  actor: string;
  body: string;
  created_at: string;
  flag: string | null;
  kind: string;
}

interface AgentRecord {
  agent_status?: string;
  name?: string;
  workspace_id?: string;
}

interface PaneMemory {
  changedAt: number;
  text: string;
}

export interface SlpObserveConfig {
  env?: Record<string, string | undefined>;
  generation: number;
  pollMs?: number;
  projectPath: string;
  teamId: string;
  tickMs?: number;
  workspaceId: string;
}

interface StoreSnapshot {
  observer: string;
  roles: ObserveRole[];
  work: Array<{
    latest: ObserveEntry | null;
    row: ObserveWork;
    stallNotes: number;
  }>;
}

// "pending" while the local team row does not exist yet; null once it exists
// but is not RUNNING for this workspace, or once the store itself is gone
// (a removed repo or test fixture), which ends the loop.
function readStore(config: SlpObserveConfig): StoreSnapshot | "pending" | null {
  const storePath = resolveStoreLocation(config.projectPath).path;
  if (!existsSync(storePath)) return null;
  const store = new Store(storePath, { readonly: true });
  try {
    const team = store.database
      .query<{ state: string; workspace_id: string }, [string, number]>(
        `SELECT state, workspace_id FROM slp_local_teams
         WHERE team_id = ? AND generation = ?`,
      )
      .get(config.teamId, config.generation);
    if (!team) return "pending";
    if (team.state !== "RUNNING" || team.workspace_id !== config.workspaceId) return null;
    const roles = store.database
      .query<ObserveRole, [string, number]>(
        `SELECT name, pane_id, role FROM slp_local_roles
         WHERE team_id = ? AND generation = ?
         ORDER BY CASE role WHEN 'team-supervisor' THEN 0 WHEN 'lead' THEN 1 ELSE 2 END, name`,
      )
      .all(config.teamId, config.generation);
    const observer = roles.find((role) => role.role === "observer")?.name ?? "";
    const latest = store.database.query<ObserveEntry, [string]>(
      `SELECT kind, actor, body, flag, created_at FROM slp_work_entries
       WHERE work_id = ? ORDER BY id DESC LIMIT 1`,
    );
    const stalls = store.database.query<{ count: number }, [string]>(
      `SELECT COUNT(*) AS count FROM slp_work_entries
       WHERE work_id = ? AND flag LIKE 'stall:%'`,
    );
    const work = store.database
      .query<ObserveWork, [string, number]>(
        `SELECT id, state, created_by, assigned_to, owner, return_revision, updated_at
         FROM slp_work WHERE team_id = ? AND generation = ? AND state <> 'DONE'
         ORDER BY created_at, id`,
      )
      .all(config.teamId, config.generation)
      .map((row) => ({
        latest: latest.get(row.id) ?? null,
        row,
        stallNotes: stalls.get(row.id)?.count ?? 0,
      }));
    return { observer, roles: roles.filter((role) => role.role !== "observer"), work };
  } finally {
    store.close();
  }
}

function ago(iso: string | number, now: number): string {
  const then = typeof iso === "number" ? iso : Date.parse(iso);
  const minutes = Math.max(0, Math.round((now - then) / 60_000));
  return minutes < 60 ? `${minutes}m` : `${Math.floor(minutes / 60)}h${minutes % 60}m`;
}

function clip(text: string, limit: number): string {
  const line = text.split("\n").map((part) => part.trim()).find((part) => part !== "") ?? "";
  return line.length > limit ? `${line.slice(0, limit - 3)}...` : line;
}

function repeatedLine(lines: string[]): { count: number; line: string } | null {
  const counts = new Map<string, number>();
  for (const raw of lines) {
    const line = raw.trim();
    if (line.length < 8) continue;
    counts.set(line, (counts.get(line) ?? 0) + 1);
  }
  let best: { count: number; line: string } | null = null;
  for (const [line, count] of counts) {
    if (count >= sentinelRepeatThreshold && (!best || count > best.count)) best = { count, line };
  }
  return best;
}

export async function renderPacket(
  config: SlpObserveConfig,
  snapshot: StoreSnapshot,
  agents: AgentRecord[],
  memory: Map<string, PaneMemory>,
  reason: string,
  now: number,
): Promise<string> {
  const status = new Map(
    agents
      .filter((agent) => agent.workspace_id === config.workspaceId && typeof agent.name === "string")
      .map((agent) => [agent.name as string, agent.agent_status ?? "unknown"]),
  );
  const lines = [
    `[SLP sentinel ${config.teamId} g${config.generation}] ${reason} at ${new Date(now).toISOString()}`,
    `thresholds: silence ${Math.round(sentinelSilenceMs / 60_000)}m, repeat ${sentinelRepeatThreshold}x`,
    "work:",
  ];
  if (snapshot.work.length === 0) lines.push("  none open");
  for (const item of snapshot.work) {
    const row = item.row;
    const held = row.owner ? `held by ${row.owner}` : "not held";
    const last = item.latest
      ? `${item.latest.kind.toLowerCase()}${item.latest.flag ? ` [${item.latest.flag}]` : ""} by ${item.latest.actor} ${ago(item.latest.created_at, now)} ago: ${clip(item.latest.body, 120)}`
      : "none";
    lines.push(
      `  ${row.id} ${row.state} ${row.created_by} -> ${row.assigned_to}; ${held}; unchanged ${ago(row.updated_at, now)}; revision ${row.return_revision}; last entry: ${last}; stall notes: ${item.stallNotes}`,
    );
  }
  lines.push("panes:");
  const tails: string[] = [];
  for (const role of snapshot.roles) {
    const state = status.get(role.name) ?? "absent";
    let tail: string[] = [];
    try {
      const output = await runSlpProcess(
        ["herdr", "agent", "read", role.name, "--source", "recent-unwrapped", "--lines", String(tailLines), "--format", "text"],
        config.projectPath,
        config.env ?? {},
      );
      tail = output.trimEnd().split("\n");
    } catch (error) {
      tail = [`[unreadable: ${error instanceof Error ? error.message : String(error)}]`];
    }
    const text = tail.join("\n");
    const remembered = memory.get(role.name);
    const changedAt = remembered && remembered.text === text ? remembered.changedAt : now;
    memory.set(role.name, { changedAt, text });
    const repeat = repeatedLine(tail);
    lines.push(
      `  ${role.name} [${role.role}] ${state}; unchanged ${ago(changedAt, now)}; repeats: ${
        repeat ? `${repeat.count}x "${clip(repeat.line, 80)}"` : "none"
      }`,
    );
    tails.push(`--- ${role.name} tail ---`, ...tail);
  }
  lines.push(...tails);
  lines.push(
    'reply: maestro work note <id> "<evidence>" --stall repeat|silence|dialog for a stalled item, or: observed: nothing stalled',
  );
  return lines.join("\n");
}

export async function runSlpObserve(config: SlpObserveConfig): Promise<number> {
  const runtimeDirectory = slpWatchRuntimeDirectory(
    config.projectPath,
    config.teamId,
    config.generation,
  );
  const lockPath = join(runtimeDirectory, "observe.lock");
  await mkdir(runtimeDirectory, { recursive: true });
  await acquireProcessLock(lockPath, "Sentinel");
  let stopping = false;
  const stop = () => {
    stopping = true;
  };
  process.on("SIGINT", stop);
  process.on("SIGTERM", stop);
  const tickMs = Math.max(25, config.tickMs ?? sentinelTickMs);
  const pollMs = Math.max(25, config.pollMs ?? sentinelPollMs);
  const memory = new Map<string, PaneMemory>();
  const startedAt = Date.now();
  let seen = false;
  let lastTick = 0;
  let blocked = new Set<string>();
  const poll = async (snapshot: StoreSnapshot) => {
    const now = Date.now();
    const raw = JSON.parse(
      await runSlpProcess(["herdr", "agent", "list"], config.projectPath, config.env ?? {}),
    ) as { result?: { agents?: unknown } };
    const agents = Array.isArray(raw.result?.agents) ? raw.result.agents as AgentRecord[] : [];
    const roleNames = new Set(snapshot.roles.map((role) => role.name));
    const nowBlocked = new Set(
      agents
        .filter((agent) =>
          agent.workspace_id === config.workspaceId &&
          typeof agent.name === "string" &&
          roleNames.has(agent.name) &&
          agent.agent_status === "blocked"
        )
        .map((agent) => agent.name as string),
    );
    const fresh = [...nowBlocked].filter((name) => !blocked.has(name));
    blocked = nowBlocked;
    const reason = fresh.length > 0
      ? `blocked: ${fresh.join(", ")}`
      : now - lastTick >= tickMs
      ? "tick"
      : null;
    if (reason) {
      const packet = await renderPacket(config, snapshot, agents, memory, reason, now);
      lastTick = now;
      if (snapshot.observer === "") {
        process.stderr.write("maestro-slp-observe: no observer role in this generation\n");
      } else {
        await runSlpProcess(
          ["herdr", "agent", "prompt", snapshot.observer, packet],
          config.projectPath,
          config.env ?? {},
        );
        process.stdout.write(`${new Date(now).toISOString()} packet (${reason}) -> ${snapshot.observer}\n`);
      }
    }
  };
  try {
    while (!stopping) {
      try {
        const snapshot = readStore(config);
        if (snapshot === "pending") {
          if (seen || Date.now() - startedAt >= sentinelStartGraceMs) return 0;
        } else if (snapshot === null) {
          return 0;
        } else {
          seen = true;
          await poll(snapshot);
        }
      } catch (error) {
        process.stderr.write(
          `maestro-slp-observe: ${error instanceof Error ? error.message : String(error)}\n`,
        );
      }
      if (!stopping) await Bun.sleep(pollMs);
    }
    return 0;
  } finally {
    process.off("SIGINT", stop);
    process.off("SIGTERM", stop);
    const holder = Number((await readFile(lockPath, "utf8").catch(() => "")).trim());
    if (holder === process.pid) await rm(lockPath, { force: true });
  }
}
