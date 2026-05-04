export type { DerivedRiskInput, DerivedRiskResult } from "./domain/types.js";
export { RISK_CLASS_ORDER, compareRiskClass, maxRiskClass } from "./usecases/risk-class-order.js";
export { deriveRiskClassFromDiff } from "./usecases/derive-risk-class.js";
export { computeRisk } from "./usecases/compute-risk.js";
export type { ComputeRiskInput } from "./usecases/compute-risk.js";
export { buildRiskServices } from "./services.js";
export type { RiskServices } from "./services.js";
