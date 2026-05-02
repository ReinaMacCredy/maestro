import { MaestroError } from "@/shared/errors.js";
import {
  buildActiveOverlapError,
  canAmendContract,
  canCloseContract,
  canDiscardContract,
  canEditContract,
  canReopenContract,
  CONTRACT_ID_PATTERN,
  generateContractAmendmentId,
  generateDoneWhenId,
  isActiveContract,
  isContractLockable,
  normalizeStoredContractRepoRoot,
  snapshotForAmendment,
} from "../domain/contract/contract-state.js";
import type {
  Contract,
  ContractConfigSnapshot,
  ContractScope,
  ContractStatus,
  ContractVerdict,
  DoneWhenCriterion,
} from "../domain/contract/contract-types.js";
import {
  computeContractVerdict,
  type ComputedContractVerdict,
} from "../domain/contract/verdict.js";
import { isTaskId } from "../domain/task-id.js";
import { taskAlreadyCompleted, taskNotFound } from "../domain/task-errors.js";
import type { Task, TaskReceipt } from "../domain/task-types.js";
import type {
  ContractStorePort,
  ContractStoreQueryPort,
} from "../ports/contract-store.port.js";
import type { GitAnchorPort } from "../ports/git-anchor.port.js";
import type { TaskStorePort } from "../ports/task-store.port.js";

export interface ContractCriterionDraftInput {
  readonly id?: string;
  readonly text: string;
  readonly kind?: DoneWhenCriterion["kind"];
}

export interface CreateContractInput {
  readonly taskId: string;
  readonly repoRoot: string;
  readonly intent: string;
  readonly scope: ContractScope;
  readonly doneWhen: readonly Array<{
    readonly text: string;
    readonly kind?: DoneWhenCriterion["kind"];
  }>;
  readonly createdBy: string;
  readonly configSnapshot: ContractConfigSnapshot;
}

export interface EditContractInput {
  readonly ref: string;
  readonly intent: string;
  readonly scope: ContractScope;
  readonly doneWhen: readonly Array<{
    readonly id?: string;
    readonly text: string;
    readonly kind?: DoneWhenCriterion["kind"];
  }>;
}

export interface LockContractInput {
  readonly ref: string;
  readonly actorId: string;
  readonly claimedAtCommit?: string;
  readonly configSnapshot: ContractConfigSnapshot;
}

export interface ListContractsFilters {
  readonly status?: ContractStatus;
  readonly taskId?: string;
}

export type ContractAmendmentCommand =
  | {
    readonly kind: "replace";
    readonly ref: string;
    readonly actorId: string;
    readonly reason: string;
    readonly intent: string;
    readonly scope: ContractScope;
    readonly doneWhen: readonly ContractCriterionDraftInput[];
  }
  | {
    readonly kind: "addCriterion";
    readonly ref: string;
    readonly actorId: string;
    readonly text: string;
  }
  | {
    readonly kind: "removeCriterion";
    readonly ref: string;
    readonly actorId: string;
    readonly criterionId: string;
  }
  | {
    readonly kind: "markCriterion";
    readonly ref: string;
    readonly actorId: string;
    readonly criterionId: string;
    readonly met?: boolean;
    readonly evidence?: string;
  };

export interface ContractVerdictInput {
  readonly contract: Contract;
  readonly task: Pick<Task, "assignee" | "receipt" | "updatedAt">;
  readonly receiptOverride?: TaskReceipt;
  readonly runtimeRepoRoot?: string;
}

export interface ContractWorkflows {
  load(ref: string): Promise<Contract>;
  list(filters?: ListContractsFilters): Promise<readonly Contract[]>;
  draft(input: CreateContractInput): Promise<Contract>;
  editDraft(input: EditContractInput): Promise<Contract>;
  discard(ref: string): Promise<Contract>;
  lock(input: LockContractInput): Promise<Contract>;
  amend(input: ContractAmendmentCommand): Promise<Contract>;
  previewVerdict(input: ContractVerdictInput): Promise<ComputedContractVerdict & { readonly closedAtCommit?: string }>;
  closeForTask(task: Task, runtimeRepoRoot: string): Promise<Contract | undefined>;
  prepareReopen(task: Pick<Task, "id" | "contractId">): Promise<Contract | undefined>;
  reopenForTask(task: Pick<Task, "id" | "contractId">, loaded?: Contract): Promise<Contract | undefined>;
  transferOwnership(
    taskId: string,
    newActor: string,
    reason?: "claim_reclaim" | "handoff_pickup",
  ): Promise<Contract | undefined>;
}

