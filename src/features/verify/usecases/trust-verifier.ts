import type { Contract } from "@/features/task/index.js";
import type { GitSignatureProbePort } from "../ports/git-signature.port.js";
import type { TrustFinding, TrustVerifierResult } from "../domain/types.js";
import { checkScope } from "./checks/check-scope.js";
import { checkLockfileParity } from "./checks/check-lockfile-parity.js";
import { checkGeneratedFileParity } from "./checks/check-generated-file-parity.js";
import { checkSensitivePaths } from "./checks/check-sensitive-paths.js";
import { checkCommitMetadata } from "./checks/check-commit-metadata.js";
import { checkSecretsInDiff } from "./checks/check-secrets-in-diff.js";

export interface TrustVerifierInput {
  readonly contract: Contract;
  readonly diff: {
    readonly changedPaths: readonly string[];
    readonly addedLines: readonly string[];
    readonly base: string;
    readonly head: string;
  };
  readonly projectRoot: string;
}

export interface TrustVerifierDeps {
  readonly gitSignatureProbe: GitSignatureProbePort;
}

/**
 * Runs all 6 trust checks in parallel and returns a flat list of findings.
 * This function is deterministic given the same inputs — it performs no writes
 * and does not mutate any shared state.
 */
export async function runTrustVerifier(
  input: TrustVerifierInput,
  deps: TrustVerifierDeps,
): Promise<TrustVerifierResult> {
  const [
    scopeFindings,
    lockfileFindings,
    generatedFindings,
    sensitiveFindings,
    metadataFindings,
    secretFindings,
  ] = await Promise.all([
    checkScope(input.diff.changedPaths, input.contract),
    checkLockfileParity(input.diff.changedPaths, input.projectRoot),
    checkGeneratedFileParity(input.projectRoot),
    checkSensitivePaths(input.diff.changedPaths, input.projectRoot),
    checkCommitMetadata(input.diff.base, input.diff.head, input.projectRoot, deps.gitSignatureProbe),
    checkSecretsInDiff(input.diff.addedLines),
  ]);

  const findings: readonly TrustFinding[] = [
    ...scopeFindings,
    ...lockfileFindings,
    ...generatedFindings,
    ...sensitiveFindings,
    ...metadataFindings,
    ...secretFindings,
  ];

  return { findings };
}
