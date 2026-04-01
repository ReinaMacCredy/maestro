import { z } from "zod";
import { MaestroError } from "./errors.js";
import type { WorkerRuntime } from "./runtime-types.js";
import { FEATURE_ID_PATTERN } from "./mission-validators.js";

const ISO_DATE_PATTERN = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?Z$/;

const RecoveryHistoryEntrySchema = z.object({
  timestamp: z.string().regex(ISO_DATE_PATTERN),
  reason: z.string().min(1),
  fromState: z.enum(["starting", "live", "stale", "failed", "recoverable", "completed"]),
  toState: z.enum(["starting", "live", "stale", "failed", "recoverable", "completed"]),
}).strict();

const RecoveryMetadataSchema = z.object({
  retryCount: z.number().int().nonnegative(),
  lastRecoveryAt: z.string().regex(ISO_DATE_PATTERN).optional(),
  lastRecoveryReason: z.string().min(1).optional(),
  history: z.array(RecoveryHistoryEntrySchema),
}).strict();

export const WorkerRuntimeSchema = z.object({
  featureId: z.string().regex(FEATURE_ID_PATTERN),
  attemptId: z.string().min(1),
  attempt: z.number().int().positive(),
  agent: z.string().min(1),
  sessionId: z.string().min(1).optional(),
  runtimeState: z.enum(["starting", "live", "stale", "failed", "recoverable", "completed"]),
  startedAt: z.string().regex(ISO_DATE_PATTERN),
  lastSeenAt: z.string().regex(ISO_DATE_PATTERN),
  leaseExpiresAt: z.string().regex(ISO_DATE_PATTERN),
  failureReason: z.string().min(1).optional(),
  recoveryMetadata: RecoveryMetadataSchema,
}).strict();

export function validateWorkerRuntime(value: unknown): WorkerRuntime {
  const parsed = WorkerRuntimeSchema.safeParse(value);
  if (parsed.success) {
    return parsed.data;
  }

  const issues = parsed.error.issues.map((issue) => {
    const path = issue.path.length > 0 ? issue.path.join(".") : "<root>";
    return `${path}: ${issue.message}`;
  });

  throw new MaestroError("Invalid worker runtime data", issues);
}