export function buildContractWorkflows(
  contractStore: ContractStorePort,
  taskStore: TaskStorePort,
  gitAnchor: GitAnchorPort,
): ContractWorkflows {
  return {
    load: (ref) => resolveContractRef(contractStore, ref),

    list: (filters = {}) => listContracts(contractStore, filters),

    draft: (input) => createContract(taskStore, contractStore, input),

    editDraft: (input) => editContract(contractStore, input),

    discard: (ref) => discardContract(taskStore, contractStore, ref),

    lock: (input) => lockContract(contractStore, input),

    amend: (input) => amendContract(contractStore, input),

    previewVerdict: (input) => computeContractVerdictForTask(
      contractStore,
      gitAnchor,
      input.contract,
      input.task,
      input.receiptOverride,
      input.runtimeRepoRoot,
    ),

    closeForTask: (task, runtimeRepoRoot) => closeContractForTask(
      contractStore,
      gitAnchor,
      task,
      runtimeRepoRoot,
    ),

    prepareReopen: (task) => loadContractForReopen(contractStore, gitAnchor, task),

    reopenForTask: (task, loaded) => reopenContractForTask(contractStore, gitAnchor, task, loaded),

    transferOwnership: (taskId, newActor, reason = "claim_reclaim") =>
      transferContractOwnership(contractStore, taskId, newActor, reason),
  };
}

async function createContract(
  taskStore: TaskStorePort,
  contractStore: ContractStorePort,
  input: CreateContractInput,
): Promise<Contract> {
  const task = await taskStore.get(input.taskId);
  if (!task) {
    throw taskNotFound(input.taskId);
  }
  if (task.status === "completed") {
    throw taskAlreadyCompleted(task.id);
  }
  if (task.contractId) {
    const linked = await contractStore.get(task.contractId);
    if (linked?.status === "discarded") {
      await taskStore.syncMetadata(task.id, { contractId: null });
    } else {
      throw new MaestroError(`Task ${task.id} already has a contract: ${task.contractId}`, [
        `Show it: maestro task contract show ${task.id}`,
        "Discard the draft first if you need to stop using it",
      ]);
    }
  }

  const contract = await contractStore.create({
    taskId: input.taskId,
    repoRoot: normalizeStoredContractRepoRoot(input.repoRoot),
    createdAt: new Date().toISOString(),
    intent: input.intent.trim(),
    scope: normalizeScope(input.scope),
    doneWhen: input.doneWhen.map((criterion) => ({
      id: generateDoneWhenId(),
      text: criterion.text.trim(),
      kind: criterion.kind ?? "manual",
    })),
    createdBy: input.createdBy,
    configSnapshot: input.configSnapshot,
  });

  try {
    await taskStore.syncMetadata(task.id, { contractId: contract.id });
  } catch (linkError) {
    try {
      await contractStore.delete(contract.id, {
        taskId: contract.taskId,
        at: new Date().toISOString(),
        reason: "task_link_failed",
      });
    } catch (rollbackError) {
      if (linkError instanceof Error) {
        Object.defineProperty(linkError, "rollbackError", {
          value: rollbackError,
          enumerable: false,
          configurable: true,
        });
      }
    }
    throw linkError;
  }

  return contract;
}

async function editContract(
  contractStore: ContractStorePort,
  input: EditContractInput,
): Promise<Contract> {
  const contract = await resolveContractRef(contractStore, input.ref);
  if (!canEditContract(contract)) {
    throw new MaestroError(`Contract ${contract.id} cannot be edited from status '${contract.status}'`, [
      "Only draft contracts can be edited directly",
      `Use 'maestro task contract amend ${contract.id} --reason "..."' once the contract is locked`,
    ]);
  }

  return contractStore.save({
    ...contract,
    intent: input.intent.trim(),
    scope: normalizeScope(input.scope),
    doneWhen: normalizeAmendedCriteria(contract.doneWhen, input.doneWhen),
  });
}

