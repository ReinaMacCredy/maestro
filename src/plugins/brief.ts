import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";

interface WorkListEnvelope {
  data?: {
    works?: Array<{ state: string }>;
  };
  ok?: boolean;
}

async function registeredRepos(home: string): Promise<string[]> {
  const registry = join(home, "maestro", "registry");
  if (!existsSync(registry)) return [];
  return (await readFile(registry, "utf8")).split(/\r?\n/).filter(Boolean);
}

async function hasOpenWork(repo: string): Promise<boolean> {
  if (!existsSync(repo)) return true;
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
  if (exitCode !== 0) return true;
  try {
    const envelope = JSON.parse(stdout) as WorkListEnvelope;
    return (envelope.data?.works ?? []).some(
      (work) => work.state === "open" || work.state === "active",
    );
  } catch {
    return true;
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
          const states = await Promise.all(repos.map(hasOpenWork));
          const text = states.some(Boolean)
            ? "Some registered projects need attention."
            : "All registered projects are running normally.";
          return { data: { repos: repos.length }, text };
        },
        { description: "Summarize registered repository work without changing project stores." },
      ),
    );
  },
};
