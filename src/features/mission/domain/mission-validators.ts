import { z } from "zod";
import { MaestroError } from "@/shared/errors.js";
import type { PlanInput } from "./types.js";

const RISK_CLASSES = ["low", "medium", "high", "critical"] as const;

const EVIDENCE_KINDS = [
  "command",
  "manual-note",
  "verifier",
  "contract-amendment",
  "contract-amendment-blocked",
  "ai-review",
  "plan-check",
  "threat-model",
  "review-ack",
  "rollback-exercised",
  "verdict-override",
  "runtime-signal",
  "deploy-readiness",
  "cross-task-conflict",
] as const;

export const PlanInputSchema = z.object({
  intendedFiles: z.array(z.string().min(1)),
  proofSet: z.array(
    z.object({
      criterionId: z.string().min(1),
      evidenceKinds: z.array(z.enum(EVIDENCE_KINDS)),
    }),
  ),
  riskClass: z.enum(RISK_CLASSES),
  notes: z.string().optional(),
});

/**
 * Validate a parsed plan-file payload (anything coming out of YAML/JSON parse).
 * Throws a MaestroError with field-level hints when the shape is wrong, so
 * agents see actionable guidance instead of a TypeError stack.
 */
export function validatePlanInput(raw: unknown, planFilePath: string): PlanInput {
  const parsed = PlanInputSchema.safeParse(raw);
  if (parsed.success) return parsed.data;

  const issues = parsed.error.issues.map((issue) => {
    const path = issue.path.length > 0 ? issue.path.join(".") : "(root)";
    return `${path}: ${issue.message}`;
  });

  throw new MaestroError(`Invalid plan file: ${planFilePath}`, [
    ...issues,
    "",
    "Plan file shape (YAML or JSON):",
    "  intendedFiles: [<path>, ...]      # required, list of paths the plan touches",
    "  proofSet: [{criterionId, evidenceKinds: [...]}]   # required (may be empty list)",
    `  riskClass: <${RISK_CLASSES.join("|")}>          # required`,
    "  notes: <string>                   # optional",
  ]);
}