async function discardContract(
  taskStore: TaskStorePort,
  contractStore: ContractStorePort,
  ref: string,
): Promise<Contract> {
  const contract = await resolveContractRef(contractStore, ref);
  if (!canDiscardContract(contract)) {
    throw new MaestroError(`Contract ${contract.id} cannot be discarded from status '${contract.status}'`, [
      "Only draft contracts can be discarded",
      `Show the contract: maestro task contract show ${contract.id}`,
    ]);
  }

  const discarded = await contractStore.save({
    ...contract,
    status: "discarded",
    discardedAt: new Date().toISOString(),
  });

  try {
    await taskStore.syncMetadata(discarded.taskId, { contractId: null });
  } catch {
    // Future creates self-heal stale discarded links; discard itself should still succeed.
  }

  return discarded;
}

async function lockContract(
  contractStore: ContractStorePort,
  input: LockContractInput,
): Promise<Contract> {
  const contract = await resolveContractRef(contractStore, input.ref);
  if (!isContractLockable(contract)) {
    throw new MaestroError(`Contract ${contract.id} cannot be locked from status '${contract.status}'`, [
      "Draft contracts need a non-empty intent, at least one expected file glob, and at least one done-when criterion",
      `Show the draft: maestro task contract show ${contract.id}`,
    ]);
  }

  const now = new Date().toISOString();
  return contractStore.save({
    ...contract,
    status: "locked",
    lockedAt: now,
    lockedBy: input.actorId,
    claimedAtCommit: input.claimedAtCommit ?? contract.claimedAtCommit,
    configSnapshot: input.configSnapshot,
  });
}

async function amendContract(
  contractStore: ContractStorePort,
  input: ContractAmendmentCommand,
): Promise<Contract> {
  switch (input.kind) {
    case "replace":
      return replaceContract(contractStore, input);
    case "addCriterion":
      return addContractCriterion(contractStore, input);
    case "removeCriterion":
      return removeContractCriterion(contractStore, input);
    case "markCriterion":
      return markContractCriterion(contractStore, input);
  }
}

async function replaceContract(
  contractStore: ContractStorePort,
  input: Extract<ContractAmendmentCommand, { readonly kind: "replace" }>,
): Promise<Contract> {
  const contract = await resolveActiveContract(contractStore, input.ref);
  const nextIntent = input.intent.trim();
  const nextScope = normalizeScope(input.scope);
  const nextDoneWhen = normalizeAmendedCriteria(contract.doneWhen, input.doneWhen);

  return contractStore.save(
    withContractAmendment(contract, {
      actorId: input.actorId,
      reason: input.reason,
      intent: nextIntent,
      scope: nextScope,
      doneWhen: nextDoneWhen,
    }),
  );
}

async function addContractCriterion(
  contractStore: ContractStorePort,
  input: Extract<ContractAmendmentCommand, { readonly kind: "addCriterion" }>,
): Promise<Contract> {
  const contract = await resolveActiveContract(contractStore, input.ref);
  const text = input.text.trim();
  if (text.length === 0) {
    throw new MaestroError("Contract criteria need non-empty text");
  }

  const nextCriterion: DoneWhenCriterion = {
    id: generateDoneWhenId(),
    text,
    kind: "manual",
  };
  return contractStore.save(
    withContractAmendment(contract, {
      actorId: input.actorId,
      reason: `Added criterion ${nextCriterion.id}`,
      doneWhen: [...contract.doneWhen, nextCriterion],
    }),
  );
}

async function removeContractCriterion(
  contractStore: ContractStorePort,
  input: Extract<ContractAmendmentCommand, { readonly kind: "removeCriterion" }>,
): Promise<Contract> {
  const contract = await resolveActiveContract(contractStore, input.ref);
  const criterion = findCriterion(contract, input.criterionId);
  return contractStore.save(
    withContractAmendment(contract, {
      actorId: input.actorId,
      reason: `Removed criterion ${criterion.id}`,
      doneWhen: contract.doneWhen.filter((candidate) => candidate.id !== criterion.id),
    }),
  );
}

async function markContractCriterion(
  contractStore: ContractStorePort,
  input: Extract<ContractAmendmentCommand, { readonly kind: "markCriterion" }>,
): Promise<Contract> {
  const contract = await resolveActiveContract(contractStore, input.ref);
  const criterion = findCriterion(contract, input.criterionId);
  const met = input.met ?? true;
  const evidence = input.evidence?.trim();
  if (!met && evidence) {
    throw new MaestroError("--evidence only applies when marking a criterion met");
  }

  const at = new Date().toISOString();
  const nextCriterion = met
    ? {
        ...criterion,
        met: true,
        metAt: at,
        metBy: input.actorId,
        ...(evidence ? { metEvidence: evidence } : {}),
      }
    : {
        id: criterion.id,
        text: criterion.text,
        kind: criterion.kind,
      };

  return contractStore.save(
    withContractAmendment(contract, {
      actorId: input.actorId,
      reason: `Marked criterion ${criterion.id} ${met ? "met" : "unmet"}`,
      at,
      doneWhen: contract.doneWhen.map((candidate) => candidate.id === criterion.id ? nextCriterion : candidate),
    }),
  );
}

