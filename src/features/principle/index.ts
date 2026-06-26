export type PrincipleMode = "advisory" | "gate";

export interface Principle {
  readonly id: string;
  readonly name: string;
  readonly mode: PrincipleMode;
}

export interface PrincipleOutcomeRecord {
  readonly principleId: string;
  readonly outcome: "helpful" | "unhelpful" | "pending";
}

export interface PrincipleStorePort {
  readonly list?: () => Promise<readonly Principle[]>;
}
