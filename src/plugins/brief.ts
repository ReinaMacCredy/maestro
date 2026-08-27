import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { CliResult } from "../kernel/cli.ts";
import type { BuiltInPlugin } from "../kernel/loader.ts";

interface AttentionEnvelope {
  data?: {
    detections?: AttentionSummary[];
  };
  ok?: boolean;
}

interface AttentionSummary {
  kind: string;
  packet: string;
  route?: "lead" | "supervisor";
}

interface RepoBrief {
  error: boolean;
  findings: AttentionSummary[];
  missing: boolean;
  repo: string;
}

async function registeredRepos(home: string): Promise<string[]> {
  const registry = join(home, "maestro", "registry");
  if (!existsSync(registry)) return [];
  return (await readFile(registry, "utf8")).split(/\r?\n/).filter(Boolean);
}

async function scanRepo(repo: string): Promise<RepoBrief> {
  if (!existsSync(repo)) return { error: false, findings: [], missing: true, repo };
  const cli = resolve(process.argv[1] ?? join(import.meta.dir, "..", "..", "bin", "maestro.ts"));
  const child = Bun.spawn([process.execPath, cli, "attention", "--json"], {
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
  if (exitCode !== 0) return { error: true, findings: [], missing: false, repo };
  try {
    const envelope = JSON.parse(stdout) as AttentionEnvelope;
    return {
      error: false,
      findings: envelope.data?.detections ?? [],
      missing: false,
      repo,
    };
  } catch {
    return { error: true, findings: [], missing: false, repo };
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
          const findingLines = results.flatMap((result) =>
            result.findings
              .filter(
                (finding) =>
                  finding.kind !== "REPEATED_FAILURE" || finding.route === "supervisor",
              )
              .map(
                (finding) => `${result.repo}: ${finding.packet.split("\n")[0] ?? finding.kind}`,
              )
          );
          const unavailableLines = results.flatMap((result) =>
            result.missing
              ? [`Missing repository: ${result.repo}`]
              : result.error
                ? [`Unreadable repository: ${result.repo}`]
                : []
          );
          const attentionLines = [...findingLines, ...unavailableLines];
          const text = attentionLines.length > 0
            ? ["Needs attention:", ...attentionLines].join("\n")
            : "All registered projects are running normally.";
          return { data: { results }, text };
        },
        {
          description: "Summarize registered repository work without changing project stores.",
          mutates: false,
        },
      ),
    );
  },
};
