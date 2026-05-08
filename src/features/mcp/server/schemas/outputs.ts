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

// Evidence payloads — one schema per `kind`. Used as a discriminated union so
// clients can pattern-match without a runtime guard. Each payload object stays
// `.passthrough()` so future fields don't break older clients.
const SeveritySchema = z.enum(["info", "warn", "error"]);

const evidencePayloadByKind = {
  command: z
    .object({
      command: z.string(),
      exit: z.number().int(),
      log_path: z.string().optional(),
      duration_ms: z.number().optional(),
      criterion_id: z.string().optional(),
    })
    .passthrough(),
  "manual-note": z
    .object({
      note: z.string(),
      criterion_id: z.string().optional(),
    })
    .passthrough(),
  verifier: z
    .object({
      check: z.string(),
      severity: SeveritySchema,
      paths: z.array(z.string()),
      details: z.string().optional(),
    })
    .passthrough(),
  "contract-amendment": z
    .object({
      amendmentId: z.string(),
      addedPaths: z.array(z.string()),
      removedPaths: z.array(z.string()),
      reason: z.string(),
    })
    .passthrough(),
  "contract-amendment-blocked": z
    .object({
      reason: z.enum(["budget_exhausted", "forbidden_path", "validation"]),
      attemptedPaths: z.array(z.string()),
      details: z.string().optional(),
    })
    .passthrough(),
  "ai-review": z
    .object({
      reviewer: z.enum(["bug", "security", "architecture"]),
      findings: z.array(
        z
          .object({
            severity: SeveritySchema,
            message: z.string(),
            paths: z.array(z.string()).optional(),
            suggestion: z.string().optional(),
          })
          .passthrough(),
      ),
      confidence: z.number(),
      criterion_id: z.string().optional(),
    })
    .passthrough(),
  "plan-check": z
    .object({
      planFileSha: z.string(),
      findings: z.array(
        z
          .object({
            check: z.string(),
            severity: SeveritySchema,
            message: z.string(),
          })
          .passthrough(),
      ),
      errorCount: z.number().int(),
      warnCount: z.number().int(),
    })
    .passthrough(),
  "threat-model": z
    .object({
      assets: z.array(z.string()),
      threatCategories: z.array(z.string()),
      mitigations: z.array(
        z
          .object({ threat: z.string(), mitigation: z.string() })
          .passthrough(),
      ),
      residualRisk: z.enum(["low", "medium", "high"]),
      criterion_id: z.string().optional(),
      source_file: z.string().optional(),
    })
    .passthrough(),
  "review-ack": z
    .object({
      verdictId: z.string(),
      ackedBy: z.string(),
      criteria: z.array(z.string()),
    })
    .passthrough(),
  "rollback-exercised": z
    .object({
      command: z.string(),
      exit: z.number().int(),
    })
    .passthrough(),
  "verdict-override": z
    .object({
      verdictId: z.string(),
      overriddenBy: z.string(),
      reason: z.string(),
    })
    .passthrough(),
  "runtime-signal": z
    .object({
      signal_name: z.string(),
      provider: z.string(),
      query: z.string(),
      value: z.number(),
      threshold: z.number(),
      operator: z.string(),
      pass: z.boolean(),
      sampled_at: isoTimestamp,
      note: z.string().optional(),
    })
    .passthrough(),
  "deploy-readiness": z
    .object({
      task_id: z.string(),
      checks: z
        .object({
          feature_flag: z.object({ ok: z.boolean(), value: z.string().optional() }).passthrough(),
          canary_plan: z.object({ ok: z.boolean(), stages: z.number().optional() }).passthrough(),
          rollback: z
            .object({ ok: z.boolean(), witness_evidence_id: z.string().optional() })
            .passthrough(),
          owner: z
            .object({ ok: z.boolean(), approvers: z.array(z.string()).optional() })
            .passthrough(),
        })
        .passthrough(),
      gate: z.enum(["pass", "fail"]),
    })
    .passthrough(),
  "cross-task-conflict": z
    .object({
      thisPr: z.number().int(),
      conflictingPrs: z.array(z.number().int()),
      overlappingPaths: z.array(z.string()),
    })
    .passthrough(),
} as const;

const EvidenceSchema = z
  .discriminatedUnion("kind", [
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("command"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind.command,
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("manual-note"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["manual-note"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("verifier"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind.verifier,
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("contract-amendment"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["contract-amendment"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("contract-amendment-blocked"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["contract-amendment-blocked"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("ai-review"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["ai-review"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("plan-check"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["plan-check"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("threat-model"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["threat-model"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("review-ack"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["review-ack"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("rollback-exercised"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["rollback-exercised"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("verdict-override"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["verdict-override"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("runtime-signal"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["runtime-signal"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("deploy-readiness"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["deploy-readiness"],
    }).passthrough(),
    z.object({
      schema_version: z.union([z.literal(1), z.literal(2), z.literal(3)]),
      id: z.string(),
      task_id: z.string(),
      session_id: z.string().optional(),
      kind: z.literal("cross-task-conflict"),
      witness_level: WitnessLevelSchema,
      created_at: isoTimestamp,
      payload: evidencePayloadByKind["cross-task-conflict"],
    }).passthrough(),
  ])
  .describe(
    "A maestro evidence row, discriminated by `kind`. Each kind carries a typed payload; passthrough on the inner objects keeps it forward-compatible.",
  );

const RiskClassSchema = z.enum(["low", "medium", "high", "critical"]);

const VerdictDecisionSchema = z.enum(["PASS", "FAIL", "HUMAN", "BLOCK"]);

const VerdictCategorySchema = z.enum([
  "trust",
  "evidence",
  "policy",
  "risk",
  "amendment",
  "cost-budget",
]);

const VerdictReasonCodeSchema = z.enum([
  "cost-budget-exhausted",
  "trust-findings-error",
  "amendment-budget-high",
  "effective-risk-critical",
  "evidence-witness-level-insufficient",
  "auto-merge-not-allowed",
  "all-checks-passed",
  "threat-model-required",
]);

const VerdictReasonSchema = z
  .object({
    category: VerdictCategorySchema,
    code: VerdictReasonCodeSchema,
    message: z.string(),
    evidenceIds: z.array(z.string()).optional(),
    findingChecks: z.array(z.string()).optional(),
    findingPaths: z.array(z.string()).optional(),
    policyRuleIds: z.array(z.string()).optional(),
  })
  .passthrough();

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
    reasons: z.array(VerdictReasonSchema),
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
