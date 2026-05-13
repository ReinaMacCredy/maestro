import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";
import type { ContractStorePort } from "../ports/contract-store.port.js";
import type { EvidenceStorePort } from "@/features/evidence";
import { recordEvidence } from "@/features/evidence";
import { getCurrentContract } from "./get-current-contract.usecase.js";
import { getContractHistory } from "./get-contract-history.usecase.js";
import type { Contract } from "../domain/contract/contract-types.js";

export interface ContractSprintDeps {
  readonly contractVersionStore: ContractVersionStorePort;
  readonly contractStore: ContractStorePort;
  readonly evidenceStore: EvidenceStorePort;
}

export interface ContractSprintArgs {
  readonly taskId: string;
  readonly propose?: string;
  readonly proposedBy?: string;
}

export interface SprintSnapshot {
  readonly taskId: string;
  readonly contractId?: string;
  readonly status?: Contract["status"];
  readonly criteriaCount: number;
  readonly metCount: number;
  readonly amendmentBudget?: {
    readonly maxAmendments: number;
    readonly used: number;
    readonly remaining: number;
  };
  readonly recentAmendments: readonly RecentAmendment[];
}

export interface RecentAmendment {
  readonly id: string;
  readonly at: string;
  readonly addedPathsCount: number;
  readonly removedPathsCount: number;
  readonly reason: string;
}

export interface ContractSprintResult {
  readonly snapshot: SprintSnapshot;
  readonly proposalRecorded?: {
    readonly evidenceId: string;
    readonly proposal: string;
  };
}

export async function contractSprint(
  deps: ContractSprintDeps,
  args: ContractSprintArgs,
): Promise<ContractSprintResult> {
  const current = await getCurrentContract(
    deps.contractVersionStore,
    deps.contractStore,
    args.taskId,
  );
  const history = await getContractHistory(
    deps.contractVersionStore,
    deps.contractStore,
    args.taskId,
  );

  const allAmendments: RecentAmendment[] = history
    .flatMap((c) => c.amendments)
    .sort((a, b) => b.at.localeCompare(a.at))
    .slice(0, 5)
    .map((a) => {
      const before = new Set(a.before.scope?.filesExpected ?? []);
      const after = new Set(a.after.scope?.filesExpected ?? []);
      let added = 0;
      let removed = 0;
      for (const p of after) if (!before.has(p)) added++;
      for (const p of before) if (!after.has(p)) removed++;
      return {
        id: a.id,
        at: a.at,
        addedPathsCount: added,
        removedPathsCount: removed,
        reason: a.reason,
      };
    });

  const usedAmendments = current ? current.amendments.length : 0;
  const max = current?.amendmentBudget?.maxAmendments;

  const criteria = current?.doneWhen ?? [];
  const met = criteria.filter((c) => c.met).length;

  const snapshot: SprintSnapshot = {
    taskId: args.taskId,
    contractId: current?.id,
    status: current?.status,
    criteriaCount: criteria.length,
    metCount: met,
    amendmentBudget:
      typeof max === "number"
        ? { maxAmendments: max, used: usedAmendments, remaining: Math.max(0, max - usedAmendments) }
        : undefined,
    recentAmendments: allAmendments,
  };

  if (args.propose === undefined || args.propose.trim().length === 0) {
    return { snapshot };
  }

  const proposal = args.propose.trim();
  const ev = await recordEvidence(deps.evidenceStore, {
    task_id: args.taskId,
    kind: "manual-note",
    witness_level: "agent-claimed-and-not-reproducible",
    payload: {
      note: `[contract-sprint-proposal] ${proposal}${args.proposedBy ? ` (proposed by ${args.proposedBy})` : ""}`,
    },
  });

  return {
    snapshot,
    proposalRecorded: { evidenceId: ev.id, proposal },
  };
}

export function formatContractSprintLines(r: ContractSprintResult): string[] {
  const lines: string[] = [];
  const s = r.snapshot;
  lines.push(`Contract sprint for ${s.taskId}`);
  if (!s.contractId) {
    lines.push(`  No active contract`);
    return lines;
  }
  lines.push(`  Contract: ${s.contractId} (${s.status})`);
  lines.push(`  Criteria: ${s.metCount}/${s.criteriaCount} met`);
  if (s.amendmentBudget) {
    const b = s.amendmentBudget;
    lines.push(`  Amendment budget: ${b.used}/${b.maxAmendments} used (${b.remaining} remaining)`);
  }
  if (s.recentAmendments.length > 0) {
    lines.push(`  Recent amendments:`);
    for (const a of s.recentAmendments) {
      lines.push(`    ${a.at} +${a.addedPathsCount}/-${a.removedPathsCount}: ${a.reason}`);
    }
  }
  if (r.proposalRecorded) {
    lines.push("");
    lines.push(`  Proposal recorded as evidence ${r.proposalRecorded.evidenceId}:`);
    lines.push(`    "${r.proposalRecorded.proposal}"`);
    lines.push(`  Run \`maestro contract amend\` to apply if approved.`);
  }
  return lines;
}