async function closeContractForTask(
  contractStore: ContractStorePort,
  gitAnchor: GitAnchorPort,
  task: Task,
  runtimeRepoRoot: string,
): Promise<Contract | undefined> {
  if (!task.contractId) {
    return undefined;
  }

  const contract = await contractStore.get(task.contractId);
  if (!contract) {
    throw new MaestroError(`Contract ${task.contractId} not found for task ${task.id}`, [
      "Inspect the contract index under .maestro/tasks/contracts/",
    ]);
  }
  if (contract.status === "discarded") {
    return contract;
  }
  if (contract.status === "fulfilled" || contract.status === "broken") {
    return contract;
  }
  if (!canCloseContract(contract)) {
    throw new MaestroError(`Contract ${contract.id} must be locked before task completion`, [
      `Lock it first: maestro task contract lock ${contract.id}`,
    ]);
  }

  const computed = await computeContractVerdictForTask(
    contractStore,
    gitAnchor,
    contract,
    task,
    undefined,
    runtimeRepoRoot,
  );
  const verdict = withOwnershipNotes(contract, computed.verdict);
  return contractStore.save({
    ...contract,
    status: verdict.fulfilled ? "fulfilled" : "broken",
    closedAt: task.updatedAt,
    closedAtCommit: computed.closedAtCommit,
    closedBy: task.assignee ?? contract.lockedBy ?? contract.createdBy,
    doneWhen: computed.criteria,
    verdict,
  });
}

async function loadContractForReopen(
  contractStore: ContractStorePort,
  gitAnchor: GitAnchorPort,
  task: Pick<Task, "id" | "contractId">,
): Promise<Contract | undefined> {
  if (!task.contractId) {
    return undefined;
  }

  const contract = await contractStore.get(task.contractId);
  if (!contract) {
    throw new MaestroError(`Contract ${task.contractId} not found for task ${task.id}`, [
      "Inspect the contract index under .maestro/tasks/contracts/",
    ]);
  }
  if (!canReopenContract(contract)) {
    return contract;
  }

  if (contract.configSnapshot.overlapPolicy === "fail") {
    if (!contract.claimedAtCommit || !contract.closedAtCommit) {
      const activeContractIds = await listOtherActiveContractIds(contractStore, contract.id);
      if (activeContractIds.length > 0) {
        throw buildActiveOverlapError(contract.id, activeContractIds);
      }
    } else {
      const overlap = await detectContractOverlap(
        contractStore,
        gitAnchor,
        contract,
        contract.closedAtCommit,
        contract.repoRoot,
        isActiveContract,
      );
      if (overlap?.otherContractIds.length) {
        throw buildActiveOverlapError(contract.id, overlap.otherContractIds);
      }
    }
  }

  return contract;
}

async function reopenContractForTask(
  contractStore: ContractStorePort,
  gitAnchor: GitAnchorPort,
  task: Pick<Task, "id" | "contractId">,
  loadedContract?: Contract,
): Promise<Contract | undefined> {
  const contract = loadedContract ?? await loadContractForReopen(contractStore, gitAnchor, task);
  if (!contract) {
    return undefined;
  }

  return reopenLoadedContract(contractStore, contract);
}

async function reopenLoadedContract(
  contractStore: ContractStorePort,
  contract: Contract,
): Promise<Contract> {
  if (!canReopenContract(contract)) {
    return contract;
  }

  return contractStore.save({
    ...contract,
    status: contract.amendments.length > 0 ? "amended" : "locked",
    closedAt: undefined,
    closedAtCommit: undefined,
    closedBy: undefined,
    verdict: undefined,
  });
}

