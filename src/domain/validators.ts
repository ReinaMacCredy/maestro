import { z } from "zod";
import type { Handoff, HandoffEnvelope } from "./types.js";

export const HandoffSessionSchema = z.object({
  agent: z.string().min(1),
  sessionId: z.string().min(1),
  sourcePath: z.string().min(1),
  cassIndexed: z.boolean(),
});

export const PlanTaskSchema = z.object({
  id: z.string().min(1),
  description: z.string().min(1),
  status: z.enum(["pending", "done", "blocked"]),
  dependsOn: z.array(z.string()),
});

export const HandoffPlanSchema = z.object({
  tasks: z.array(PlanTaskSchema),
  completed: z.number().int().nonnegative(),
  remaining: z.number().int().nonnegative(),
});

export const GitStateSchema = z.object({
  branch: z.string().min(1),
  recentCommits: z.array(z.string()),
  changedFiles: z.array(z.string()),
  workingTreeClean: z.boolean(),
  diffStat: z.string(),
});

export const HandoffSchema = z.object({
  id: z.string().regex(/^\d{4}-\d{2}-\d{2}-\d{3}$/),
  timestamp: z.string().datetime(),
  message: z.string().min(1),
  session: HandoffSessionSchema,
  plan: HandoffPlanSchema.optional(),
  sitrep: z.string().min(1),
  quickstart: z.string().min(1),
  git: GitStateSchema,
});

export const HandoffEnvelopeSchema = z.object({
  handoff: HandoffSchema,
  status: z.enum(["pending", "picked-up", "completed"]),
  pickedUpAt: z.string().datetime().optional(),
  pickedUpBy: z.string().optional(),
  completedAt: z.string().datetime().optional(),
});

export function validateHandoff(data: unknown): Handoff {
  return HandoffSchema.parse(data);
}

export function validateEnvelope(data: unknown): HandoffEnvelope {
  return HandoffEnvelopeSchema.parse(data);
}
