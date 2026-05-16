// Observability port emits per-task structured events to a writable sink.
// Default adapter writes per-worktree JSONL under .maestro/runs/<task-id>/.
// Consumers can override the port to ship events to Vector/VictoriaLogs, etc.

export type ObservabilityEventKind = "transition" | "lint" | "custom";

export interface ObservabilityEvent {
  readonly task_id: string;
  readonly kind: ObservabilityEventKind;
  readonly timestamp: string;
  readonly payload: Readonly<Record<string, unknown>>;
}

export interface ObservabilityPort {
  emit(event: ObservabilityEvent): Promise<void>;
}
