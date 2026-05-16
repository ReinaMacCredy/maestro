import type { Contract } from "../domain/contract/contract-types.js";
import type { Task } from "../domain/task-types.js";
import { extractKeywords } from "../domain/extract-keywords.js";
import type { ContractStoreQueryPort } from "../ports/contract-store.port.js";
import type { TaskQueryPort } from "../ports/task-store.port.js";
import { invalidSimilarTaskLimit, taskNotFound } from "../domain/task-errors.js";

export interface SimilarTaskMatch {
  readonly task: Task;
  readonly overlap: number;
  readonly matchedKeywords: readonly string[];
}

const DEFAULT_LIMIT = 5;

export async function findSimilarTasks(
  store: TaskQueryPort,
  targetId: string,
  limit: number = DEFAULT_LIMIT,
  contractStore?: ContractStoreQueryPort,
): Promise<readonly SimilarTaskMatch[]> {
  if (limit < 0) {
    throw invalidSimilarTaskLimit(limit);
  }

  const all = await store.all();
  const target = all.find((task) => task.id === targetId);
  if (!target) {
    throw taskNotFound(targetId);
  }

  const contractsById = contractStore ? await loadContractsById(contractStore) : EMPTY_CONTRACTS;
  const targetKeywords = tokensFor(target, contractsById);
  if (targetKeywords.size === 0) return [];

  const scored: SimilarTaskMatch[] = [];
  for (const task of all) {
    if (task.id === target.id) continue;
    const otherKeywords = tokensFor(task, contractsById);
    if (otherKeywords.size === 0) continue;

    const matched: string[] = [];
    for (const kw of otherKeywords) {
      if (targetKeywords.has(kw)) {
        matched.push(kw);
      }
    }
    if (matched.length === 0) continue;

    scored.push({
      task,
      overlap: matched.length,
      matchedKeywords: matched,
    });
  }

  scored.sort((a, b) => {
    if (a.overlap !== b.overlap) return b.overlap - a.overlap;
    return b.task.updatedAt.localeCompare(a.task.updatedAt);
  });

  return limit === 0 ? scored : scored.slice(0, limit);
}

const EMPTY_CONTRACTS = new Map<string, Contract>();

async function loadContractsById(contractStore: ContractStoreQueryPort): Promise<ReadonlyMap<string, Contract>> {
  const contracts = await contractStore.all();
  return new Map(contracts.map((contract) => [contract.id, contract] as const));
}

function tokensFor(task: Task, contractsById: ReadonlyMap<string, Contract>): ReadonlySet<string> {
  const parts: string[] = [task.title];
  if (task.closeReason) parts.push(task.closeReason);
  if (task.receipt?.summary) parts.push(task.receipt.summary);
  if (task.receipt?.surprise) parts.push(task.receipt.surprise);
  const contract = task.contractId ? contractsById.get(task.contractId) : undefined;
  if (contract) {
    parts.push(contract.intent);
    for (const criterion of contract.doneWhen) {
      parts.push(criterion.text);
    }
  }
  return new Set(extractKeywords(parts.join(" ")));
}
