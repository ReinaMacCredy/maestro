export interface RunState {
  readonly retryCount?: number;
  readonly wallClockElapsedSeconds?: number;
  readonly lastUpdatedAt?: string;
}

export interface RunStateStorePort {
  readonly read: (taskId: string) => Promise<RunState | null>;
}
