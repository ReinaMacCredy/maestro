import type { Command } from "commander";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { ralphReview, type RalphReviewResult } from "../usecases/ralph-review.usecase.js";
import type { RalphIterationPayload } from "@/features/evidence";

interface RalphDeps {
  readonly getServices: () => Pick<Services, "evidenceStore" | "projectRoot">;
}

export function registerRalphCommand(
  program: Command,
  deps: RalphDeps = { getServices },
): void {
  const ralphCmd = program
    .command("ralph")
    .description("Convergence-loop verbs: review iterations, stuck detection");

  ralphCmd
    .command("review")
    .description("Run convergence review for a task: arch lints + verifier + AI review + threat model")
    .requiredOption("--task <id>", "Task id")
    .option("--stuck-threshold <n>", "Iterations of identical findings that mark the loop stuck", (v: string) => Number.parseInt(v, 10), 3)
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await ralphReview(
        {
          evidenceStore: services.evidenceStore,
          previousIterations: async (): Promise<readonly { iteration: number; findingsHash: string }[]> => {
            const rows = await services.evidenceStore.list({
              task_id: opts.task,
              kind: "ralph-iteration",
            });
            return rows.map((r) => {
              const p = r.payload as RalphIterationPayload;
              return { iteration: p.iteration, findingsHash: p.findingsHash };
            });
          },
        },
        {
          taskId: opts.task,
          projectRoot: services.projectRoot,
          stuckThreshold: opts.stuckThreshold,
        },
      );

      output(isJson, result, (r) => formatRalphLines(r, opts.task));

      if (result.stuck) process.exit(2);
      if (!result.converged) process.exit(1);
    });
}

function formatRalphLines(r: RalphReviewResult, taskId: string): string[] {
  const lines: string[] = [];
  lines.push(`Ralph iteration #${r.iteration} for ${taskId}`);
  lines.push(`  Findings: ${r.findings.length} (hash ${r.findingsHash})`);
  lines.push(`  Sources: ${r.sources.length === 0 ? "(none)" : r.sources.join(", ")}`);
  lines.push(`  Converged: ${r.converged}`);
  lines.push(`  Stuck: ${r.stuck}`);
  for (const f of r.findings) {
    const paths = f.paths && f.paths.length > 0 ? ` ${f.paths.join(", ")}` : "";
    lines.push(`  [${f.severity}] ${f.source}/${f.check}:${paths} ${f.message}`);
  }
  if (r.stuck) {
    lines.push("");
    lines.push("[stuck] Findings have not changed across the threshold; revisit approach.");
  }
  return lines;
}
