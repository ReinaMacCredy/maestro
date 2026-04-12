import type { MilestoneProfile } from "./mission-types.js";

export type PrincipleMode = "advisory" | "gate";
export type GateCheckType = `array_min_length:${number}` | "object_non_empty" | "array_all_passed";
export type PrincipleSource = "karpathy" | "custom";

export interface Principle {
  readonly id: string;
  readonly name: string;
  readonly source: PrincipleSource;
  readonly rule: string;
  readonly profiles: readonly MilestoneProfile[];
  readonly mode: PrincipleMode;
  readonly gateField?: string;
  readonly gateCheck?: GateCheckType;
}

export interface CreatePrincipleInput {
  readonly id: string;
  readonly name: string;
  readonly source?: PrincipleSource;
  readonly rule: string;
  readonly profiles: readonly string[];
  readonly mode: PrincipleMode;
  readonly gateField?: string;
  readonly gateCheck?: string;
}
