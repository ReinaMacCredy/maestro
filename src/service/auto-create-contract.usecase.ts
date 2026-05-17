import { readFile } from "node:fs/promises";
import { parseSpecFile } from "../repo/fs-spec-store.adapter.js";
import {
  generateContractId,
  generateDoneWhenId,
} from "@/shared/domain/legacy-task/domain/contract/contract-state.js";
import {
  CONTRACT_SCHEMA_VERSION,
  type Contract,
} from "@/shared/domain/legacy-task/domain/contract/contract-types.js";
import type {
  ContractStorePort,
  ContractVersionStorePort,
} from "@/shared/domain/legacy-task/index.js";
import type { TaskId } from "../types/task.js";

export interface AutoCreateContractDeps {
  readonly repoRoot: string;
  readonly contractStore: ContractStorePort;
  readonly contractVersionStore: ContractVersionStorePort;
  readonly clock?: () => Date;
}

export interface AutoCreateContractInput {
  readonly taskId: TaskId;
  readonly specPath: string;
  readonly title: string;
  readonly agentId?: string;
  readonly missionId?: string;
  readonly riskClass?: "low" | "medium" | "high" | "critical";
}

// Synthesize a locked contract for a v2 task at claim time so that downstream
// verdict requests find a contract without the agent running a separate
// `task contract new`/`lock` flow. v2 has no agent-facing contract verbs;
// contracts are internal to the trust substrate.
export async function autoCreateContract(
  deps: AutoCreateContractDeps,
  input: AutoCreateContractInput,
): Promise<Contract | undefined> {
  const existing = await deps.contractStore.getByTaskId(input.taskId);
  if (existing && existing.status !== "discarded") {
    return existing;
  }

  let raw: string;
  try {
    raw = await readFile(input.specPath, "utf8");
  } catch {
    return undefined;
  }
  const spec = parseSpecFile(raw, input.specPath);
  const criteria = spec.frontmatter.acceptance_criteria;
  if (criteria.length === 0) return undefined;

  const now = (deps.clock ?? (() => new Date()))().toISOString();
  const draft: Contract = {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: generateContractId(),
    taskId: input.taskId,
    repoRoot: deps.repoRoot,
    status: "locked",
    createdAt: now,
    lockedAt: now,
    intent: input.title.trim().length > 0 ? input.title.trim() : spec.frontmatter.slug,
    scope: {
      filesExpected: ["**/*"],
      filesForbidden: [],
    },
    doneWhen: criteria.map((text) => ({
      id: generateDoneWhenId(),
      text,
      kind: "manual" as const,
    })),
    amendments: [],
    createdBy: input.agentId ?? "maestro",
    lockedBy: input.agentId ?? "maestro",
    configSnapshot: {
      strict: false,
      overlapPolicy: "annotate",
      rebaseFallback: "best-effort",
      staleReclaimContractPolicy: "inherit",
    },
    ...(input.missionId ? { missionId: input.missionId } : {}),
    ...(input.riskClass ? { riskClass: input.riskClass } : {}),
  };

  const saved = await deps.contractStore.save(draft);
  await deps.contractVersionStore.write(saved.taskId, 1, saved);
  return saved;
}
