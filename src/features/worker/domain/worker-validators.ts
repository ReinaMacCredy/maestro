import { z } from "zod";
import { MaestroError } from "@/shared/errors.js";
import type { WorkerConfig } from "./worker-types.js";

/**
 * Phase 1 strip removed the A2A transport and execution/runtime event
 * stores. Phase 3 deletes the surviving supervision / parallel config
 * validators and the `isCliWorkerConfig` helper because they have no
 * live callers. Only the CLI worker config schema remains because
 * `maestro doctor` and the Mission Control config inspector still
 * render CLI worker definitions from `.maestro/config.yaml`.
 */

const CliWorkerConfigSchema = z.object({
  enabled: z.boolean(),
  transport: z.literal("cli"),
  command: z.string().min(1),
  args: z.array(z.string()).optional(),
  outputMode: z.enum(["raw", "stream-json"]).optional(),
  env: z.record(z.string()).optional(),
}).strict();

export const WorkerConfigSchema = CliWorkerConfigSchema;

function formatIssues(issues: readonly z.ZodIssue[]): string[] {
  return issues.map((issue) => {
    const path = issue.path.length > 0 ? issue.path.join(".") : "<root>";
    return `${path}: ${issue.message}`;
  });
}

export function validateWorkerConfig(value: unknown): WorkerConfig {
  const parsed = WorkerConfigSchema.safeParse(value);
  if (parsed.success) {
    return parsed.data as WorkerConfig;
  }

  throw new MaestroError("Invalid worker config", formatIssues(parsed.error.issues));
}
