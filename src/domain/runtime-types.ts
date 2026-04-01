import type { AgentSlug } from "./types.js";

export type RuntimeState =
  | "starting"
  | "live"
  | "stale"
  | "failed"
  | "recoverable"
  | "completed";

export interface RecoveryHistoryEntry {
  readonly timestamp: string;
  readonly reason: string;
  readonly fromState: RuntimeState;
  readonly toState: RuntimeState;
}

export interface RecoveryMetadata {
  readonly retryCount: number;
  readonly lastRecoveryAt?: string;
  readonly lastRecoveryReason?: string;
  readonly history: readonly RecoveryHistoryEntry[];
}

export interface WorkerRuntime {
  readonly featureId: string;
  readonly attemptId: string;
  readonly attempt: number;
  readonly agent: AgentSlug;
  readonly sessionId?: string;
  readonly runtimeState: RuntimeState;
  readonly startedAt: string;
  readonly lastSeenAt: string;
  readonly leaseExpiresAt: string;
  readonly failureReason?: string;
  readonly recoveryMetadata: RecoveryMetadata;
}
