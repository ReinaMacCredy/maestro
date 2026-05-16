import type { RunState } from "../types/contract.js";

export type RunStateDelta = Partial<Pick<RunState, "retryCount" | "wallClockElapsedSeconds" | "tokensUsed">>;

export interface RunStateStorePort {
  read(taskId: string): Promise<RunState | undefined>;
  write(taskId: string, state: RunState): Promise<void>;
  /** Atomic read-modify-write: reads (or initializes to zeros), adds delta values, writes back, returns the new state. */
  increment(taskId: string, delta: RunStateDelta): Promise<RunState>;
}
