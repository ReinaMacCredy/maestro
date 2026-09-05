import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { Store, resolveStoreLocation } from "../kernel/store.ts";
import { acquireProcessLock, runSlpProcess, slpRuntimeDirectory } from "./slp-process.ts";

interface WatchRole {
  name: string;
  role: "team-supervisor" | "lead" | "peer";
}

interface AgentRecord {
  name?: string;
  workspace_id?: string;
}

export interface SlpWatchConfig {
  env?: Record<string, string | undefined>;
  generation: number;
  intervalMs?: number;
  projectPath: string;
  teamId: string;
  workspaceId: string;
}

export const slpWatchRuntimeDirectory = slpRuntimeDirectory;

function resultRecord(value: Record<string, unknown>): Record<string, unknown> {
  const result = value.result;
  return result && typeof result === "object" ? result as Record<string, unknown> : {};
}

async function runJson(
  args: string[],
  cwd: string,
  env: Record<string, string | undefined>,
): Promise<Record<string, unknown>> {
  const stdout = await runSlpProcess(args, cwd, env);
  try {
    return JSON.parse(stdout) as Record<string, unknown>;
  } catch {
    throw new Error(`${args.slice(0, 3).join(" ")} returned invalid JSON`);
  }
}

function watchRoles(config: SlpWatchConfig): WatchRole[] {
  const store = new Store(resolveStoreLocation(config.projectPath).path, { readonly: true });
  try {
    const team = store.database
      .query<{ workspace_id: string }, [string, number]>(
        `SELECT workspace_id FROM slp_local_teams
         WHERE team_id = ? AND generation = ? AND state = 'RUNNING'`,
      )
      .get(config.teamId, config.generation);
    if (!team || team.workspace_id !== config.workspaceId) {
      throw new Error(
        `no running ${config.teamId}:g${config.generation} in workspace ${config.workspaceId}`,
      );
    }
    return store.database
      .query<WatchRole, [string, number]>(
        `SELECT name, role FROM slp_local_roles
         WHERE team_id = ? AND generation = ?
         ORDER BY CASE role WHEN 'team-supervisor' THEN 0 WHEN 'lead' THEN 1 ELSE 2 END, name`,
      )
      .all(config.teamId, config.generation);
  } finally {
    store.close();
  }
}

async function renderWatch(config: SlpWatchConfig): Promise<string> {
  const roles = watchRoles(config);
  const response = await runJson(["herdr", "agent", "list"], config.projectPath, config.env ?? {});
  const rawAgents = resultRecord(response).agents;
  const agents = Array.isArray(rawAgents) ? rawAgents as AgentRecord[] : [];
  const live = new Set(
    agents
      .filter((agent) => agent.workspace_id === config.workspaceId && typeof agent.name === "string")
      .map((agent) => agent.name as string),
  );
  const sections = [`SLP Watch ${config.teamId}:g${config.generation}`];
  for (const role of roles) {
    if (!live.has(role.name)) {
      sections.push(`=== ${role.name} [${role.role}] ===\n[unavailable]`);
      continue;
    }
    const output = await runSlpProcess(
      ["herdr", "agent", "read", role.name, "--source", "recent-unwrapped", "--lines", "200"],
      config.projectPath,
      config.env ?? {},
    );
    sections.push(`=== ${role.name} [${role.role}] ===\n${output.trimEnd()}`);
  }
  return `${sections.join("\n\n")}\n`;
}

export async function runSlpWatch(config: SlpWatchConfig): Promise<number> {
  const runtimeDirectory = slpWatchRuntimeDirectory(
    config.projectPath,
    config.teamId,
    config.generation,
  );
  const lockPath = join(runtimeDirectory, "watch.lock");
  const transcriptPath = join(runtimeDirectory, "transcript.txt");
  const pendingPath = join(runtimeDirectory, `transcript.${process.pid}.tmp`);
  await mkdir(runtimeDirectory, { recursive: true });
  await acquireProcessLock(lockPath, "Watch");
  let stopping = false;
  const stop = () => {
    stopping = true;
  };
  process.on("SIGINT", stop);
  process.on("SIGTERM", stop);
  try {
    watchRoles(config);
    const intervalMs = Math.max(25, config.intervalMs ?? 1_000);
    while (!stopping) {
      try {
        const rendered = await renderWatch(config);
        await writeFile(pendingPath, rendered);
        await rename(pendingPath, transcriptPath);
        process.stdout.write(`\u001b[2J\u001b[H${rendered}`);
      } catch (error) {
        process.stderr.write(
          `maestro-slp-watch: ${error instanceof Error ? error.message : String(error)}\n`,
        );
      }
      if (!stopping) await Bun.sleep(intervalMs);
    }
    return 0;
  } finally {
    process.off("SIGINT", stop);
    process.off("SIGTERM", stop);
    await rm(pendingPath, { force: true });
    const holder = Number((await readFile(lockPath, "utf8").catch(() => "")).trim());
    if (holder === process.pid) await rm(lockPath, { force: true });
  }
}
