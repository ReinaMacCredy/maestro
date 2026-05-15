export interface RunState {
  readonly schemaVersion: 1;
  readonly taskId: string;
  readonly retryCount: number;
  readonly wallClockElapsedSeconds: number;
  readonly tokensUsed?: number;
  readonly lastUpdatedAt: string; // ISO-8601
}