async function transferContractOwnership(
  contractStore: ContractStorePort,
  taskId: string,
  newActor: string,
  reason: "claim_reclaim" | "handoff_pickup",
): Promise<Contract | undefined> {
  const contract = await contractStore.getByTaskId(taskId);
  if (!contract || !isActiveContract(contract) || contract.lockedBy === newActor) {
    return contract;
  }

  const shouldRecordHistory = contract.lockedBy !== undefined
    && !(contract.lockedBy === contract.createdBy && contract.createdBy === "user" && (contract.ownershipHistory?.length ?? 0) === 0);

  return contractStore.save({
    ...contract,
    lockedBy: newActor,
    ...(shouldRecordHistory
      ? {
          ownershipHistory: [
            ...(contract.ownershipHistory ?? []),
            {
              from: contract.lockedBy ?? contract.createdBy,
              to: newActor,
              at: new Date().toISOString(),
              reason,
            },
          ],
        }
      : {}),
  });
}

async function computeContractVerdictForTask(
  contractStore: ContractStoreQueryPort,
  gitAnchor: GitAnchorPort,
  contract: Contract,
  task: Pick<Task, "assignee" | "receipt" | "updatedAt">,
  receiptOverride?: TaskReceipt,
  runtimeRepoRoot?: string,
): Promise<ComputedContractVerdict & { readonly closedAtCommit?: string }> {
  const repoRoot = runtimeRepoRoot ?? contract.repoRoot;
  const gitResult = await gitAnchor.collectTouchedFiles({
    repoRoot,
    claimedAtCommit: contract.claimedAtCommit,
    rebaseFallback: contract.configSnapshot.rebaseFallback,
  });
  const at = task.updatedAt;
  const actorId = task.assignee ?? contract.lockedBy ?? contract.createdBy;
  const receipt = receiptOverride ?? task.receipt;
  const overlapDetected = await detectContractOverlap(
    contractStore,
    gitAnchor,
    contract,
    gitResult.closedAtCommit,
    repoRoot,
  );
  const computed = computeContractVerdict(contract, gitResult, receipt, actorId, at, {
    overlapDetected,
  });

  return {
    ...computed,
    closedAtCommit: gitResult.closedAtCommit,
  };
}

async function detectContractOverlap(
  contractStore: ContractStoreQueryPort,
  gitAnchor: GitAnchorPort,
  contract: Contract,
  currentClosedAtCommit: string | undefined,
  runtimeRepoRoot: string,
  includeCandidate?: (candidate: Contract) => boolean,
): Promise<ContractVerdict["overlapDetected"] | undefined> {
  if (!contract.claimedAtCommit || !currentClosedAtCommit) {
    return undefined;
  }

  const candidates = (await contractStore.all()).filter((candidate) =>
    candidate.id !== contract.id
    && (includeCandidate
      ? includeCandidate(candidate)
      : candidate.status !== "draft" && candidate.status !== "discarded"),
  );
  if (candidates.length === 0) {
    return undefined;
  }

  const overlapConcurrency = 4;
  const results: (string | undefined)[] = [];
  for (let i = 0; i < candidates.length; i += overlapConcurrency) {
    const chunk = candidates.slice(i, i + overlapConcurrency);
    const chunkResults = await Promise.all(chunk.map(async (candidate) => {
      const overlaps = await gitAnchor.windowsOverlap({
        repoRoot: runtimeRepoRoot,
        left: {
          claimedAtCommit: contract.claimedAtCommit,
          closedAtCommit: currentClosedAtCommit,
        },
        right: {
          claimedAtCommit: candidate.claimedAtCommit,
          closedAtCommit: candidate.closedAtCommit ?? currentClosedAtCommit,
        },
      });
      return overlaps ? candidate.id : undefined;
    }));
    results.push(...chunkResults);
  }
  const overlapping = results.filter((id): id is string => id !== undefined).sort();

  if (overlapping.length === 0) {
    return undefined;
  }

  return {
    otherContractIds: overlapping,
    policy: contract.configSnapshot.overlapPolicy,
  };
}

async function listOtherActiveContractIds(
  contractStore: ContractStoreQueryPort,
  contractId: string,
): Promise<readonly string[]> {
  return (await contractStore.all())
    .filter((candidate) => candidate.id !== contractId && isActiveContract(candidate))
    .map((candidate) => candidate.id)
    .sort();
}

async function listContracts(
  contractStore: ContractStoreQueryPort,
  filters: ListContractsFilters = {},
): Promise<readonly Contract[]> {
  const contracts = await contractStore.all();
  return contracts.filter((contract) => {
    if (filters.status !== undefined && contract.status !== filters.status) {
      return false;
    }
    if (filters.taskId !== undefined && contract.taskId !== filters.taskId) {
      return false;
    }
    return true;
  });
}

