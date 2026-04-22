import { MaestroError } from "@/shared/errors.js";
import { TASK_ID_PATTERN } from "../task-id.js";
import { generateTaskId } from "../task-id.js";
import type {
  AmendmentSnapshot,
  Contract,
  ContractAmendment,
  ContractConfigSnapshot,
  ContractIndexEntry,
  ContractOwnershipTransfer,
  ContractScope,
  ContractStatus,
  ContractVerdict,
  DoneWhenCriterion,
} from "./contract-types.js";
import { CONTRACT_SCHEMA_VERSION } from "./contract-types.js";

export const CONTRACT_ID_PATTERN = /^c-[0-9a-f]{6}$/;
export const DONE_WHEN_ID_PATTERN = /^dw-[0-9a-f]{6}$/;
export const AMENDMENT_ID_PATTERN = /^a-[0-9a-f]{6}$/;
export const STORED_CONTRACT_REPO_ROOT = ".";

const CONTRACT_STATUSES: readonly ContractStatus[] = [
  "draft",
  "locked",
  "amended",
  "fulfilled",
  "broken",
  "discarded",
] as const;

const DONE_WHEN_KINDS = ["manual", "receipt-hint"] as const;

export function generateContractId(): string {
  return prefixTaskShortId("c");
}

export function generateDoneWhenId(): string {
  return prefixTaskShortId("dw");
}

export function generateContractAmendmentId(): string {
  return prefixTaskShortId("a");
}

export function canEditContract(contract: Contract): boolean {
  return contract.status === "draft";
}

export function isContractLockable(contract: Contract): boolean {
  if (contract.status !== "draft") return false;
  if (contract.intent.trim().length === 0) return false;
  if (contract.scope.filesExpected.length === 0) return false;
  if (contract.doneWhen.length === 0) return false;
  if (contract.doneWhen.some((criterion) => criterion.text.trim().length === 0)) return false;
  return true;
}

export function canDiscardContract(contract: Contract): boolean {
  return contract.status === "draft";
}

export function canAmendContract(contract: Contract): boolean {
  return contract.status === "locked" || contract.status === "amended";
}

export function canCloseContract(contract: Contract): boolean {
  return contract.status === "locked" || contract.status === "amended";
}

export function canReopenContract(contract: Contract): boolean {
  return contract.status === "fulfilled" || contract.status === "broken";
}

export function isActiveContract(contract: Contract): boolean {
  return contract.status === "locked" || contract.status === "amended";
}

export function countMetCriteria(criteria: readonly DoneWhenCriterion[]): number {
  return criteria.filter((criterion) => criterion.met === true).length;
}

export function snapshotForAmendment(contract: Contract): AmendmentSnapshot {
  return {
    intent: contract.intent,
    scope: contract.scope,
    doneWhen: contract.doneWhen,
  };
}

export function validateContract(value: unknown): Contract | undefined {
  if (!isRecord(value)) return undefined;
  if (value.schemaVersion !== CONTRACT_SCHEMA_VERSION) return undefined;
  if (!isContractId(value.id)) return undefined;
  if (typeof value.taskId !== "string" || !TASK_ID_PATTERN.test(value.taskId)) return undefined;
  if (typeof value.repoRoot !== "string" || value.repoRoot.length === 0) return undefined;
  if (!isContractStatus(value.status)) return undefined;
  if (!isIsoString(value.createdAt)) return undefined;
  if (!isOptionalIsoString(value.lockedAt)) return undefined;
  if (!isOptionalIsoString(value.closedAt)) return undefined;
  if (!isOptionalIsoString(value.discardedAt)) return undefined;
  if (typeof value.intent !== "string") return undefined;

  const scope = validateScope(value.scope);
  if (!scope) return undefined;

  const doneWhen = validateDoneWhenArray(value.doneWhen);
  if (!doneWhen) return undefined;

  if (!isOptionalString(value.claimedAtCommit)) return undefined;
  if (!isOptionalString(value.closedAtCommit)) return undefined;

  const amendments = validateAmendments(value.amendments);
  if (!amendments) return undefined;

  if (typeof value.createdBy !== "string" || value.createdBy.length === 0) return undefined;
  if (!isOptionalNonEmptyString(value.lockedBy)) return undefined;
  if (!isOptionalNonEmptyString(value.closedBy)) return undefined;
  const ownershipHistory = value.ownershipHistory === undefined ? undefined : validateOwnershipHistory(value.ownershipHistory);
  if (value.ownershipHistory !== undefined && !ownershipHistory) return undefined;

  const configSnapshot = validateConfigSnapshot(value.configSnapshot);
  if (!configSnapshot) return undefined;

  let verdict: ContractVerdict | undefined;
  if (value.verdict !== undefined) {
    verdict = validateVerdict(value.verdict);
    if (!verdict) return undefined;
  }

  return {
    schemaVersion: CONTRACT_SCHEMA_VERSION,
    id: value.id,
    taskId: value.taskId,
    repoRoot: value.repoRoot,
    status: value.status,
    createdAt: value.createdAt,
    lockedAt: value.lockedAt,
    closedAt: value.closedAt,
    discardedAt: value.discardedAt,
    intent: value.intent,
    scope,
    doneWhen,
    claimedAtCommit: value.claimedAtCommit,
    closedAtCommit: value.closedAtCommit,
    amendments,
    verdict,
    createdBy: value.createdBy,
    lockedBy: value.lockedBy,
    closedBy: value.closedBy,
    ownershipHistory,
    configSnapshot,
  };
}

