export type VerdictDecision = "pass" | "fail" | "blocked";

export interface Verdict {
  readonly id: string;
  readonly decision: VerdictDecision;
  readonly computedAt: string;
}

export interface VerdictStorePort {
  readonly list?: () => Promise<readonly Verdict[]>;
  readonly readLatest: (taskId: string) => Promise<Verdict | null>;
}
