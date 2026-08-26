import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";

interface WorkListEnvelope {
  data?: {
    works?: WorkSummary[];
  };
  ok?: boolean;
}

interface WorkSummary {
  id: string;
  state: string;
  title: string;
}

interface RepoBrief {
  error: boolean;
  missing: boolean;
  repo: string;
  works: WorkSummary[];
}

async function registeredRepos(home: string): Promise<string[]> {
  const registry = join(home, "maestro", "registry");
  if (!existsSync(registry)) return [];
  return (await readFile(registry, "utf8")).split(/\r?\n/).filter(Boolean);
}

async function scanRepo(repo: string): Promise<RepoBrief> {
  if (!existsSync(repo)) return { error: false, missing: true, repo, works: [] };
  const cli = resolve(process.argv[1] ?? join(import.meta.dir, "..", "..", "bin", "maestro.ts"));
  const child = Bun.spawn([process.execPath, cli, "work", "list", "--json"], {
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
  if (exitCode !== 0) return { error: true, missing: false, repo, works: [] };
  try {
    const envelope = JSON.parse(stdout) as WorkListEnvelope;
    const works = (envelope.data?.works ?? []).filter(
      (work) => work.state === "open" || work.state === "active",
    );
    return { error: false, missing: false, repo, works };
  } catch {
    return { error: true, missing: false, repo, works: [] };
  }
}

export const briefPlugin: BuiltInPlugin = {
  name: "brief",
  apply(context) {
    context.effect(() =>
      context.cli.register(
        "brief",
        async (): Promise<CliResult> => {
          const repos = await registeredRepos(process.env.HOME ?? process.cwd());
          const results = await Promise.all(repos.map(scanRepo));
          const workLines = results.flatMap((result) =>
            result.works.map(
              (work) => `${result.repo}: ${work.id} [${work.state}] ${work.title}`,
            )
          );
          const unavailableLines = results.flatMap((result) =>
            result.missing
              ? [`Missing repository: ${result.repo}`]
              : result.error
                ? [`Unreadable repository: ${result.repo}`]
                : []
          );
          const attentionLines = [...workLines, ...unavailableLines];
          const text = attentionLines.length > 0
            ? ["Needs attention:", ...attentionLines].join("\n")
            : "All registered projects are running normally.";
          return { data: { results }, text };
        },
        { description: "Summarize registered repository work without changing project stores." },
      ),
    );
  },
};