async function resolveContractRef(
  store: ContractStoreQueryPort,
  ref: string,
): Promise<Contract> {
  const contract = CONTRACT_ID_PATTERN.test(ref)
    ? await store.get(ref)
    : (isTaskId(ref) ? await store.getByTaskId(ref) : undefined);
  if (contract) {
    return contract;
  }

  const noun = CONTRACT_ID_PATTERN.test(ref) ? "Contract" : "Task contract";
  throw new MaestroError(`${noun} ${ref} not found`, [
    "List contracts: maestro task contract list",
    "Use a contract id (c-xxxxxx) or a task id (tsk-xxxxxx)",
  ]);
}

function withContractAmendment(
  contract: Contract,
  input: {
    readonly actorId: string;
    readonly reason: string;
    readonly intent?: string;
    readonly scope?: ContractScope;
    readonly doneWhen?: readonly DoneWhenCriterion[];
    readonly at?: string;
  },
): Contract {
  const reason = input.reason.trim();
  if (reason.length === 0) {
    throw new MaestroError("Contract amendments require a non-empty reason", [
      "Pass --reason \"why the contract changed\"",
    ]);
  }

  const at = input.at ?? new Date().toISOString();
  const nextIntent = input.intent ?? contract.intent;
  const nextScope = input.scope ?? contract.scope;
  const nextDoneWhen = input.doneWhen ?? contract.doneWhen;

  return {
    ...contract,
    status: "amended",
    intent: nextIntent,
    scope: nextScope,
    doneWhen: nextDoneWhen,
    amendments: [
      ...contract.amendments,
      {
        id: generateContractAmendmentId(),
        at,
        by: input.actorId,
        reason,
        before: snapshotForAmendment(contract),
        after: {
          intent: nextIntent,
          scope: nextScope,
          doneWhen: nextDoneWhen,
        },
      },
    ],
  };
}

function normalizeScope(scope: ContractScope): ContractScope {
  return {
    filesExpected: dedupe(scope.filesExpected),
    filesForbidden: dedupe(scope.filesForbidden),
    ...(scope.maxFilesTouched !== undefined ? { maxFilesTouched: scope.maxFilesTouched } : {}),
  };
}

function normalizeAmendedCriteria(
  current: readonly DoneWhenCriterion[],
  next: readonly ContractCriterionDraftInput[],
): readonly DoneWhenCriterion[] {
  return next.map((criterion) => {
    const text = criterion.text.trim();
    const existing = criterion.id
      ? current.find((candidate) => candidate.id === criterion.id)
      : undefined;
    const kind = criterion.kind ?? existing?.kind ?? "manual";

    if (!existing) {
      return {
        id: criterion.id ?? generateDoneWhenId(),
        text,
        kind,
      };
    }

    if (existing.text === text && existing.kind === kind) {
      return existing;
    }

    return {
      id: existing.id,
      text,
      kind,
    };
  });
}

async function resolveActiveContract(
  contractStore: ContractStorePort,
  ref: string,
): Promise<Contract> {
  const contract = await resolveContractRef(contractStore, ref);
  if (!canAmendContract(contract)) {
    throw new MaestroError(`Contract ${contract.id} cannot be modified from status '${contract.status}'`, [
      "Only locked or amended contracts accept amend/criteria changes",
      `Show the contract: maestro task contract show ${contract.id}`,
    ]);
  }
  return contract;
}

function findCriterion(contract: Contract, criterionId: string): DoneWhenCriterion {
  const criterion = contract.doneWhen.find((candidate) => candidate.id === criterionId);
  if (criterion) {
    return criterion;
  }
  throw new MaestroError(`Criterion ${criterionId} not found on contract ${contract.id}`, [
    `Show the contract: maestro task contract show ${contract.id}`,
  ]);
}

function withOwnershipNotes(contract: Contract, verdict: ContractVerdict): ContractVerdict {
  if (!contract.ownershipHistory || contract.ownershipHistory.length === 0) {
    return verdict;
  }

  const chain = contract.ownershipHistory
    .map((transfer) => `${transfer.from} -> ${transfer.to} (${transfer.reason})`)
    .join("; ");
  const notes = [verdict.notes, `Ownership chain: ${chain}.`]
    .filter((value): value is string => Boolean(value))
    .join(" ");

  return {
    ...verdict,
    notes,
  };
}

function dedupe(values: readonly string[]): readonly string[] {
  const next = values
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
  return Array.from(new Set(next));
}
