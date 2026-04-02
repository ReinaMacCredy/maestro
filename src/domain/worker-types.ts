import type { WorkerReport } from "./mission-types.js";

export type TransportType = "cli";

export type WorkerOutputMode = "raw" | "stream-json";

export type SupervisionLevel = "low" | "mid" | "high";

export type FailureClass =
  | "infrastructure"
  | "worker-crash"
  | "validation"
  | "unknown";

export interface ExecutionConfig {
  readonly defaultWorker?: string;
  readonly stopOnFailure?: boolean;
  readonly retryBudget?: number;
  readonly rotateWorkerOnRetry?: boolean;
}

export interface WorkerConfig {
  readonly enabled: boolean;
  readonly transport: TransportType;
  readonly command: string;
  readonly args?: readonly string[];
  readonly outputMode?: WorkerOutputMode;
  readonly env?: Readonly<Record<string, string>>;
}

export interface SupervisionConfig {
  readonly level?: SupervisionLevel;
  readonly staleAfterMs?: number;
  readonly killGraceMs?: number;
  readonly progressIntervalMs?: number;
}

export interface ParallelConfig {
  readonly enabled?: boolean;
  readonly maxConcurrent?: number;
}

export interface WorkerResult {
  readonly success: boolean;
  readonly exitCode: number;
  readonly summary: string;
  readonly stdoutRaw: string;
  readonly stderrRaw: string;
  readonly filesChanged: readonly string[];
  readonly durationMs: number;
  readonly report?: WorkerReport;
  readonly failureClass?: FailureClass;
  readonly parsedOutput?: string;
}

export interface ExecutionRecord {
  readonly id: string;
  readonly missionId: string;
  readonly featureId: string;
  readonly worker: string;
  readonly transport: TransportType;
  readonly attemptId: string;
  readonly startedAt: string;
  readonly completedAt: string;
  readonly durationMs: number;
  readonly success: boolean;
  readonly exitCode: number;
  readonly summary: string;
  readonly stdoutRaw: string;
  readonly stderrRaw: string;
  readonly filesChanged: readonly string[];
  readonly report?: WorkerReport;
  readonly failureClass?: FailureClass;
}
