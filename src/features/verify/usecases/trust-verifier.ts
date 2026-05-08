import type { Contract } from "@/features/task/index.js";
import type { GitSignatureProbePort } from "../ports/git-signature.port.js";
import type { TrustFinding, TrustVerifierResult } from "../domain/types.js";
import { checkScope } from "./checks/check-scope.js";
import { checkLockfileParity } from "./checks/check-lockfile-parity.js";
import { checkGeneratedFileParity } from "./checks/check-generated-file-parity.js";
import { checkSensitivePaths } from "./checks/check-sensitive-paths.js";
import { checkCommitMetadata } from "./checks/check-commit-metadata.js";
import { checkSecretsInDiff } from "./checks/check-secrets-in-diff.js";
import { checkNonEmptyDiff } from "./checks/check-non-empty-diff.js";
import { checkArchitectureLints } from "./checks/check-architecture-lints.js";

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
 * Runs all 8 trust checks in parallel and returns a flat list of findings.
 * This function is deterministic given the same inputs — it performs no writes
 * and does not mutate any shared state.
 */
export async function runTrustVerifier(
  input: TrustVerifierInput,
  deps: TrustVerifierDeps,
): Promise<TrustVerifierResult> {
  const [
    emptyDiffFindings,
    scopeFindings,
    lockfileFindings,
    generatedFindings,
    sensitiveFindings,
    metadataFindings,
    secretFindings,
    archLintFindings,
  ] = await Promise.all([
    Promise.resolve(checkNonEmptyDiff(input.diff)),
    checkScope(input.diff.changedPaths, input.contract),
    checkLockfileParity(input.diff.changedPaths, input.projectRoot),
    checkGeneratedFileParity(input.projectRoot),
    checkSensitivePaths(input.diff.changedPaths, input.projectRoot),
    checkCommitMetadata(input.diff.base, input.diff.head, input.projectRoot, deps.gitSignatureProbe),
    checkSecretsInDiff(input.diff.addedLines),
    checkArchitectureLints(input.diff, input.projectRoot),
  ]);

  const findings: readonly TrustFinding[] = [
    ...emptyDiffFindings,
    ...scopeFindings,
    ...lockfileFindings,
    ...generatedFindings,
    ...sensitiveFindings,
    ...metadataFindings,
    ...secretFindings,
    ...archLintFindings,
  ];

  return { findings };
}