export function validateContractIndexEntry(value: unknown): ContractIndexEntry | undefined {
  if (!isRecord(value)) return undefined;
  if (!isContractId(value.id)) return undefined;
  if (typeof value.taskId !== "string" || !TASK_ID_PATTERN.test(value.taskId)) return undefined;
  if (!isContractStatus(value.status)) return undefined;
  if (!isIsoString(value.at)) return undefined;
  if (!isOptionalString(value.reason)) return undefined;
  return {
    id: value.id,
    taskId: value.taskId,
    status: value.status,
    at: value.at,
    reason: value.reason,
  };
}

export function buildActiveOverlapError(contractId: string, overlappingIds: readonly string[]): MaestroError {
  return new MaestroError(
    `Contract ${contractId} overlaps an active contract in the same repo: ${overlappingIds.join(", ")}`,
    [
      "Discard or finish the other contract first",
      "Or switch contracts.overlapPolicy to annotate if you intentionally allow overlap",
    ],
  );
}

export function lastContractIndexedAt(contract: Contract): string {
  const latestAmendment = contract.amendments.at(-1)?.at;
  return contract.closedAt
    ?? contract.discardedAt
    ?? latestAmendment
    ?? contract.lockedAt
    ?? contract.createdAt;
}

export function normalizeStoredContractRepoRoot(_repoRoot: string): string {
  return STORED_CONTRACT_REPO_ROOT;
}

function prefixTaskShortId(prefix: string): string {
  return `${prefix}-${generateTaskId().slice(4)}`;
}

function isContractId(value: unknown): value is string {
  return typeof value === "string" && CONTRACT_ID_PATTERN.test(value);
}

function isContractStatus(value: unknown): value is ContractStatus {
  return typeof value === "string" && (CONTRACT_STATUSES as readonly string[]).includes(value);
}

function validateScope(value: unknown): ContractScope | undefined {
  if (!isRecord(value)) return undefined;
  if (!isStringArray(value.filesExpected)) return undefined;
  if (!isStringArray(value.filesForbidden)) return undefined;
  if (!isOptionalPositiveInteger(value.maxFilesTouched)) return undefined;
  return {
    filesExpected: value.filesExpected,
    filesForbidden: value.filesForbidden,
    maxFilesTouched: value.maxFilesTouched,
  };
}

function validateDoneWhenArray(value: unknown): readonly DoneWhenCriterion[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const criteria: DoneWhenCriterion[] = [];
  for (const item of value) {
    const parsed = validateDoneWhenCriterion(item);
    if (!parsed) return undefined;
    criteria.push(parsed);
  }
  return criteria;
}

function validateDoneWhenCriterion(value: unknown): DoneWhenCriterion | undefined {
  if (!isRecord(value)) return undefined;
  if (typeof value.id !== "string" || !DONE_WHEN_ID_PATTERN.test(value.id)) return undefined;
  if (typeof value.text !== "string") return undefined;
  if (!isDoneWhenKind(value.kind)) return undefined;
  if (!isOptionalBoolean(value.met)) return undefined;
  if (!isOptionalIsoString(value.metAt)) return undefined;
  if (!isOptionalNonEmptyString(value.metBy)) return undefined;
  if (!isOptionalString(value.metEvidence)) return undefined;
  return {
    id: value.id,
    text: value.text,
    kind: value.kind,
    met: value.met,
    metAt: value.metAt,
    metBy: value.metBy,
    metEvidence: value.metEvidence,
  };
}

