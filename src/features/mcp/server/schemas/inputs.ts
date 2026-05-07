import { z } from "zod";

const taskId = z
  .string()
  .regex(/^tsk-[a-z0-9]+$/, "Invalid task id")
  .describe("A maestro task id like 'tsk-abc123'.");
const missionId = z
  .string()
  .regex(/^msn-[a-z0-9]+$/, "Invalid mission id")
  .describe("A maestro mission id like 'msn-abc123'.");
const verdictId = z
  .string()
  .regex(/^vrd-\d{13}-[0-9a-f]{6}$/, "Invalid verdict id")
  .describe("A maestro verdict id like 'vrd-1714747200123-a1b2c3'.");
const evidenceId = z
  .string()
  .regex(/^evd-\d{13}-[0-9a-f]{6}$/, "Invalid evidence id")
  .describe("A maestro evidence id like 'evd-1714747200123-a1b2c3'.");

const taskStatus = z
  .enum(["pending", "in_progress", "completed"])
  .describe("Filter by task status.");
const taskType = z
  .enum(["task", "bug", "feature", "epic", "chore"])
  .describe("Filter by task type.");
const taskPriority = z
  .union([z.literal(0), z.literal(1), z.literal(2), z.literal(3), z.literal(4)])
  .describe("Filter by priority (0=critical, 4=backlog).");
const witnessLevel = z
  .enum([
    "witnessed-by-maestro",
    "witnessed-by-ci",
    "agent-claimed-locally",
    "agent-claimed-and-not-reproducible",
  ])
  .describe(
    "Trust ladder for evidence. Strongest to weakest: witnessed-by-maestro, witnessed-by-ci, agent-claimed-locally, agent-claimed-and-not-reproducible.",
  );
const riskClass = z.enum(["low", "medium", "high", "critical"]);

const limit = z
  .number()
  .int()
  .min(1)
  .max(100)
  .optional()
  .describe("Page size, 1..100. Defaults to 20 when omitted.");
const offset = z
  .number()
  .int()
  .min(0)
  .optional()
  .describe("Zero-based page offset. Defaults to 0 when omitted.");

export const TaskListInput = z
  .object({
    missionId: missionId.optional(),
    status: taskStatus.optional(),
    type: taskType.optional(),
    priority: taskPriority.optional(),
    label: z
      .string()
      .min(1)
      .optional()
      .describe("Filter to tasks carrying this label (exact match)."),
    parentId: taskId.optional(),
    assignee: z
      .string()
      .min(1)
      .optional()
      .describe("Filter by assignee/session id."),
    limit,
    offset,
  })
  .strict();

export const TaskGetInput = z
  .object({
    id: taskId,
  })
  .strict();

export const TaskCreateInput = z
  .object({
    title: z
      .string()
      .min(1)
      .max(200)
      .describe("Task title, 1..200 chars. Slug is derived from this automatically."),
    description: z
      .string()
      .optional()
      .describe("Optional task description; supports markdown."),
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
    summary: z
      .string()
      .optional()
      .describe("Optional one-line completion summary stored on the task receipt."),
  })
  .strict();

export const TaskBlockInput = z
  .object({
    id: taskId,
    blockedTaskIds: z
      .array(taskId)
      .min(1)
      .describe("Task ids that this task should block. Edges are bidirectional."),
    force: z
      .boolean()
      .optional()
      .describe("Bypass cycle detection. Use only when you understand the consequences."),
  })
  .strict();

export const TaskUnblockInput = z
  .object({
    id: taskId,
    blockedTaskIds: z
      .array(taskId)
      .min(1)
      .describe("Task ids whose blocker edge from this task should be removed."),
    force: z.boolean().optional(),
  })
  .strict();

export const EvidenceListInput = z
  .object({
    taskId,
    kind: z
      .string()
      .optional()
      .describe(
        "Optional kind filter, e.g. 'command', 'manual-note', 'plan-check', 'ai-review', 'threat-model'.",
      ),
    witnessLevel: witnessLevel.optional(),
    limit,
    offset,
  })
  .strict();

export const EvidenceRecordInput = z
  .object({
    taskId,
    command: z
      .string()
      .min(1)
      .optional()
      .describe(
        "Shell command that was executed. Pair with exitCode. Mutually exclusive with note.",
      ),
    exitCode: z
      .number()
      .int()
      .optional()
      .describe("Exit code of the command (0=success). Required when command is set."),
    note: z
      .string()
      .min(1)
      .optional()
      .describe(
        "Free-text note for manual evidence (e.g. 'Verified UI on staging'). Mutually exclusive with command.",
      ),
    witnessLevel: witnessLevel.optional(),
  })
  .strict()
  .refine(
    (d) => (d.command !== undefined) !== (d.note !== undefined),
    {
      message: "Provide exactly one of: command (with exitCode) or note",
      path: ["command"],
    },
  )
  .refine(
    (d) => d.command === undefined || d.exitCode !== undefined,
    {
      message: "exitCode is required when command is provided",
      path: ["exitCode"],
    },
  );

export const VerdictShowInput = z
  .object({
    taskId,
    id: verdictId
      .optional()
      .describe("Optional specific verdict id. Omit to fetch the latest verdict for the task."),
  })
  .strict();

export const VerdictRequestInput = z
  .object({
    taskId,
    base: z
      .string()
      .optional()
      .describe(
        "Optional git base ref (e.g. 'main', 'origin/main'). Defaults to the contract's claimedAtCommit or the repo default base.",
      ),
  })
  .strict();

export const ContractShowInput = z
  .object({
    taskId,
    version: z
      .number()
      .int()
      .min(1)
      .optional()
      .describe("Optional 1-based version number. Omit to fetch the current contract."),
  })
  .strict();

export const ContractAmendInput = z
  .object({
    taskId,
    addPaths: z
      .array(z.string())
      .optional()
      .describe("Paths to add to filesExpected. May include glob patterns."),
    removePaths: z
      .array(z.string())
      .optional()
      .describe("Paths to remove from filesExpected (exact-string match)."),
    reason: z
      .string()
      .min(1)
      .describe("Required free-text reason for the amendment, recorded in evidence."),
  })
  .strict();

export const PolicyCheckInput = z
  .object({
    taskId,
  })
  .strict();

export { taskId, missionId, verdictId, evidenceId };
