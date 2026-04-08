import { z } from "zod";

/**
 * Phase 1 strip: the handoff validators (HandoffSessionSchema,
 * HandoffPlanSchema, HandoffSchema, HandoffEnvelopeSchema, the
 * validateHandoff / validateEnvelope functions) are gone. Phase 2
 * re-introduces validators in `src/lib/uki-format.ts` for the new
 * UKI handoff format. Only shared primitives that other domain
 * files still need belong here.
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
