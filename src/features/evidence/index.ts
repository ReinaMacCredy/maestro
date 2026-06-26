export type EvidenceKind = "command" | "file" | "note";
export type WitnessLevel = "inline" | "recorded" | "external";

export interface EvidenceRow {
  readonly id: string;
  readonly kind: EvidenceKind;
  readonly witness_level: WitnessLevel;
  readonly created_at: string;
}

export interface EvidenceStorePort {
  readonly list?: () => Promise<readonly EvidenceRow[]>;
}
