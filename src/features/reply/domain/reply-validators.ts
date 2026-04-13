/**
 * Reply validation schemas.
 *
 * Validates parsed YAML against the WorkerReply shape. Throws on invalid
 * input; callers that need tolerance (e.g. the adapter's `list()`) should
 * wrap in try/catch.
 */
import { z } from "zod";
import { FEATURE_ID_PATTERN, WorkerReportSchema } from "@/features/mission/index.js";
import { MaestroError } from "@/shared/errors.js";
import type { WorkerReply } from "./reply-types.js";

const ReplyOutcomeSchema = z.enum(["completed", "kicked-back", "abandoned"]);
const ReplyAuthorSchema = z.enum(["agent", "human"]);
const IsoDateSchema = z.string().regex(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?Z$/);

const WorkerReplySchema = z.object({
  featureId: z.string().regex(FEATURE_ID_PATTERN, "Feature id must match [A-Za-z0-9][A-Za-z0-9_-]*"),
  outcome: ReplyOutcomeSchema,
  report: WorkerReportSchema.optional(),
  notes: z.string().optional(),
  writtenAt: IsoDateSchema,
  writtenBy: ReplyAuthorSchema,
  source: z.string().optional(),
}).strict();

export function validateWorkerReply(value: unknown): WorkerReply {
  const parsed = WorkerReplySchema.safeParse(value);
  if (!parsed.success) {
    const first = parsed.error.issues[0];
    const path = first?.path.join(".") || "<root>";
    throw new MaestroError(`Invalid reply: ${path}: ${first?.message ?? "unknown"}`, [
      "Replies must conform to the WorkerReply schema",
      "See `.maestro/replies/<id>.yaml` in the prompt's Reply Contract",
    ]);
  }
  return parsed.data as WorkerReply;
}
