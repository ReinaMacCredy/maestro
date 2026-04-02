import type { WorkerReport } from "./mission-types.js";

export type TransportType = "cli" | "a2a";

export type WorkerOutputMode = "raw" | "stream-json";

export type SupervisionLevel = "low" | "mid" | "high";

export type FailureClass =
  | "infrastructure"
  | "worker-crash"
  | "validation"
  | "unknown";

export type WorkerProgressEventKind =
  | "status"
  | "stdout"
  | "stderr"
  | "heartbeat";

export interface ExecutionConfig {
  readonly defaultWorker?: string;
  readonly stopOnFailure?: boolean;
  readonly retryBudget?: number;
  readonly rotateWorkerOnRetry?: boolean;
}

export interface WorkerBaseConfig {
  readonly enabled: boolean;
  readonly transport: TransportType;
}

export interface CliWorkerConfig extends WorkerBaseConfig {
  readonly transport: "cli";
  readonly command: string;
  readonly args?: readonly string[];
  readonly outputMode?: WorkerOutputMode;
  readonly env?: Readonly<Record<string, string>>;
}

export interface A2aWorkerConfig extends WorkerBaseConfig {
  readonly transport: "a2a";
  readonly url: string;
  readonly agentCardPath?: string;
  readonly headers?: Readonly<Record<string, string>>;
}

export type WorkerConfig = CliWorkerConfig | A2aWorkerConfig;

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

export interface WorkerProgressEvent {
  readonly timestamp: string;
  readonly kind: WorkerProgressEventKind;
  readonly worker: string;
  readonly text?: string;
  readonly sessionId?: string;
  readonly runtimeState?: "starting" | "live" | "stale" | "failed" | "recoverable" | "completed";
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

export interface RuntimeEventRecord {
  readonly id: string;
  readonly missionId: string;
  readonly featureId: string;
  readonly attemptId: string;
  readonly worker: string;
  readonly timestamp: string;
  readonly kind: WorkerProgressEventKind;
  readonly text?: string;
  readonly sessionId?: string;
  readonly runtimeState?: "starting" | "live" | "stale" | "failed" | "recoverable" | "completed";
}