function validateAmendments(value: unknown): readonly ContractAmendment[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const amendments: ContractAmendment[] = [];
  for (const item of value) {
    const parsed = validateAmendment(item);
    if (!parsed) return undefined;
    amendments.push(parsed);
  }
  return amendments;
}

function validateAmendment(value: unknown): ContractAmendment | undefined {
  if (!isRecord(value)) return undefined;
  if (typeof value.id !== "string" || !AMENDMENT_ID_PATTERN.test(value.id)) return undefined;
  if (!isIsoString(value.at)) return undefined;
  if (typeof value.by !== "string" || value.by.length === 0) return undefined;
  if (typeof value.reason !== "string" || value.reason.trim().length === 0) return undefined;
  const before = validateAmendmentSnapshot(value.before);
  const after = validateAmendmentSnapshot(value.after);
  if (!before || !after) return undefined;
  return {
    id: value.id,
    at: value.at,
    by: value.by,
    reason: value.reason,
    before,
    after,
  };
}

function validateAmendmentSnapshot(value: unknown): AmendmentSnapshot | undefined {
  if (!isRecord(value)) return undefined;
  if (!isOptionalString(value.intent)) return undefined;
  const scope = value.scope === undefined ? undefined : validateScope(value.scope);
  if (value.scope !== undefined && !scope) return undefined;
  const doneWhen = value.doneWhen === undefined ? undefined : validateDoneWhenArray(value.doneWhen);
  if (value.doneWhen !== undefined && !doneWhen) return undefined;
  return {
    intent: value.intent,
    scope,
    doneWhen,
  };
}

function validateConfigSnapshot(value: unknown): ContractConfigSnapshot | undefined {
  if (!isRecord(value)) return undefined;
  if (typeof value.strict !== "boolean") return undefined;
  if (!isOptionalPositiveInteger(value.defaultMaxFilesTouched)) return undefined;
  if (value.overlapPolicy !== "fail" && value.overlapPolicy !== "annotate") return undefined;
  if (value.rebaseFallback !== "best-effort" && value.rebaseFallback !== "fail") return undefined;
  if (value.staleReclaimContractPolicy !== "inherit" && value.staleReclaimContractPolicy !== "block") return undefined;
  return {
    strict: value.strict,
    defaultMaxFilesTouched: value.defaultMaxFilesTouched,
    overlapPolicy: value.overlapPolicy,
    rebaseFallback: value.rebaseFallback,
    staleReclaimContractPolicy: value.staleReclaimContractPolicy,
  };
}

function validateVerdict(value: unknown): ContractVerdict | undefined {
  if (!isRecord(value)) return undefined;
  if (typeof value.fulfilled !== "boolean") return undefined;
  if (!isIsoString(value.computedAt)) return undefined;
  if (!isStringArray(value.actualFilesTouched)) return undefined;
  if (!isStringArray(value.expectedFilesMatched)) return undefined;
  if (!isStringArray(value.outOfScopeFiles)) return undefined;
  if (!isStringArray(value.forbiddenTouched)) return undefined;
  if (!isStringArray(value.filesExpectedUnused)) return undefined;

  const unmetCriteria = validateDoneWhenArray(value.unmetCriteria);
  const metCriteria = validateDoneWhenArray(value.metCriteria);
  if (!unmetCriteria || !metCriteria) return undefined;

  const actualFilesTouchedTruncated = value.actualFilesTouchedTruncated === undefined
    ? undefined
    : validateTouchedFilesTruncated(value.actualFilesTouchedTruncated);
  if (value.actualFilesTouchedTruncated !== undefined && !actualFilesTouchedTruncated) return undefined;

  const capExceeded = value.capExceeded === undefined ? undefined : validateCapExceeded(value.capExceeded);
  if (value.capExceeded !== undefined && !capExceeded) return undefined;

  const overlapDetected = value.overlapDetected === undefined ? undefined : validateOverlapDetected(value.overlapDetected);
  if (value.overlapDetected !== undefined && !overlapDetected) return undefined;

  const receiptLinked = value.receiptLinked === undefined ? undefined : validateReceiptLinked(value.receiptLinked);
  if (value.receiptLinked !== undefined && !receiptLinked) return undefined;

  if (
    value.anchorFallback !== undefined
    && value.anchorFallback !== "direct"
    && value.anchorFallback !== "reflog"
    && value.anchorFallback !== "merge-base"
    && value.anchorFallback !== "lost"
  ) {
    return undefined;
  }
  if (!isOptionalString(value.notes)) return undefined;

  return {
    fulfilled: value.fulfilled,
    computedAt: value.computedAt,
    actualFilesTouched: value.actualFilesTouched,
    actualFilesTouchedTruncated,
    expectedFilesMatched: value.expectedFilesMatched,
    outOfScopeFiles: value.outOfScopeFiles,
    forbiddenTouched: value.forbiddenTouched,
    filesExpectedUnused: value.filesExpectedUnused,
    capExceeded,
    unmetCriteria,
    metCriteria,
    overlapDetected,
    receiptLinked,
    anchorFallback: value.anchorFallback,
    notes: value.notes,
  };
}

