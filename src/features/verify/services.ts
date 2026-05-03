import { GitSignatureAdapter } from "./adapters/git-signature.adapter.js";
import { runTrustVerifier } from "./usecases/trust-verifier.js";
import type { TrustVerifierInput, TrustVerifierDeps } from "./usecases/trust-verifier.js";
import type { TrustVerifierResult } from "./domain/types.js";

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
