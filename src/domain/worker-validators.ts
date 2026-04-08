import { z } from "zod";
import { MaestroError } from "./errors.js";
import type {
  CliWorkerConfig,
  ParallelConfig,
  SupervisionConfig,
  WorkerConfig,
} from "./worker-types.js";

/**
 * Phase 1 strip: the A2A transport and execution/runtime event stores
 * were removed. Validators for `a2a` worker configs, execution records,
 * and runtime event records were deleted. Only the CLI worker config
 * schema survives because `maestro doctor` and Mission Control still
 * render CLI worker definitions.
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

export const SupervisionConfigSchema = z.object({
  level: z.enum(["low", "mid", "high"]).optional(),
  staleAfterMs: z.number().int().positive().optional(),
  killGraceMs: z.number().int().positive().optional(),
  progressIntervalMs: z.number().int().positive().optional(),
}).strict();

export const ParallelConfigSchema = z.object({
  enabled: z.boolean().optional(),
  maxConcurrent: z.number().int().positive().optional(),
}).strict();

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

export function isCliWorkerConfig(value: WorkerConfig): value is CliWorkerConfig {
  return value.transport === "cli";
}

export function validateSupervisionConfig(value: unknown): SupervisionConfig {
  const parsed = SupervisionConfigSchema.safeParse(value);
  if (parsed.success) {
    return parsed.data;
  }

  throw new MaestroError("Invalid supervision config", formatIssues(parsed.error.issues));
}

export function validateParallelConfig(value: unknown): ParallelConfig {
  const parsed = ParallelConfigSchema.safeParse(value);
  if (parsed.success) {
    return parsed.data;
  }

  throw new MaestroError("Invalid parallel config", formatIssues(parsed.error.issues));
}
