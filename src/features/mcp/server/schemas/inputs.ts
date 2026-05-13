import { z } from "zod";
import { PROJECTION_VIEWS } from "@/shared/lib/projection.js";

const taskId = z
  .string()
  .regex(/^tsk-[a-z0-9]+$/, "Invalid task id")
  .describe("A maestro task id like 'tsk-abc123'.");
const missionId = z
  .string()
  .regex(/^[a-z0-9][a-z0-9-]*$/, "Invalid mission id")
  .describe(
    "A maestro mission id. Maestro generates dated ids like '2026-05-07-001'; legacy and seeded ids may use prefixes like 'msn-billing'.",
  );
const verdictId = z
  .string()
  .regex(/^vrd-\d{13}-[0-9a-f]{6}$/, "Invalid verdict id")
  .describe("A maestro verdict id like 'vrd-1714747200123-a1b2c3'.");
const evidenceId = z
  .string()
  .regex(/^evd-\d{13}-[0-9a-f]{6}$/, "Invalid evidence id")
  .describe("A maestro evidence id like 'evd-1714747200123-a1b2c3'.");
const handoffId = z
  .string()
  .regex(
    /^(\d{4}-\d{2}-\d{2}-\d{3}|[a-z]+-[a-z]+-\d+)$/,
    "Invalid handoff id",
  )
  .describe("A maestro handoff id like 'bold-otter-1' or '2026-05-08-001'.");
const handoffAgent = z
  .enum(["codex", "claude", "hermes"])
  .describe(
    "Acting agent identifier when picking up a handoff. Claude Code agents pass `claude` (not `claude-code`); the Codex CLI passes `codex`.",
  );

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

const evidenceKind = z
  .enum([
    "command",
    "manual-note",
    "verifier",
    "contract-amendment",
    "contract-amendment-blocked",
    "ai-review",
    "plan-check",
    "threat-model",
    "review-ack",
    "rollback-exercised",
    "verdict-override",
    "runtime-signal",
    "deploy-readiness",
    "cross-task-conflict",
  ])
  .describe("Evidence kind. Mirrors the EvidenceKind union in the evidence domain.");

const limit = z
  .number()
  .int()
  .min(1)
  .max(100)
  .optional()
  .describe("Page size, 1..100. Defaults to 20 when omitted.");
const view = z
  .enum(PROJECTION_VIEWS)
  .optional()
  .describe(
    "Projection: 'summary' (default) returns lean per-item shape for token-budget; 'full' returns detail-grade items.",
  );
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
    view,
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
    // CLI uses `--reason`; accept it as an alias so agents that read CLI
    // docs and then call the MCP tool don't trip on naming drift.
    reason: z
      .string()
      .optional()
      .describe("Alias for `summary` (matches the CLI's --reason flag)."),
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
    kind: evidenceKind.optional(),
    witnessLevel: witnessLevel.optional(),
    limit,
    offset,
    view,
  })
  .strict();

// Raw shape exported so the MCP SDK can introspect properties for the
// tools/list JSON Schema. Z.object().refine() returns ZodEffects, which
// strips the `.shape` accessor — surfacing as `"properties": {}` to
// agents calling tools/list. Refines apply in EvidenceRecordInput
// (used for runtime parsing only).
export const EvidenceRecordShape = {
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
} as const;

export const EvidenceRecordInput = z
  .object(EvidenceRecordShape)
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

const handoffDisplayState = z
  .enum(["open", "consumed", "completed", "failed"])
  .describe(
    "Handoff display state. 'open' = launching/launched and not consumed; 'consumed' = picked up; 'completed'/'failed' = launched session terminated.",
  );

export const HandoffListInput = z
  .object({
    openOnly: z
      .boolean()
      .optional()
      .describe(
        "When true, return only packets that have not been consumed. Equivalent to displayState='open'. Do not combine with displayState.",
      ),
    displayState: handoffDisplayState
      .optional()
      .describe(
        "Filter by computed display state. Do not combine with openOnly.",
      ),
    taskId: taskId
      .optional()
      .describe("Filter to packets whose refs.taskId equals this task id."),
    agent: handoffAgent
      .optional()
      .describe("Filter by launching agent (the receiving session's agent)."),
    limit,
    offset,
    view,
  })
  .strict();

export const HandoffShowInput = z
  .object({
    id: handoffId,
  })
  .strict();

export const HandoffOpenForTaskInput = z
  .object({
    taskId,
  })
  .strict();

export const HandoffPickupInput = z
  .object({
    id: handoffId,
    actorAgent: handoffAgent,
    actorSessionId: z
      .string()
      .min(1)
      .optional()
      .describe(
        "Session id to record on the pickup. Defaults to the MCP session id (MAESTRO_SESSION_ID/CLAUDECODE_SESSION_ID/CODEX_THREAD_ID, else username@host).",
      ),
    ownerId: z
      .string()
      .min(1)
      .optional()
      .describe(
        "Optional task owner id when resuming a task-linked packet. Defaults to buildTaskOwnerId(actorAgent, actorSessionId).",
      ),
    standalone: z
      .boolean()
      .optional()
      .describe(
        "Consume the packet without resuming its linked task. Required when picking up a packet from a different project than the one that created it.",
      ),
  })
  .strict();

export { taskId, missionId, verdictId, evidenceId, handoffId };