function validateTouchedFilesTruncated(
  value: unknown,
): { readonly stored: number; readonly actual: number } | undefined {
  if (!isRecord(value)) return undefined;
  if (!isPositiveInteger(value.stored)) return undefined;
  if (!isPositiveInteger(value.actual)) return undefined;
  if (value.stored >= value.actual) return undefined;
  return {
    stored: value.stored,
    actual: value.actual,
  };
}

function validateCapExceeded(value: unknown): { readonly cap: number; readonly actual: number } | undefined {
  if (!isRecord(value)) return undefined;
  if (!isPositiveInteger(value.cap)) return undefined;
  if (!isPositiveInteger(value.actual)) return undefined;
  return {
    cap: value.cap,
    actual: value.actual,
  };
}

function validateOverlapDetected(value: unknown): {
  readonly otherContractIds: readonly string[];
  readonly policy: "fail" | "annotate";
} | undefined {
  if (!isRecord(value)) return undefined;
  if (!Array.isArray(value.otherContractIds)) return undefined;
  if (!value.otherContractIds.every((item) => typeof item === "string" && CONTRACT_ID_PATTERN.test(item))) {
    return undefined;
  }
  if (value.policy !== "fail" && value.policy !== "annotate") return undefined;
  return {
    otherContractIds: value.otherContractIds,
    policy: value.policy,
  };
}

function validateOwnershipHistory(value: unknown): readonly ContractOwnershipTransfer[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const history: ContractOwnershipTransfer[] = [];
  for (const item of value) {
    const parsed = validateOwnershipTransfer(item);
    if (!parsed) return undefined;
    history.push(parsed);
  }
  return history;
}

function validateOwnershipTransfer(value: unknown): ContractOwnershipTransfer | undefined {
  if (!isRecord(value)) return undefined;
  if (!isNonEmptyString(value.from)) return undefined;
  if (!isNonEmptyString(value.to)) return undefined;
  if (!isIsoString(value.at)) return undefined;
  if (value.reason !== "claim_reclaim" && value.reason !== "handoff_pickup") return undefined;
  return {
    from: value.from,
    to: value.to,
    at: value.at,
    reason: value.reason,
  };
}

function validateReceiptLinked(value: unknown): {
  readonly summary?: string;
  readonly surprise?: string;
  readonly verifiedBy?: readonly string[];
} | undefined {
  if (!isRecord(value)) return undefined;
  if (!isOptionalString(value.summary)) return undefined;
  if (!isOptionalString(value.surprise)) return undefined;
  if (value.verifiedBy !== undefined && !isStringArray(value.verifiedBy)) return undefined;
  return {
    summary: value.summary,
    surprise: value.surprise,
    verifiedBy: value.verifiedBy,
  };
}

function isDoneWhenKind(value: unknown): value is DoneWhenCriterion["kind"] {
  return typeof value === "string" && (DONE_WHEN_KINDS as readonly string[]).includes(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isStringArray(value: unknown): value is readonly string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isIsoString(value: unknown): value is string {
  if (typeof value !== "string") return false;
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) return false;
  return new Date(parsed).toISOString() === value;
}

function isOptionalIsoString(value: unknown): value is string | undefined {
  return value === undefined || isIsoString(value);
}

function isOptionalString(value: unknown): value is string | undefined {
  return value === undefined || typeof value === "string";
}

function isOptionalNonEmptyString(value: unknown): value is string | undefined {
  return value === undefined || isNonEmptyString(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function isPositiveInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0;
}

function isOptionalPositiveInteger(value: unknown): value is number | undefined {
  return value === undefined || isPositiveInteger(value);
}

function isOptionalBoolean(value: unknown): value is boolean | undefined {
  return value === undefined || typeof value === "boolean";
}
