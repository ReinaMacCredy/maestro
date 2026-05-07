export interface RatchetAssertion {
  readonly id: string;
  readonly correctionId: string;
  readonly rule: string;
  readonly check: string;
  readonly createdAt: string;
}

export interface RatchetSuite {
  readonly assertions: readonly RatchetAssertion[];
}

export interface RatchetBaseline {
  readonly passCount: number;
  readonly lastRunAt: string;
}
