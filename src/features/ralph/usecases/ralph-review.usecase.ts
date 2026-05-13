import { createHash } from "node:crypto";
import { recordEvidence, type EvidenceStorePort } from "@/features/evidence";
import {
  checkArchitectureRules,
  type ArchitectureViolation,
} from "@/features/verify";

export type RalphFindingSource = "trust-verifier" | "ai-review" | "lint-arch" | "threat-model";

export interface RalphFinding {
  readonly source: RalphFindingSource;
  readonly check: string;
  readonly severity: "info" | "warn" | "error";
  readonly message: string;
  readonly paths?: readonly string[];
}

export interface RalphReviewDeps {
  readonly evidenceStore: EvidenceStorePort;
  readonly listAiReviewFindings?: (taskId: string) => Promise<readonly RalphFinding[]>;
  readonly listThreatModelFindings?: (taskId: string) => Promise<readonly RalphFinding[]>;
  readonly listVerifierFindings?: (taskId: string) => Promise<readonly RalphFinding[]>;
  readonly previousIterations?: () => Promise<readonly { iteration: number; findingsHash: string }[]>;
}

export interface RalphReviewArgs {
  readonly taskId: string;
  readonly projectRoot: string;
  readonly stuckThreshold?: number;
}

export interface RalphReviewResult {
  readonly iteration: number;
  readonly findings: readonly RalphFinding[];
  readonly findingsHash: string;
  readonly stuck: boolean;
  readonly converged: boolean;
  readonly sources: readonly RalphFindingSource[];
  readonly evidenceId?: string;
}

const DEFAULT_STUCK_THRESHOLD = 3;

export async function ralphReview(
  deps: RalphReviewDeps,
  args: RalphReviewArgs,
): Promise<RalphReviewResult> {
  const [violations, verifier, ai, threat, previous] = await Promise.all([
    checkArchitectureRules({ repoRoot: args.projectRoot }),
    deps.listVerifierFindings ? deps.listVerifierFindings(args.taskId) : Promise.resolve([]),
    deps.listAiReviewFindings ? deps.listAiReviewFindings(args.taskId) : Promise.resolve([]),
    deps.listThreatModelFindings ? deps.listThreatModelFindings(args.taskId) : Promise.resolve([]),
    deps.previousIterations ? deps.previousIterations() : Promise.resolve([]),
  ]);

  const findings: RalphFinding[] = [
    ...violations.map(architectureViolationToFinding),
    ...verifier,
    ...ai,
    ...threat,
  ];

  const sources: RalphFindingSource[] = [];
  if (violations.length > 0) sources.push("lint-arch");
  if (verifier.length > 0) sources.push("trust-verifier");
  if (ai.length > 0) sources.push("ai-review");
  if (threat.length > 0) sources.push("threat-model");

  const findingsHash = hashFindings(findings);
  const iteration = previous.length + 1;
  const threshold = args.stuckThreshold ?? DEFAULT_STUCK_THRESHOLD;
  const stuck =
    previous.length >= threshold - 1 &&
    previous.slice(-(threshold - 1)).every((p) => p.findingsHash === findingsHash);
  const converged = !findings.some((f) => f.severity === "error");

  const row = await recordEvidence(deps.evidenceStore, {
    task_id: args.taskId,
    kind: "ralph-iteration",
    witness_level: "witnessed-by-maestro",
    payload: {
      iteration,
      findingsHash,
      findingsCount: findings.length,
      stuck,
      sources,
    },
  });

  return {
    iteration,
    findings,
    findingsHash,
    stuck,
    converged,
    sources,
    evidenceId: row.id,
  };
}

function architectureViolationToFinding(v: ArchitectureViolation): RalphFinding {
  return {
    source: "lint-arch",
    check: v.ruleId,
    severity: v.severity,
    message: v.message,
    paths: v.file ? [v.file] : undefined,
  };
}

function hashFindings(findings: readonly RalphFinding[]): string {
  const normalized = findings
    .map((f) => `${f.source}|${f.check}|${f.severity}|${(f.paths ?? []).join(",")}|${f.message}`)
    .sort();
  return createHash("sha256").update(normalized.join("\n")).digest("hex").slice(0, 16);
}
