/**
 * Reply validation schemas.
 *
 * Validates parsed YAML against the AgentReply shape. Throws on invalid
 * input; callers that need tolerance (e.g. the adapter's `list()`) should
 * wrap in try/catch.
 */
import { z } from "zod";
import {
  FEATURE_ID_PATTERN,
  MISSION_ID_PATTERN,
} from "@/shared/domain/legacy-mission";
import { AgentReportSchema } from "@/features/plan";
import { MaestroError } from "@/shared/errors.js";
import type { AgentReply } from "./types.js";
import { REPLY_OUTCOMES } from "./types.js";

const ReplyOutcomeSchema = z.enum(REPLY_OUTCOMES);
const ReplyAuthorSchema = z.enum(["agent", "human"]);
const IsoDateSchema = z.string().regex(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?Z$/);

const AgentReplySchema = z.object({
  missionId: z.string().regex(MISSION_ID_PATTERN, "Mission id must match YYYY-MM-DD-NNN"),
  featureId: z.string().regex(FEATURE_ID_PATTERN, "Feature id must match [A-Za-z0-9][A-Za-z0-9_-]*"),
  outcome: ReplyOutcomeSchema,
  report: AgentReportSchema.optional(),
  notes: z.string().optional(),
  writtenAt: IsoDateSchema,
  writtenBy: ReplyAuthorSchema,
  source: z.string().optional(),
}).strict();

export function validateAgentReply(value: unknown): AgentReply {
  const parsed = AgentReplySchema.safeParse(value);
  if (!parsed.success) {
    const first = parsed.error.issues[0];
    const path = first?.path.join(".") || "<root>";
    throw new MaestroError(`Invalid reply: ${path}: ${first?.message ?? "unknown"}`, [
      "Replies must conform to the AgentReply schema",
      "See `.maestro/replies/<mission-id>/<id>.yaml` in the prompt's Reply Contract",
    ]);
  }
  return parsed.data as AgentReply;
}
