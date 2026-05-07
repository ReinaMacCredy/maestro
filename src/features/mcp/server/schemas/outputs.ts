import { z } from "zod";

// Output schemas describe the success-path `structuredContent` for each tool.
// The MCP SDK skips validation when `isError: true`, so error payloads
// (`{ code, message, hints }`) don't need to be modeled here.
//
// Inner objects use `.passthrough()` so adding a new field to a domain type
// (Task, Evidence, Verdict, Contract) does NOT break clients that have
// validated against an older outputSchema. Top-level wrappers use `.strict()`
// to surface accidental shape regressions during local development.

const isoTimestamp = z.string().describe("ISO-8601 timestamp.");

const TaskSchema = z
  .object({
    id: z.string(),
    title: z.string(),
    description: z.string().optional(),
    type: z.enum(["task", "bug", "feature", "epic", "chore"]),
    priority: z.union([
      z.literal(0),
      z.literal(1),
      z.literal(2),
      z.literal(3),
      z.literal(4),
    ]),
    status: z.enum(["pending", "in_progress", "completed"]),
    parentId: z.string().optional(),
    slug: z.string().optional(),
    labels: z.array(z.string()),
    blocks: z.array(z.string()),
    blockedBy: z.array(z.string()),
    assignee: z.string().optional(),
    claimedAt: isoTimestamp.optional(),
    missionId: z.string().optional(),
    contractId: z.string().optional(),
    claimedAtCommit: z.string().optional(),
    lastActivityAt: isoTimestamp.optional(),
    closeReason: z.string().optional(),
    receipt: z
      .object({
        summary: z.string(),
        surprise: z.string().optional(),
        verifiedBy: z.array(z.string()).optional(),
        capturedAt: isoTimestamp,
      })
      .passthrough()
      .optional(),
    createdAt: isoTimestamp,
    updatedAt: isoTimestamp,
  })
  .passthrough()
  .describe("A maestro task.");

const WitnessLevelSchema = z.enum([
  "witnessed-by-maestro",
  "witnessed-by-ci",
  "agent-claimed-locally",
  "agent-claimed-and-not-reproducible",
]);

const EvidenceSchema = z
  .object({
    schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
    id: z.string(),
    task_id: z.string(),
    session_id: z.string().optional(),
    kind: z.string(),
    witness_level: WitnessLevelSchema,
    created_at: isoTimestamp,
    payload: z.record(z.unknown()).describe("Kind-specific payload; shape varies by `kind`."),
  })
  .passthrough()
  .describe("A maestro evidence row.");

const RiskClassSchema = z.enum(["low", "medium", "high", "critical"]);

const VerdictDecisionSchema = z.enum(["PASS", "FAIL", "HUMAN", "BLOCK"]);

const VerdictSchema = z
  .object({
    schemaVersion: z.literal(1),
    id: z.string(),
    taskId: z.string(),
    subject: z
      .object({
        pr: z.number().optional(),
        tree_sha: z.string(),
      })
      .passthrough()
      .optional(),
    contractVersion: z.number(),
    computedAt: isoTimestamp,
    decision: VerdictDecisionSchema,
    proposedRiskClass: RiskClassSchema.optional(),
    effectiveRiskClass: RiskClassSchema,
    reasons: z.array(z.record(z.unknown())),
    evidenceConsulted: z.array(z.string()),
    policiesConsulted: z.array(
      z
        .object({ file: z.string(), version: z.string() })
        .passthrough(),
    ),
    trustVerifier: z
      .object({
        findingsCount: z.number(),
        errors: z.number(),
        warns: z.number(),
        infos: z.number(),
      })
      .passthrough(),
  })
  .passthrough()
  .describe("A maestro verdict.");

const ContractScopeSchema = z
  .object({
    filesExpected: z.array(z.string()),
    filesForbidden: z.array(z.string()),
    maxFilesTouched: z.number().optional(),
  })
  .passthrough();

const ContractSchema = z
  .object({
    schemaVersion: z.union([z.literal(1), z.literal(2)]),
    id: z.string(),
    taskId: z.string(),
    repoRoot: z.string(),
    status: z.enum(["draft", "locked", "amended", "fulfilled", "broken", "discarded"]),
    createdAt: isoTimestamp,
    intent: z.string(),
    scope: ContractScopeSchema,
    doneWhen: z.array(z.record(z.unknown())),
    amendments: z.array(z.record(z.unknown())),
    createdBy: z.string(),
    configSnapshot: z.record(z.unknown()),
    riskClass: RiskClassSchema.optional(),
  })
  .passthrough()
  .describe("A maestro contract version.");

const PaginationSchema = z
  .object({
    total: z.number().int().min(0),
    limit: z.number().int().min(1),
    offset: z.number().int().min(0),
    hasMore: z.boolean(),
  })
  .strict()
  .describe("Pagination metadata for list responses.");

// ---- Tool output schemas ----

export const TaskListOutput = z
  .object({
    items: z.array(TaskSchema),
    pagination: PaginationSchema,
  })
  .strict();

export const TaskOutput = z
  .object({
    task: TaskSchema,
    autoClaimed: z.boolean().optional(),
  })
  .strict();

export const EvidenceListOutput = z
  .object({
    items: z.array(EvidenceSchema),
    pagination: PaginationSchema,
  })
  .strict();

export const EvidenceRecordOutput = z
  .object({
    evidence: EvidenceSchema,
  })
  .strict();

export const VerdictOutput = z
  .object({
    verdict: VerdictSchema,
  })
  .strict();

export const ContractShowOutput = z
  .object({
    contract: ContractSchema,
  })
  .strict();

export const ContractAmendOutput = z
  .object({
    amendmentId: z.string(),
    newVersion: z.number().int().min(1),
    skippedAddPaths: z.array(z.string()),
  })
  .strict();

export const PolicyCheckOutput = z
  .object({
    taskId: z.string(),
    contractRiskClass: RiskClassSchema,
    derivedRiskClass: RiskClassSchema,
    effectiveRiskClass: RiskClassSchema,
    matchedRiskPolicyRow: z
      .object({
        signal: z.string(),
        description: z.string().optional(),
      })
      .passthrough()
      .nullable(),
    autoMergeAllowed: z.boolean(),
    requiredWitnessLevel: WitnessLevelSchema,
    releaseRules: z
      .object({
        requireSignedCommits: z.boolean(),
        requireProofMapComplete: z.boolean(),
      })
      .passthrough(),
    sensitivePaths: z
      .object({
        globs: z.array(z.string()),
        matchedPaths: z.array(z.string()),
      })
      .passthrough(),
  })
  .strict();
