import type { Spec } from "@/features/spec/index.js";
import type { EvidenceRow, EvidenceKind, WitnessLevel } from "@/features/evidence/index.js";

export interface ProofMapEvidence {
  readonly id: string;
  readonly kind: EvidenceKind;
  readonly witnessLevel: WitnessLevel;
  readonly createdAt: string;
}

export interface ProofMapEntry {
  readonly criterionId: string;
  readonly criterionText: string;
  readonly evidence: readonly ProofMapEvidence[];
  readonly covered: boolean;
}

export interface ProofMap {
  readonly taskId: string;
  readonly missionId?: string;
  readonly entries: readonly ProofMapEntry[];
  readonly uncoveredCount: number;
}

export function buildProofMap(args: {
  readonly taskId: string;
  readonly spec: Spec | undefined;
  readonly evidenceRows: readonly EvidenceRow[];
}): ProofMap {
  if (!args.spec) {
    return { taskId: args.taskId, entries: [], uncoveredCount: 0 };
  }
  const entries: ProofMapEntry[] = args.spec.acceptance_criteria.map((c) => {
    const evidence = args.evidenceRows
      .filter((row) => extractCriterionId(row) === c.id)
      .map((row) => ({
        id: row.id,
        kind: row.kind,
        witnessLevel: row.witness_level,
        createdAt: row.created_at,
      }));
    return {
      criterionId: c.id,
      criterionText: c.text,
      evidence,
      covered: evidence.length > 0,
    };
  });
  const uncoveredCount = entries.filter((e) => !e.covered).length;
  return {
    taskId: args.taskId,
    missionId: args.spec.mission_id,
    entries,
    uncoveredCount,
  };
}

function extractCriterionId(row: EvidenceRow): string | undefined {
  if (row.kind === "command" || row.kind === "manual-note") {
    return (row.payload as { criterion_id?: string }).criterion_id;
  }
  return undefined;
}
