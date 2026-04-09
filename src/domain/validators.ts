import { z } from "zod";
import type { UkiHandoff, UkiHandoffContent } from "./uki-types.js";
import {
  SUPPORTED_UKI_HANDOFF_VERSIONS,
  UKI_HANDOFF_MODES,
  UKI_HANDOFF_STATUSES,
} from "./uki-types.js";

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

const ConfidenceSchema = z.object({
  work: z.number().optional(),
  summary: z.number().optional(),
}).strict();

const MaestroRefsSchema = z.object({
  missionId: z.string().min(1).optional(),
  featureId: z.string().min(1).optional(),
  milestoneId: z.string().min(1).optional(),
  planPath: z.string().min(1).optional(),
  specPath: z.string().min(1).optional(),
}).strict();

const BaseHandoffContentSchema = z.object({
  mode: z.enum(UKI_HANDOFF_MODES),
  currentState: z.string().min(1),
  sessionCore: z.string().min(1),
  decisions: z.array(z.string()),
  artifacts: z.array(z.string()),
  readMore: z.array(z.string()),
  nextAction: z.string().min(1),
  summary: z.string().min(1),
  maestroRefs: MaestroRefsSchema,
  cs: ConfidenceSchema,
  signalDelta: z.array(z.string()),
  boundaryState: z.array(z.string()),
  risks: z.array(z.string()),
  blindSpot: z.string().min(1).optional(),
  metaphor: z.string().min(1).optional(),
  causalDrivers: z.array(z.string()),
  divergences: z.array(z.string()),
}).strict();

const PlanHandoffContentSchema = BaseHandoffContentSchema.extend({
  mode: z.literal("plan"),
  planPaths: z.array(z.string()),
  maestroSync: z.array(z.string()),
}).strict();

const ExecuteHandoffContentSchema = BaseHandoffContentSchema.extend({
  mode: z.literal("execute"),
  touchedFiles: z.array(z.string()),
  completedWork: z.array(z.string()),
  validation: z.array(z.string()),
}).strict();

export const UkiHandoffContentSchema = z.discriminatedUnion("mode", [
  PlanHandoffContentSchema,
  ExecuteHandoffContentSchema,
]);

export const UkiHandoffSchema = z.object({
  id: z.string().min(1),
  version: z.enum(SUPPORTED_UKI_HANDOFF_VERSIONS),
  timestamp: z.string().min(1),
  status: z.enum(UKI_HANDOFF_STATUSES),
  agent: z.string().min(1),
  sessionId: z.string().min(1),
  content: UkiHandoffContentSchema,
  uki: z.string().min(1),
  pickedUpAt: z.string().optional(),
  pickedUpBy: z.string().optional(),
  completedAt: z.string().optional(),
  report: z.string().optional(),
}).passthrough();

export function validateUkiHandoffContent(value: unknown): UkiHandoffContent {
  return UkiHandoffContentSchema.parse(value);
}

export function validateUkiHandoff(value: unknown): UkiHandoff {
  return UkiHandoffSchema.parse(value);
}
