import { z } from "zod";
import { HANDOFF_TRIGGERS } from "@/repo/handoff-emitter.port.js";
import { PROJECTION_VIEWS } from "@/shared/lib/projection.js";
import { TASK_STATES } from "@/types/task-state.js";

// Accepts both v1 (tsk-aabbcc) and v2 (tsk-x-y) task ID formats.
// v1 IDs are 6 lowercase hex chars; v2 IDs have two dash-separated alphanumeric
// segments. The broader pattern here accepts both without special-casing.
const taskId = z
  .string()
  .regex(/^tsk-[0-9a-f]{6}$|^tsk-[a-z0-9]+-[a-z0-9]+$/, "Invalid task id")
  .describe("A maestro task id like 'tsk-abc123' (v1) or 'tsk-lp1abc-xy1234' (v2).");
const planId = z
  .string()
  .regex(/^pln-[a-z0-9]+-[a-z0-9]+$/, "Invalid exec-plan id")
  .describe(
    "A maestro exec-plan id like 'pln-1a2b3c4d5e6f-a1b2c3'.",
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
  .regex(/^hnd-[a-z0-9]+-[a-z0-9]+$/, "Invalid handoff id")
  .describe("A maestro handoff envelope id like 'hnd-lp1abc-xy1234'.");
const handoffTrigger = z
  .enum(HANDOFF_TRIGGERS)
  .describe(
    "Lifecycle verb that prompted the handoff: task:claim, task:block, task:abandon, task:ship, task:verify.",
  );
// v2 task state enum. Replaces v1 status (pending|in_progress|completed).
const taskState = z
  .enum(TASK_STATES)
  .describe(
    "Filter by v2 task state. Values: draft, claimed, doing, verifying, blocked, ready, shipped, abandoned.",
  );
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

// v2 task list: only plan_id and state filters are supported.
// Removed v1-only filters: type, priority, label, parentId, assignee.
export const TaskListInput = z
  .object({
    plan_id: planId.optional(),
    state: taskState
      .optional()
      .describe(
        "Filter by v2 task state. v1 status (pending/in_progress/completed) is not supported; use state with v2 values.",
      ),
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

export const TaskClaimInput = z
  .object({
    id: taskId,
    agent_id: z
      .string()
      .min(1)
      .optional()
      .describe("Agent identifier recorded on the task and evidence row. Defaults to MCP session id when omitted."),
  })
  .strict();

// Task ship (renamed from task_complete). v2: pr_url replaces the receipt.
// The verdict-PASS path is the authoritative completion receipt in v2.
export const TaskShipInput = z
  .object({
    id: taskId,
    pr_url: z
      .string()
      .url()
      .optional()
      .describe("Optional PR URL recorded on the task when merging."),
  })
  .strict();

// v2 task block: marks the task itself as blocked with a mandatory reason.
// Removed v1 fields: blockedTaskIds[], force (bidirectional graph edges).
export const TaskBlockInput = z
  .object({
    id: taskId,
    reason: z
      .string()
      .min(1)
      .describe("Human-readable explanation of what is blocking this task."),
  })
  .strict();

// task_from_spec creates a v2 task in draft state from a product-spec markdown file.
// Takes a file path (absolute or relative to repo root), not a spec ID.
export const TaskFromSpecInput = z
  .object({
    spec_path: z
      .string()
      .min(1)
      .describe(
        "Absolute or repo-root-relative path to the product-spec markdown file. Example: 'docs/specs/add-caching.md'.",
      ),
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

// --- New v2 hot-path inputs ---

export const PrinciplePromoteInput = z
  .object({
    correction_id: z
      .string()
      .min(1)
      .describe(
        "Evidence row id (evd-*) for a lint-violation row to promote to a principle. Use maestro_evidence_list to find candidates.",
      ),
  })
  .strict();

// setupCheck takes no user inputs — it reads the project root from context.
export const SetupCheckInput = z.object({}).strict();

export const HandoffListInput = z
  .object({
    task_id: taskId.optional(),
    trigger_verb: handoffTrigger.optional(),
    include_picked_up: z
      .boolean()
      .optional()
      .describe(
        "When true, surfaces envelopes that already have a pickup sidecar. Defaults to false (open work only).",
      ),
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

export const HandoffEmitShape = {
  task_id: taskId,
  trigger_verb: handoffTrigger,
  agent_id: z
    .string()
    .min(1)
    .optional()
    .describe(
      "Agent identifier recorded on the envelope. Defaults to MCP session id when omitted.",
    ),
  worktree_path: z
    .string()
    .min(1)
    .optional()
    .describe("Absolute or repo-root-relative worktree path the receiver should enter."),
  spec_path: z
    .string()
    .min(1)
    .optional()
    .describe("Spec markdown the receiver should load first."),
  reason: z
    .string()
    .min(1)
    .optional()
    .describe(
      "Required when trigger_verb is 'task:block'. Free-text reason recorded on the envelope.",
    ),
} as const;

export const HandoffEmitInput = z
  .object(HandoffEmitShape)
  .strict()
  .refine(
    (d) => d.trigger_verb !== "task:block" || d.reason !== undefined,
    {
      message: "reason is required when trigger_verb is 'task:block'",
      path: ["reason"],
    },
  );

export const HandoffPickupInput = z
  .object({
    id: handoffId,
    picked_up_by: z
      .string()
      .min(1)
      .optional()
      .describe("Identifier of the agent picking up the handoff. Defaults to MCP session id."),
    note: z
      .string()
      .min(1)
      .optional()
      .describe("Optional free-text note recorded on the pickup sidecar."),
  })
  .strict();

export { taskId, planId, verdictId, evidenceId, handoffId };
