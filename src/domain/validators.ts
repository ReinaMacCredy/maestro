import { z } from "zod";
import type { UkiHandoff } from "./uki-types.js";

/**
 * Phase 1 strip: the handoff validators (HandoffSessionSchema,
 * HandoffPlanSchema, HandoffSchema, HandoffEnvelopeSchema, the
 * validateHandoff / validateEnvelope functions) are gone. Phase 2
 * re-introduces UkiHandoffSchema below plus the dedicated
 * `validateUki()` function in `src/lib/uki-format.ts`.
 */

export const GitStateSchema = z.object({
  branch: z.string().min(1),
  recentCommits: z.array(z.string()),
  changedFiles: z.array(z.string()),
  fileChanges: z.array(z.object({
    path: z.string().min(1),
    kind: z.enum(["added", "modified", "deleted", "renamed", "copied", "typechange", "untracked", "conflicted"]),
  })).optional(),
  workingTreeClean: z.boolean(),
  diffStat: z.string(),
});

const UkiSlotsSchema = z.object({
  sessionCore: z.string().min(1),
  causalDrivers: z.array(z.string()),
  divergences: z.array(z.string()),
  keyDecisions: z.array(z.string()),
  signalDelta: z.array(z.string()),
  artifacts: z.array(z.string()),
  executionState: z.string().min(1),
  boundaryState: z.array(z.string()),
  stanceCollapse: z.string().min(1),
  nextAction: z.string().min(1),
  cs: z.object({
    work: z.number().optional(),
    summary: z.number().optional(),
  }).passthrough(),
  summary: z.string().min(1),
}).passthrough();

export const UkiHandoffSchema = z.object({
  id: z.string().min(1),
  version: z.literal("5.2"),
  timestamp: z.string().min(1),
  status: z.enum(["pending", "picked-up", "completed"]),
  agent: z.string().min(1),
  sessionId: z.string().min(1),
  slots: UkiSlotsSchema,
  uki: z.string().min(1),
  pickedUpAt: z.string().optional(),
  pickedUpBy: z.string().optional(),
  completedAt: z.string().optional(),
  report: z.string().optional(),
}).passthrough();

/**
 * Validate an unknown value as a UkiHandoff. Throws a ZodError with a
 * readable hint chain on failure. Callers that want a non-throwing
 * variant can use `UkiHandoffSchema.safeParse(value)`.
 */
export function validateUkiHandoff(value: unknown): UkiHandoff {
  return UkiHandoffSchema.parse(value) as unknown as UkiHandoff;
}
