import os from "node:os";
import type { Command } from "commander";
import { MaestroError } from "@/shared/errors.js";
import { output, resolveJsonFlag } from "@/shared/lib/output.js";
import { getServices, type Services } from "@/services.js";
import { recordEvidence } from "@/features/evidence/index.js";
import type { ReviewAckPayload } from "@/features/evidence/index.js";

interface ReviewAckCommandDeps {
  readonly getServices: () => Pick<Services, "evidenceStore">;
  readonly recordEvidence: typeof recordEvidence;
  readonly getUsername: () => string;
}

const defaultDeps: ReviewAckCommandDeps = {
  getServices,
  recordEvidence,
  getUsername: () => os.userInfo().username,
};

export function registerReviewCommand(
  program: Command,
  deps: ReviewAckCommandDeps = defaultDeps,
): void {
  const reviewCmd = program
    .command("review")
    .description("Review acknowledgement commands");

  reviewCmd
    .command("ack")
    .description("Acknowledge one or more review criteria for a verdict")
    .addHelpText(
      "after",
      `
Examples:
  maestro review ack --task tsk-aaaaaa --verdict vrd-bbbbbb --criterion "All tests pass"
  maestro review ack --task tsk-aaaaaa --verdict vrd-bbbbbb \\
    --criterion "All tests pass" --criterion "No critical findings"
`,
    )
    .requiredOption("--task <id>", "Task this review belongs to")
    .requiredOption("--verdict <id>", "Verdict being acknowledged")
    .option(
      "--criterion <text>",
      "Criterion text being acknowledged (repeatable)",
      (val: string, acc: string[]) => { acc.push(val); return acc; },
      [] as string[],
    )
    .option("--json", "Output as JSON")
    .action(async (opts: {
      task: string;
      verdict: string;
      criterion: string[];
      json?: boolean;
    }) => {
      const { task: taskId, verdict: verdictId, criterion: criteria } = opts;
      const services = deps.getServices();
      const isJson = resolveJsonFlag(opts, program);

      if (!Array.isArray(criteria) || criteria.length === 0) {
        throw new MaestroError(
          "--criterion is required (pass at least one)",
          [
            `maestro review ack --task ${taskId} --verdict ${verdictId} --criterion "All tests pass"`,
          ],
        );
      }

      const payload: ReviewAckPayload = {
        verdictId,
        ackedBy: deps.getUsername(),
        criteria,
      };

      const row = await deps.recordEvidence(services.evidenceStore, {
        task_id: taskId,
        kind: "review-ack",
        payload,
        witness_level: "agent-claimed-locally",
      });

      output(isJson, row, (r) => [
        `[ok] Review acknowledged: ${r.id}`,
        `  Task:    ${r.task_id}`,
        `  Verdict: ${payload.verdictId}`,
        `  By:      ${payload.ackedBy}`,
        `  Criteria (${payload.criteria.length}):`,
        ...payload.criteria.map((c) => `    - ${c}`),
        `  Witness: ${r.witness_level}`,
        `  Created: ${r.created_at}`,
        "",
        "Next step: this ack is consumed by `maestro merge auto`,",
        "  not by `verdict request` — re-running `verdict request` will",
        "  still return HUMAN. Run `maestro merge auto --pr <n> --task <id>`",
        "  to check eligibility now that the ack is recorded.",
      ]);
    });
}
