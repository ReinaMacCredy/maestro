import { z } from "zod";
import { MaestroError } from "./errors.js";
import type {
  A2aWorkerConfig,
  CliWorkerConfig,
  ExecutionRecord,
  ParallelConfig,
  SupervisionConfig,
  WorkerConfig,
} from "./worker-types.js";

const ISO_DATE_PATTERN = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?Z$/;

const CliWorkerConfigSchema = z.object({
  enabled: z.boolean(),
  transport: z.literal("cli"),
  command: z.string().min(1),
  args: z.array(z.string()).optional(),
  outputMode: z.enum(["raw", "stream-json"]).optional(),
  env: z.record(z.string()).optional(),
}).strict();

const A2aWorkerConfigSchema = z.object({
  enabled: z.boolean(),
  transport: z.literal("a2a"),
  url: z.string().url(),
  agentCardPath: z.string().min(1).optional(),
  headers: z.record(z.string()).optional(),
}).strict();

export const WorkerConfigSchema = z.discriminatedUnion("transport", [
  CliWorkerConfigSchema,
  A2aWorkerConfigSchema,
]);

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

export const ExecutionRecordSchema = z.object({
  id: z.string().min(1),
  missionId: z.string().min(1),
  featureId: z.string().min(1),
  worker: z.string().min(1),
  transport: z.enum(["cli", "a2a"]),
  attemptId: z.string().min(1),
  startedAt: z.string().regex(ISO_DATE_PATTERN),
  completedAt: z.string().regex(ISO_DATE_PATTERN),
  durationMs: z.number().int().nonnegative(),
  success: z.boolean(),
  exitCode: z.number().int(),
  summary: z.string(),
  stdoutRaw: z.string(),
  stderrRaw: z.string(),
  filesChanged: z.array(z.string()),
  report: z.record(z.unknown()).optional(),
  failureClass: z.enum(["infrastructure", "worker-crash", "validation", "unknown"]).optional(),
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

export function isA2aWorkerConfig(value: WorkerConfig): value is A2aWorkerConfig {
  return value.transport === "a2a";
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

export function validateExecutionRecord(value: unknown): ExecutionRecord {
  const parsed = ExecutionRecordSchema.safeParse(value);
  if (parsed.success) {
    return parsed.data as ExecutionRecord;
  }

  throw new MaestroError("Invalid execution record", formatIssues(parsed.error.issues));
}
