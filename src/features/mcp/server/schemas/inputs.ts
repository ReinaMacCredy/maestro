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

export const TaskListInput = {
  missionId: missionId.optional(),
  status: taskStatus.optional(),
  limit: z.number().int().min(1).max(100).optional(),
  offset: z.number().int().min(0).optional(),
};

export const TaskGetInput = {
  id: taskId,
};

export const TaskCreateInput = {
  title: z.string().min(1).max(200),
  description: z.string().optional(),
};

export const TaskClaimInput = {
  id: taskId,
};

export const TaskCompleteInput = {
  id: taskId,
  summary: z.string().optional(),
};

export const TaskBlockInput = {
  id: taskId,
  blockedTaskIds: z.array(taskId).min(1),
  force: z.boolean().optional(),
};

export const TaskUnblockInput = {
  id: taskId,
  blockedTaskIds: z.array(taskId).min(1),
  force: z.boolean().optional(),
};

export const EvidenceListInput = {
  taskId: taskId,
  kind: z.string().optional(),
  witnessLevel: witnessLevel.optional(),
  limit: z.number().int().min(1).max(100).optional(),
  offset: z.number().int().min(0).optional(),
};

export const EvidenceRecordInput = {
  taskId: taskId,
  command: z.string().optional(),
  exitCode: z.number().int().optional(),
  note: z.string().optional(),
  witnessLevel: witnessLevel.optional(),
};

export const VerdictShowInput = {
  taskId: taskId,
  id: verdictId.optional(),
};

export const VerdictRequestInput = {
  taskId: taskId,
  base: z.string().optional(),
};

export const ContractShowInput = {
  taskId: taskId,
  version: z.number().int().min(1).optional(),
};

export const ContractAmendInput = {
  taskId: taskId,
  addPaths: z.array(z.string()).optional(),
  removePaths: z.array(z.string()).optional(),
  reason: z.string().min(1),
};

export const PolicyCheckInput = {
  taskId: taskId,
};

export { taskId, missionId, verdictId, evidenceId };
