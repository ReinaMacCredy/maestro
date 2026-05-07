import type { RuntimeSignalOperator } from "@/features/spec";

export interface RuntimeSignalResult {
  readonly value: number;
  readonly threshold: number;
  readonly operator: RuntimeSignalOperator;
  readonly pass: boolean;
  readonly sampled_at: string; // ISO 8601
}
