export type { TrustFinding, TrustVerifierResult, Severity } from "./domain/types.js";
export type { TrustVerifierInput, TrustVerifierDeps } from "./usecases/trust-verifier.js";
export { runTrustVerifier } from "./usecases/trust-verifier.js";
export { buildVerifyServices } from "./services.js";
export type { VerifyServices } from "./services.js";
export { buildProofMap } from "./usecases/proof-map.js";
export type { ProofMap, ProofMapEntry, ProofMapEvidence } from "./usecases/proof-map.js";
