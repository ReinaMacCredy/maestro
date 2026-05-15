import { GitSignatureAdapter } from "./adapters/git-signature.adapter.js";
import { runTrustVerifier } from "./trust-verifier.js";
import type { TrustVerifierInput, TrustVerifierDeps } from "./trust-verifier.js";
import type { TrustVerifierResult } from "@/v2/types/trust.js";

export interface VerifyServices {
  readonly runTrustVerifier: (
    input: Omit<TrustVerifierInput, "projectRoot">,
  ) => Promise<TrustVerifierResult>;
}

export function buildVerifyServices(projectRoot: string): VerifyServices {
  const deps: TrustVerifierDeps = {
    gitSignatureProbe: new GitSignatureAdapter(),
  };

  return {
    runTrustVerifier: (input) =>
      runTrustVerifier({ ...input, projectRoot }, deps),
  };
}
