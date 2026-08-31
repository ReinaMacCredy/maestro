import { Database } from "bun:sqlite";
import { existsSync } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { CliError } from "../kernel/cli.ts";
import { resolveStoreLocation } from "../kernel/store.ts";

interface DispatchEnvelope {
  data?: {
    dispatches?: Array<{
      heldBy?: string | null;
      state?: string;
    }>;
  };
}

interface StatusEnvelope {
  data?: {
    livePeers?: Array<{
      heldWork?: unknown[];
      id?: string;
    }>;
  };
}

interface RepoActivationScan {
  holders: number;
  legacyTeams: string[];
  repo: string;
  unsafe: boolean;
}

const cli = resolve(process.argv[1] ?? join(import.meta.dir, "..", "..", "bin", "maestro.ts"));

async function registeredRepos(home: string): Promise<string[] | null> {
  const registry = join(home, "maestro", "registry");
  if (!existsSync(registry)) return [];
  try {
    return (await readFile(registry, "utf8")).split(/\r?\n/).filter(Boolean);
  } catch {
    return null;
  }
}

async function readRepoJson(repo: string, args: string[]): Promise<unknown | null> {
  try {
    const child = Bun.spawn([process.execPath, cli, ...args], {
      cwd: repo,
      env: { ...process.env, MAESTRO_READ_ONLY: "1" },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [stdout, , exitCode] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    if (exitCode !== 0) return null;
    return JSON.parse(stdout) as unknown;
  } catch {
    return null;
  }
}

function liveLegacyTeams(repo: string): string[] | null {
  let path: string;
  try {
    path = resolveStoreLocation(repo).path;
  } catch {
    return null;
  }
  if (!existsSync(path)) return [];
  let database: Database;
  try {
    database = new Database(path, { create: false, readonly: true, strict: true });
  } catch {
    return null;
  }
  try {
    const table = database
      .query<{ present: number }, []>(
        "SELECT 1 AS present FROM sqlite_master WHERE type = 'table' AND name = 'team_lifecycle'",
      )
      .get();
    if (!table) return [];
    return database
      .query<{ generation: number; team_id: string }, []>(
        "SELECT team_id, generation FROM team_lifecycle WHERE stage <> 'STOPPED' ORDER BY team_id",
      )
      .all()
      .map((team) => `${team.team_id}:g${team.generation}`);
  } catch {
    return null;
  } finally {
    database.close();
  }
}

async function scanRepo(repo: string, caller: string): Promise<RepoActivationScan> {
  try {
    const [repoStat, storeStat] = await Promise.all([stat(repo), stat(join(repo, ".maestro"))]);
    if (!repoStat.isDirectory() || !storeStat.isDirectory()) {
      return { holders: 0, legacyTeams: [], repo, unsafe: true };
    }
  } catch {
    return { holders: 0, legacyTeams: [], repo, unsafe: true };
  }
  const legacyTeams = liveLegacyTeams(repo);
  const [statusValue, dispatchValue] = await Promise.all([
    readRepoJson(repo, ["status", "--live", "--json"]),
    readRepoJson(repo, ["dispatch", "list", "--json"]),
  ]);
  const status = statusValue as StatusEnvelope | null;
  const dispatch = dispatchValue as DispatchEnvelope | null;
  // status --live excludes dead sessions; under MAESTRO_READ_ONLY the caller
  // is not the child's current session, so it is excluded here.
  const peers = status?.data?.livePeers;
  if (!Array.isArray(peers) || !Array.isArray(dispatch?.data?.dispatches)) {
    return { holders: 0, legacyTeams: legacyTeams ?? [], repo, unsafe: true };
  }
  const openDispatchHolders = new Set(
    dispatch.data.dispatches
      .filter((item) => item?.state === "open" && typeof item.heldBy === "string")
      .map((item) => item.heldBy as string),
  );
  const holders = peers.filter((peer) => {
    if (typeof peer?.id !== "string" || peer.id === caller) return false;
    return (Array.isArray(peer.heldWork) && peer.heldWork.length > 0) ||
      openDispatchHolders.has(peer.id);
  }).length;
  return { holders, legacyTeams: legacyTeams ?? [], repo, unsafe: legacyTeams === null };
}

export async function warnBeforeRuntimeActivation(
  home: string,
  action: "install" | "update",
): Promise<void> {
  const repos = await registeredRepos(home);
  if (repos === null) {
    process.stderr.write(`[${action}] repository registry unreadable; treating as unsafe\n`);
    return;
  }
  const caller = process.env.MAESTRO_SESSION_ID ?? "";
  const results = await Promise.all(repos.map((repo) => scanRepo(repo, caller)));
  const liveLegacy = results.flatMap((result) =>
    result.legacyTeams.map((team) => ({ repo: result.repo, team }))
  );
  if (liveLegacy.length > 0) {
    throw new CliError(
      "SLP_V1_TEAM_RUNNING",
      `stop every SLP v1 team before maestro ${action}: ${liveLegacy.map(({ repo, team }) => `${team} in ${repo}`).join(", ")}; run the old runtime's maestro team stop first`,
      { action, teams: liveLegacy },
    );
  }
  const holders = results.reduce((count, result) => count + result.holders, 0);
  const holderRepos = results.filter((result) => result.holders > 0).map((result) => result.repo);
  if (holders > 0) {
    const subject = holders === 1 ? "1 live session holds" : `${holders} live sessions hold`;
    process.stderr.write(
      `[${action}] ${subject} work or an open dispatch (repos: ${holderRepos.join(", ")}); they load the new runtime on their next maestro call\n`,
    );
  }
  const unsafeRepos = results.filter((result) => result.unsafe).map((result) => result.repo);
  if (unsafeRepos.length > 0) {
    const subject = unsafeRepos.length === 1
      ? "1 registered repository"
      : `${unsafeRepos.length} registered repositories`;
    process.stderr.write(
      `[${action}] ${subject} unreadable (repos: ${unsafeRepos.join(", ")}); treating as unsafe\n`,
    );
  }
}
