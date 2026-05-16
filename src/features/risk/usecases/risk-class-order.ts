import type { RiskClass } from "@/types/product-spec.js";

export const RISK_CLASS_ORDER: readonly RiskClass[] = ["low", "medium", "high", "critical"];

export function compareRiskClass(a: RiskClass, b: RiskClass): -1 | 0 | 1 {
  const ai = RISK_CLASS_ORDER.indexOf(a);
  const bi = RISK_CLASS_ORDER.indexOf(b);
  if (ai < bi) return -1;
  if (ai > bi) return 1;
  return 0;
}

export function maxRiskClass(a: RiskClass, b: RiskClass): RiskClass {
  return compareRiskClass(a, b) >= 0 ? a : b;
}
