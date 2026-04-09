/**
 * Worker configuration shapes read from maestro.yaml.
 *
 * Phase 3 strip: after Phase 1 removed the worker execution layer and
 * Phase 3 removed the Mission Control worker/runtime panes, only the
 * configuration shapes survive. They describe what `.maestro/config.yaml`
 * can express so the config inspector, `maestro doctor`, and worker
 * fit recommendation can render CLI worker definitions.
 *
 * Deleted as dead in Phase 3:
 *   - A2aWorkerConfig, TransportType union (CLI is the only transport)
 *   - WorkerResult, ExecutionRecord, RuntimeEventRecord, WorkerProgressEvent*
 *   - FailureClass, SupervisionLevel union members other than the presets
 *   - runtime supervision / parallel execution were already stubs in Phase 1
 */

export type WorkerOutputMode = "raw" | "stream-json";

export type SupervisionLevel = "low" | "mid" | "high";

export interface ExecutionConfig {
  readonly defaultWorker?: string;
  readonly stopOnFailure?: boolean;
  readonly retryBudget?: number;
  readonly rotateWorkerOnRetry?: boolean;
}

/**
 * CLI worker profile.
 *
 * Phase 3 strip: `transport` is fixed to `"cli"`. A2A transport was
 * removed in Phase 1 and the field is kept on the struct to avoid
 * cascading a rename through every config consumer.
 */
export interface CliWorkerConfig {
  readonly enabled: boolean;
  readonly transport: "cli";
  readonly command: string;
  readonly args?: readonly string[];
  readonly outputMode?: WorkerOutputMode;
  readonly env?: Readonly<Record<string, string>>;
}

export type WorkerConfig = CliWorkerConfig;

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
