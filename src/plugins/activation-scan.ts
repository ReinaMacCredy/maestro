import { existsSync } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import { join, resolve } from "node:path";

interface WorkListEnvelope {
  data?: {
    works?: Array<{
      holder?: { id?: string; live?: boolean } | null;
    }>;
  };
}

interface RepoActivationScan {
  holders: number;
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

async function scanRepo(repo: string, caller: string): Promise<RepoActivationScan> {
  try {
    const [repoStat, storeStat] = await Promise.all([stat(repo), stat(join(repo, ".maestro"))]);
    if (!repoStat.isDirectory() || !storeStat.isDirectory()) {
      return { holders: 0, repo, unsafe: true };
    }
  } catch {
    return { holders: 0, repo, unsafe: true };
  }
  const listed = await readRepoJson(repo, ["work", "list", "--json"]) as WorkListEnvelope | null;
  const works = listed?.data?.works;
  if (!Array.isArray(works)) {
    return { holders: 0, repo, unsafe: true };
  }
  const holders = new Set(
    works
      .map((work) => work?.holder)
      .filter((holder) => holder?.live === true && typeof holder.id === "string" && holder.id !== caller)
      .map((holder) => holder?.id),
  ).size;
  return { holders, repo, unsafe: false };
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
  const holders = results.reduce((count, result) => count + result.holders, 0);
  const holderRepos = results.filter((result) => result.holders > 0).map((result) => result.repo);
  if (holders > 0) {
    const subject = holders === 1 ? "1 live session holds" : `${holders} live sessions hold`;
    process.stderr.write(
      `[${action}] ${subject} work (repos: ${holderRepos.join(", ")}); they load the new runtime on their next maestro call\n`,
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
