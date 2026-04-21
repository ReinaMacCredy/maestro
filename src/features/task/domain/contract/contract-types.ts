export const CONTRACT_SCHEMA_VERSION = 1;

export type ContractStatus =
  | "draft"
  | "locked"
  | "amended"
  | "fulfilled"
  | "broken"
  | "discarded";

export type ActorId = string;

export interface ContractOwnershipTransfer {
  readonly from: ActorId;
  readonly to: ActorId;
  readonly at: string;
  readonly reason: "claim_reclaim" | "handoff_pickup";
}

export interface ContractScope {
  readonly filesExpected: readonly string[];
  readonly filesForbidden: readonly string[];
  readonly maxFilesTouched?: number;
}

export interface DoneWhenCriterion {
  readonly id: string;
  readonly text: string;
  readonly kind: "manual" | "receipt-hint";
  readonly met?: boolean;
  readonly metAt?: string;
  readonly metBy?: ActorId;
  readonly metEvidence?: string;
}

export interface AmendmentSnapshot {
  readonly intent?: string;
  readonly scope?: ContractScope;
  readonly doneWhen?: readonly DoneWhenCriterion[];
}

export interface ContractAmendment {
  readonly id: string;
  readonly at: string;
  readonly by: ActorId;
  readonly reason: string;
  readonly before: AmendmentSnapshot;
  readonly after: AmendmentSnapshot;
}

export interface ContractVerdict {
  readonly fulfilled: boolean;
  readonly computedAt: string;
  readonly actualFilesTouched: readonly string[];
  readonly actualFilesTouchedTruncated?: {
    readonly stored: number;
    readonly actual: number;
  };
  readonly expectedFilesMatched: readonly string[];
  readonly outOfScopeFiles: readonly string[];
  readonly forbiddenTouched: readonly string[];
  readonly filesExpectedUnused: readonly string[];
  readonly capExceeded?: {
    readonly cap: number;
    readonly actual: number;
  };
  readonly unmetCriteria: readonly DoneWhenCriterion[];
  readonly metCriteria: readonly DoneWhenCriterion[];
  readonly overlapDetected?: {
    readonly otherContractIds: readonly string[];
    readonly policy: "fail" | "annotate";
  };
  readonly receiptLinked?: {
    readonly summary?: string;
    readonly surprise?: string;
    readonly verifiedBy?: readonly string[];
  };
  readonly anchorFallback?: "direct" | "reflog" | "merge-base" | "lost";
  readonly notes?: string;
}

export interface ContractConfigSnapshot {
  readonly strict: boolean;
  readonly defaultMaxFilesTouched?: number;
  readonly overlapPolicy: "fail" | "annotate";
  readonly rebaseFallback: "best-effort" | "fail";
  readonly staleReclaimContractPolicy: "inherit" | "block";
}

export interface Contract {
  readonly schemaVersion: typeof CONTRACT_SCHEMA_VERSION;
  readonly id: string;
  readonly taskId: string;
  readonly repoRoot: string;
  readonly status: ContractStatus;
  readonly createdAt: string;
  readonly lockedAt?: string;
  readonly closedAt?: string;
  readonly discardedAt?: string;
  readonly intent: string;
  readonly scope: ContractScope;
  readonly doneWhen: readonly DoneWhenCriterion[];
  readonly claimedAtCommit?: string;
  readonly closedAtCommit?: string;
  readonly amendments: readonly ContractAmendment[];
  readonly verdict?: ContractVerdict;
  readonly createdBy: ActorId;
  readonly lockedBy?: ActorId;
  readonly closedBy?: ActorId;
  readonly ownershipHistory?: readonly ContractOwnershipTransfer[];
  readonly configSnapshot: ContractConfigSnapshot;
}

export interface CreateContractRecordInput {
  readonly id?: string;
  readonly taskId: string;
  readonly repoRoot: string;
  readonly createdAt: string;
  readonly intent: string;
  readonly scope: ContractScope;
  readonly doneWhen: readonly DoneWhenCriterion[];
  readonly createdBy: ActorId;
  readonly configSnapshot: ContractConfigSnapshot;
}

export interface DeleteContractRecordInput {
  readonly taskId: string;
  readonly status?: ContractStatus;
  readonly at: string;
  readonly reason?: string;
}

export interface ContractIndexEntry {
  readonly id: string;
  readonly taskId: string;
  readonly status: ContractStatus;
  readonly at: string;
  readonly reason?: string;
}
