import { z } from "zod";

const taskId = z.string().regex(/^tsk-[a-z0-9]+$/, "Invalid task id");
const missionId = z.string().regex(/^msn-[a-z0-9]+$/, "Invalid mission id");
const verdictId = z.string().regex(/^vdt-[a-z0-9]+$/, "Invalid verdict id");
const evidenceId = z.string().regex(/^evd-[a-z0-9]+$/, "Invalid evidence id");

const taskStatus = z.enum(["pending", "in_progress", "completed"]);
const witnessLevel = z.enum([
  "witnessed-by-maestro",
  "witnessed-by-ci",
  "agent-claimed-locally",
  "agent-claimed-and-not-reproducible",
]);
const riskClass = z.enum(["low", "medium", "high", "critical"]);

export const TaskListInput = z
  .object({
    missionId: missionId.optional(),
    status: taskStatus.optional(),
    limit: z.number().int().min(1).max(100).optional(),
    offset: z.number().int().min(0).optional(),
  })
  .strict();

export const TaskGetInput = z
  .object({
    id: taskId,
  })
  .strict();

export const TaskCreateInput = z
  .object({
    title: z.string().min(1).max(200),
    description: z.string().optional(),
  })
  .strict();

export const TaskClaimInput = z
  .object({
    id: taskId,
  })
  .strict();

export const TaskCompleteInput = z
  .object({
    id: taskId,
    summary: z.string().optional(),
  })
  .strict();

export const TaskBlockInput = z
  .object({
    id: taskId,
    blockedTaskIds: z.array(taskId).min(1),
    force: z.boolean().optional(),
  })
  .strict();

export const TaskUnblockInput = z
  .object({
    id: taskId,
    blockedTaskIds: z.array(taskId).min(1),
    force: z.boolean().optional(),
  })
  .strict();

export const EvidenceListInput = z
  .object({
    taskId: taskId,
    kind: z.string().optional(),
    witnessLevel: witnessLevel.optional(),
    limit: z.number().int().min(1).max(100).optional(),
    offset: z.number().int().min(0).optional(),
  })
  .strict();

export const EvidenceRecordInput = z
  .object({
    taskId: taskId,
    command: z.string().optional(),
    exitCode: z.number().int().optional(),
    note: z.string().optional(),
    witnessLevel: witnessLevel.optional(),
  })
  .strict();

export const VerdictShowInput = z
  .object({
    taskId: taskId,
    id: verdictId.optional(),
  })
  .strict();

export const VerdictRequestInput = z
  .object({
    taskId: taskId,
    base: z.string().optional(),
  })
  .strict();

export const ContractShowInput = z
  .object({
    taskId: taskId,
    version: z.number().int().min(1).optional(),
  })
  .strict();

export const ContractAmendInput = z
  .object({
    taskId: taskId,
    addPaths: z.array(z.string()).optional(),
    removePaths: z.array(z.string()).optional(),
    reason: z.string().min(1),
  })
  .strict();

export const PolicyCheckInput = z
  .object({
    taskId: taskId,
  })
  .strict();

export { taskId, missionId, verdictId, evidenceId };
