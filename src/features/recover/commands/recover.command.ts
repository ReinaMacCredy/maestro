import type { Command } from "commander";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { recoverTask, type RecoverResult } from "../usecases/recover.usecase.js";

interface RecoverDeps {
  readonly getServices: () => Pick<
    Services,
    "evidenceStore" | "verdictStore" | "projectRoot"
  >;
}

export function registerRecoverCommand(
  program: Command,
  deps: RecoverDeps = { getServices },
): void {
  program
    .command("recover")
    .description("Reset task state to the last green tree (or an explicit commit), drop run state, record evidence")
    .requiredOption("--task <id>", "Task id")
    .option("--to <commit>", "Reset to this commit instead of the last PASS verdict's tree")
    .option("--force", "Reset even if the working tree is dirty (destructive)")
    .option("--dry-run", "Show what would happen without resetting")
    .option("--json", "Output as JSON")
    .action(async (opts): Promise<void> => {
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      const result = await recoverTask(
        {
          evidenceStore: services.evidenceStore,
          verdictStore: services.verdictStore,
        },
        {
          taskId: opts.task,
          projectRoot: services.projectRoot,
          to: opts.to,
          force: opts.force === true,
          dryRun: opts.dryRun === true,
        },
      );

      output(isJson, result, (r) => formatRecoverLines(r, opts.task, opts.dryRun === true));
    });
}

function formatRecoverLines(r: RecoverResult, taskId: string, dryRun: boolean): string[] {
  const lines: string[] = [];
  lines.push(`Recover task ${taskId}`);
  lines.push(`  From: ${r.plan.fromCommit.slice(0, 7)}`);
  lines.push(`  To:   ${r.plan.toCommit.slice(0, 7)} (${r.plan.reason})`);
  if (r.plan.anchorVerdictId) lines.push(`  Anchor verdict: ${r.plan.anchorVerdictId}`);
  lines.push(`  Working tree dirty: ${r.plan.dirty}`);
  if (r.applied) {
    lines.push(`  Reset applied; run state at ${r.plan.runStatePath} dropped`);
    if (r.evidenceId) lines.push(`  Evidence: ${r.evidenceId}`);
  } else if (dryRun) {
    lines.push("  (dry-run; no changes made)");
  } else if (r.plan.fromCommit === r.plan.toCommit) {
    lines.push("  (already at target commit; nothing to do)");
  }
  return lines;
}
